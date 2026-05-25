# Phase 3: Gemini (conditional) + Cache & Refresh Policy - Context

**Gathered:** 2026-05-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 3 ships three互相獨立但同期落地的增量，全部對應已 LOCKED 的前段決定：

1. **Gemini stub adapter（NO-GO path）** — Phase 0 `GEMINI_SPIKE.md` 結論為 **NO-GO**（gemini-cli 0.41.2 不提供 non-interactive、穩定、可 parse 的 stats endpoint；web 端點被 Pitfall 1 永久排除）。Phase 3 把 Phase 2 CR-01 已 land 的 `GeminiUnimplementedProvider`（永遠回 `Err(ProviderError::Unavailable)`）正式定為 v1 Gemini 行為，並交付 README §Gemini status section（D-22 hand-off 字面）。

2. **Per-provider `refresh_interval`（CFG-03 / TUI-03）** — `[providers.<id>] refresh_interval = <seconds>` 進 config schema；engine 在 TUI tick 時依此判斷該 provider 是否到下次 fetch 時點，否則 reuse 上次 `ProviderState` 並繼續 render（fresh-cache hit，**不是 stale**）。

3. **moka stale-on-error cache（SC-4 / TUI-03）** — adapter 在 transient 失敗時，TUI 沿用 cache 中最近一次成功的 `ProviderState` 並在該 row 末尾加 `(stale Ns ago)` 後綴（整行 Yellow）。Cache 為純 in-memory；CLI 短命模式不參與（cache 永遠空）。

**不做：**
- Gemini HTTP adapter（含 ETag / If-Modified-Since / daily ceiling）— NO-GO 已永久排除
- Disk-persisted cache — v2（含 disk format / multi-process 互斥 / 權限 / schema bump）
- `--experimental-gemini` CLI flag — v2 trigger（GEMINI_SPIKE.md § Kill criteria 滿足時重新評估）
- separate `cache_ttl` config field — v1 不分離（D-71）
- HTTP wiremock 200/304/401/429+Retry-After/500/slow 測試 infra — v1 沒有 HTTP adapter 可測；改用 mock provider 間歇失敗驗 stale-on-error
- `--refresh` CLI flag（強制立即 fetch）— v2
- Daemon mode / 持久化 background fetcher — v2 OPS-01
- Distribution polish — Phase 4

**User-observable artifact：** 在 Claude Code + Codex CLI 都裝好的機器上跑 `AHB tui`，看到 claude + codex 兩條 HP bar；故意切網路造成 transient 失敗時，row 不消失、變黃並印 `(stale Ns ago)`；網路回來後 stale 後綴消失、顏色復原。`AHB` / `AHB --json` / `AHB --detailed` 行為與 Phase 2 完全一致（無 stale 字面、無 schema_version bump）。`[providers.gemini] enabled = true` 啟動時看到 `gemini  ERROR: Gemini adapter deferred to v2 — see README §Gemini status` row。

</domain>

<decisions>
## Implementation Decisions

### Gemini stub gating（ADP-05 NO-GO path）

- **D-63 (Gemini stub 唯一 gate = `[providers.gemini] enabled = true`):**
  - **不**新增 `--experimental-gemini` CLI flag
  - 理由：v1 是自用工具；stub 本身 harmless（`Err(Unavailable)`、無 IO、無 secrets 觸碰）；CLI flag 二重守門對「自己改自己 config」場景多餘
  - 既有 push 邏輯（`src/engine/mod.rs:71`）已就緒；Phase 3 不動 engine 端
  - `GeminiUnimplementedProvider` (`src/provider/gemini.rs:31`) 已完整 — Phase 3 對該檔的 code 變動：**僅** error message 字面更新（見 D-65）

- **D-64 (Default template 註解字面更新):**
  - `src/templates/config.toml` 內 `[providers.gemini] enabled = false` 旁註解
  - 從現有 `# Gemini CLI subscription — not yet implemented (Phase 3)`
  - 改為 `# Gemini CLI subscription — deferred to v2 stub (see README §Gemini status)`
  - 反映 Phase 3 確認為 deferred 而非 pending

