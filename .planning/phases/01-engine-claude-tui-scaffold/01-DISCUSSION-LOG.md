# Phase 1: Engine + Claude + TUI Scaffold - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-23
**Phase:** 1 — Engine + Claude + TUI Scaffold
**Areas discussed:** Engine concurrency + refresh loop, Claude JSONL parsing, Config schema + first-run UX, Secrets / keyring scope

---

## Engine concurrency + refresh loop

### Q1 — engine.refresh_all() fan-out

| Option | Description | Selected |
|--------|-------------|----------|
| tokio::task::JoinSet | 每 adapter spawn 進 JoinSet；drain `.join_next()`；可 cancel/abort 慢 adapter | ✓ |
| join_all + tokio::time::timeout | 簡單但 wait-for-slowest，不能提前 cancel | |
| futures::future::select_all loop | 手動 drain，UX 最好但複雜 | |

**User's choice:** JoinSet
**Notes:** TUI 模式下慢 adapter must not block render — JoinSet 能在新 tick 來臨時主動 abort 上一輪沒回來的 task。

### Q2 — per-adapter timeout location

| Option | Description | Selected |
|--------|-------------|----------|
| Engine 包 tokio::time::timeout | Provider trait 不變，timeout 集中管理 | ✓ |
| FetchCtx 加 deadline 欄位 | adapter 自己看著辦，但責任轉移 | |
| 雙重 timeout（engine + adapter 內部） | 兩層都有，設定點多 | |

**User's choice:** Engine 包 timeout
**Notes:** 為 Phase 3 CFG-03 per-provider override 鋪路 — 只動 engine config 即可。

### Q3 — TUI refresh loop architecture

| Option | Description | Selected |
|--------|-------------|----------|
| 單一 15s tick 驅動 engine.refresh_all() | tokio::time::interval(15s) 一個 task | ✓ |
| per-adapter 獨立 tick + 各自 push | Gemini 600s + Claude 15s 並行 perfect，但 Phase 1 over-engineering | |
| TUI 訂閱 mpsc + 引擎模式由 main 選 | engine 提供兩種介面 | |

**User's choice:** 單一 15s tick
**Notes:** per-provider refresh interval 是 Phase 3 CFG-03 的事，現在不預釋。

### Q4 — countdown re-render cadence

| Option | Description | Selected |
|--------|-------------|----------|
| 每 1s redraw | ratatui differential rendering 成本接近 0 | ✓ |
| 只在 fetch 返回時畫（15s 跳 1 次） | countdown 顯得「死的」 | |
| 1s render + 15s fetch 雙 timer | 變體多但關心點分明 | |

**User's choice:** 每 1s redraw
**Notes:** UI-SPEC 標記 countdown 為「第二重要」資訊，每 1s 跳避免「此刻畫面是不是死的？」疑慮。

---

## Claude JSONL parsing

### Q1 — *.jsonl 檔案探查

| Option | Description | Selected |
|--------|-------------|----------|
| glob crate | glob("~/.claude/projects/**/*.jsonl") | ✓ |
| walkdir（手動 prune） | 控制多一點 | |
| std::fs::read_dir 手推 | 零依賴但 boilerplate 多 | |

**User's choice:** glob
**Notes:** REQ / PROJECT 字面就是這條 pattern。

### Q2 — 5h rolling-window session anchor

| Option | Description | Selected |
|--------|-------------|----------|
| Cluster anchor | 從最新往回走，> 5h gap 視為 cluster 邊界；session_start = cluster 內最早 user message | ✓ |
| Sliding 5h window | 最近 5h 內所有訊息 sum tokens | |
| Latest user message - 5h | sliding 但 user msg 為錨點 | |

**User's choice:** Cluster anchor
**Notes:** 與 Claude Code「閒置 5h 重置」實際語意一致。

### Q3 — ADP-03 schema-drift sentinel trigger

| Option | Description | Selected |
|--------|-------------|----------|
| 連續 3 條 assistant 無 message.usage | 低 false-positive | ✓ |
| 任何一條最新無 usage 就觸發 | 反應最快但 false-positive 最高 | |
| Version pin 信任閾值 | 依賴 jsonl 內 version 欄位 | |

**User's choice:** 連續 3 條無 message.usage（≥ 2 條缺）
**Notes:** schema rename 一序 catch，但偶發 tool-only 消息不會誤觸。

### Q4 — hot file（正在被寫入的 jsonl）

| Option | Description | Selected |
|--------|-------------|----------|
| BufRead::lines() 隱藏 parse 失敗的最後一行 | 視為 truncated trailing line | ✓ |
| 預先讀 mtime + 讀到最後完整換行為止 | 避免「最後行 silently skip」但複雜 | |
| mmap + scan | 最快但 active write 下有安全顧慮 | |

**User's choice:** BufRead 容忍 truncated trailing line
**Notes:** 中間行 serde 失敗 → warn + skip；最後行失敗 → silently skip（append-only JSONL 標準慣例）。

---

## Config schema + first-run UX

### Q1 — TOML schema

| Option | Description | Selected |
|--------|-------------|----------|
| Per-provider table [providers.claude/codex/gemini] | 表頭明示，心智負擔低 | ✓ |
| Array of tables [[provider]] | 序滑但可同名多 entry | |
| Flat enabled_providers = [...] | 最簡但雙重表達 | |

**User's choice:** Per-provider table
**Notes:** ProviderId 是 closed enum (D-08)，表頭 hardcode 三家。

### Q2 — config.toml 不存在時的 first-run UX

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-create 帶 enabled = false 的 default | 可重現 + 讓使用者看見所有可選 keys | ✓ |
| 不造檔，stderr hint + exit 2 | 不碰使用者檔案系統 | |
| Interactive walk-through（TTY 時才開） | 最友善但不 pipe-safe | |

