# Phase 2: Codex + Output Formats - Context

**Gathered:** 2026-05-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 2 同時交付兩件事，二者互相驗證：

1. **Codex adapter（第二個 real provider）** — read-only 開 `~/.codex/state_*.sqlite` + `busy_timeout`、優先讀 `~/.codex/sessions/**/rollout-*.jsonl`、`rate_limits: null` 視為 unknown，與 Claude / Mock 一起跑同一個 Engine fan-out。
2. **CLI output formats lockdown** — 把 `--compact` / `--detailed` / `--json schema_version:1` 三條輸出路徑與 exit code 0/1/2 全鎖死，**在任何 tmux / Starship / shell pipeline user 開始接 `AHB` 之前**就把契約固化下來。

不做：Gemini adapter（Phase 3 ADP-05）、per-provider refresh interval（Phase 3 CFG-03）、moka cache / stale-on-error（Phase 3）、daemon mode（v2 OPS-01）、distribution polish（Phase 4）、Claude 5h limit auto-detection（Phase 2 仍 hardcoded const）。

User-observable artifact：在裝有 Claude Code + Codex CLI 的機器上跑 `AHB`，看到 claude + codex 兩條 HP bar；`AHB --detailed` 多行顯示 5h + weekly；`AHB --json | jq` 乾淨 round-trip；`AHB --json | grep -E '[A-Za-z0-9]{20,}'` 抓不到任何 secret-shaped 字串；`AHB; echo $?` 在 1+ provider 成功時印 0、全失敗印 1、config/secrets 不可用印 2。

</domain>

<decisions>
## Implementation Decisions

### Codex adapter — data source strategy

- **D-45 (主訊號來源 — JSONL primary + SQLite read-only 補 metadata):**
  - HP signal（token usage / rate_limits）一律走 `~/.codex/sessions/**/rollout-*.jsonl`（append-only、最安全）
  - `state_*.sqlite` 僅供 thread metadata（session start timestamp / current model 等 JSONL 不一定有的欄位）
  - SQLite 開啟 flags：`OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX`
  - `busy_timeout = 250ms`（per Pitfall 3 建議）— fail-fast 不掛住 15s tick
  - **絕不寫**：不 `PRAGMA journal_mode=WAL`、不 INSERT/UPDATE
  - rusqlite 同步 API 跑在 `tokio::task::spawn_blocking` 內、不阻塞 engine 的 tokio executor

- **D-46 (state_*.sqlite version glob — pick-highest + warn):**
  - `glob("~/.codex/state_*.sqlite")` 抓所有 match、依檔名末尾數字 `_N` 倒序排
  - 取最高 N 的檔
  - 若 match 數 > 1（mid-migration coexistence）：`tracing::warn!("multi-version Codex state files detected — picked state_{N}.sqlite, found {list}")`
  - 不中斷 render、不 error row；只是 stderr 留痕跡讓使用者察覺
  - 0 個 match：當作 Codex 未安裝 → `ProviderError::Unavailable { reason: "no ~/.codex/state_*.sqlite found — is Codex CLI installed?" }`

- **D-47 (rate_limits: null 政策 — 純 unknown，不推估):**
  - 任何時刻 rollout 中 `rate_limits` 欄位為 null 或缺、或所有 windows 都 null：判定 `ProviderError::SchemaDrift { missing: vec!["rate_limits"] }`（重用 Phase 1 已有 variant，UI render 路徑也已就緒）
  - **不**從 token_count events 推估 percent（避免「user 以為 100%」的最危險誤導）
  - render 結果：`codex  ▒▒▒▒▒▒▒▒▒▒ ??% • Codex adapter may be out-of-date` —— Phase 1 `format_error_row` SchemaDrift 分支已 LOCKED、`id_label(ProviderId::Codex)` 自動產出 `codex` 標籤
  - 若未來想開「estimate」開關，留作 Phase 3 或 v2 opt-in flag

- **D-48 (Codex HpWindow emit — upstream passthrough):**
  - Codex rollout `rate_limits` 上游回報幾個 window 就 emit 幾個 `HpWindow`
  - `HpWindow.label: Cow<'static, str>` 直接用上游語意對應字串（建議 `"primary"` / `"secondary"` / `"weekly"` 取決於上游 schema，研究階段確認後鎖）
  - 不在 adapter 內排序、不在 adapter 內合併；engine + render 端維持 BL-02 deterministic provider row order，但同 provider 內 window 順序 = adapter passthrough
  - 若上游只有一個 window：detailed 模式下該 provider 只印 header + 1 條 indented bar（不補佔位）