- **D-65 (README §Gemini status section 字面 LOCKED — 沿用 D-22 hand-off):**
  - 必印於 README.md，section title `## Gemini adapter status — deferred to v2`
  - 內文兩段：(1) 為何 deferred（引用 `.planning/research/GEMINI_SPIKE.md`、列三條 NO-GO 主因：non-interactive trigger 失敗 / `--output-format json` 觸發 LocalAgentExecutor / 無 quota 欄位）+ (2) v2 trigger 條件（指向 GEMINI_SPIKE.md § Kill criteria）
  - 並列 ToS warning 句（SC-2）：明確說「web-scraping `gemini.google.com/usage` 永久排除，理由見 PITFALLS #1」
  - `GeminiUnimplementedProvider::fetch` 的 error reason 與此 section 對齊，建議改為：
    ```
    Gemini adapter deferred to v2 — see README §Gemini status
    ```
    (短句、不換行、符合 Phase 1 `format_one_line` sanitizer 規則)

### Cache 儲存範圍

- **D-66 (純 in-memory moka，不 disk-persist):**
  - cache 只活在 process 生命週期內；行程結束即蒸發
  - **CLI 短命模式**（`AHB` / `AHB --compact` / `AHB --detailed` / `AHB --json`）：cache 永遠空 → 遇 transient error **不** stale-fallback、直接走 Phase 1/2 既有 `format_error_row_colored` ERROR row 路徑
  - **TUI 長駐模式**（`AHB tui`）：cache 在 process 內累積；stale-on-error 完整生效
  - 理由：v1 自用、Claude/Codex 皆 local FS（transient 罕見）、避免 disk format / multi-process 互斥 / cross-OS path / 權限 / schema bump 等 v1 用不到的複雜度
  - v2 若 Gemini GO 後真的有 HTTP adapter，再考慮 disk persist（cross-restart stale window 才有實益）

- **D-67 (Cache key = `ProviderId`):**
  - `moka::sync::Cache<ProviderId, CacheEntry>`（或 `moka::future::Cache`，由 planner 依 engine fan-out 型別決定；建議 sync 版本，避免額外 future await — engine 已是 async 環境但 cache 操作本身瞬時）
  - `CacheEntry = { state: ProviderState, fetched_at: jiff::Timestamp }`
  - 不把 `FetchCtx` / config 揉進 key — 同 provider 同 process 只受一個 enabled 旗標控制
  - 不同 ProviderId 的 cache entry 各自獨立，互不干擾

### Stale indicator UI

- **D-68 (Stale row 渲染路徑唯一在 TUI):**
  - `src/tui/ui.rs` 是唯一 stale-aware 渲染入口
  - `src/cli/render_text.rs`（compact / detailed）、`src/cli/render_json.rs`（JSON）**不**加 stale path
  - JSON `schema_version: 1` **不** bump 到 2、**不**新增 `stale_age_secs` 欄位、**不**改 `status` enum
  - CLI 短命模式無 cache（D-66）→ 沒有 stale 狀態要表達

- **D-69 (TUI stale row 字面 LOCKED):**

  Format（單行）：
  ```
  claude  ██████░░░░ 60% • resets in 2h00m  (stale 32s ago)
  ```
  規則：
  - 共用 Phase 1 `compact_line_colored` 的左半段（bar + percent + countdown）
  - 之後**兩個空格**、`(stale Ns ago)` 後綴（含小括號）
  - `N` = `(now - cache_entry.fetched_at).total(Unit::Second) as u64`，整數秒，向下取整
  - **整行套 Yellow color attribute**（覆蓋原本依 percent threshold 的 Green / Yellow / Red 分段顏色，避免「bar 綠 / 字黃」這種混淆訊號）
  - `--ascii` 模式：bar 字元仍走 Phase 1 ASCII 替換規則（`#`/`-`/`|`），`(stale Ns ago)` 字面**不變**（純 ASCII 已可）
  - `--color=never` / `NO_COLOR=1`：去 Yellow attribute，但 `(stale Ns ago)` **後綴仍顯示**（語意層 vs 視覺層分開；machine consumers 仍能 grep 出 stale）
  - TUI 不受 `--color=never` 影響時的視覺：整行 yellow 顯示

- **D-70 (新增 `RowState::StaleOk` variant，不擴 `ProviderState`):**
  - `src/tui/app.rs` 的 `RowState` enum 加新 variant：
    ```rust
    RowState::StaleOk { state: ProviderState, stale_age_secs: u64 }
    ```
  - `ui::draw` 在 `match RowState` 時可 cheap 拿到 `stale_age_secs`、不需要再從 `jiff::Timestamp` 算
  - **不**把 stale 概念塞進 `ProviderState`（保 DTO 純淨；JSON schema 不被影響、CLI 行為與 Phase 2 等價）
  - `RowState::SchemaDrift` / `Err` 不變
  - `AppState::apply_results` 內部多一層判斷：engine 對該 provider 回 Err + cache 有命中 → 寫入 `StaleOk`；其他組合維持 Phase 1 邏輯