**User's choice:** Auto-create default
**Notes:** 寫法用 include_str!() 把 default 鎖在 binary 裡。CLI 訊息 `initialized ~/.config/ahb/config.toml — enable providers and rerun`，exit 0。

### Q3 — unknown key 處理

| Option | Description | Selected |
|--------|-------------|----------|
| Warn + ignore | forward-compatible | ✓ |
| Strict reject (deny_unknown_fields) | 抓 typo 最累，但版本升級要重寫 config | |
| Warn + opt-in --strict-config flag | 靈活但多一個 flag | |

**User's choice:** Warn + ignore
**Notes:** stderr 警告，但不 block。

### Q4 — ProjectDirs args

| Option | Description | Selected |
|--------|-------------|----------|
| qualifier="" organization="" application="ahb" | 符合 REQ ~/.config/ahb/ 字面 | ✓ |
| qualifier="dev" organization="" application="ahb" | macOS 變 ~/Library/Application Support/dev.ahb/ | |
| 手推 XDG_CONFIG_HOME fallback | 最輕但 macOS/Windows 差 | |

**User's choice:** qualifier="" organization="" application="ahb"
**Notes:** REQ CFG-02 明訂用 directories crate，~/.config/ahb/ 是預期路徑。

---

## Secrets / keyring

### Q1 — Phase 1 keyring use scope

| Option | Description | Selected |
|--------|-------------|----------|
| 不被碰，但 keyring 整條路徑 wire 起來 | Phase 2 Codex 一 plug 就跨來 | ✓ |
| Wire + 加一個 real demo entry | round-trip 驗證但 demo entry 在 prod binary 是 code smell | |
| Defer 到 Phase 2 Codex | REQ SEC-01 明訂 Phase 1，re-scope roadmap | |

**User's choice:** Wire 但不真用
**Notes:** Claude adapter 不需 secret；但 SEC-01..04 在這 phase 鎖 — API + Secret<T> + redact + CI grep 全建好。

### Q2 — Linux headless（無 D-Bus）fallback

| Option | Description | Selected |
|--------|-------------|----------|
| Hard error + 提示 set secret_storage = "file" | 不違背 STACK.md「never fall back silently」 | ✓ |
| Silently fallback ~/.config/ahb/secrets.toml 0600 | 進出最順但違背非「never silent」原則 | |
| Auto-fallback + AHB_DISABLE_KEYRING env | 靈活但 spec 複雜 | |

**User's choice:** Hard error
**Notes:** exit 2，stderr message 提到 `set [secrets].storage = "file" in ~/.config/ahb/config.toml`。Phase 1 不實作 file backend、僅 error message 提到。

### Q3 — Secret<T> 型部設計

| Option | Description | Selected |
|--------|-------------|----------|
| Secret<T: Zeroize + Clone>、Debug = "***"、#[serde(skip)] | Drop 自動 zeroize；唯一 .expose() 拆封 | ✓ |
| 不限 trait，只寫 Debug + serde::skip，不動 zeroize | 簡單 30 行但 secret RAM leftover | |
| 用現成 secrecy crate (SecretBox<T>) | 現成完整但多一個 dep，STACK.md 未列 | |

**User's choice:** Secret<T: Zeroize + Clone>
**Notes:** 自製 30 行 newtype；新增 zeroize dep（Phase 1 唯一新 secret-related dep）；不引 secrecy crate。

### Q4 — CI grep test 規則

| Option | Description | Selected |
|--------|-------------|----------|
| Static fixture + high-entropy [A-Za-z0-9]{20,} pattern | 雙重 assert（字面 + entropy） | ✓ |
| Property test（quickcheck 隨機 Secret） | 更 thorough 但多依賴 | |
| Bash grep 跨 AHB --json 樣本 | 最簡但只能測 stdout、不能測 Debug | |

**User's choice:** Static fixture + entropy regex
**Notes:** Unit test 跨 Debug 與 serde 路徑；Integration test 跨 AHB --json 真正輸出。雙重 assert 抓「直接洩 + 編碼後洩」兩種失誤。

---

## Claude's Discretion

- mpsc channel buffer 大小（建議 unbounded 或 bounded(64)）
- EngineEvent enum 具體型別（Refresh / TickError / Shutdown 三種起跳）
- 預設 per-adapter timeout 全局值（建議 Claude=2s）
- Claude 5h limit 具體 token 數字 — phase-researcher 挖 + 硬編 const
- cache_read_input_tokens 計不計入用量 — 跟 Anthropic billing 對齊（建議：cache_creation 計、cache_read 不計）
- ratatui Gauge vs Paragraph+Span — Claude 選乾淨那條
- panic-injection 整合測試注入機制 — planner 決定（建議 AHB_DEBUG_PANIC env var，prod build 不 compile）

## Deferred Ideas

- per-provider refresh_interval override (CFG-03) — Phase 3
- per-provider auth_source / cookie path — Phase 3
- `secret_storage = "file"` 0600 file backend 實作 — Phase 4 or backlog
- AHB_DISABLE_KEYRING env var override — 不做（用 config 統一）
- `--strict-config` flag — 不做（warn + ignore 已足夠）
- Interactive first-run walk-through — Phase 4 polish 或 backlog
- AHB_CONFIG_PATH env var override — Phase 4
- config_dir vs preference_dir 選哪個 — planner 確認（建議 config_dir）
- ratatui Gauge widget 選擇 — Claude's discretion
- mpsc channel buffer 大小、EngineEvent 具體 enum — planner 補