### --json schema (schema_version: 1)

- **D-49 (Stable DTO 與 internal model 解耦):**
  - 新增 `src/cli/render_json.rs`，內定 `JsonRoot` / `JsonProvider` / `JsonWindow` / `JsonError` 四個 DTO 型別
  - 從 `Vec<(ProviderId, Result<ProviderState, ProviderError>)>` 手刻轉換到 DTO
  - schema_version 鎖定 DTO shape — 將來 refactor `ProviderState` 不會意外破壞下游
  - 所有 DTO derive `Serialize`（**不需** `Deserialize`，shell pipeline 不會 round-trip back into AHB）

- **D-50 (頂層結構 — array of objects with `id` field):**
  ```json
  {
    "schema_version": 1,
    "generated_at": "2026-05-25T13:45:22Z",
    "providers": [
      { "id": "claude", "status": "ok", "source": "jsonl", "fetched_at": "2026-05-25T13:45:22Z", "windows": [...] },
      { "id": "codex",  "status": "error", "error": { "kind": "schema_drift", "message": "missing rate_limits", "missing": ["rate_limits"] }, "windows": [] }
    ]
  }
  ```
  - `providers` array 順序遵守 BL-02（Claude=0 / Codex=1 / Gemini=2 / Mock=3），mock 預設不 emit 到 production JSON（除非 config enabled）
  - `generated_at` 格式：RFC3339 / ISO 8601 UTC（`jiff::Timestamp::to_string()` 已是該格式）
  - `id` 值固定 lowercase（snake_case 對應 `ProviderId` 的 serde repr）

- **D-51 (Error envelope — status binary + sub-object):**
  - 每個 provider 物件含 `"status": "ok" | "error"`
  - `status = "ok"` 時：含 `windows: [...]`、`source: "..."`、`fetched_at: "..."`，**不**含 `error` 欄
  - `status = "error"` 時：含 `error: { "kind": "...", "message": "..." }`、`windows: []`，**不**含 `source` / `fetched_at`
  - `error.kind` 取值（snake_case，對應 `ProviderError` variant）：
    - `"unconfigured"` / `"unavailable"` / `"schema_drift"` / `"network"` / `"rate_limited"` / `"internal"`
  - `error.message` 一律 one-line（沿用 Phase 1 `format_one_line` sanitizer 規則）
  - **特例欄位（按 kind 加，未來 additive）：**
    - `schema_drift` → `error.missing: ["field1", ...]`
    - `rate_limited` → `error.retry_after_seconds: <integer>`（若上游有 retry_after）
  - jq 慣用 query：`.providers[] | select(.status == "error") | .id` 一行篩失敗 provider

- **D-52 (schema_version semver 政策):**
  - **Additive（不 bump）：** 新增 provider id、新增 `JsonWindow` 欄位、新增 `error.kind` 取值、新增 `error.<kind-specific>` 子欄位 → 消費者必須 tolerate unknown fields
  - **Breaking（bump 到 2）：** 移除欄位、rename 欄位、改變語意（如 `percent_remaining` 變成 `percent_used`）、改變頂層結構（providers array → map）
  - bump 時策略：新增 `--json-schema=2` flag 並讓 v1 持續輸出一個小段期間（具體 deprecation window 留到實際 bump 時決定，Phase 2 不預寫 code）
  - **README 必須註明：** "v1 schema guarantees: providers array order is stable per BL-02; status field is always present; unknown fields may be added without bumping schema_version — consumers must ignore unknown fields"

### --detailed layout