### refresh_interval 語意 & 預設

- **D-71 (refresh_interval = rate-limit cap + cache TTL 同值):**
  - **語意**：provider 下次允許 fetch 的最早時點 = `last_successful_fetch_at + refresh_interval`
  - TUI tick 時 engine 為每個 provider 比對該時點；未到則 **cache hit** → 該 row 繼續 render 上次 `ProviderState`（`RowState::Ok`，**不是 StaleOk** — 因為這是「fresh cache 在 TTL 內」不是「失敗 fallback」）
  - SC-4「cache TTL 與 refresh interval 解耦」這裡讀作：**stale-on-error fallback 不受 TTL 限制**（cache 過期後仍可被 error path 取用、stale row 可遠超 TTL 顯示），不是「兩個 config 欄位分離」
  - 三態時間軸（單一 provider）：
    | t 段 | 條件 | TUI render |
    |---|---|---|
    | `t < last + refresh_interval` | 仍在 TTL 內 | reuse cache → `Ok` (no stale tag) |
    | `t ≥ last + refresh_interval` & fetch succeeds | 過期且成功 | 更新 cache → `Ok` |
    | `t ≥ last + refresh_interval` & fetch fails (transient) | 過期但 fallback | reuse cache → `StaleOk` |
    | 從未成功過 + fetch fails | 無 cache 可用 | `Err` (or `SchemaDrift`) — Phase 1 既有路徑 |

- **D-72 (Per-provider 預設值表):**

  | Provider | 預設 `refresh_interval` | 備註 |
  |---|---|---|
  | claude | 15s | local JSONL；與 Phase 1 TUI tick 一致 |
  | codex  | 15s | local SQLite+JSONL；同上 |
  | mock   | 15s | 測試用 provider |
  | gemini | 15s | stub `Err(Unavailable)`、cache 永不命中；該值無實際效果但保留 default 一致性 |

  - config 未寫 `refresh_interval` 時：用 provider 預設（per-provider const，建議放在各 provider module 內，如 `claude::DEFAULT_REFRESH_INTERVAL_SECS = 15`）
  - **Clamp ≥ 5s**：解析後若 < 5s，一律 raise 到 5s 並 `tracing::warn!("refresh_interval for {id} clamped to 5s (was {raw}s)")`；理由：避免 local FS 被不必要地高頻打、避免 future Gemini-GO 時誤觸 ToS heuristic
  - 上限不設（user 可設 600s / 1h+ / 24h；該 row 大半時間都會 cache hit，符合 TUI-03 「network adapter 可拉長到 ≥5min」精神）

- **D-73 (CLI 不 honor refresh_interval):**
  - `AHB` / `AHB --compact` / `AHB --detailed` / `AHB --json` 一律 fetch（每次都當 `last_fetch_at` 是 epoch，cache 不命中、每次都打）
  - 不新增 `--no-cache` flag — CLI 行為已是「不 cache」
  - 理由：CLI 短命、cache 短暫意義不大；且 user 主動跑 `AHB` 通常就是想看當下最新值

- **D-74 (TUI 全域 tick 仍 15s，不動):**
  - `src/tui/mod.rs` 的 `tokio::time::interval(Duration::from_secs(15))` 不改
  - 全域 tick 是「ratatui event loop + engine fan-out wakeup 頻率」；per-provider 是否真打由 engine 內部依 `refresh_interval` 過濾
  - 等於：全域 tick 15s ≤ 最短 per-provider refresh_interval（15s 預設）→ 不會有 tick 過頻問題；user 把某 provider 拉到 600s 時，每 40 個 tick 才真打一次該 provider，其他 39 個 tick reuse cache

### Claude's Discretion

