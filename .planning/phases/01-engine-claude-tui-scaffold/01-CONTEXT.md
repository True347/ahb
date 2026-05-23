# Phase 1: Engine + Claude + TUI Scaffold - Context

**Gathered:** 2026-05-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 1 是 AHB 的「load-bearing」整合 phase。一次把五件互相驗證的東西立到位、之後 Codex / Gemini 只是換 adapter 就好：

1. **Engine 骨架** — 多 adapter fan-out + per-adapter 隔離 + 統一 mpsc 事件流
2. **Claude adapter** — 從 `~/.claude/projects/**/*.jsonl` 算 5h rolling-window 用量 + reset 時間 + schema-drift sentinel
3. **TUI scaffold** — `AHB tui` 固定全螢幕、15s 自動 refresh、panic-safe terminal restore、非 TTY 拒絕 graceful
4. **Config** — TOML schema、cross-platform 路徑、unknown-key 寬容、未配置 provider silent skip
5. **Secrets / keyring** — keyring-core 1.0 entry 全 wire、`Secret<T>` Zeroize+redact newtype、CI grep 守住

不做：Codex adapter（Phase 2）、Gemini adapter（Phase 3）、cache / stale-on-error（Phase 3）、`--detailed`/`--json` formats（Phase 2）、distribution（Phase 4）、per-provider refresh interval override（Phase 3 CFG-03）。

User-observable artifact：在裝有 Claude Code 的機器上跑 `AHB`，看到一條真實 Claude session % + reset countdown；跑 `AHB tui`，看到固定畫面每 15s refresh；`AHB | cat` 沒有 ANSI；`AHB tui` 在 pipe 裡 graceful 拒絕。

</domain>

<decisions>
## Implementation Decisions

### Engine concurrency & refresh loop

- **D-28 (fan-out):** `engine.refresh_all()` 用 `tokio::task::JoinSet`。每 adapter spawn 一個 task 進 JoinSet，engine drain `.join_next()`。可 cancel/abort 慢 adapter，不會拖住下一個 15s tick。比 `join_all` + `tokio::time::timeout` 強的點是：能在新 tick 來臨時主動 abort 上一輪沒回來的 task。

- **D-29 (per-adapter timeout location):** Engine 在 spawn task 時包 `tokio::time::timeout(per_provider_timeout, provider.fetch(ctx))`。Provider trait 不變（D-13 鎖定的 `async fn fetch(&self, ctx: &FetchCtx) -> Result<...>` 不加 deadline）、adapter 實作保持簡單、timeout policy 集中在 engine config。Phase 3 CFG-03 加 per-provider override 時只動 engine。Phase 1 預設 timeout 值留給 planner 補（Claude=2s 是建議起點，因為純本機 IO）。

- **D-30 (TUI refresh loop architecture):** 單一 `tokio::time::interval(Duration::from_secs(15))` tick → 觸發 `engine.refresh_all()` → 結果一次 push 到 mpsc。Phase 1 只有 1 個 real adapter，Phase 2 加 Codex 也夠用。per-provider refresh interval 是 Phase 3 CFG-03 的事、現在不預釋。

- **D-31 (countdown re-render cadence):** TUI 每 1s redraw（獨立的 `tokio::time::interval(1s)` render tick）；refresh tick 仍是 15s。Render task 只用本地 cache state 畫面、不觸發 fetch；fetch tick 只 push state 到 cache。ratatui differential rendering 成本接近 0。理由：UI-SPEC 標記 countdown 為「第二重要」資訊，每 1s 跳避免「此刻畫面是不是死的？」疑慮。

### Claude JSONL parsing

- **D-32 (file discovery):** `glob` crate，pattern `glob("~/.claude/projects/**/*.jsonl")`。REQ ADP-02 與 PROJECT.md 都用這條 pattern 字面表達。目錄結構只一層 `projects/<project-name>/<session-uuid>.jsonl`，glob 足夠。

