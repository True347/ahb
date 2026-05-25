# Phase 2: Codex + Output Formats - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-25
**Phase:** 2-codex-output-formats
**Areas discussed:** Codex 資料來源策略, --json schema (schema_version:1), --detailed layout, CLI flag 組合 + exit codes

---

## Gray-area selection

| Option | Description | Selected |
|--------|-------------|----------|
| Codex 資料來源策略 | JSONL vs SQLite 主從、rate_limits null 處理、state 多版本選擇 | ✓ |
| --json schema (schema_version:1) | 頂層結構 / error envelope / windows 表達 / semver 政策 | ✓ |
| --detailed layout | 每 provider 行數、window 順序、Claude weekly 是否在 Phase 2 補 | ✓ |
| CLI flag 組合 + exit codes | flag 互斥/precedence、--ascii × --json、exit code 邊界 | ✓ |

**User's choice:** 全部四個 area 都選

---

## Area 1 — Codex 資料來源策略

### Q1.1 — Codex 主訊號來源

| Option | Description | Selected |
|--------|-------------|----------|
| JSONL primary + SQLite 只讀 metadata | rollout-*.jsonl 算 HP signal；state_*.sqlite READ_ONLY + busy_timeout 250ms 只補 thread metadata | ✓ (Recommended) |
| JSONL only — 不碰 SQLite | 完全不開 state_*.sqlite，避開 Pitfall 3 所有風險 | |
| SQLite primary + JSONL supplemental | 反轉；不推薦 | |

**User's choice:** JSONL primary + SQLite 只讀 metadata（Recommended）
**Notes:** 沿用 RESEARCH/PITFALLS 建議；append-only JSONL 最安全 + SQLite 只在需要 metadata 時補上

### Q1.2 — rate_limits: null 時的 fallback

| Option | Description | Selected |
|--------|-------------|----------|
| 純 unknown（??% + DarkGray） | 重用 Phase 1 SchemaDrift sentinel；render `codex  ▒▒▒▒▒▒▒▒▒▒ ??% • Codex adapter may be out-of-date` | ✓ (Recommended) |
| 從 token_count events 推估 + 註明 estimate | 加總 token_count、render `~40% (est)` | |
| 混合：estimate 有值就顯，沒值才 unknown | 雙路徑 | |

**User's choice:** 純 unknown（Recommended）
**Notes:** 避免「user 以為 100%」最危險的誤導；重用既有 SchemaDrift 路徑無需新 enum 變體

### Q1.3 — state_*.sqlite 多版本共存策略

| Option | Description | Selected |
|--------|-------------|----------|
| 選最高版 + stderr warn | glob 倒序、取第一、若 >1 match emit `tracing::warn!` | ✓ (Recommended) |
| 選最高版 silent | 不 warn 不提醒 | |
| ERROR row 拒絕 render Codex | 視為不確定狀態 | |

**User's choice:** 選最高版 + stderr warn（Recommended）
**Notes:** 動作最少 + 可追蹤；user 看到 stderr warn 知道 mid-migration

### Q1.4 — Codex HpWindow emit 策略

| Option | Description | Selected |
|--------|-------------|----------|
| 上游有幾個 window 就 emit 幾個（passthrough） | label 依上游語意；不在 adapter 內排序合併 | ✓ (Recommended) |
| 鎖「只 emit primary session window」 | 強制單 window；簡單但 CORE-03 detailed 部分留白 | |
| 讓 Claude decide—在 RESEARCH 內定 | 推延 | |

**User's choice:** 上游有幾個就 emit 幾個（Recommended）
**Notes:** adapter passthrough 為原則；尊重上游 ground truth

---

## Area 2 — --json schema (schema_version:1)

### Q2.1 — Stable DTO vs 直接 serialize ProviderState

| Option | Description | Selected |
|--------|-------------|----------|
| 另定 JsonDto 與 stable schema 解耦 | render_json.rs 內定 JsonRoot/JsonProvider/JsonWindow/JsonError 並手刻轉換 | ✓ (Recommended) |
| 直接 #[derive(Serialize)] ProviderState 不倍譯 | 簡單但 model 任何改動就是 public API 改動 | |

**User's choice:** 另定 JsonDto（Recommended）
**Notes:** internal model 與外曝 schema 解耦；refactor 自由度保留