- **Stale-on-error error variant 分類** — 建議：`Network` + `RateLimited` 走 stale-fallback；`Internal` / `SchemaDrift` / `Unavailable` **不** stale（`SchemaDrift` 上次的值已不適用上游新 schema；`Unavailable` 是 Gemini stub 永遠失敗、stale 也只是延遲使用者察覺）。planner 在 PLAN.md 確認 variant 對照表完整、寫成 helper fn（如 `fn is_transient(err: &ProviderError) -> bool`）。
- **moka builder 具體配置** — `Cache::builder().max_capacity(8)` 即可（provider 數量本來就小）；建議**不**設 `time_to_live` / `time_to_idle`（讓 stale 邏輯純 manual 化、避免 moka 替我們 evict cache entry 反而失去 fallback 來源）。planner 確認 moka 0.12+ API 沒有意外 default eviction。
- **Cache 寫入時機 / 介面層級** — 兩條 path：(a) `Engine::refresh_all` 在收到 `Vec<(ProviderId, Result<...>)>` 後對成功項寫 cache；(b) `engine::fanout::refresh_all_inner` 內部直接寫。planner 依「測試最好寫」選乾淨那條；建議 (a) — fanout 保純 fetch，cache 是 engine 高層 concern。
- **`RowState::StaleOk` 字段命名** — `state` vs `cached_state` vs `last_good`；`stale_age_secs` vs `cached_at: Timestamp`。建議 `{ state, stale_age_secs }` 對應 D-69 字面，planner 微調。
- **TUI stale row 顏色實作** — `ratatui::style::Style::default().fg(Color::Yellow)` 套整個 `Line`，要不要再加 `.add_modifier(Modifier::DIM)` 視覺強化「這不是當下值」由 planner 決；Phase 3 PLAN snapshot 鎖定後不再動。
- **Integration test 形態** — 因應 D-66 + SC-3 wiremock 改寫：建議用一個新的 `IntermittentFailureProvider`（每呼叫第 N 次失敗）配合 TUI snapshot 序列（成功 → 失敗顯 stale → 再成功 stale 消失），驗 D-71 三態時間軸；wiremock 整段 v1 不裝。planner 設計具體 snapshot 結構。
- **`Engine::new` signature** — 是否需要把 `Cache` 注入點打開（測試時注入 `Cache::new(1)` mock）vs `Engine` 內部 own cache。建議內部 own、測試時用 short refresh_interval 模擬時間流；planner 依 BL-01 clock-injection 既有 pattern 決。

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 3 直接 input
- `.planning/ROADMAP.md` § Phase 3 — goal + SC-1..SC-4 + requirement IDs (TUI-03 / CFG-03 / ADP-05)
- `.planning/REQUIREMENTS.md` — TUI-03 / CFG-03 / ADP-05 原句
- `.planning/PROJECT.md` — Active requirement list、Constraints (Refresh budget / Privacy / Distribution)

### Phase 3 critical priors（必讀，否則決策失準）
- `.planning/research/GEMINI_SPIKE.md` **§ Phase 3 hand-off (no-go path)** — D-22 結論字面、stub 行為要求、README §Gemini status section 草稿、v2 trigger 條件
- `.planning/research/PITFALLS.md`:
  - **§ Pitfall 1** — Gemini ToS / account-ban；解釋為何 web-scraping 永久排除
  - **§ Pitfall 4 (per-adapter `Vec<Result>` isolation)** — engine fan-out 失敗隔離契約；Phase 3 cache layer 須保證不破壞此契約
  - **§ Pitfall 12 (per-adapter refresh interval)** — TUI 15s tick × 24h ÷ 15 = 5,760 req/day 的論述，CFG-03 預設值來源

### 前段 Phase 決策（避免決策衝突）
- `.planning/phases/02-codex-output-formats/02-CONTEXT.md`:
  - **§ D-59 exit code grid** — Gemini stub `Err(Unavailable)` 落入 `1` (all fail) 或保留 `0`（只要 ≥1 other provider OK）— Phase 3 cache 不應改變此邏輯
  - **§ D-49..D-52 JSON DTO** — schema_version: 1 鎖定；Phase 3 D-68 確認不 bump
  - **§ D-53..D-56 detailed layout** — Phase 3 stale 不進 detailed CLI path（D-68）
- `.planning/phases/01-engine-claude-tui-scaffold/01-CONTEXT.md`:
  - **§ BL-01 clock injection** — `FetchCtx::now`；Phase 3 cache 的 `fetched_at` 必須來自同一個 clock source
  - **§ BL-02 deterministic provider row order** — Phase 3 不改 row 順序
  - **§ format_error_row_colored / format_one_line** — Phase 3 error reason 字面 sanitizer 規則
  - **§ RowState / AppState::apply_results** — Phase 3 在此擴 `StaleOk` variant 的 anchor