- **D-33 (5h rolling-window 錨點 — cluster anchor):** [AMENDED 2026-05-23 per RESEARCH Pitfall L1]
  1. 從所有 jsonl 收集所有 message，按 `timestamp` 排序
  2. 從最新一條往回走，找到「上一條 message 距離 > 5h」的斷點 → cluster 邊界
  3. `session_start` = 該 cluster 裡最早的 user message timestamp
  4. `reset_at` = `session_start + 5h`
  5. cluster 內所有 assistant message 的 **`message.usage.cache_creation_input_tokens`** 加總、與 `CLAUDE_5H_TOKEN_LIMIT`（見 D-44）比 → percent_remaining

  符合 Claude Code「閒置 5h 重置」實際語意。`session_start` 取 first user message 而非 first assistant message，避免「Claude 先發初始化 prompt」的 edge case。

  **為什麼是 `cache_creation_input_tokens` 而不是 `input_tokens + output_tokens`？**（ccusage issue #866）：upstream Claude Code 把 `input_tokens` / `output_tokens` 當 streaming placeholder 寫進 JSONL，~75% 是 0 或 1、不會 finalize。實測 `input_tokens` 低估 100-174×、`output_tokens` 在 Opus 上低估 10-17×（thinking tokens 完全沒寫進去）。可信欄位是 `cache_creation_input_tokens`（~0.9× ground truth，這是 Anthropic 真正 bill against 5h budget 的數字）與 `cache_read_input_tokens`（~1.1× ground truth，但 cache reads 對 budget 攤銷後成本接近 0）。Phase 1 只 sum `cache_creation_input_tokens`、`cache_read_input_tokens` 不算進 budget。README 必須註記「best-effort estimate，upstream JSONL 部分不完整」。

- **D-34 (ADP-03 schema-drift sentinel trigger):** [AMENDED 2026-05-23 per D-33 update] 讀最近 N=3 條 `type:"assistant"` message；若其中 ≥ 2 條的 `message.usage` 不存在、或缺 `cache_creation_input_tokens` 欄、觸發 sentinel。Sentinel 字面（UI-SPEC LOCKED）：`claude  ▒▒▒▒▒▒▒▒▒▒ ??% • Claude adapter may be out-of-date`。低 false-positive（偶發 tool-only 消息不會誤觸）、schema rename 一序 catch。

- **D-35 (hot-file truncated-trailing-line tolerance):**
  - 用 `BufReader::new(File::open(path)?).lines()` 一行一行讀
  - 中間行 `serde_json::from_str::<JsonlEntry>(&line?)` 失敗 → `tracing::warn!` + skip
  - 最後一行 failed → silently skip（視為 Claude 正在 append 中的 truncated line，append-only JSONL 標準慣例）
  - 不 mmap、不 stat-then-read tricks。最簡 + 正確。

### Config schema & first-run UX

- **D-36 (TOML schema):** Per-provider table。
  ```toml
  [providers.claude]
  enabled = true

  [providers.codex]
  enabled = false  # Phase 2 上線後改

  [providers.gemini]
  enabled = false  # Phase 3 上線後改
  ```
  ProviderId 是 closed enum（Phase 0 D-08）、表頭 hardcode 三家。Phase 3 加 `refresh_interval` / `auth_source` 時只是同 table 加欄、不破壞 schema。

- **D-37 (first-run UX — auto-create default):** `AHB` 第一次跑沒看到 `~/.config/ahb/config.toml`：
  1. `mkdir -p` config_dir
  2. 寫一個帶 markdown 註解的 default：三家 `enabled = false`，每個 table 上方有一行說明
  3. 印一行到 stdout：`initialized ~/.config/ahb/config.toml — enable providers and rerun`
  4. `exit(0)`
  
  可重現、讓使用者一眼看見所有可選 keys、不依賴 CLI 互動（pipe-safe）。寫法用 `include_str!("../templates/default-config.toml")` 把 default 鎖在 binary 裡。

- **D-38 (unknown-key policy):** Warn + ignore。`#[serde(deny_unknown_fields)]` 不開啟。遇 unknown key 寫一行 stderr warn：`unrecognized config key '{}' — see README`。Forward-compatible：未來版本新 key 不會讓舊 binary 爆。

- **D-39 (ProjectDirs args):** `directories::ProjectDirs::from("", "", "ahb")` — qualifier 與 organization 都留空、application = `"ahb"`。產生：
  - Linux: `~/.config/ahb/config.toml`
  - macOS: `~/Library/Application Support/ahb/config.toml`
  - Windows: `%APPDATA%\ahb\config.toml`

  符合 REQ 文字 `~/.config/ahb/`、不引入 reverse-DNS 慣例（使用者本人不需要記 `dev.author.ahb`）。