- **D-53 (Header + indented window lines):**
  ```
  claude
    5h      ██████░░░░ 60% • resets in 2h00m
    weekly  ███░░░░░░░ 30% • resets in 4d06h

  codex
    primary █████████░ 90% • resets in 1h12m
  ```
  - 每 provider：**1 行 header（`id_label(id)`）+ N 行 window**
  - Window line：`  ` (2-space indent) + label（建議 8-char 左對齊或自然空格，planner 微調）+ `  ` (2-space gap) + 共享 compact bar + ` ` + `pct%` + ` • ` + `resets in {countdown}`
  - **Provider 之間空一行**（empty line）便於視覺分群
  - **Provider 失敗（含 SchemaDrift）：** 印 header line + 1 行 indented error row（沿用 `format_error_row_colored` 字面、加 2-space indent 適配 detailed），不印 `windows: []` 空白
  - 空 providers list 仍走 Phase 1 `EMPTY_STATE_HEADING` + `EMPTY_STATE_BODY` 雙行（與 compact 一致）

- **D-54 (Phase 2 補 Claude weekly bar):**
  - `ClaudeProvider` 在 Phase 2 改為 emit **2 個 HpWindow**：`5h` + `weekly`
  - 同一個 JSONL scan pass 計算兩個 window（不增加 IO）：
    - **5h:** Phase 1 D-33 邏輯不動（cluster anchor + `cache_creation_input_tokens` 加總 + `CLAUDE_5H_TOKEN_LIMIT = 44_000`）
    - **weekly:** anchor 規則 + token limit const 留 research / planner 補（建議 anchor = ISO week 第一天 00:00 local、限額待 phase-researcher 從 ccusage / claude-code-usage-monitor 挖出最新 Pro/Max 估值；commit 為 `CLAUDE_WEEKLY_TOKEN_LIMIT` const + 「best-effort estimate; revisit quarterly」註解）
  - compact 模式仍只印 `windows[0]` = 5h（與 Phase 1 行為不變、不破壞 UI-SPEC LOCKED compact line）
  - JSON 模式：兩個 window 都 emit
  - **若 phase-researcher 確認 weekly 限額無可信來源：** fallback 是 emit weekly window 但 `percent_remaining = NaN-marker`（具體標記 planner 決，建議走 SchemaDrift sub-variant 或 `Option<f32>` DTO 路徑），README 註明 weekly 為 best-effort
  - **deferred 子議題：** weekly 限額 const 的具體值 + anchor 規則細節 → 留給 `02-RESEARCH.md` 抓答案、planner 在 PLAN.md Task X 落實

- **D-55 (Window 順序 — adapter passthrough):**
  - 同 provider 內 window 順序 = adapter emit 順序
  - Claude 內鎖 `[5h, weekly]`（D-54）
  - Codex 依上游 rate_limits array 順序 passthrough（D-48）
  - 不在 render 層做「shortest reset 在上」或「lowest % 在上」排序 — 避免與 ground truth 衝突

- **D-56 (--detailed 100% 共享 compact 樣式):**
  - 共享：`BAR_WIDTH = 10`、`U+2588`/`U+2591`/`U+2022`、Green/Yellow/Red threshold (≥30 / 10..30 / <10)、`--ascii` 替換 `#`/`-`/`|`、`--color=auto|always|never` + `NO_COLOR` 邏輯
  - 差異：**只多 header line + 2-space indent + provider 間空行**
  - 重用 Phase 1 `pub(crate) fn filled_cells / format_countdown / id_label / compact_line_colored` — 不複製、不重新設計
  - snapshot test 加新檔（`detailed_*.snap`），不動現有 compact snapshot

### CLI flag composition & exit codes

- **D-57 (--compact / --detailed / --json clap conflicts_with 互斥):**
  - `Cli` struct 三個 flag 透過 clap `conflicts_with_all` 標記互斥
  - 同時下 `--json --detailed`：clap 自動 emit 錯誤 + exit `2`（clap convention）
  - **沒下任何 flag = 預設 compact**（與 Phase 1 `run_compact` 路徑一致、不變）
  - `--compact` flag 顯式存在意義：tmux 設定強制 single-line 時 grep-friendly；行為與不下任何 flag 等價

- **D-58 (--ascii / --color 對 --json 靜默忽略):**
  - `--json` mode 一律：**不上色**（與 Phase 1 `tty::should_colorize_env(json_mode=true)` 約定一致）+ **使用 Unicode 原始字串**（label/source/reason 直接 Unicode，不轉 ASCII fold）
  - 同時下 `--json --ascii` 或 `--json --color=always`：**不 error**、**不 warn**，靜默忽略 `--ascii`/`--color`
  - 理由：`--json` 是「機器消費路徑」，視覺 flag 對它無語意；顯式 error/warn 反而打亂 pipeline
  - `--ascii` / `--color` 與 `--compact` / `--detailed` 正常生效