### Q2.2 — JSON 頂層結構

| Option | Description | Selected |
|--------|-------------|----------|
| Array of objects with `id` field | providers:[{id:..., ...}, ...] 順序 BL-02；jq friendly | ✓ (Recommended) |
| Map keyed by id | providers:{claude:{...},codex:{...}} 簡單但丟順序 | |
| Mix：providers array + summary object | 多寫 summary 子物件 | |

**User's choice:** Array of objects with `id` field（Recommended）
**Notes:** 保留 BL-02 deterministic order；jq query 慣例

### Q2.3 — Error envelope shape

| Option | Description | Selected |
|--------|-------------|----------|
| status: ok / error 二元 + error sub-object | error:{kind, message, ...} 一致 envelope | ✓ (Recommended) |
| windows: [] + 頂層加 error 欄 | 單字串 error，簡單但無 kind 分類 | |
| Union type（result discriminated） | type:ok / type:error；Rust-y 但 shell 不友善 | |

**User's choice:** status binary + sub-object（Recommended）
**Notes:** kind snake_case 對應 ProviderError variant；additive 欄位（missing / retry_after_seconds）依 kind 增

### Q2.4 — schema_version semver 政策

| Option | Description | Selected |
|--------|-------------|----------|
| Additive 不升；remove/rename/語義改 才 bump | 與 JSON web API 慣例一致 | ✓ (Recommended) |
| 任何變動都 bump | 安全但 user 常看到 unsupported schema | |
| 只有打破 jq query 才 bump | 預規則難 enforce | |

**User's choice:** Additive 不升 / breaking 才 bump（Recommended）
**Notes:** v1 contract 對 consumer：providers array 順序穩定、status 永遠存在、unknown fields 必須容忍

---

## Area 3 — --detailed layout

### Q3.1 — 每 provider 的排版

| Option | Description | Selected |
|--------|-------------|----------|
| Header line + indented window lines | provider header + 2-space indent window lines + 空行分群 | ✓ (Recommended) |
| Flat：每 window 一行，provider 名在行首 | grep/awk friendly 但 label 重複 | |
| Table（列對齊） | ASCII border；視覺重 | |

**User's choice:** Header + indented（Recommended）
**Notes:** 視覺一眼分群；最貼近現有 compact 風格

### Q3.2 — Claude weekly bar 是否在 Phase 2 補

| Option | Description | Selected |
|--------|-------------|----------|
| Phase 2 補 Claude weekly | ClaudeProvider 改為 emit 2 個 HpWindow（5h + weekly）；同一個 JSONL scan pass | ✓ (Recommended) |
| 推 v2 / Phase 4 | detailed 只 Codex 有 weekly | |
| 補但僅 best-effort placeholder | emit weekly window 但標 unknown | |

**User's choice:** Phase 2 補（Recommended）
**Notes:** 不隱藏 CORE-03 願景；CLAUDE_WEEKLY_TOKEN_LIMIT + weekly anchor 規則留 phase-researcher 抓答案、planner 落實

### Q3.3 — 同 provider 內 window 順序

| Option | Description | Selected |
|--------|-------------|----------|
| Adapter passthrough | 上游 emit 順序為準 | ✓ (Recommended) |
| Sort by shortest reset 在上 | render 排序覆蓋 adapter | |
| Sort by lowest % remaining 在上 | 最危險在上 | |

**User's choice:** Adapter passthrough（Recommended）
**Notes:** 不與 ground truth 衝突；Claude 內鎖 [5h, weekly]、Codex 依上游 rate_limits array

### Q3.4 — --detailed 與 compact 共享什麼樣式

| Option | Description | Selected |
|--------|-------------|----------|
| 100% 共享，只多 header + indent | BAR_WIDTH=10 / U+2588░/Green-Yellow-Red/--ascii/--color 全沿用 | ✓ (Recommended) |
| --detailed 重新設計寬體 + 色類 | 體驗豐筆但 UI-SPEC 需補 2.x、snapshot test 雙套 | |

**User's choice:** 100% 共享（Recommended）
**Notes:** Phase 1 helpers 都 pub(crate) 可重用；snapshot test 加新檔不動現有 compact

---

## Area 4 — CLI flag 組合 + exit codes

### Q4.1 — --compact / --detailed / --json 互斥