### Secrets / keyring

- **D-40 (Phase 1 keyring use):** **Wire 整條路徑、但 Phase 1 不真的 store/load 任何 secret**。
  - `keyring-core::Entry::new("ahb", "<provider-id>")` API 在 `src/secrets.rs` 全 wire
  - `Secret<T>` newtype 完整實作（D-42）
  - `Secrets` 結構是 engine `FetchCtx` 的一個 field、Claude adapter 拿到的 `secrets` 是空的（Claude 讀本機 JSONL 不需要）
  - CI grep test 用人造 Secret 走 Debug / serde 路徑、不打 keyring 真實 service
  - Phase 2 Codex 第一次 plug 進來時、`secrets.get("codex")` 直接用既有 wired API
  
  理由：keyring-core 1.0 是新 API、Phase 1 wire 起來把 platform-specific bug（macOS Keychain prompt、Windows Credential Manager session、Linux Secret Service dbus）一次撞掉，Phase 2 / 3 才不會撞。

- **D-41 (Linux headless fallback policy — hard error):** keyring 不可用時、**hard error + clear next step**：
  - exit code: `2`（REQ CORE-06 = config / secrets 不可用）
  - stderr message: `no secret store available on this system; set [secrets].storage = "file" in ~/.config/ahb/config.toml to opt into 0600 file storage`
  - Phase 1 **不實作** `[secrets].storage = "file"` backend，只是 error message 提到、留給未來 phase 補
  - **絕不 silently fallback**（STACK.md："never fall back silently"）

  Phase 1 Claude adapter 不需 secret、所以這條 error 在實際使用上不會撞到；但 secrets API contract 鎖定。

- **D-42 (`Secret<T>` shape):** `Secret<T: Zeroize + Clone>`，自製 newtype（不引 `secrecy` crate）：
  ```rust
  pub struct Secret<T: Zeroize + Clone>(T);

  impl<T: Zeroize + Clone> Drop for Secret<T> {
      fn drop(&mut self) { self.0.zeroize(); }
  }

  impl<T: Zeroize + Clone> Debug for Secret<T> {
      fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result { write!(f, "***") }
  }

  impl<T: Zeroize + Clone> Serialize for Secret<T> {
      fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
          s.serialize_str("[REDACTED]")
      }
  }
  // 不 derive / 不實作 Deserialize（secret 從 keyring 讀進來、不從 toml/json）

  impl<T: Zeroize + Clone> Secret<T> {
      pub fn new(inner: T) -> Self { Self(inner) }
      pub fn expose(&self) -> &T { &self.0 }  // 唯一拆封路徑
  }
  ```
  
  唯一拿到底層 `T` 的方式是 `.expose()`，逼 grep 看得到所有曝光點。新增 `zeroize` dep（Phase 1 唯一新 secret-related dep；不引 `secrecy` 因 STACK.md 沒列、自己 30 行夠用）。

- **D-43 (CI grep test — static fixture + high-entropy assertion):**
  - Unit test：建一個 `Secret::new("deadbeefcafe1234567890abcdef".to_string())`
  - 走兩條路徑：`format!("{:?}", secret)` 與 `serde_json::to_string(&secret).unwrap()`
  - 雙重 assert：
    1. 字面字串 `"deadbeefcafe1234567890abcdef"` 不出現
    2. 任何 `[A-Za-z0-9]{20,}` 連續字元 pattern 不出現（用 `regex` crate；test-dep 即可）
  - 加 integration test：跑完整 `AHB --json` （用 mock Claude fixture），grep stdout，同樣兩條 assert
  
  雙重 assert 抓「直接洩 + 編碼後洩」兩種失誤。Test 寫一次、未來 adapter 加 secret 自動承襲守護。

### Claude 5h limit constant