- **D-59 (Exit code 邊界 mapping):**
  | 情境 | Exit code | 邏輯 |
  |---|---|---|
  | ≥1 provider `Result::Ok` | `0` | success - 至少有訊號 |
  | 全 provider `Result::Err`（含 SchemaDrift / Unavailable / Network / RateLimited / Internal） | `1` | failure - 沒有可用訊號 |
  | Config / Secrets 載入失敗（含 D-41 hard-error path） | `2` | unloadable |
  | **零 provider enabled**（CFG-04 silent skip 後 list 為空） | `0` | 走 Phase 1 EMPTY_STATE 訊息、視為「尚未配置」非錯誤 |
  | clap parse error（如 `--json --detailed`） | `2`（clap default） | usage error |
  | Panic（被 panic-hook 攔下） | 非 0（OS default） | 不顯式管 |

- **D-60 (SchemaDrift 算 fail 不算 degraded success):**
  - 統一規則：`Result::Ok` = success；`Result::Err` = fail（不論哪個 variant）
  - SchemaDrift 雖然 render 出 sentinel line（有視覺訊息），但 `percent_remaining` 不可信、視為「沒拿到有用 signal」、exit code 計入 fail 側
  - 邏輯一致 + jq query 簡單（`.providers[] | select(.status=="ok")` 一行得「真的有 signal」provider）
  - 若未來想分「partial signal」狀態，留作 v2 三態 enum（current/degraded/unknown）— Phase 2 不開

- **D-61 (--help 文件曝光 exit codes):**
  - clap `Cli` struct 加 `after_help` 或 `long_about`，明列：
    ```
    Exit codes:
      0  at least one provider returned data (or no providers configured)
      1  all configured providers failed
      2  config / secrets unloadable, or invalid command-line usage
    ```
  - 與 README EXIT_CODES section 對齊（README 更新由 planner 排）

### SEC-03 enforcement

- **D-62 (CI grep test 涵蓋 --json 輸出):**
  - Phase 1 `tests/secret_leak_subprocess.rs` 已用 `--debug-emit-fake-secret` flag 跑通「Debug + Serialize」雙 path；Phase 2 擴展：
  - 新增 integration test：跑 `AHB --json`（用 Mock provider + 注入 fake secret 到 `Secrets` map 的測試版 API），grep stdout JSON 含：
    1. 字面字串 `"deadbeefcafe1234567890abcdef"` 不出現
    2. `regex` crate `[A-Za-z0-9]{20,}` 連續字元 pattern 不出現
    3. `"[REDACTED]"` 字串 **必須**出現（證明 Secret 確實走 Serialize path）
  - test 必須在 release build 也可跑（用 `#[cfg(debug_assertions)]` 的注入機制需確認，否則改用獨立的 test-only feature flag）

### Claude's Discretion

- **CLAUDE_WEEKLY_TOKEN_LIMIT 具體數字** — phase-researcher 從 ccusage / claude-code-usage-monitor / tokenmix.ai 等社群挖最新 Pro/Max 估值，planner 訂常數值並在註解寫「best-effort estimate; revisit quarterly」
- **Claude weekly anchor 規則** — 建議 ISO week 起始（Mon 00:00 local），但 Anthropic 實際 reset 邊界 phase-researcher 要確認
- **Codex rate_limits schema 上的 window label 字串** — passthrough 上游語意；具體字串名 phase-researcher 從 codex-rs 原始碼 / rollout fixture 確認後 planner 鎖
- **spawn_blocking 包裝層級** — 整個 `CodexProvider::fetch` 包一層 spawn_blocking，或只在 SQLite 段包；planner 選乾淨那條（建議只在 SQLite + JSONL IO 段包，error mapping 維持 async layer）
- **JsonWindow 是否曝 `source: &str` / `bar_color: Option<...>`** — Phase 2 v1 schema 保守起見只包 `label / percent_remaining / reset_at`；planner 視 tmux user feedback 決定是否擴
- **--detailed window label 對齊寬度** — 建議 8 char 左對齊（5h + weekly + primary 等都裝得下）；planner 微調
- **`error.message` 對 `Internal(anyhow::Error)` 的 sanitize 策略** — anyhow chain 可能洩漏 internal path / secret-shaped string；planner 確保只 emit 頂層 Display 字串（不展開 cause chain）