### Code anchors (Phase 3 變更點)
- `src/provider/gemini.rs:31` — `GeminiUnimplementedProvider`（D-65 改 error reason 字面）
- `src/engine/mod.rs:71` — Gemini push site（不動）
- `src/engine/fanout.rs` — `refresh_all_inner`（cache 寫入時機由 planner 決，建議在外層 wrapper）
- `src/config.rs:33` — `ProviderConfig` 加 `refresh_interval: Option<u64>` field
- `src/config.rs:30` — `KNOWN_PROVIDER_FIELD_KEYS` 加 `"refresh_interval"` (forward-compat warning 過 D-38)
- `src/templates/config.toml` — Gemini 註解字面更新 (D-64)
- `src/tui/app.rs` — `RowState` 加 `StaleOk` variant (D-70)；`AppState::apply_results` 擴 cache lookup 路徑
- `src/tui/ui.rs` — 渲染加 stale row 路徑 (D-69)
- `src/tui/mod.rs` — `tokio::time::interval(Duration::from_secs(15))` 不動 (D-74)
- `src/cli/render_text.rs` — Phase 3 不動（D-68 CLI 不參與 stale）
- `src/cli/render_json.rs` — Phase 3 不動（D-68）
- `Cargo.toml` — 新增 `moka = "0.12"` （或 planner 確認 STACK 對齊版本；features 看是否要 sync vs future）

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`GeminiUnimplementedProvider`** (`src/provider/gemini.rs:31`) — Phase 2 CR-01 已完整實作；Phase 3 只改 `error.reason` 字面 (D-65)。Engine push 邏輯也已就緒 (D-63)。
- **`compact_line_colored` / `filled_cells` / `format_countdown` / `id_label`** (`src/cli/render_text.rs`) — Phase 1 已 promoted to `pub(crate)`；Phase 3 TUI stale row 重用其前半段 + 後綴 + Yellow override (D-69)。
- **`format_error_row_colored`** (`src/cli/render_text.rs`) — CLI 短命模式 transient 失敗仍走此 path (D-66)；Phase 3 不擴。
- **`RowState` enum + `AppState::apply_results`** (`src/tui/app.rs`) — Phase 1 已建好雙態（Ok / SchemaDrift / Err）+ clock injection；Phase 3 加 `StaleOk` 是 additive 擴展 (D-70)。
- **`ProviderConfig` + `KNOWN_PROVIDER_FIELD_KEYS`** (`src/config.rs:30-36`) — Phase 1 D-38 forward-compat warning 機制已建好；Phase 3 只需把 `"refresh_interval"` 加進 known key list、`ProviderConfig` 加 `refresh_interval: Option<u64>` field。
- **`FetchCtx::now`** (`src/provider/mod.rs`) — BL-01 clock injection；Phase 3 cache `fetched_at` 須用同一 clock source、不直接呼 `jiff::Timestamp::now()`。

### Established Patterns
- **Per-adapter `Vec<Result>` isolation** (Phase 1 ADP-01 / Pitfall 4) — engine fan-out 每個 provider 各自獨立；Phase 3 cache layer 必須保此契約，**不**讓 cache miss 失敗影響其他 provider。
- **Forward-compat unknown key warn (D-38)** — `ProviderConfig` 對未知 key emit `tracing::warn!` 但不 error；Phase 3 加 `refresh_interval` 進 `KNOWN_PROVIDER_FIELD_KEYS` 即可，**不**讓既有未升級 config 在 Phase 3 報錯。
- **BL-01 clock injection（adapter / TUI 不直接呼 `Timestamp::now()`）** — Phase 3 cache 寫入 `fetched_at` 要走 `ctx.now`；render stale age 用 `AppState::now` snapshot；tests grep-rejection 規則仍要 honor（如 `tests/no_walltime_in_adapter.rs`）。
- **`pub(crate)` render helpers 共用、不複製** (Phase 1 / Phase 2 共識) — TUI stale row 重用 `compact_line_colored` 前半段、不另寫一份 bar 渲染。
- **Snapshot tests via `ratatui::backend::TestBackend`**（Phase 1 / Phase 2 已建）— Phase 3 stale 序列（fresh → stale 出現 → 再 fresh stale 消失）走同一條 snapshot infra。