- **D-44 (CLAUDE_5H_TOKEN_LIMIT):** [ADDED 2026-05-23 per RESEARCH] 硬編 `pub const CLAUDE_5H_TOKEN_LIMIT: u64 = 44_000;`（Pro tier 估算值，來源：tokenmix.ai 2026-05 整理 + ccusage 社群測量）。Phase 1 不開放給 config override（這條留給 Phase 2 配 plan auto-detection 時再開）。Anthropic 不公告精確數字、所以 const 旁註明「best-effort estimate; revisit quarterly」並提供修改說明。Max5 / Max20 user 跑會看到 bar 跑得比實際慢（denominator 偏小、percent_used 偏高）— 文件先告知、Phase 2 解。

### Claude's Discretion

- mpsc channel buffer 大小（建議 `unbounded` 或 `bounded(64)`，三 adapter × 15s tick 不會撞 bound）
- `EngineEvent` enum 具體型別（Refresh / TickError / Shutdown 三種起跳）
- 預設 per-adapter timeout 全局值（建議 Claude=2s，純本機 IO；HTTP adapter Phase 3 再 raise）
- ratatui widget 選擇：用 `Gauge` 直接 render bar、或自己 `Paragraph` + Span 拼。Claude 選乾淨那一條
- panic-injection 整合測試的注入方式（thread::spawn panic / 故意 unwrap fixture / 環境變數）— planner 決定

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project & roadmap
- `.planning/PROJECT.md` — Core Value（HP bar 視覺隱喻必要）、Constraints、Out of Scope
- `.planning/REQUIREMENTS.md` — Phase 1 涵蓋 15 個 requirement：CORE-01/05、TUI-01/02/04/05、CFG-01/02/04、SEC-01/02/04、ADP-01/02/03
- `.planning/ROADMAP.md` § Phase 1 — goal、mode=mvp、5 success criteria
- `.planning/STATE.md` — Phase 0 完成紀錄、Accumulated Decisions（D-01..D-27 完整鎖定列表）

### Phase 0 lock-in
- `.planning/phases/00-spike-spine/00-CONTEXT.md` — D-08..D-14 model.rs contract（ProviderState / HpWindow / ResetInfo / HpUnit / ProviderError）、D-15..D-19 charset & 視覺、D-25 mock format、D-27 panic-hook 契約
- `.planning/research/GEMINI_SPIKE.md` — Phase 3 範圍前置依據（與 Phase 1 無直接依賴、但 engine concurrency D-28 預想 3 adapter）

### Research synthesis（**全部 MUST READ before planning**）
- `.planning/research/SUMMARY.md` — executive synthesis
- `.planning/research/STACK.md` — locked stack（ratatui 0.30 / crossterm 0.29 via re-export / keyring-core 1.0 / jiff 0.2 / glob / serde / toml / directories / tracing），NOT-list（keyring v4、async-std、tui-rs、parallel crossterm、native-tls）
- `.planning/research/ARCHITECTURE.md` — Provider trait shape、engine pattern、**channel-not-mutex**、codex-rs reference、**spine pattern**
- `.planning/research/FEATURES.md` — table-stakes vs differentiator
- `.planning/research/PITFALLS.md` — **Pitfall 5（terminal restore via panic-hook composition）必讀**、Pitfall 1（Gemini ToS — Phase 1 不碰但 trait 要相容）

### Phase 1 visual contract（剛 approved，LOCKED）
- `.planning/phases/01-engine-claude-tui-scaffold/01-UI-SPEC.md` — **全 6 維度 PASS**：
  - spacing（bar-width=10、inline-gap-sm=1、inline-gap-md=2）
  - 色彩（60/30/10 ANSI 角色 / NO_COLOR / `--color` / `--json` 全四條色彩-off 路徑）
  - typography（4 intensity role）
  - copywriting（schema-drift sentinel 字面、TUI 非 TTY refusal copy、per-row error 必含 next-step hint）
  - registry（cargo deny）

### External docs（impl 階段必查）
- ratatui 0.30 release notes — Backend trait 變化、HorizontalAlignment rename、`ratatui::init()` / `restore()` 與 panic-hook 的組合方式
- `keyring-core` 1.0 docs.rs — `set_default_store` 模式（不是 v4 的 feature flag 模式）、Linux Secret Service / Keychain / Credential Manager backend 行為
- `jiff` 0.2.x docs — `Timestamp::since((Unit::Hour, *now))` 顯式單位指定（Phase 0-03 deviation 1 教訓）
- `glob` crate docs — `glob_with` options（要不要 follow symlinks、隱藏檔等）
- Claude Code 官方 5h session limit 文件（若 user-facing 不存在，phase-researcher 從社群 ccusage / claude-code-usage-monitor 整理挖）