### Folded Todos

無 — todos 流程 0 個 match（`gsd-sdk query todo.match-phase 2` 跳過顯示）。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project & roadmap
- `.planning/PROJECT.md` — Core Value（HP bar 視覺隱喻必要）、Constraints、Out of Scope
- `.planning/REQUIREMENTS.md` — Phase 2 涵蓋 6 個 requirement：CORE-02 / CORE-03 / CORE-04 / CORE-06 / SEC-03 / ADP-04
- `.planning/ROADMAP.md` § Phase 2 — goal、mode=mvp、4 success criteria
- `.planning/STATE.md` — Phase 1 完成紀錄、Accumulated Decisions（D-01..D-44 完整鎖定列表）

### Phase 0 / 1 lock-in
- `.planning/phases/00-spike-spine/00-CONTEXT.md` — D-08..D-14 model.rs contract（ProviderState / HpWindow / ResetInfo / HpUnit / ProviderError）、D-15..D-19 charset & 視覺、D-25 mock format、D-27 panic-hook 契約
- `.planning/phases/01-engine-claude-tui-scaffold/01-CONTEXT.md` — D-28..D-44 engine + Claude + TUI + config + keyring 全 LOCKED；尤其 D-33（cache_creation_input_tokens 加總理由）、D-42（Secret<T> shape）、D-43（CI grep 雙 assert pattern）、D-44（CLAUDE_5H_TOKEN_LIMIT 常數）
- `.planning/phases/01-engine-claude-tui-scaffold/01-UI-SPEC.md` — 全 6 維度 LOCKED：bar-width=10、Green/Yellow/Red 閾值、SchemaDrift sentinel 字面、format_error_row 字面、empty-state 字面
- `.planning/phases/01-engine-claude-tui-scaffold/01-VERIFICATION.md` — BL-01 / BL-02 / BL-03 / WR-06 / WR-08 修補結果
- `.planning/phases/01-engine-claude-tui-scaffold/01-HUMAN-UAT.md` — Phase 1 platform-bound UAT 項目（Phase 2 跑整合測試時可確認）

### Research synthesis（**全部 MUST READ before planning**）
- `.planning/research/SUMMARY.md` — executive synthesis
- `.planning/research/STACK.md` — locked stack（`rusqlite` 0.39 with `bundled` feature、`serde_json` 1、`jiff` 0.2、`tokio::task::spawn_blocking`）
- `.planning/research/ARCHITECTURE.md` — Provider trait shape、engine fan-out 模式、codex-rs 對標
- `.planning/research/PITFALLS.md` — **Pitfall 3（Codex SQLite locking）+ Pitfall 7（ANSI leak to pipes）+ Pitfall 13（exit codes）+ Pitfall 15（TUI non-TTY）必讀**；Pitfall 2 對 SchemaDrift 思路也仍然適用
- `.planning/research/FEATURES.md` — table-stakes vs differentiator（--json 是 table-stakes）

### External docs（impl 階段必查）
- `rusqlite` 0.39 docs.rs — `OpenFlags::SQLITE_OPEN_READ_ONLY` / `SQLITE_OPEN_NO_MUTEX`、`Connection::busy_timeout` 設定方式、`pragma_query_value` 用法
- Codex CLI 原始碼 `openai/codex` — `~/.codex/state_*.sqlite` schema（threads 表 / 欄位）、`~/.codex/sessions/**/rollout-*.jsonl` 的 `token_count` event shape、`rate_limits` 欄位 schema（可能為 array of window objects）、實際 schema 版本 migration 行為
- Codex issue #14880 — `rate_limits: null` 廣泛存在的證據
- Codex issue #21750 / #23848 — SQLite 損壞情境（影響 D-46 multi-version 警告語意）
- `glob` crate 0.3.x docs — `glob_with` options、版本號排序 helper
- `tokio::task::spawn_blocking` docs — spawn / catch panic 行為（影響 D-48 + planner 對 panic-injection test 的策略）
- ccusage / claude-code-usage-monitor / tokenmix.ai — Claude weekly limit 社群估值來源（D-54 const 值）