### Integration Points
- **Cache layer interpose 點** — `Engine` 是高層 owner；建議 `Engine::refresh_all(now)` 內部先檢查 cache TTL、決定該 provider 是否進 fan-out；fan-out 結果寫回 cache。`fanout::refresh_all_inner` 保純 fetch。
- **Config schema extension** — `ProviderConfig` 從 `{ enabled }` 變成 `{ enabled, refresh_interval: Option<u64> }`。所有 four provider config blocks (claude / codex / gemini / mock) 自動受惠（Phase 1 D-38 forward-compat 已涵蓋）。
- **TUI `AppState::apply_results` 擴 cache lookup** — engine 回傳的 `Vec<(ProviderId, Result<_, _>)>` 若某 provider Err 且 cache 有命中 → 寫 `RowState::StaleOk`；否則沿用 Phase 1 邏輯。`apply_results` signature 可能要加 cache 參數（或 engine 直接把 cache snapshot 一起 pass）— planner 選介面層級。
- **Error variant → stale 決策** — `engine` 收到 `Err(ProviderError::X)` 後判斷 X 是否 transient（建議 helper fn `fn is_transient(&ProviderError) -> bool`）；transient + cache 命中 → emit cached state with stale tag；non-transient → 走 Err path。

</code_context>

<specifics>
## Specific Ideas

- **TUI stale row 完整視覺範例**（D-69 落實）：
  ```
  claude  ██████░░░░ 60% • resets in 2h00m
  codex   █████████░ 90% • resets in 1h12m  (stale 47s ago)     ← 此行 Yellow
  gemini  ERROR: Gemini adapter deferred to v2 — see README §Gemini status
  ```
- **`AHB tui` 在 Claude 暫時失敗下的時間軸**（D-71 三態時間軸落實參考）：
  - `t=0s`：成功 fetch、cache 寫入、row 綠色（≥60%）
  - `t=15s`：refresh_interval 到期、嘗試 fetch、succeeds、cache 更新、row 維持
  - `t=30s`：fetch 拋 `Network`、cache 命中、row 變黃 `(stale 0s ago)`（剛剛變 stale）
  - `t=45s`：再 fetch、拋 `Network`、cache 仍命中、row `(stale 15s ago)`
  - `t=60s`：fetch 成功、cache 更新、row 復原綠色
- **README §Gemini status 段落結構建議**（D-65 落實）：兩段、八行左右；第一段三個短句說 NO-GO 原因；第二段一個短句說 v2 trigger 條件 + 一個 link 指 GEMINI_SPIKE.md。

</specifics>

<deferred>
## Deferred Ideas

- **Disk-persist last-good cache** → v2 — 含 disk format / cross-OS path / multi-process 互斥 / 權限 / schema bump；只有 cross-restart stale window 真有用時才裝
- **`--experimental-gemini` CLI flag** → v2 — GEMINI_SPIKE.md § Kill criteria 滿足時（gemini-cli 開非互動 slash command / `--output-format json` 改 thin envelope / OAuth 用戶得 stats endpoint）重新評估
- **Cache trait abstraction**（`Cache` trait + `MemoryCache` + 未來 `DiskCache`）→ v2 — 第二實作出現再抽，避免 YAGNI
- **HTTP wiremock infra (200/304/401/429+Retry-After/500/slow 六種測試)** → v2 — Gemini GO 時實裝；v1 SC-3 改寫為「mock provider 間歇失敗驗 stale-on-error」
- **Separate `cache_ttl` config field** → v2 — 若 stale 視覺體驗需要區分「fresh cache」「expired but still in-window」兩態才考慮；v1 D-71 合一
- **`--refresh` CLI flag**（強制立即 fetch）→ v2 — 若 stale 卡太久要手動戳一下時加
- **`AHB doctor` 子命令**（檢查 cache state / per-provider last_fetch_at）→ v2 debugging 用
- **Daemon mode（背景 fetcher + 持久 IPC）→ v2 OPS-01**
- **Conditional GET / ETag / If-Modified-Since 機制** → v2 — 屬於 HTTP adapter 的工作；v1 沒 HTTP adapter
- **Per-account daily request ceiling** → v2 — 同上

</deferred>

---

*Phase: 3-Gemini (conditional) + Cache & Refresh Policy*
*Context gathered: 2026-05-25*