### Code 慣例（既有 src/ 已立）
- `src/main.rs` — wall-clock `jiff::Timestamp::now()` **唯一**呼叫點（acceptance grep guards mock.rs；Phase 1 Claude adapter 走 `ctx.now`）
- `src/cli/render_text.rs` — `compact_line` format string LOCKED；Phase 1 把 mock 輸出換成 real ClaudeProvider 輸出、format string 不動
- `src/lib.rs` crate-root lint floor — `deny(unwrap/expect/panic) + warn(pedantic)` 自動套到 Phase 1 新 module

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/model.rs` — Phase 0 D-08..D-14 model 完整 LOCKED：`ProviderState { id, windows, fetched_at, source }`、`HpWindow { label, percent_remaining, reset, bar_color }`、`ResetInfo { resets_at }`、`HpUnit = f32`、`ProviderError` closed enum、`Provider` trait（async + Send + Sync + 'static）。Phase 1 直接拿來用、不重新設計。
- `src/provider/mock.rs` — MockProvider 保留作 panic-injection integration test 素材（D-25 literal 仍是 fixture）
- `src/provider/mod.rs` — module entry，Phase 1 新增 `src/provider/claude.rs` 走同 trait
- `src/cli/render_text.rs` — `compact_line(state)` 已存在，吃 `ProviderState`；Phase 1 只是換 source 不改 render
- `src/secrets.rs` — Phase 0 stub，Phase 1 在這裡 wire keyring-core
- `src/main.rs` — `install_phase0_panic_hook()` 是第一行；Phase 1 TUI mode 內 `ratatui::init()` 用 `take_hook + set_hook` 組合包

### Established Patterns
- **Vec<Result<...>> aggregation at engine level** — `engine.refresh_all() -> Vec<(ProviderId, Result<ProviderState, ProviderError>)>`，Phase 1 從 1 個 adapter 開始、簽名不變
- **Wall-clock 注入** — adapter 用 `ctx.now`（jiff::Timestamp Copy）、只有 `main.rs` 呼叫 `Timestamp::now()`。acceptance grep guards mock.rs
- **`jiff::Timestamp::since` 需顯式 `(Unit::Hour, *now)`** — 否則 default Unit::Second 出來、會壞 2h00m balanced span 顯示（Phase 0-03 deviation 1）
- **`ProviderState.source` 為 `Cow<'static, str>`** — D-8 補丁，serde round-trip 不會 lifetime 出錯
- **`ProviderError` serde shape** — `Network { source }` / `Internal { source }` 為 struct variants 而非 newtype variants，serde internally-tagged enum 才能 round-trip；`Internal` 用 `#[serde(serialize_with = serialize_display)]` 只 emit Display-only JSON
- **Phase 0 dep 最小化** — Cargo.toml 鎖 9 prod + 1 dev dep；Phase 1 新增 dep 集中：`tokio` 加 features（`rt-multi-thread`、`fs`、`time`、`signal`、`sync`）、新增 `ratatui`、`reqwest`（trait 預備、Phase 1 不真打 HTTP）、`keyring-core`、`zeroize`、`glob`、`directories`、`toml`。每加一個 dep 在 Cargo.toml comment 註明 phase 來源
- **`Clippy disallowed-types`** — 已用具體 crossterm path（`event::Event`、`style::Color`），Phase 1 加 ratatui 後可能需要加更多 entry；不能用 glob

### Integration Points
- `src/lib.rs` — Phase 1 在這裡 export `engine` module 與新的 `claude` adapter
- `Cargo.toml` — workspace boundary、CHANGELOG 註記 dep 為何加
- `.github/workflows/ci.yml` — Phase 0 floor（build + test + clippy）夠用、Phase 4 才加 fmt-check / audit / deny；Phase 1 不動
- `src/cli/mod.rs` — clap entry；Phase 1 加 `tui` subcommand（之前只有預設 compact）

</code_context>

<specifics>
## Specific Ideas