### Code 慣例（既有 src/ 已立）
- `src/cli/mod.rs` — `Cli` struct + clap derive + `run_compact`；Phase 2 在這裡加 `--compact`/`--detailed`/`--json` flag + `conflicts_with` + 新增 `run_detailed` / `run_json` 分發 + `after_help` exit-code 文件
- `src/cli/render_text.rs` — `compact_line_colored` / `filled_cells` / `format_countdown` / `id_label` / `format_error_row_colored` 已是 `pub(crate)`；Phase 2 新增 `detailed_block` 函式（reuse 全部）
- `src/cli/tty.rs` — `should_colorize_env(color_flag, json_mode)` 已支援 `json_mode=true` 強制關色（Phase 2 `--json` 路徑直接呼叫 `should_colorize_env(_, true)`）
- `src/model.rs` — `ProviderState` / `HpWindow` / `ResetInfo` / `ProviderError` 不動；Phase 2 不破壞既有 serde shape
- `src/provider/mod.rs` — Phase 2 新增 `src/provider/codex/{mod.rs,jsonl.rs,sqlite.rs,window.rs}`，仿照 `src/provider/claude/` 子模組分層
- `src/provider/claude/window.rs` — Phase 2 在這裡加 `WEEKLY_TOKEN_LIMIT` const + weekly anchor 規則 + `compute_weekly_window` fn；不破壞既有 5h 路徑
- `src/secrets.rs` — Codex 目前不需 secret，但 `secrets.get(ProviderId::Codex)` 介面已 wired（D-40），Phase 2 直接呼叫即可
- `src/lib.rs` — crate-root lint floor `deny(unwrap/expect/panic) + warn(pedantic)` 自動套到新 module（不需在 codex 子 module 重複 deny）

### 新增 dep（Phase 2 唯一新增）
- `rusqlite = { version = "0.39", features = ["bundled"] }` — 必要；Cargo.toml comment 註明 `# Phase 2 ADP-04 Codex state DB read-only`
- `glob = "0.3"` — Phase 1 已有，不重複加（cargo tree 確認）
- 不引入 `figment`（toml + serde 已足夠）
- 不引入 `sqlx`（過度且會與 rusqlite 競爭 libsqlite3-sys）

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/cli/render_text.rs` — `pub(crate) fn compact_line_colored / filled_cells / format_countdown / id_label / format_error_row_colored`：Phase 2 `detailed` 與 `json` 路徑全靠 reuse；不會重寫 bar render
- `src/cli/tty.rs::should_colorize_env(color_flag, json_mode)` — 已內建 `--json` 強制 off 規則；Phase 2 `run_json` 直接呼叫 `should_colorize_env(cli.color, true)`（語意自動正確）
- `src/cli/mod.rs::EMPTY_STATE_HEADING / EMPTY_STATE_BODY` — Phase 1 LOCKED 字面；Phase 2 detailed 路徑共用，json 路徑可選擇也 emit（或 schema_version + `providers: []`，planner 視 tmux user 取向決）
- `src/provider/claude/` 子模組架構（`mod.rs` / `jsonl.rs` / `window.rs`）— Phase 2 Codex 仿照（`mod.rs` / `jsonl.rs` / `sqlite.rs` / `window.rs`）
- `src/engine/fanout.rs` — Engine fan-out + per-adapter timeout + Vec<Result> 聚合已 wired；Phase 2 註冊 `CodexProvider` 進 engine 即接入，不動 fan-out 邏輯
- `src/provider/claude/window.rs::WindowBuilder`（如已存在）— 若 Phase 1 已抽出 anchor / cluster 計算 helper，Phase 2 weekly 直接 plug-in；planner 確認

### Established Patterns
- **Vec<Result<...>> aggregation at engine level** — `engine.refresh_all(now) -> Vec<(ProviderId, Result<ProviderState, ProviderError>)>`，Phase 2 不變
- **Wall-clock 注入** — adapter 用 `ctx.now`（jiff::Timestamp Copy）、只有 `main.rs` + `tui_loop` render tick 呼叫 `Timestamp::now()`；Codex adapter 必須走 `ctx.now`，acceptance grep 同樣守住
- **`jiff::Timestamp::since` 顯式 `(Unit::Hour, *now)`** — Phase 0-03 deviation 1 教訓；Codex countdown 計算同樣遵守
- **`ProviderError` serde shape** — `Network { source }` / `Internal { source }` struct variants；Phase 2 不新增 variant（rate_limits null 重用 SchemaDrift）
- **Cow<'static, str> for `ProviderState.source` and `HpWindow.label`** — Phase 2 Codex 用 `Cow::Borrowed("codex-jsonl")` / `Cow::Borrowed("primary")` 等靜態字串
- **Phase 1 keyring wired but Codex needs no secret** — `secrets.get(ProviderId::Codex)` 返回空 Secret，Codex adapter 不查詢任何 secret API
- **`Secret<T>` redact 雙 path** — `Debug` → `***`、`Serialize` → `[REDACTED]`、`serde(skip)` 防意外 deserialize；Phase 2 JsonProvider DTO 若帶任何 Secret-typed 欄位必須走 Serialize path（Phase 2 不預期出現此情境，但 SEC-03 test 強制守住）
- **clap derive 模式** — `Cli` struct + `Command` subcommand enum；Phase 2 加 flag 沿用 derive 不切手刻 parse
- **Cargo.toml dep 註解模式** — 每加一個 dep 在 Cargo.toml comment 註明 phase 來源（如 `# Phase 2 ADP-04 Codex state DB`）