| Option | Description | Selected |
|--------|-------------|----------|
| clap conflicts_with 互斥 | 同時下會 error + exit 2（clap convention） | ✓ (Recommended) |
| Precedence：json > detailed > compact | 同時下不 error、json 贏 | |
| First-wins（按 argv 順序） | clap 不原生支援，需手刻 | |

**User's choice:** clap conflicts_with 互斥（Recommended）
**Notes:** 意圖明確、不留隱藏 precedence

### Q4.2 — --ascii / --color × --json

| Option | Description | Selected |
|--------|-------------|----------|
| --json 一律不上色 + Unicode；--ascii/--color 對 --json 靜默忽略 | tty::should_colorize_env 已鎖；不 error 不 warn | ✓ (Recommended) |
| --ascii + --json stderr warn | 顯式告知忽略 | |
| --json 所有 string fields ASCII fold | label/source/reason 都 ASCII-only | |

**User's choice:** 靜默忽略（Recommended）
**Notes:** --json 是機器消費路徑；視覺 flag 無語意，不打亂 pipeline

### Q4.3 — 零 provider enabled 的 exit code

| Option | Description | Selected |
|--------|-------------|----------|
| 零 enabled = exit 0 + empty-state stdout | 與 Phase 1 EMPTY_STATE 行為一致；CFG-04 silent skip 語意 | ✓ (Recommended) |
| 零 enabled = exit 1 | 視為「全失敗」 | |
| 零 enabled = exit 2 | 視為「config 不可用」 | |

**User's choice:** exit 0 + empty-state（Recommended）
**Notes:** user 看到提示 + exit 0 = 「請去 enable provider」，不被 CI 當故障

### Q4.4 — SchemaDrift / Unavailable 是否算 exit 1

| Option | Description | Selected |
|--------|-------------|----------|
| 凡 ProviderError 都算 fail；SchemaDrift 算 fail | Result::Ok=success、Result::Err=fail 一致 | ✓ (Recommended) |
| SchemaDrift 特例為 degraded success（exit 0 側） | render 了 sentinel 算 alive | |
| RateLimited 特例為 ok-ish | 明確 signal 不算錯 | |

**User's choice:** 凡 Err 都算 fail（Recommended）
**Notes:** 邏輯簡單一致；jq `.providers[] | select(.status=="ok")` 即得「真有 signal」

---

## Claude's Discretion

以下推給 phase-researcher / planner，不在這場 discuss 鎖：

- CLAUDE_WEEKLY_TOKEN_LIMIT 的具體 token 數字（從 ccusage / claude-code-usage-monitor / tokenmix.ai 挖）
- Claude weekly anchor 規則（ISO week Mon 00:00 local 為起始 vs Anthropic 實際 reset 邊界）
- Codex rate_limits schema 上 window label 字串（passthrough 上游語意；具體名稱依 codex-rs 原始碼確認）
- spawn_blocking 包裝層級（整個 fetch 或只 SQLite/JSONL IO 段）
- JsonWindow 是否曝 source / bar_color（Phase 2 v1 保守只包 label/percent_remaining/reset_at）
- --detailed window label 對齊寬度（建議 8-char 左對齊）
- ProviderError::Internal anyhow chain sanitize 策略（建議只 emit 頂層 Display）

## Deferred Ideas

- bar_color hint passthrough 到 JsonWindow（Phase 3+ additive）
- `--json --pretty` flag（Phase 4 polish）
- `AHB --json --watch <secs>`（不做；用 `watch -n 15 AHB --json`）
- SIGPIPE 顯式 exit 0 on broken pipe（planner 視 verification 結果決）
- ProviderError::Internal anyhow cause chain 展開（不做，避免洩漏 internal path）
- Schema-drift sentinel 字面 generalize 為 `{Label} adapter may be out-of-date`（若 Codex 觸發後 planner 覺得「Claude」字樣不對再 generalize）
- Codex rate_limits estimate fallback（Phase 3 或 v2 opt-in）
- 三態 status enum (current/degraded/unknown) 取代 ok/error 二態（v2 schema_version=2）
- `schema_version=2` migration deprecation window 設計（實際 bump 時再決定）
- Codex weekly window 是否存在的事實確認（research 階段；passthrough 規則自然處理）