- **HP bar 字面格式（D-25 + UI-SPEC LOCKED）：** `claude  ██████░░░░ 60% • resets in 2h00m`
  - 10-cell bar（`█` filled / `░` empty）
  - `•` U+2022 分隔 % 與 reset
  - `--ascii` flag 把 `█/░/•` 換成 `#/-/|`
  - 空 bar 時 `▒▒▒▒▒▒▒▒▒▒`（schema-drift sentinel 專用 `▒` U+2592 medium-shade）

- **schema-drift sentinel 字面（UI-SPEC LOCKED）：** `claude  ▒▒▒▒▒▒▒▒▒▒ ??% • Claude adapter may be out-of-date`

- **per-row error 字面（UI-SPEC LOCKED）：** `claude  ERROR: ~/.claude/projects not found — is Claude Code installed?` — 結尾 next-step hint **必須有**

- **TUI 非 TTY refusal 字面（UI-SPEC LOCKED）：** `AHB tui requires a terminal (stdout is not a TTY). Run AHB without 'tui' for piped / non-interactive output.`

- **first-run init 字面：** `initialized ~/.config/ahb/config.toml — enable providers and rerun`

- **default-config.toml fixture 範本：**
  ```toml
  # AHB — AI HP Bar config. Enable the providers you have subscriptions for.
  #
  # Run `AHB` after editing — uncovered providers are silently skipped (not flagged as failures).

  [providers.claude]
  enabled = false  # Claude Code subscription — reads ~/.claude/projects/**/*.jsonl

  [providers.codex]
  enabled = false  # Codex CLI subscription — not yet implemented (Phase 2)

  [providers.gemini]
  enabled = false  # Gemini CLI subscription — not yet implemented (Phase 3)
  ```

- **engine FetchCtx 增補（Phase 0 D-13 預備、Phase 1 落實）：** `pub struct FetchCtx<'a> { pub now: jiff::Timestamp, pub secrets: &'a Secrets }`，Phase 1 不再加更多 fields（per-adapter timeout 在 engine 外面、不傳 deadline）

</specifics>

<deferred>
## Deferred Ideas

These came up during discussion and belong in later phases or backlog:

- **per-provider refresh_interval override（CFG-03）** — Phase 3
- **per-provider auth_source / cookie path（CFG-03 / SEC-04 補丁）** — Phase 3
- **`secret_storage = "file"` 0600 file backend 實作** — Phase 1 只在 error message 提到、留給未來 phase（可能 Phase 4 distribution polish）；REQ SEC-01 字面是 keyring + 不寫 file backend，所以這個是 nice-to-have 而非必須
- **`AHB_DISABLE_KEYRING=1` env var override** — 不做、用 config 統一 secret_storage 機制
- **`--strict-config` flag（deny_unknown_fields opt-in）** — 不做、warn + ignore 已足夠
- **Interactive first-run walk-through（TTY 偵測下做 wizard）** — Phase 4 polish 或 backlog
- **`AHB_CONFIG_PATH` env var override** — Phase 4 distribution polish 再決定（debug 友善 + CI 友善）
- **`config_dir` vs `preference_dir`** — directories crate 兩個都提供；macOS 上 `preference_dir = ~/Library/Preferences/ahb` 是 plist 慣例，TOML 用 `config_dir = ~/Library/Application Support/ahb` 比較合理；planner 確認
- **ratatui Gauge widget vs 手刻 Paragraph + Span** — Claude's discretion
- **Claude 5h limit 具體 token 數字** — phase-researcher 從 Claude Code 官方文件 / ccusage / claude-code-usage-monitor 挖、planner 訂 `CLAUDE_5H_TOKEN_LIMIT` const
- **`cache_read_input_tokens` 計不計入用量** — 建議跟 Anthropic billing 對齊（cache_creation 計、cache_read 不計、但留 const flag 切換）
- **panic-injection integration test 注入機制** — planner 決定（建議：環境變數 `AHB_DEBUG_PANIC=adapter:claude` 觸發 panic adapter，prod build 不 compile 這條）
- **mpsc channel buffer 大小、EngineEvent 具體 enum 型別** — planner 補

</deferred>

---

*Phase: 1-Engine + Claude + TUI Scaffold*
*Context gathered: 2026-05-23*