### Integration Points
- `src/lib.rs` — Phase 2 在這裡 export `codex` adapter module
- `src/cli/mod.rs::run_compact` — Phase 2 加 `run_detailed` / `run_json` 平行函式；`main.rs` 根據 flag 分發
- `src/main.rs` — Phase 2 修 dispatch logic（match flag → call 三者之一），但 `install_phase0_panic_hook()` 第一行不動
- `tests/secret_leak_subprocess.rs` — Phase 2 新增 `--json` path 的 grep 測試（D-62）
- `Cargo.toml` — Phase 2 加 `rusqlite` dep 與 comment
- `tests/integration_*.rs` — Phase 2 新增 Codex 整合測試（spawn codex CLI in parallel + assert AHB not crash）；新增三條 output format 的 stdout snapshot test
- `.github/workflows/ci.yml` — 不動（Phase 4 才補 fmt-check / audit / deny）

</code_context>

<specifics>
## Specific Ideas

- **Codex SchemaDrift sentinel 字面（複用 Phase 1 LOCKED 形式，label 自動換）：**
  - render: `codex  ▒▒▒▒▒▒▒▒▒▒ ??% • Codex adapter may be out-of-date`
  - 沿用 `format_error_row_colored` SchemaDrift 分支，`id_label(ProviderId::Codex)` 自動產 `codex`，無需 hardcode
  - 若覺得 `Claude adapter may be out-of-date` 字面對 Codex 不對，再 generalize 為 `{label-cased} adapter may be out-of-date` — planner 看訴求調

- **Codex unavailable 字面（建議；planner 鎖時可調）：**
  - render: `codex  ERROR: no ~/.codex/state_*.sqlite found — is Codex CLI installed?`
  - 「next-step hint 必須有」這條 UI-SPEC LOCKED 規則繼續適用

- **Codex Pitfall 3 守衛測試（建議 planner 排）：**
  - 整合測試：`tokio::process::Command::new("codex").arg("exec").arg("/status")` 在背景跑、同時 5 次連續 `engine.refresh_all()`、assert 全部 Ok 或最差是 RateLimited（不是 Network / Internal「database is locked」）
  - CI 上若無 codex CLI 安裝：fallback 用 `tempfile::tempdir` 建 fake `state_5.sqlite` + 另一個 writer process 持 RESERVED lock 模擬

- **--json 範例輸出（成功 + 失敗 mix）：**
  ```json
  {
    "schema_version": 1,
    "generated_at": "2026-05-25T13:45:22Z",
    "providers": [
      {
        "id": "claude",
        "status": "ok",
        "source": "jsonl",
        "fetched_at": "2026-05-25T13:45:22Z",
        "windows": [
          { "label": "5h",     "percent_remaining": 60.0, "reset_at": "2026-05-25T15:45:22Z" },
          { "label": "weekly", "percent_remaining": 30.0, "reset_at": "2026-05-29T20:00:00Z" }
        ]
      },
      {
        "id": "codex",
        "status": "error",
        "error": {
          "kind": "schema_drift",
          "message": "missing rate_limits",
          "missing": ["rate_limits"]
        },
        "windows": []
      }
    ]
  }
  ```
  - 注意 `windows` 永遠是 array（不會省略；失敗時 `[]`）
  - `windows[].reset_at` 用 jiff RFC3339 UTC、不用 `resets_at` 名（與 internal `ResetInfo.resets_at` 略不同，避免 schema 上的 plural-`s`；planner 確認偏好）

- **--detailed 範例輸出：**
  ```
  claude
    5h      ██████░░░░ 60% • resets in 2h00m
    weekly  ███░░░░░░░ 30% • resets in 4d06h

  codex
    primary █████████░ 90% • resets in 1h12m

  ```
  - 末行不額外空行；最後一個 provider 後不重複 newline
  - error 變體：`  codex  ERROR: ~/.codex/state_*.sqlite locked — codex process actively writing` 縮排到 indent 行

- **default-config.toml 擴更新（建議）：**
  ```toml
  [providers.codex]
  enabled = false  # Codex CLI subscription — reads ~/.codex/sessions/**/rollout-*.jsonl + state_*.sqlite (read-only)
  ```
  Phase 1 註解 `not yet implemented (Phase 2)` 改為實作描述

</specifics>

<deferred>
## Deferred Ideas

These came up during discussion and belong in later phases or backlog:

- **`bar_color: Option<BarColor>` hint passthrough 到 JsonWindow** — Phase 2 不曝（UI hint，shell pipeline 不需）；若 tmux user 想自訂顏色，留作 Phase 3+ 加 JsonWindow.bar_color_hint 子欄位（additive，不需 bump schema_version）
- **`generated_at` 是否含 timezone offset** — Phase 2 鎖 UTC（`Z` 結尾、jiff default）；若 user 抱怨可加 `--json-local-tz` flag，留 Phase 4 distribution polish 考慮
- **Codex rate_limits estimate fallback（從 token_count events 推估）** — D-47 不做；留 Phase 3 或 v2 opt-in flag（`--codex-estimate-when-null`）
- **三態 status enum（current / degraded / unknown）取代 ok/error 二態** — D-60 不做；留 v2 schema_version=2 時統一改
- **`source` 欄位曝 JSON（如 `"jsonl"` / `"sqlite-state-5"` / `"jsonl+sqlite"`）** — Phase 2 預設曝（已在範例 schema），但具體值字串 planner 視 Codex adapter 實作完整度再鎖
- **`fetched_at` 與 `generated_at` 是否合併** — Phase 2 保持分開（fetched_at = 該 provider fetch 完成時刻、generated_at = JSON envelope 產生時刻，差異 1-2ms）；不合併方便 user 看「資料新舊」獨立於「render 時刻」
- **`--json --pretty` flag** — 預設 compact JSON（無縮排，pipe-friendly）；`--pretty` 留 Phase 4 polish 加（jq 自己會 pretty-print）
- **`AHB --json --watch <secs>`（subprocess polling）** — 不做；user 自己 `watch -n 15 AHB --json` 即可
- **SIGPIPE 處理（user 跑 `AHB | head -1`）** — Phase 2 不顯式管；Rust default 對 SIGPIPE 是 panic，但 panic-hook 已就緒；planner 視 verification 結果決定是否加 explicit `std::process::exit(0)` on broken pipe
- **`error.message` Internal variant 展開 cause chain** — D-49 鎖只 emit 頂層 Display；不展 anyhow chain（避免洩漏 internal path）
- **Schema-drift sentinel 字面 generalize（從 hardcoded `Claude adapter may be out-of-date` 改為 `{Label} adapter may be out-of-date`）** — 若 planner 看了發現 Codex 用「Claude adapter」字樣不對，generalize；UI-SPEC 需同步更新
- **Codex weekly window 是否存在** — Codex 上游可能只有 5h-style window 沒 weekly；passthrough 規則自然處理，但 README 或 detailed 文件描述要對齊上游實情（research 階段確認後 planner 寫 README section）
- **`schema_version=2` migration 具體 deprecation window 設計** — D-52 不預寫；實際 bump 時再決定

### Reviewed Todos (not folded)
無 — `gsd-sdk query todo.match-phase 2` 跳過顯示，無 todos 待 review。

</deferred>

---

*Phase: 2-Codex + Output Formats*
*Context gathered: 2026-05-25*
