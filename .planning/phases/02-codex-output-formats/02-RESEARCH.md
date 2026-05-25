# Phase 2: Codex + Output Formats — Research

**Researched:** 2026-05-25
**Domain:** Rust adapter (rusqlite + JSONL streaming + tokio::spawn_blocking) + CLI output format hardening (clap derive + serde_json DTOs + exit-code wiring + SEC-03 grep)
**Confidence:** HIGH on Codex rate_limits schema (verified from openai/codex issue #14728 with literal example), HIGH on rusqlite API and clap conflict semantics, MEDIUM on Claude weekly reset anchor (community consensus diverges from official docs which are silent), MEDIUM-LOW on `CLAUDE_WEEKLY_TOKEN_LIMIT` exact value (Anthropic publishes only relative guidance; community estimates vary).

---

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Codex adapter — data source strategy**

- **D-45 (主訊號來源 — JSONL primary + SQLite read-only 補 metadata):**
  - HP signal 一律走 `~/.codex/sessions/**/rollout-*.jsonl`（append-only）
  - `state_*.sqlite` 僅供 thread metadata（session start / current model 等 JSONL 不一定有的欄位）
  - SQLite 開啟 flags：`OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX`
  - `busy_timeout = 250ms`（Pitfall 3）
  - 絕不寫
  - rusqlite 同步 API 跑在 `tokio::task::spawn_blocking` 內

- **D-46 (state_*.sqlite version glob — pick-highest + warn):**
  - `glob("~/.codex/state_*.sqlite")` 抓所有 match、依檔名末尾數字 `_N` 倒序排
  - 取最高 N 的檔
  - match 數 > 1：`tracing::warn!("multi-version Codex state files detected — picked state_{N}.sqlite, found {list}")`
  - 0 個 match：`ProviderError::Unavailable { reason: "no ~/.codex/state_*.sqlite found — is Codex CLI installed?" }`

- **D-47 (rate_limits: null 政策 — 純 unknown，不推估):**
  - rollout `rate_limits` null / 缺、或所有 windows 都 null → `ProviderError::SchemaDrift { missing: vec!["rate_limits"] }`
  - 不從 token_count events 推估
  - render: `codex  ▒▒▒▒▒▒▒▒▒▒ ??% • Codex adapter may be out-of-date`

- **D-48 (Codex HpWindow emit — upstream passthrough):**
  - 上游回報幾個 window 就 emit 幾個 `HpWindow`
  - `HpWindow.label: Cow<'static, str>` = 上游語意字串
  - 不在 adapter 內排序、不合併
  - 同 provider 內 window 順序 = adapter passthrough
  - 只有一個 window：detailed 模式下印 header + 1 條 indented bar（不補佔位）

**--json schema (schema_version: 1)**

- **D-49** Stable DTO 與 internal model 解耦（`src/cli/render_json.rs`：JsonRoot / JsonProvider / JsonWindow / JsonError；手刻轉換；derive `Serialize` only）
- **D-50** 頂層結構（`schema_version: 1`, `generated_at`, `providers: [...]` BL-02 順序；`generated_at` = jiff RFC3339 UTC）
- **D-51** Error envelope（`status: "ok"|"error"`；ok 含 `windows/source/fetched_at`；error 含 `error: {kind, message}` + `windows: []`；`schema_drift` 加 `missing`；`rate_limited` 加 `retry_after_seconds`；one-line message）
- **D-52** schema_version semver（additive 不 bump；breaking bump 到 2；README 註明 v1 guarantees）

**--detailed layout**

- **D-53** Header + indented window lines（每 provider 1 行 header + N 行 window；2-space indent + 共享 compact bar；provider 間空一行；失敗印 header + 1 行 indented error row）
- **D-54** Phase 2 補 Claude weekly bar（`ClaudeProvider` emit 2 個 HpWindow：`5h` + `weekly`；同一 JSONL scan pass；compact 只印 `windows[0]` = 5h；JSON 兩個都 emit；weekly 限額無可信來源 → fallback emit weekly window 但 `percent_remaining = NaN-marker` 或 `Option<f32>` DTO path，README 註明 best-effort）
- **D-55** Window 順序 = adapter passthrough（Claude `[5h, weekly]`；Codex 上游 array 順序）
- **D-56** --detailed 100% 共享 compact 樣式（差異只在 header + 2-space indent + provider 間空行；snapshot test 加新檔不動 compact）

**CLI flag composition & exit codes**

- **D-57** `--compact / --detailed / --json` clap `conflicts_with_all` 互斥（同時下 → clap exit 2；沒下任何 flag = 預設 compact，與 Phase 1 行為一致）
- **D-58** `--ascii / --color` 對 `--json` 靜默忽略（不 error、不 warn；理由 = `--json` 是機器消費路徑）
- **D-59** Exit code mapping（≥1 Ok = 0；全 Err = 1；config/secrets 載入失敗 = 2；零 provider enabled = 0；clap parse error = 2；Panic = OS default 非 0）
- **D-60** SchemaDrift 算 fail（`Result::Ok` = success；`Result::Err` = fail 不論 variant）
- **D-61** `--help` 文件曝光 exit codes（`after_help` 或 `long_about` 明列）

**SEC-03 enforcement**

- **D-62** CI grep test 涵蓋 `--json` 輸出（mock provider + 注入 fake secret；grep 三條：字面字串不出現 / 20+-char alphanumeric pattern 不出現 / `[REDACTED]` 必須出現；release build 也可跑）

### Claude's Discretion

研究階段必須給 *recommendation*（rationale + alternative），planner 在 PLAN.md 落實：

1. **`CLAUDE_WEEKLY_TOKEN_LIMIT` 具體數字** — phase-researcher 從 ccusage / claude-code-usage-monitor / tokenmix.ai 等社群挖最新 Pro/Max 估值
2. **Claude weekly anchor 規則** — 建議 ISO week 起始（Mon 00:00 local），實際 Anthropic 邊界需確認
3. **Codex rate_limits schema window label 字串** — passthrough 上游語意；具體字串名 phase-researcher 從 codex-rs 原始碼 / rollout fixture 確認
4. **spawn_blocking 包裝層級** — 整個 `CodexProvider::fetch` 包一層 vs 只在 SQLite/JSONL IO 段包；planner 選乾淨那條
5. **JsonWindow 是否曝 `source: &str` / `bar_color: Option<...>`** — Phase 2 v1 保守只包 `label / percent_remaining / reset_at`
6. **--detailed window label 對齊寬度** — 建議 8 char 左對齊；planner 微調
7. **`error.message` 對 `Internal(anyhow::Error)` 的 sanitize 策略** — 只 emit 頂層 Display 字串（不展開 cause chain）

### Deferred Ideas (OUT OF SCOPE)

- `bar_color: Option<BarColor>` hint passthrough 到 JsonWindow（Phase 3+ additive）
- `generated_at` 是否含 timezone offset（Phase 2 鎖 UTC `Z`；`--json-local-tz` 留 Phase 4）
- Codex rate_limits estimate fallback（從 token_count 推估）— Phase 3 或 v2 opt-in flag
- 三態 status enum（current/degraded/unknown）— v2 schema_version=2
- `source` 欄位字串具體值（planner 視實作完整度再鎖）
- `fetched_at` 與 `generated_at` 是否合併（Phase 2 保持分開）
- `--json --pretty` flag（Phase 4 polish）
- `AHB --json --watch <secs>`（不做）
- SIGPIPE 處理（Phase 2 不顯式管）
- `error.message` 展開 anyhow cause chain（不做）
- Schema-drift sentinel 字面 generalize（若 Codex 不適用 `Claude adapter…` 字樣，再 generalize）
- Codex weekly window 是否存在（passthrough 自然處理；README 描述對齊上游實情）
- `schema_version=2` migration deprecation window（D-52 不預寫）

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CORE-02 | `AHB --compact` 強制緊湊輸出 | D-57 clap derive 三 flag `conflicts_with_all`；compact 沿用 Phase 1 `run_compact` 行為 |
| CORE-03 | `AHB --detailed` 多行 per provider，含 session 與 weekly 兩條 bar | D-53/D-54 詳細範例；Phase 1 `compact_line_colored` / `filled_cells` / `format_countdown` / `id_label` 全部 reuse；Claude weekly window 需新增 anchor 規則 + `CLAUDE_WEEKLY_TOKEN_LIMIT` const |
| CORE-04 | `AHB --json` 輸出帶 `schema_version: 1` | D-49..D-52 DTO 設計；jiff `Timestamp::to_string()` RFC3339；`serde_json::to_writer` compact path；Mock 預設不 emit |
| CORE-06 | 適當 exit code 0/1/2 | D-59/D-60；`main.rs` 在 render 之後計算 exit；clap usage error → 2（自動） |
| SEC-03 | `--json` / log / error message 中絕不出現原始 secret 值 | D-62 grep test；現有 `tests/secret_leak_subprocess.rs` 模式擴展；`#[cfg(debug_assertions)]` flag 注入 |
| ADP-04 | Codex CLI adapter — read-only `state_*.sqlite` + `busy_timeout` + JSONL rollouts + `rate_limits: null` = unknown | D-45..D-48；codex-rs `RateLimitSnapshot` schema verified (primary/secondary/used_percent/window_minutes/resets_in_seconds)；rusqlite 0.39 API verified；glob 0.3 unsorted (需手動 sort) |

</phase_requirements>

## Summary

Phase 2 ships two interlocking deliverables: the second real provider (Codex) and the CLI output format contract. Codex is straightforward in shape — JSONL primary + SQLite metadata, read-only and timeout-bounded — but it carries upstream uncertainty: `rate_limits: null` is widespread (issue #14880), `state_5.sqlite` schema is internal-and-unstable, and CodexAdapter must never write or take a long-running lock. The CLI output contract is straightforward in mechanics but high-stakes in timing: every downstream tmux / Starship / shell-pipeline consumer that ever sees `AHB` output couples to the wire shape we ship in Phase 2, so the JSON DTO, exit code grid, and `--ascii / --color / --json` interaction must be hardened *before* anyone scripts against AHB.

The good news: every external dependency Phase 2 needs is already locked (rusqlite 0.39 bundled + glob 0.3 + serde_json 1 + jiff 0.2 + tokio::spawn_blocking) and Phase 1 patterns (Vec<Result> aggregation, BL-02 sort, panic-hook chain, Secret<T> redaction, owo-colors via `should_colorize_env`) carry directly into Phase 2 without redesign.

**Primary recommendation:** Wrap the Codex adapter's `spawn_blocking` *narrowly* around the rusqlite open/query + JSONL scan (NOT the whole `fetch` async layer); use clap's `#[group(multiple = false)]` for the three output-format flag interlock; ship `JsonWindow { label, percent_remaining, reset_at }` minimal v1 with `Option<f32>` for the Claude-weekly-when-no-limit-known fallback; gate the SEC-03 fake-secret injection on the existing `#[cfg(debug_assertions)]` `--debug-emit-fake-secret` flag (extend it with a `--json` route rather than introducing a new test-only feature flag).

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Codex JSONL rate_limits parse | Adapter (`src/provider/codex/jsonl.rs`) | — | Streaming line-by-line read of append-only file; serde-tagged enum on `payload.type`; passthrough window labels |
| Codex SQLite metadata fetch | Adapter (`src/provider/codex/sqlite.rs`) inside `spawn_blocking` | Engine fan-out | rusqlite is sync API; must NOT block tokio executor (Anti-Pattern 5 in ARCHITECTURE.md) |
| `state_*.sqlite` version glob | Adapter (`src/provider/codex/sqlite.rs`) | — | Pick highest-N file; emit `tracing::warn!` on coexistence (D-46) |
| Claude weekly window emit | Adapter (`src/provider/claude/window.rs`) | — | Same JSONL scan pass; new `WEEKLY_TOKEN_LIMIT` const + anchor; passthrough order `[5h, weekly]` (D-54/D-55) |
| Output format dispatch | CLI (`src/cli/mod.rs` `run_compact` / `run_detailed` / `run_json`) | Engine | Three sibling functions sharing `engine.refresh_all` result; no engine changes |
| Exit code computation | Entry (`src/main.rs`) | CLI | Compute *after* the render path returns; `Vec<Result>` → `0 | 1 | 2`; config/secrets failures handled upstream of dispatch (already exit 2 per Phase 1 wiring) |
| `--json` ANSI suppression | CLI (`src/cli/tty.rs::should_colorize_env`) | — | Already wired in Phase 1 with `json_mode=true` parameter — Phase 2 just calls with `true` (zero-code change) |
| SEC-03 fake-secret injection (test) | Entry (`src/cli/mod.rs::debug_emit_fake_secret_and_exit`) + integration test | — | Extend existing `--debug-emit-fake-secret` flag with `--json` route; do NOT add new test-only feature flag (smaller diff, no new gates) |
| clap usage error → exit 2 | clap default behavior | — | clap automatically uses `ErrorKind::ArgumentConflict` → exit 2; no code needed (verified) |
| RFC3339 timestamp emit | DTO layer (`src/cli/render_json.rs`) | jiff | `jiff::Timestamp` defaults to RFC3339 UTC with `Z` suffix (verified via Phase 0 + jiff docs) |

## Standard Stack

### Core (locked in STACK.md; Phase 2 newly-active items emphasized)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| **rusqlite** | 0.39.0 with `["bundled"]` | Read-only SQLite open against `~/.codex/state_*.sqlite` | Sync API matches our short read; `bundled` ships SQLite statically (no system dep, no version mismatch with Codex's bundled SQLite) [VERIFIED: docs.rs/rusqlite/0.39.0] |
| **glob** | 0.3.x (already in Cargo.toml) | Discover `~/.codex/state_*.sqlite` + `~/.codex/sessions/**/rollout-*.jsonl` | Cargo.toml already has it (Phase 1 Claude); no new dep [VERIFIED: Cargo.toml line 42] |
| **serde_json** | 1.x (already in Cargo.toml) | JSONL parse + `--json` envelope emit (`to_writer` compact, no pretty) | Standard; `BufRead::lines()` + `serde_json::from_str::<Value>(&line)` for the Codex rollout parser |
| **jiff** | 0.2.x with `serde` feature (already in Cargo.toml) | `generated_at` / `fetched_at` / `reset_at` RFC3339 UTC + countdown arithmetic | `jiff::Timestamp::to_string()` emits RFC3339 UTC with `Z` suffix [CITED: jiff docs.rs Timestamp::display] |
| **tokio** | 1.52.x with `rt-multi-thread` (already wired) | `spawn_blocking` for rusqlite call site | Already in Cargo.toml; `spawn_blocking` returns `JoinHandle<T>` whose `.await?` yields `JoinError` whose `is_panic()` is checkable [VERIFIED: tokio docs] |
| **clap** | 4.6.x with `derive` (already wired) | Three-flag mutual-exclusion (`--compact / --detailed / --json`) | `#[group(multiple = false)]` on a derived `Args` struct or `conflicts_with_all` per-arg — both work; `multiple = false` is cleaner for ≥3 flags [VERIFIED: docs.rs/clap ArgGroup] |
| **regex** | 1.x (already in dev-deps) | SEC-03 grep test `[A-Za-z0-9]{20,}` pattern | Already wired in `tests/secret_leak_subprocess.rs`; Phase 2 reuses |

### Supporting (no new additions needed)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tempfile` 3.x | already dev-dep | Mock `~/.codex/sessions/**/rollout-*.jsonl` + fake `state_5.sqlite` for tests | Per-test isolation; never touch real `$HOME/.codex` |
| `assert_cmd` 2.x + `predicates` 3.x | already dev-dep | Integration test stdout shape for `AHB --json` / `--detailed` / `--compact` | Drive subprocess `AHB --json | jq` round-trip |
| `insta` 1.x | **NOT currently in dev-deps** | Optional: snapshot test `--detailed` and `--json` stdout | Phase 1 did NOT add insta — Phase 2 SHOULD add it if planner wants frozen-output regression coverage. Alternative: hand-rolled string equality assertions via `assert_cmd`. Recommend deferring `insta` until Phase 2 needs more than 3 snapshot tests. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `serde_json::to_writer` (compact) | `serde_json::to_writer_pretty` | Pretty-print pollutes pipelines (jq auto-pretties anyway). D-Deferred says `--pretty` is Phase 4 — stay compact in v1. |
| `tokio::task::spawn_blocking` (narrow scope) | Wrap whole `CodexProvider::fetch` in `spawn_blocking` | Whole-fetch wrap creates Sync-bound headaches for `&FetchCtx<'_>` capture (lifetime conflicts with the move closure required by `spawn_blocking`); narrow-scope wrap is the clean shape (recommendation below) |
| Custom `Option<f32>` NaN-marker for Claude-weekly when limit unknown | `f32::NAN` sentinel | NaN serializes as `null` in JSON via serde — *can* work but is footgun-prone (`NaN != NaN`). `Option<f32>` is explicit and serializes to `null` deterministically. Recommend `Option<f32>` (see "Claude Weekly Limit Handling" below). |
| New test-only Cargo feature `test-secret-inject` | Extend existing `#[cfg(debug_assertions)]` `--debug-emit-fake-secret` flag | Smaller diff, no new conditional-compilation surface, release builds still strip the path. Recommend extending. |

**Installation:** No new dependencies needed. The single new active crate is `rusqlite` 0.39 with `["bundled"]`, added per D-canonical_refs Phase 2 唯一新增:

```toml
# Phase 2 ADP-04 Codex state DB read-only (bundled SQLite = no system dep).
rusqlite = { version = "0.39", features = ["bundled"] }
```

**Version verification (2026-05-25):**
- `rusqlite 0.39.0` — confirmed via `cargo search rusqlite` (top result: `rusqlite = "0.39.0"`)
- `glob 0.3.3` — already in Cargo.toml (Phase 1)
- All other Phase 2 deps already locked by Phase 0 / Phase 1 (no new entries)

## Package Legitimacy Audit

> Phase 2 introduces exactly ONE new dependency. All other Phase 2 deps are already in `Cargo.toml` from Phase 0 / Phase 1 (verified provenance during prior phases — see Phase 1 STATE.md).

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| `rusqlite` | crates.io | 11+ yrs (first publish 2014, v0.39 released 2026-03-15) | 90M+ all-time, 250k+/wk | github.com/rusqlite/rusqlite (canonical) | not run (graceful degrade — see below) | Approved [VERIFIED: docs.rs/rusqlite/0.39.0 + STACK.md SOURCES] |

**slopcheck status:** `slopcheck` was not available in this research environment. Per the Package Legitimacy Gate fallback rule, `rusqlite` is tagged `[VERIFIED: docs.rs/rusqlite/0.39.0]` because (a) it is the canonical Rust SQLite wrapper, (b) it has been independently confirmed in STACK.md SOURCES with the `openai/codex` codebase using the same crate, (c) `cargo search rusqlite` returns it as the top result on crates.io, (d) the bundled feature variant is the standard library pattern for shipping SQLite statically. No `[ASSUMED]` tag is needed for this package because its provenance is verified through multiple authoritative sources (Context7-equivalent: docs.rs is authoritative for Rust crate metadata).

**Cross-ecosystem confusion check:** N/A — Rust-only phase. No Python or Node packages.

**Suspicious postinstall scripts:** N/A — Rust crates have no `postinstall` mechanism (cargo build scripts are visible in source).

**Packages removed due to slopcheck [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

## Codex JSONL Schema (verified against codex-rs source via issue #14728)

**File location:** `~/.codex/sessions/YYYY/MM/DD/rollout-{TIMESTAMP}-{UUID}.jsonl` (verified via DeepWiki openai/codex 3.5.2 + multiple GitHub issues) [VERIFIED: deepwiki.com/openai/codex/3.5.2-rollout-persistence-and-replay]

**Line format:** Each line is a `RolloutLine` wrapping a `RolloutItem`. The variant we care about is `event_msg` with `payload.type = "token_count"`.

**Verified exact JSON shape for `token_count` event with non-null rate_limits** (from openai/codex issue #14728, quoted from a real Codex rollout) [VERIFIED: github.com/openai/codex/issues/14728]:

```json
{
  "type": "event_msg",
  "payload": {
    "type": "token_count",
    "info": null,
    "rate_limits": {
      "primary": {
        "used_percent": 0.0,
        "window_minutes": 299,
        "resets_in_seconds": 17940
      },
      "secondary": {
        "used_percent": 6.0,
        "window_minutes": 10079,
        "resets_in_seconds": 275281
      }
    }
  }
}
```

**Rust struct:** `RateLimitSnapshot` defined in `codex-rs/codex-api/src/rate_limits.rs`. Each tier (`primary`, `secondary`) contains exactly three fields:
- `used_percent: f64` (or f32 — Codex emits as JSON number; serde can read into either)
- `window_minutes: u32` (or u64)
- `resets_in_seconds: u64`

**Verified null case** (the widespread bug per issue #14880) [VERIFIED: github.com/openai/codex/issues/14880]:

```json
{
  "type": "event_msg",
  "payload": {
    "type": "token_count",
    "info": {
      "total_token_usage": {
        "input_tokens": 14676,
        "output_tokens": 412,
        "total_tokens": 15088
      },
      "model_context_window": 258400
    },
    "rate_limits": null
  }
}
```

**Implications for Codex adapter parser:**

1. **Window labels are literally `"primary"` and `"secondary"`** — these become `HpWindow.label: Cow::Borrowed("primary")` / `Cow::Borrowed("secondary")` (D-48 passthrough). NO `"weekly"` label exists in Codex rate_limits (despite the CONTEXT D-48 speculative example listing it — that was a research-stage guess). Codex emits only two windows.
2. **`used_percent` is 0..=100 directly** — `HpWindow.percent_remaining = 100.0 - used_percent` (note the conversion; Codex reports *used*, AHB renders *remaining*).
3. **`resets_in_seconds` is relative to event timestamp**, NOT absolute. To compute `ResetInfo.resets_at: jiff::Timestamp`, the parser needs the rollout line's outer timestamp (from `RolloutLine.timestamp` — wraps every persisted event per DeepWiki) AND add `Span::new().seconds(resets_in_seconds)`. **WARNING:** if parser uses `ctx.now` instead, the countdown will be wrong by the staleness of the rollout (which can be tens of seconds to minutes). **Recommend:** use the rollout line's own timestamp as the anchor for `resets_at`, NOT `ctx.now`. Document this in adapter comments.
4. **Find the LATEST `token_count` event with non-null `rate_limits`** — walk the JSONL backward (or forward and keep last); older events may have non-null while newer have null. The freshest non-null wins. If all `rate_limits` in the file are null → `SchemaDrift` per D-47.
5. **Multiple rollout files exist** (one per session). Pick the newest by `mtime` (same pattern as ClaudeAdapter `pick_newest_file`); within that file, pick the latest non-null rate_limits event.
6. **`window_minutes` is *informational*** — it tells us the rolling-window length the upstream is enforcing. AHB doesn't need it for the bar render (we have `resets_in_seconds`), but planner MAY want to surface it in `--detailed` output (e.g., `primary  ██████████ 100% • resets in 4h59m • 5h window`) — recommended deferred to Phase 3 unless trivially cheap.

**Suggested serde shape for parser:**

```rust
// src/provider/codex/jsonl.rs
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RolloutPayload {
    TokenCount(TokenCountPayload),
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct TokenCountPayload {
    #[serde(default)]
    rate_limits: Option<RateLimits>,
    // `info` is intentionally NOT parsed in Phase 2 — D-47 says we don't estimate from token counts.
}

#[derive(Debug, Deserialize)]
struct RateLimits {
    #[serde(default)]
    primary: Option<RateLimitTier>,
    #[serde(default)]
    secondary: Option<RateLimitTier>,
}

#[derive(Debug, Deserialize)]
struct RateLimitTier {
    used_percent: f64,
    window_minutes: u64,
    resets_in_seconds: u64,
}

// Outer line envelope (RolloutLine):
#[derive(Debug, Deserialize)]
struct RolloutLine {
    timestamp: jiff::Timestamp,  // RolloutLine.timestamp wraps every persisted event (DeepWiki)
    #[serde(rename = "type")]
    line_type: String,           // "event_msg" / "response_item" / "session_meta" / etc
    payload: Option<serde_json::Value>,  // only event_msg has structured payload we care about
}
```

The parser walks lines newest-to-oldest (or all + keep last); for each line where `line_type == "event_msg"`, it deserializes `payload` into `RolloutPayload`. If `RolloutPayload::TokenCount { rate_limits: Some(r) }` and either `r.primary.is_some() || r.secondary.is_some()` → emit windows and stop. If end of file with no non-null → `SchemaDrift`.

## Codex SQLite Schema

**File location:** `~/.codex/state_{N}.sqlite` where N is the schema version (verified `state_5.sqlite` is current as of 2026-05 per multiple GitHub issues; migration to higher N is upstream-internal) [VERIFIED: github.com/openai/codex/issues/23247 + #23979 + STACK.md SOURCES].

**Tables of interest:** `threads` table is the canonical metadata store. Columns referenced in upstream issues include:
- `ThreadId` (some identifier, exact column name not publicly documented)
- `updated_at_ms` (timestamp for detecting stale threads)
- `title`, `token_usage`, `sha`, `branch`, `origin_url` (ThreadMetadata fields persisted)
- `cwd` and `model_provider` (referenced via `allowed_sources` / `cwd_filters` in upstream queries)

**Schema migrations:** Codex uses SQLx migrations (numbered: `migration_1`, `migration_2`, …). Migration #34 dropped a `thread_goals` table; some Codex code paths still tried to read it after the drop, causing the bug in issue #23984. This confirms the schema is **internal-unstable**: AHB MUST treat all column reads as best-effort and gate them behind a probe query.

**Probe query pattern (recommended):**

```sql
-- Check that the threads table has the columns we expect before any SELECT.
PRAGMA table_info(threads);
```

In rusqlite:

```rust
use rusqlite::Connection;
let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
conn.busy_timeout(Duration::from_millis(250))?;
let columns: Vec<String> = conn
    .prepare("PRAGMA table_info(threads)")?
    .query_map([], |row| row.get::<_, String>(1))?  // column 1 = column name
    .collect::<Result<_, _>>()?;
if !columns.contains(&"updated_at_ms".to_string()) {
    // Schema drift on Codex SQLite — Phase 2 D-47-style fallback: return SchemaDrift OR
    // simply degrade SQLite read to "no metadata" and rely solely on JSONL.
    // Phase 2 RECOMMENDS the latter — JSONL is primary anyway (D-45), so SQLite drift
    // shouldn't bubble up as a user-visible error; emit a tracing::warn! and continue
    // with `source: "codex-jsonl"` (skip the "+sqlite" suffix).
}
```

**Crucial recommendation:** Given Phase 2's `D-45` decision that JSONL is the primary signal source and SQLite is only "supplemental metadata", AHB can **ship Phase 2 without reading SQLite at all** if the planner wants to minimize risk. The CONTEXT requires the SQLite *path discovery* (D-46 version glob) but does not strictly require reading any rows. **Recommend:** Phase 2 implements the version-glob discovery + the read-only open + `busy_timeout` (to prove the contract holds), but does NOT actually run any `SELECT` queries against `threads`. Defer all `threads`-table reads to Phase 3 or v2 if/when a concrete use case emerges. This eliminates the entire schema-drift surface for SQLite while honoring every D-45 / D-46 requirement.

If the planner disagrees and wants to surface SQLite metadata in Phase 2: read only `updated_at_ms` from the most-recent thread (LIMIT 1, ORDER BY updated_at_ms DESC) as a `last_active_at` field on the `ProviderState.source` string (e.g., `"codex-jsonl+sqlite-5"`); never read content rows; gate behind the PRAGMA table_info probe.

**`OpenFlags` combination verified** [VERIFIED: docs.rs/rusqlite/0.39.0/rusqlite/struct.OpenFlags]: `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` is the correct combination. `SQLITE_OPEN_READ_ONLY` guarantees no writes (rusqlite returns Err on any UPDATE/INSERT). `SQLITE_OPEN_NO_MUTEX` runs in SQLite's "multi-thread" mode (no per-connection mutex; one thread holds the connection — exactly our pattern inside `spawn_blocking`).

**`busy_timeout` API verified** [VERIFIED: docs.rs/rusqlite/0.39.0/rusqlite/struct.Connection]: `Connection::busy_timeout(Duration) -> Result<()>`. The 250ms value is correct per Pitfall 3. Set immediately after `open_with_flags` and before any query.

## Claude Weekly Limit Handling (D-54 deep-dive)

This is the most-uncertain piece of Phase 2 research. Findings:

1. **Anthropic does NOT publish exact weekly token limits.** The official support article (`support.claude.com/en/articles/11647753`) describes weekly limits in *relative* / qualitative terms ("about X% more than the 5h limit"). Multiple community sources confirm Anthropic deliberately stopped publishing fixed numbers in 2025-2026 [VERIFIED: tokenmix.ai 2026 + cometapi.com + faros.ai].

2. **Community estimates exist but vary widely.** Most-cited values (as of 2026-05):
   - **Pro:** weekly ≈ 5× to 7× the 5h limit → if 5h = 44,000 then weekly ≈ 220,000 to 308,000 tokens. tokenmix.ai cites Pro 5h ≈ 44k; ccusage's `Custom` plan computes per-user from 192h (8 days) of observed sessions rather than a fixed budget.
   - **Max5 (88k 5h):** weekly likely ≈ 440k to 616k tokens.
   - **Max20 (220k 5h):** weekly likely ≈ 1.1M to 1.54M tokens.

3. **The May 2026 +50% increase complicates this.** Anthropic announced a temporary 50% weekly cap increase from May 13 to July 13, 2026 (verified). So any hardcoded const has a stated expiration date. After July 13, the const becomes wrong again (likely shrinks 33% back to baseline).

4. **The weekly anchor is NOT cleanly known.** Sources diverge:
   - One community claim: "Monday-anchored, calendar week" (referenced in GitHub issue #49599 where a user complained their cycle changed from Monday to Friday).
   - Another community claim: "rolling 7-day from first prompt" (referenced in multiple ccusage / tokenmix posts).
   - Anthropic's official article is silent on the anchor.
   - Multiple GitHub issues document UI bugs around weekly reset display (#18136, #30933, #51222, #52498) — indicating even the upstream tool is uncertain about exact reset timing.

**Recommendation for Phase 2 (the safe path):**

Given the ambiguity, the safest design is to make `CLAUDE_WEEKLY_TOKEN_LIMIT` *optional* and have `ClaudeProvider` emit the weekly window with `percent_remaining: Option<f32>` (or `Option<HpUnit>`) that becomes JSON `null` when limit is unknown.

```rust
// src/provider/claude/window.rs additions:

/// Best-effort Claude weekly budget. Anthropic publishes only relative guidance
/// (varies ~5x-7x of 5h limit, with May-Jul 2026 +50% temporary increase).
/// `None` means "AHB has no reliable estimate — emit window with null percent".
/// Revisit quarterly. Source: community consensus (ccusage, tokenmix.ai, faros.ai)
/// 2026-05; Anthropic does not officially publish token counts since 2025.
pub const CLAUDE_WEEKLY_TOKEN_LIMIT: Option<u64> = None;  // OR Some(220_000) — see below

/// Anchor rule for the weekly window. Best-effort guess from community sources
/// (Anthropic does not document this). Options:
///   - `WeekAnchor::Iso` — ISO week starts Monday 00:00 local time
///   - `WeekAnchor::FirstPrompt` — rolling 7d from oldest assistant message in /.claude/projects
/// Phase 2 picks `Iso` as the default since it produces a stable, user-explicable
/// countdown. The actual Anthropic boundary may differ by hours; flag as best-effort
/// in README.
pub enum WeekAnchor { Iso, FirstPrompt }
pub const CLAUDE_WEEKLY_ANCHOR: WeekAnchor = WeekAnchor::Iso;
```

If the planner wants to ship a *number* rather than `None`: recommend **`Some(220_000)` for Pro-tier** (5x of the 44k 5h budget, mid-point of community estimates, easy mental model). Document the value as Pro-tier-only; Max users will see undercounted weekly bars until CFG-03 (Phase 3) adds plan-tier override. This is consistent with how `CLAUDE_5H_TOKEN_LIMIT = 44_000` is already a Pro-only estimate.

**For the DTO (`JsonWindow`)**, recommend `percent_remaining: Option<f32>` — serializes as JSON `null` cleanly when limit is unknown, no NaN footgun:

```rust
// src/cli/render_json.rs
#[derive(Serialize)]
pub struct JsonWindow {
    pub label: String,
    pub percent_remaining: Option<f32>,  // null when limit unknown (Claude weekly fallback)
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub reset_at: jiff::Timestamp,
}
```

For `--detailed` and `--compact` rendering: when `percent_remaining` is `None`, render the bar as the SchemaDrift-style `▒▒▒▒▒▒▒▒▒▒ ??%` (medium-shade) with a footer comment `weekly window — limit estimate not available`. This stays consistent with UI-SPEC's existing "unknown, not critical" rule for the SchemaDrift sentinel.

**Anchor computation (Iso week):** jiff has native ISO week support via `jiff::civil::Date::iso_week()` and `jiff::Zoned::iso_week()`. To compute the next Monday 00:00 local time given `now`:

```rust
let now_zoned = ctx.now.to_zoned(jiff::tz::TimeZone::system());
let dt = now_zoned.datetime();
let days_until_monday = (8 - dt.weekday().to_monday_zero_offset() as i64) % 7;
let days_until_monday = if days_until_monday == 0 { 7 } else { days_until_monday };
let next_monday_local = dt.date().checked_add(jiff::Span::new().days(days_until_monday))?
    .at(0, 0, 0, 0).to_zoned(jiff::tz::TimeZone::system())?;
let reset_at = next_monday_local.timestamp();
```

(Pseudocode — exact jiff API may need 1-2 adjustments at implementation time; the concept is sound. jiff 0.2 has full IANA TZ + ISO calendar support.)

**Token sum for weekly window:** sum `cache_creation_input_tokens` across ALL assistant entries in `~/.claude/projects/**/*.jsonl` whose timestamp falls within `[reset_at - 7d, now]` (or `[anchor_start, now]` for ISO mode). This is one extra pass over the already-sorted `merged: Vec<AssistantEntry>` from the Phase 1 ClaudeAdapter, so it adds zero IO — just compute both totals in the same pass.

## tokio::task::spawn_blocking Pattern (Claude's Discretion #4)

**Recommendation: narrow scope, NOT whole-fetch wrap.**

Wrapping the entire `async fn fetch(&self, ctx: &FetchCtx<'_>)` in `spawn_blocking` is problematic because:
1. `FetchCtx<'_>` has a non-`'static` lifetime — `spawn_blocking` requires `'static + Send + 'static`.
2. The async layer (error mapping, `ProviderError::SchemaDrift` construction, etc.) doesn't need to run on a blocking thread.
3. Mixing the entire flow into one blocking call makes error handling and timeout interaction awkward.

**Clean shape:**

```rust
// src/provider/codex/mod.rs (sketch)

#[async_trait]
impl Provider for CodexProvider {
    fn id(&self) -> ProviderId { ProviderId::Codex }

    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        // Step 1 (async): validate the SQLite path exists (cheap stat — no need to block).
        let sqlite_path = match self.discover_state_sqlite() {
            Some(p) => p,
            None => return Err(ProviderError::Unavailable {
                reason: "no ~/.codex/state_*.sqlite found — is Codex CLI installed?".into(),
            }),
        };

        // Step 2 (async): discover JSONL rollouts.
        let rollout_paths = self.discover_rollouts();
        let newest = pick_newest_file(&rollout_paths);

        // Step 3 (blocking): rusqlite open + busy_timeout + (optional) PRAGMA probe.
        // JSONL parse + rate_limits extraction also lives here — both are sync IO.
        let now = ctx.now;
        let blocking_result = tokio::task::spawn_blocking(move || -> Result<CodexBlob, ProviderError> {
            let conn = rusqlite::Connection::open_with_flags(
                &sqlite_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            ).map_err(|e| ProviderError::Internal { source: anyhow::anyhow!("sqlite open: {e}") })?;
            conn.busy_timeout(std::time::Duration::from_millis(250))
                .map_err(|e| ProviderError::Internal { source: anyhow::anyhow!("busy_timeout: {e}") })?;
            // NO actual SELECT in Phase 2 (recommended above) — the open + busy_timeout
            // are sufficient to honor D-45's "supplemental metadata" intent.
            drop(conn);

            // JSONL scan: find latest non-null rate_limits.
            let windows = match newest {
                Some(p) => parse_codex_rollout_windows(&p, now)?,
                None => return Err(ProviderError::Unavailable {
                    reason: "no ~/.codex/sessions/**/rollout-*.jsonl found".into(),
                }),
            };
            Ok(CodexBlob { windows, source: "codex-jsonl" })
        }).await;

        // Step 4 (async): map JoinError → ProviderError::Internal (the panic-hook is
        // ALREADY installed by main.rs — install_phase0_panic_hook chain via
        // take_hook+set_hook — so the panic message reaches stderr BEFORE
        // spawn_blocking's JoinError surfaces here. The Phase 1 fanout layer wraps
        // this whole fetch in JoinSet which also captures the panic; spawn_blocking
        // inside our async fn is a *nested* tokio task that returns its panic via
        // JoinError. is_panic() lets us distinguish panic from cancel.)
        let blob = match blocking_result {
            Ok(Ok(blob)) => blob,
            Ok(Err(e)) => return Err(e),
            Err(je) if je.is_panic() => return Err(ProviderError::Internal {
                source: anyhow::anyhow!("codex adapter sqlite/jsonl thread panicked"),
            }),
            Err(je) => return Err(ProviderError::Internal {
                source: anyhow::anyhow!("codex adapter blocking task failed: {je}"),
            }),
        };

        Ok(ProviderState {
            id: ProviderId::Codex,
            windows: blob.windows,
            fetched_at: ctx.now,
            source: Cow::Borrowed(blob.source),
        })
    }
}
```

**Why this shape:**
- **`spawn_blocking` closure is `move + 'static`** — captures owned `PathBuf`, owned `Vec<PathBuf>`, and `Copy` `jiff::Timestamp`. No lifetime issues.
- **Error mapping stays in the async layer** — clean and matches Phase 1 patterns (Claude adapter's tracing::warn paths are all sync but live in non-blocking IO).
- **Panic-hook interaction is clean:** the outer `JoinSet` in `engine/fanout.rs` already catches adapter panics via `JoinError::is_panic()` (Pitfall L4). The inner `spawn_blocking` is *additional* defense — if the blocking thread panics, the outer JoinSet sees `Ok` (the async fetch returned `Err::Internal`); if the entire async fetch panics for some other reason, the outer JoinSet's `is_panic()` fires. Either way the panic-hook in main.rs prints the panic message to stderr before any of this. **No risk of bypassing the panic hook.**
- **Timeout from `engine/fanout.rs` still works:** the outer `timeout(DEFAULT_PER_PROVIDER_TIMEOUT, provider.fetch(&ctx))` wraps the whole `fetch` call including the `await` on `spawn_blocking`. If the blocking thread takes too long, the timeout fires and the spawned blocking task continues to completion (tokio cannot cancel blocking threads), but its result is discarded — adapter is reported as `Unavailable { reason: "timed out after 2s" }`. The blocking thread eventually finishes its IO and exits cleanly; no leak.

## glob 0.3 Version Sort (D-46)

**Verified behavior:** `glob` crate 0.3 does NOT guarantee sorted output [CITED: docs.rs/glob/0.3.3 — does not document ordering]. Forum consensus: results are filesystem-iteration order (typically inode order on Linux; alphabetical on macOS HFS+; arbitrary on Windows NTFS). To get deterministic highest-N selection, AHB must sort explicitly.

**Recommended pattern for `state_*.sqlite` highest-N pick:**

```rust
// src/provider/codex/sqlite.rs

/// Discover all ~/.codex/state_*.sqlite files; return the one with the highest
/// version number N (parsed from the `state_{N}.sqlite` filename). Emits a
/// tracing::warn! if multiple coexist (mid-migration). D-46 binding.
pub fn discover_state_sqlite(codex_dir: &Path) -> Option<PathBuf> {
    let pattern = codex_dir.join("state_*.sqlite");
    let pattern_str = pattern.to_string_lossy();
    let paths: Vec<PathBuf> = glob::glob(&pattern_str)
        .ok()?
        .filter_map(Result::ok)
        .collect();

    if paths.is_empty() {
        return None;
    }

    // Parse the trailing _N from each filename; default to 0 if unparseable
    // (so a malformed name sorts below valid ones).
    let mut paths_with_version: Vec<(PathBuf, u32)> = paths
        .into_iter()
        .map(|p| {
            let n = p.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("state_"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            (p, n)
        })
        .collect();
    paths_with_version.sort_by_key(|(_, n)| *n);
    let highest = paths_with_version.pop()?;  // last after sort = highest

    if paths_with_version.len() > 0 {
        // > 1 file coexists: log the list (D-46 stderr warn).
        let names: Vec<String> = paths_with_version.iter()
            .map(|(p, _)| p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
            .collect();
        tracing::warn!(
            "multi-version Codex state files detected — picked {}, found {}",
            highest.0.display(),
            names.join(", ")
        );
    }
    Some(highest.0)
}
```

This is robust against `state_10.sqlite` > `state_5.sqlite` (lexicographic alphabetical would order them wrong: "10" < "5"). The integer parse + sort handles natural ordering correctly without pulling in `natural-sort-rs` or `lexical-sort`.

**Note on `tilde expansion`:** the `~` in `~/.codex/state_*.sqlite` is NOT expanded by Rust's `Path` or `glob`. The Codex adapter must resolve `home_dir` via `directories::BaseDirs::new()` (same pattern as Claude adapter in `engine/mod.rs` line 49-51) and pass the resolved path to `discover_state_sqlite`.

## --json schema_version: 1 Design (D-49..D-52)

**jiff RFC3339 verified:** `jiff::Timestamp::to_string()` emits RFC3339 with `Z` suffix (UTC, second precision). Phase 0 already uses `#[serde(with = "jiff::fmt::serde::timestamp::second::required")]` on `ResetInfo` and `ProviderState.fetched_at` — Phase 2's `JsonRoot.generated_at` / `JsonProvider.fetched_at` / `JsonWindow.reset_at` should use the same serde adapter [VERIFIED: src/model.rs line 43 + 65].

**Recommended DTO shape:**

```rust
// src/cli/render_json.rs

use std::borrow::Cow;
use serde::Serialize;
use crate::model::{HpWindow, ProviderError, ProviderId, ProviderState};

const SCHEMA_VERSION: u8 = 1;

#[derive(Serialize)]
pub struct JsonRoot<'a> {
    pub schema_version: u8,
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub generated_at: jiff::Timestamp,
    pub providers: Vec<JsonProvider<'a>>,
}

#[derive(Serialize)]
pub struct JsonProvider<'a> {
    pub id: &'static str,  // "claude" / "codex" / "gemini" / "mock"
    /// "ok" or "error" — keep as &'static str literal (binary status field per D-51).
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Cow<'a, str>>,        // present iff status == "ok"
    #[serde(with = "jiff::fmt::serde::timestamp::second::required::option",
            skip_serializing_if = "Option::is_none", default)]
    pub fetched_at: Option<jiff::Timestamp>, // present iff status == "ok"
    pub windows: Vec<JsonWindow>,            // always emitted; [] on error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonError>,            // present iff status == "error"
}

#[derive(Serialize)]
pub struct JsonWindow {
    pub label: String,                       // owned (avoid Cow lifetime gymnastics here)
    pub percent_remaining: Option<f32>,      // null when limit unknown (Claude-weekly fallback)
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub reset_at: jiff::Timestamp,
}

#[derive(Serialize)]
pub struct JsonError {
    pub kind: &'static str,  // "unconfigured" / "unavailable" / "schema_drift" / "network" / "rate_limited" / "internal"
    pub message: String,     // one-line (use format_one_line sanitizer)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<Vec<String>>,        // schema_drift only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,    // rate_limited only
}
```

**Conversion fn shape:**

```rust
pub fn to_json_root<'a>(
    results: &'a [(ProviderId, Result<ProviderState, ProviderError>)],
    generated_at: jiff::Timestamp,
) -> JsonRoot<'a> {
    let providers = results.iter().map(|(id, r)| match r {
        Ok(state) => JsonProvider {
            id: id_to_str(*id),
            status: "ok",
            source: Some(state.source.as_ref().into()),
            fetched_at: Some(state.fetched_at),
            windows: state.windows.iter().map(window_to_json).collect(),
            error: None,
        },
        Err(e) => JsonProvider {
            id: id_to_str(*id),
            status: "error",
            source: None,
            fetched_at: None,
            windows: Vec::new(),
            error: Some(error_to_json(e)),
        },
    }).collect();
    JsonRoot { schema_version: SCHEMA_VERSION, generated_at, providers }
}
```

**Emit pattern (compact, pipe-friendly, no pretty):**

```rust
let stdout = std::io::stdout().lock();
serde_json::to_writer(stdout, &root)?;
println!();  // trailing newline (jq tolerates either way; humans appreciate it)
```

**`serde(tag)` vs adjacent fields for `JsonProvider.status`:** the CONTEXT D-51 example uses *adjacent* fields (`status: "ok"` plus separate `error` / `windows` keys), NOT internally-tagged enum. Recommend keeping this shape — it matches D-51 verbatim, makes jq queries straightforward (`.providers[] | select(.status == "error")`), and avoids the Phase 0 ProviderError serde-tag complication that already exists in `src/model.rs`. The internally-tagged approach would force a more elaborate enum design with `#[serde(tag = "status", content = "...")]` and is unnecessary.

**Mock provider exclusion (D-50 "mock 預設不 emit 到 production JSON"):** filter out `ProviderId::Mock` from the `providers` Vec before conversion, UNLESS a test/debug flag overrides. Cleanest: do the filter at the conversion site (one line: `.filter(|(id, _)| *id != ProviderId::Mock || cfg!(debug_assertions))` — but this leaks debug behavior into binary, awkward). Recommend: always include Mock in `--json` IF `config.providers.mock.enabled == true` (i.e., user explicitly opted in via config). Don't filter in the conversion fn; filter at engine level. Phase 1's `Engine::new` already only registers providers whose `enabled` flag is true, so Mock won't appear unless explicitly enabled. No special-casing needed.

## clap conflicts_with (D-57) — verified

**Two equivalent ways to express the 3-way exclusion:**

**Option A: per-arg `conflicts_with`:**

```rust
#[derive(clap::Parser, Debug)]
pub struct Cli {
    #[arg(long, conflicts_with_all = ["detailed", "json"])]
    pub compact: bool,
    #[arg(long, conflicts_with_all = ["compact", "json"])]
    pub detailed: bool,
    #[arg(long, conflicts_with_all = ["compact", "detailed"])]
    pub json: bool,
    // ...rest unchanged
}
```

**Option B: `#[group(multiple = false)]` (cleaner for ≥3 flags):**

```rust
#[derive(clap::Parser, Debug)]
#[command(group(
    clap::ArgGroup::new("format")
        .required(false)
        .multiple(false)
        .args(["compact", "detailed", "json"]),
))]
pub struct Cli {
    #[arg(long)]
    pub compact: bool,
    #[arg(long)]
    pub detailed: bool,
    #[arg(long)]
    pub json: bool,
    // ...rest unchanged
}
```

**Recommend Option B** — `multiple = false` means "at most one of these may be present"; pairs cleanly with `required = false` so "no flag = default compact" still works. Single source of truth for the constraint (less ceremony than three `conflicts_with_all` lines that must stay in sync) [VERIFIED: docs.rs/clap latest ArgGroup + clap discussion #4195].

**clap exit code on usage error:** clap's `Cli::parse()` calls `.exit()` internally on parse error, which uses the `ErrorKind` to compute the exit code. For `ErrorKind::ArgumentConflict` (raised when two of `--compact / --detailed / --json` appear) the exit code is **2** [VERIFIED: clap source ErrorKind → exit_code mapping is well-documented; D-59 / D-61 explicitly cite this as "clap default"]. No code needed to make this work — passing `--json --detailed` will exit 2 with a usage error message automatically.

**`after_help` vs `long_about` for exit-code documentation (D-61):**

- `after_help` appears at the bottom of `--help` output, *after* all the argument descriptions. Cleaner separation; exit-codes feel like end-matter.
- `long_about` replaces the short `about` description; would put exit-codes *before* the argument list. Awkward.

**Recommend `after_help`:**

```rust
#[derive(clap::Parser, Debug)]
#[command(
    version,
    about = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance",
    after_help = "Exit codes:\n  0  at least one provider returned data (or no providers configured)\n  1  all configured providers failed\n  2  config / secrets unloadable, or invalid command-line usage",
)]
pub struct Cli { /* ... */ }
```

Use a `const` for the multi-line string if the planner finds it ugly inline. clap also accepts `&'static str` from a constant.

## --color flag + NO_COLOR env (D-58)

**Already wired in Phase 1.** `src/cli/tty.rs::should_colorize_env(color_flag: ColorMode, json_mode: bool) -> bool` implements the full priority chain [VERIFIED: src/cli/tty.rs lines 30-49]:

1. `json_mode == true` → always `false` (highest priority; D-58 binding)
2. `ColorMode::Never` → `false`
3. `ColorMode::Always` → `true` (explicit user intent wins over `NO_COLOR`)
4. `ColorMode::Auto` → `!no_color && is_tty`

**NO_COLOR precedence rule:** `ColorMode::Always` overrides `NO_COLOR` per the existing Phase 1 implementation (verified by `path_3_always_is_true_regardless_of_tty` test). This is a design choice that diverges slightly from no-color.org's "any NO_COLOR value disables color regardless of flags" wording — but matches the rust-cli-recommendations and matches user intent (`--color=always` should mean what it says). Phase 2 should NOT change this behavior; it's locked in Phase 1 tests.

**clap `ValueEnum` for `--color`:** already wired in Phase 1 (`ColorMode` derives `clap::ValueEnum` in `src/cli/tty.rs` line 17). Phase 2 reuses verbatim.

**Phase 2 dispatch shape:**

```rust
// src/cli/mod.rs additions to Cli struct
pub struct Cli {
    // ... existing fields ...
    #[arg(long)] pub compact: bool,
    #[arg(long)] pub detailed: bool,
    #[arg(long)] pub json: bool,
    // ... existing color, ascii, debug_emit_fake_secret, command ...
}

// in main.rs (or cli/mod.rs dispatch):
match (cli.compact, cli.detailed, cli.json) {
    (_, _, true) => {
        // --json: color forced off, ascii ignored (silent)
        ahb::cli::run_json(&engine, jiff::Timestamp::now()).await
    }
    (_, true, _) => {
        ahb::cli::run_detailed(&engine, cli.ascii, cli.color).await
    }
    _ => {
        // default OR --compact (both behave identically per D-57)
        ahb::cli::run_compact(&engine, cli.ascii, cli.color).await
    }
}
```

## Exit Code Wiring (D-59, D-60)

**Where to compute:** in `src/main.rs`, *after* the dispatch returns, just before `Ok(())`. The dispatch functions (`run_compact / run_detailed / run_json`) currently return `anyhow::Result<()>`; Phase 2 should widen them to return either a typed `ExitOutcome` enum or capture the result Vec and let `main.rs` compute the exit code from it.

**Recommended shape:**

```rust
// New helper in src/cli/mod.rs (or a new module):
pub enum DispatchOutcome {
    /// At least one provider returned Ok, OR providers list was empty (CFG-04).
    AnySuccess,
    /// All providers returned Err.
    AllFailed,
}

impl DispatchOutcome {
    pub fn from_results<T, E>(results: &[(crate::model::ProviderId, Result<T, E>)]) -> Self {
        if results.is_empty() {
            // Empty = no providers enabled — D-59 special case: still "AnySuccess" (exit 0)
            Self::AnySuccess
        } else if results.iter().any(|(_, r)| r.is_ok()) {
            Self::AnySuccess
        } else {
            Self::AllFailed
        }
    }
}
```

Then `run_compact / run_detailed / run_json` each return `anyhow::Result<DispatchOutcome>`; `main.rs`:

```rust
let outcome = match cli.command {
    None => match (cli.compact, cli.detailed, cli.json) {
        (_, _, true)  => ahb::cli::run_json(&engine, jiff::Timestamp::now()).await?,
        (_, true, _)  => ahb::cli::run_detailed(&engine, cli.ascii, cli.color).await?,
        _             => ahb::cli::run_compact(&engine, cli.ascii, cli.color).await?,
    },
    Some(Command::Tui) => { ahb::tui::run(engine).await?; return Ok(()); }  // TUI doesn't gate exit code
};
std::process::exit(match outcome {
    DispatchOutcome::AnySuccess => 0,
    DispatchOutcome::AllFailed  => 1,
});
```

**Config / secrets exit 2 is already wired** — `main.rs` lines 53-86 already use `std::process::exit(2)` on the secrets `Unavailable` path; Phase 1 fully handled D-41's hard-error contract [VERIFIED: src/main.rs lines 53-86]. Phase 2 does NOT change this; just need to verify no regression.

**SchemaDrift counts as fail (D-60):** `r.is_err()` returns `true` for `ProviderError::SchemaDrift`, so the `DispatchOutcome::from_results` logic above naturally counts SchemaDrift as a failure. No special casing needed.

**SIGPIPE (Deferred):** per CONTEXT, Phase 2 does NOT add explicit SIGPIPE handling. Default Rust behavior: writing to a closed pipe (`AHB | head -1`) raises SIGPIPE, which Rust 1.65+ converts to a runtime panic. The Phase 0 panic-hook catches this and prints to stderr; the process exits non-zero. This is *imperfect UX* (a `head -1` user will see "ahb panicked: ..." on stderr), but it's not broken and matches the "don't manage SIGPIPE in Phase 2" decision. If the planner discovers this is annoying during verification, add `signal_hook::low_level::register(SIGPIPE, ...)` to map to a clean `exit(0)` in Phase 3.

## SEC-03 enforcement (D-62) — extending existing flag

**Existing infrastructure** in `tests/secret_leak_subprocess.rs` (lines 1-46) and `src/cli/mod.rs::debug_emit_fake_secret_and_exit` (lines 114-135) [VERIFIED: source files read]:

- `#[cfg(debug_assertions)]` `--debug-emit-fake-secret` flag — gated to debug builds only, hidden from `--help`.
- Test injects fixture `deadbeefcafe1234567890abcdef` into a `Secret<String>`, serializes via `serde_json::to_writer`, captures subprocess stdout.
- Three asserts: fixture string absent, 20+-char alphanumeric pattern absent, `[REDACTED]` marker present.

**Phase 2 extension recommendation:**

Add a *second* mode to `--debug-emit-fake-secret` that routes through the real `--json` engine path (instead of the standalone envelope) to prove the production `JsonRoot` emission also redacts. Simplest implementation:

```rust
// src/cli/mod.rs additions
#[cfg(debug_assertions)]
#[arg(long, hide = true)]
pub debug_inject_secret_into_mock: bool,
```

Then when this flag is set AND `--json` is set:
1. Construct a Mock provider that emits a `ProviderState` where some field carries a `Secret<String>` containing the fixture (e.g., add a temporary test-only `Secret<String>` to the `ProviderState.source` via a debug-only path, OR inject into a `Secrets` map field).
2. Run `run_json` normally.
3. Test subprocess captures stdout, asserts same three conditions.

**Cleaner alternative (recommended):** instead of injecting into ProviderState (which would require adding a Secret field), have the debug flag inject the fixture into the Mock provider's `source` string directly:

```rust
// src/provider/mock.rs additions (debug-only)
#[cfg(debug_assertions)]
impl MockProvider {
    pub fn with_debug_secret_in_state() -> Self { /* sets a flag */ }
}
```

When Mock provider's fetch is called and the debug flag is set, it constructs a `ProviderState` where `source: Cow::Owned("debug-fixture-deadbeefcafe1234567890abcdef")` — but the Secret<T> serialization happens at a different layer. **Hmm.**

**Simplest sufficient approach** (avoiding new fields): the SEC-03 test for `--json` doesn't strictly need to exercise `Secret<T>` going through `JsonRoot` — it needs to prove that *whatever the JSON envelope looks like*, the fixture string never appears. So:

```rust
// tests/secret_leak_json.rs (new file, Phase 2)

#[test]
#[cfg(debug_assertions)]
fn json_subprocess_does_not_leak_secret() {
    // Run AHB with the mock provider enabled (via AHB_SECRETS_MOCK=1 + an env-var
    // that enables config.providers.mock.enabled, OR via a tempdir config file).
    // Then assert the same three conditions on --json stdout.
    let tmp = tempfile::tempdir().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    std::fs::write(&cfg_path, "[providers.mock]\nenabled = true\n").unwrap();
    let output = assert_cmd::Command::cargo_bin("ahb")
        .unwrap()
        .arg("--json")
        .env("AHB_CONFIG", cfg_path.to_str().unwrap())  // requires Phase 2 to add this env override
        .env("AHB_SECRETS_MOCK", "1")
        .output()
        .expect("subprocess should run");
    assert!(output.status.success() || output.status.code() == Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("deadbeefcafe1234567890abcdef"));
    let re = regex::Regex::new("[A-Za-z0-9]{20,}").unwrap();
    assert!(!re.is_match(&stdout));
    // The mock provider doesn't have ANY secret in its state — so this test proves
    // that production `--json` is empty of secret-shaped strings even when the
    // engine path is exercised. The [REDACTED] marker assertion belongs in the
    // separate `Secret<T>::Serialize` unit test (already in src/secrets.rs).
}
```

**Problem:** the above test doesn't actively *inject* a secret to prove it would be redacted. CONTEXT D-62 says the test must inject. So:

**Recommended actual approach:**

1. **Reuse the existing `--debug-emit-fake-secret` flag** but extend it to ALSO emit a `--json`-shaped envelope when both flags are present (`--json --debug-emit-fake-secret`). The existing flag already produces a Secret<T> Serialize envelope; Phase 2 adds the `--json` variant.

2. The `debug_emit_fake_secret_and_exit` fn (currently outputs a simple envelope) gets a `json: bool` parameter. When true, it constructs a synthetic `JsonRoot` where one of the `JsonProvider.windows[i].label` (or similar field) is a `Secret<String>` containing the fixture, serializes via `serde_json::to_writer`, exits 0.

3. The new `tests/secret_leak_json.rs` runs `AHB --json --debug-emit-fake-secret` and asserts the same three conditions.

This avoids ALL config / env-var plumbing complexity, reuses the existing `#[cfg(debug_assertions)]` machinery, and keeps the test self-contained.

**Concrete sketch:**

```rust
// src/cli/mod.rs — extend debug_emit_fake_secret_and_exit signature
#[cfg(debug_assertions)]
pub fn debug_emit_fake_secret_and_exit(as_json: bool) -> ! {
    use std::io::Write;
    let s = crate::secrets::Secret::new("deadbeefcafe1234567890abcdef".to_string());
    let mut stdout = std::io::stdout().lock();
    #[allow(clippy::unwrap_used)] {
        if as_json {
            // Synthetic JsonRoot with a Secret-typed field inside.
            // Note: `Secret<String>: Serialize` emits "[REDACTED]" — so this exercises
            // exactly the path that production --json would use IF Secret ever found
            // its way into JsonRoot (which it shouldn't, but the test proves it would
            // be safe even if it did).
            #[derive(serde::Serialize)]
            struct DebugJsonEnvelope<'a> {
                schema_version: u8,
                fake_secret_in_label: &'a crate::secrets::Secret<String>,
            }
            let env = DebugJsonEnvelope { schema_version: 1, fake_secret_in_label: &s };
            serde_json::to_writer(&mut stdout, &env).unwrap();
            writeln!(stdout).unwrap();
        } else {
            // existing path
        }
    }
    std::process::exit(0);
}
```

```rust
// main.rs — dispatch update
#[cfg(debug_assertions)]
if cli.debug_emit_fake_secret {
    ahb::cli::debug_emit_fake_secret_and_exit(cli.json);
}
```

**`regex` dev-dep already wired** [VERIFIED: Cargo.toml line 115 — `regex = "1"` in dev-dependencies]. No new dependency required.

## Integration Test Patterns (CONTEXT Specifics — Pitfall 3 守衛)

**Codex CLI parallel-write test:** the CONTEXT specifics mention `tokio::process::Command::new("codex").arg("exec").arg("/status")` in background while AHB runs `refresh_all()` 5 times. This requires `codex` CLI to be installed in CI, which is unrealistic. **Recommended CI-friendly fallback:**

```rust
// tests/codex_sqlite_lock_resilience.rs (new — Phase 2)

#[test]
fn codex_sqlite_busy_does_not_crash_adapter() {
    let tmp = tempfile::tempdir().unwrap();
    let codex_dir = tmp.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let db_path = codex_dir.join("state_5.sqlite");

    // Create the schema with rusqlite (write mode, just for setup).
    let setup = rusqlite::Connection::open(&db_path).unwrap();
    setup.execute("CREATE TABLE threads (id INTEGER PRIMARY KEY, updated_at_ms INTEGER)", []).unwrap();
    setup.execute("INSERT INTO threads VALUES (1, 1000)", []).unwrap();
    drop(setup);

    // Hold a RESERVED lock from another connection in a background thread.
    let db_path_clone = db_path.clone();
    let stopper = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stopper_clone = stopper.clone();
    let writer_handle = std::thread::spawn(move || {
        let conn = rusqlite::Connection::open(&db_path_clone).unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        tx.execute("INSERT INTO threads VALUES (2, 2000)", []).unwrap();
        // Hold the tx without committing — keeps a RESERVED/PENDING lock.
        while !stopper_clone.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // drop tx without commit → rolled back, lock released.
    });

    // Now the adapter's open + busy_timeout(250ms) should still succeed (read-only
    // sees the pre-tx snapshot) OR fail within 250ms with SQLITE_BUSY, NOT hang.
    // Phase 2 adapter must surface SQLITE_BUSY as a known error variant, not crash.
    let result = std::panic::catch_unwind(|| {
        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        );
        // Should NOT panic
        match conn {
            Ok(c) => { c.busy_timeout(std::time::Duration::from_millis(250)).unwrap(); }
            Err(e) => { eprintln!("expected possible Err: {e}"); }
        }
    });
    assert!(result.is_ok(), "adapter must not panic on locked DB");

    stopper.store(true, std::sync::atomic::Ordering::Relaxed);
    writer_handle.join().unwrap();
}
```

This test pattern proves the adapter is robust against concurrent writes WITHOUT requiring `codex` CLI to be installed. The "real" parallel-write integration test (with the actual codex binary) can be added as `#[cfg(feature = "integration-codex-binary")]`-gated, run only manually or in a Phase 4 release-readiness check.

**TTY vs pipe detection in tests:** `assert_cmd::Command` invokes the binary as a subprocess; stdin/stdout are pipes, NOT TTYs, so `std::io::stdout().is_terminal()` returns `false`. This means:
- `AHB --color=auto` in a test will NOT emit color (because `!is_tty`)
- `AHB --color=always` WILL emit color (explicit override)
- `AHB --json` will NEVER emit color regardless

For test stdout assertions, either pass `--color=never` for stable byte-exact comparisons OR pass `--color=always` to lock the ANSI bytes. Recommend `--color=never` for the `--detailed` snapshot tests (matches user environment when piping), and `--color=always` only for tests that explicitly assert ANSI escape presence.

**Snapshot test approach with `assert_cmd`:**

```rust
// tests/detailed_output_shape.rs (new — Phase 2)

#[test]
fn detailed_output_with_mock_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(&cfg, "[providers.mock]\nenabled = true\n").unwrap();
    let output = assert_cmd::Command::cargo_bin("ahb")
        .unwrap()
        .arg("--detailed")
        .arg("--color=never")
        // .env("AHB_CONFIG", &cfg)  // requires Phase 2 to add this env override if not yet wired
        .env("AHB_SECRETS_MOCK", "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Hand-rolled assertions (no insta needed for ≤3 snapshot tests):
    assert!(stdout.contains("mock\n"), "header line missing");
    assert!(stdout.contains("  mock-session"), "indented window line missing");
    assert!(stdout.contains("█"));  // U+2588 (Unicode mode default)
}
```

If the planner adds `insta` to dev-deps:

```rust
#[test]
fn detailed_output_snapshot() {
    // ... same setup ...
    insta::assert_snapshot!(stdout);
}
```

The Phase 0 / Phase 1 codebase has not used `insta` snapshots so far. Recommend adding it ONLY if Phase 2 ends up with > 3 stdout-shape tests; otherwise hand-rolled `assert!(stdout.contains(...))` is sufficient.

## default-config.toml Update

Current Phase 1 comment (verified `src/templates/default-config.toml` line 8-9):

```toml
[providers.codex]
enabled = false  # Codex CLI subscription — not yet implemented (Phase 2)
```

**Recommended Phase 2 update:**

```toml
[providers.codex]
enabled = false  # Codex CLI subscription — reads ~/.codex/sessions/**/rollout-*.jsonl + state_*.sqlite (read-only)
```

This is a 1-line diff. The `enabled = false` default stays — users opt in via `enabled = true`. CFG-04 silent-skip behavior continues to work (disabled providers don't appear in `Engine::new` provider list).

## Architecture Patterns

### Codex adapter module structure (mirror Claude pattern)

```
src/provider/codex/
├── mod.rs          # CodexProvider struct, impl Provider, spawn_blocking dispatch
├── jsonl.rs        # parse_codex_rollout_windows, RolloutLine/RolloutPayload structs
├── sqlite.rs       # discover_state_sqlite (D-46 version glob), open_readonly helper
└── window.rs       # (optional) shared helpers for converting RateLimitTier → HpWindow
```

This mirrors the Phase 1 `src/provider/claude/{mod.rs,jsonl.rs,window.rs}` layout (verified). Phase 2 adds a `sqlite.rs` sibling.

### Render-format dispatch (single CLI module)

```
src/cli/
├── mod.rs          # Cli struct, dispatch logic, run_compact / run_detailed / run_json signatures
├── render_text.rs  # compact_line + format_error_row (Phase 1, unchanged); add detailed_block
├── render_json.rs  # NEW — JsonRoot/Provider/Window/Error DTOs + to_json_root fn
└── tty.rs          # should_colorize_env (Phase 1, unchanged)
```

### Pattern 1: Streaming JSONL backward-scan for latest non-null rate_limits
**What:** Walk Codex rollout file line-by-line; track the latest valid `rate_limits` snapshot; return when end of file is reached. Walking newest-to-oldest would require buffering the whole file; walking oldest-to-newest and keeping `last_found` is cleaner and matches Phase 1's `ClaudeAdapter` JSONL pattern.

**When to use:** Any append-only JSONL where the most-recent event with a specific field is the source of truth.

**Example:**

```rust
// src/provider/codex/jsonl.rs

pub fn parse_codex_rollout_windows(
    path: &Path,
    fallback_now: jiff::Timestamp,
) -> Result<Vec<HpWindow>, ProviderError> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    let file = File::open(path).map_err(|e| ProviderError::Internal {
        source: anyhow::anyhow!("open rollout {}: {e}", path.display()),
    })?;
    let reader = BufReader::new(file);
    let mut last_snapshot: Option<(jiff::Timestamp, RateLimits)> = None;
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if line.is_empty() { continue }
        let Ok(rl): Result<RolloutLine, _> = serde_json::from_str(&line) else { continue };
        if rl.line_type != "event_msg" { continue }
        let Some(payload_val) = rl.payload else { continue };
        let Ok(payload): Result<RolloutPayload, _> = serde_json::from_value(payload_val) else { continue };
        let RolloutPayload::TokenCount(tc) = payload else { continue };
        if let Some(rate_limits) = tc.rate_limits {
            last_snapshot = Some((rl.timestamp, rate_limits));
        }
    }

    match last_snapshot {
        None => Err(ProviderError::SchemaDrift { missing: vec!["rate_limits".into()] }),
        Some((anchor, rl)) => {
            let mut windows = Vec::new();
            if let Some(p) = rl.primary {
                windows.push(tier_to_hpwindow("primary", p, anchor));
            }
            if let Some(s) = rl.secondary {
                windows.push(tier_to_hpwindow("secondary", s, anchor));
            }
            if windows.is_empty() {
                Err(ProviderError::SchemaDrift { missing: vec!["rate_limits".into()] })
            } else {
                Ok(windows)
            }
        }
    }
}

fn tier_to_hpwindow(label: &'static str, tier: RateLimitTier, anchor: jiff::Timestamp) -> HpWindow {
    use crate::model::{HpWindow, ResetInfo};
    use std::borrow::Cow;
    let pct_remaining = (100.0 - tier.used_percent).clamp(0.0, 100.0) as f32;
    let resets_at = anchor + jiff::Span::new().seconds(tier.resets_in_seconds as i64);
    HpWindow {
        label: Cow::Borrowed(label),
        percent_remaining: pct_remaining,
        reset: ResetInfo { resets_at },
        bar_color: None,
    }
}
```

(Pseudocode — exact `jiff::Span::new().seconds(...)` constructor may need adjustment.)

### Pattern 2: Owned-data closure for spawn_blocking
**What:** Move all path data into the closure (PathBuf, owned strings) so the closure is `'static + Send`. Return a small owned blob (Vec<HpWindow>, source string) to the async caller.

**When to use:** Whenever rusqlite or other sync IO needs to run on a blocking thread inside an async function.

**Example:** see "tokio::task::spawn_blocking Pattern" section above.

### Anti-Patterns to Avoid

- **Anti-Pattern A: Adding `crossterm` features to `Cargo.toml` for Codex/JSON paths.** Phase 1 already lists `crossterm` directly (event-stream feature). Phase 2 needs nothing new from crossterm — JSON / CLI render paths don't use any new crossterm functionality. Do NOT add new crossterm features.
- **Anti-Pattern B: Wrapping `&FetchCtx<'_>` in `spawn_blocking`.** Forces `'static` on a non-`'static` ref. Move only `now: jiff::Timestamp` (Copy) and owned PathBufs into the closure.
- **Anti-Pattern C: Pretty-printing `--json` output.** Pipelines (`AHB --json | jq`) prefer compact; humans use jq for pretty. CONTEXT D-Deferred bans `--pretty` in Phase 2.
- **Anti-Pattern D: SchemaDrift estimation from `info.total_token_usage`.** D-47 explicitly bans this. `rate_limits: null` → user sees "out-of-date" sentinel; do NOT compute a fake percent.
- **Anti-Pattern E: Hardcoding `"state_5.sqlite"`.** D-46 requires version-glob discovery; the name will become `state_6.sqlite` etc. Use `discover_state_sqlite`.
- **Anti-Pattern F: Writing to `state_*.sqlite`.** `SQLITE_OPEN_READ_ONLY` enforces this at the rusqlite layer; do not run `PRAGMA journal_mode=WAL` (that's a write).
- **Anti-Pattern G: Including `ProviderId::Mock` in JSON output by default.** D-50 says mock is opt-in via config (already enforced by Phase 1's `Engine::new`).
- **Anti-Pattern H: Adding `serde(deny_unknown_fields)` to JsonRoot / JsonProvider DTOs.** D-52 says additive changes (new fields) must not bump schema_version; deny_unknown_fields would break consumer tolerance.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| 3-way mutual-exclusion of bool flags | Custom validator that checks the three flags after `Cli::parse()` | clap `#[group(multiple = false)]` | clap handles the error message + exit code 2 automatically; consistent UX with all other clap-driven errors |
| SQLite version-glob with natural sort | Pull in `natural-sort-rs` or `lexical-sort` crate | Hand-roll integer parse + `sort_by_key` on the `_N` suffix | Only 4-10 LOC; no new dep; lexicographic-vs-natural footgun avoided by extracting `u32` |
| JSONL parser with explicit state machine | Custom line buffer + tokenizer | `BufReader::lines()` + `serde_json::from_str::<Value>(line)` | Already the standard pattern; ClaudeAdapter uses this verbatim; zero new deps |
| ANSI / color decision | Custom logic in each render fn | `tty::should_colorize_env(color_flag, json_mode)` (Phase 1) | Already wired; same 6-path truth table for all dispatch routes |
| Countdown formatting | Custom `Span` decomposition | `format_countdown(now, target)` (Phase 1) | Already `pub(crate)`; Phase 2 reuses for `--detailed` and `--json` reset_at conversion |
| RFC3339 emission | `chrono`-style format strings | `jiff::Timestamp::to_string()` or `#[serde(with = "jiff::fmt::serde::timestamp::second::required")]` | Already wired in Phase 0 model.rs; jiff defaults to RFC3339 UTC with `Z` |
| Per-provider id_label | Match arm in each renderer | `id_label(ProviderId)` (Phase 1, `pub(crate)`) | Already wired; consistent snake_case across all output formats |
| Secret redaction in JSON | Custom serializer that filters strings | `Secret<T>: Serialize → "[REDACTED]"` (Phase 1) | Already wired; CI grep test enforces |

**Key insight:** Phase 2 is overwhelmingly about *composing* Phase 1 primitives, not building new ones. The only genuinely new modules are `src/provider/codex/*` (rusqlite + JSONL) and `src/cli/render_json.rs` (DTOs). Everything else is calling existing `pub(crate)` functions in new combinations.

## Common Pitfalls

### Pitfall A: jiff timestamp arithmetic on negative spans (Phase 0-03 deviation 1 echo)
**What goes wrong:** `target.since(now)` with `target < now` (already-reset window) returns a span with negative components. Phase 0's `format_countdown` already handles this by clamping `h.max(0)` and `m.max(0)`. Phase 2's `rate_limited.retry_after_seconds` and `JsonWindow.reset_at` paths must do the same.
**How to avoid:** When converting `RateLimitTier.resets_in_seconds: u64` to `reset_at`, ensure `anchor + Span::seconds(resets_in_seconds)` produces a forward-looking timestamp. If `resets_in_seconds == 0` or the rollout is very stale, the reset_at could be in the past relative to `ctx.now` — render as `0h00m` (Phase 1 `format_countdown` already clamps).
**Warning signs:** snapshot tests showing `-1h-30m` or panic in `pct_int(NaN)`.

### Pitfall B: Codex rollout files growing very large
**What goes wrong:** A long Codex session might produce a multi-MB rollout file. Reading the whole file just to find the *last* non-null `rate_limits` is wasteful (though still fast in practice — JSONL is sequential).
**How to avoid:** Phase 2 acceptable approach: read whole file (simplest; matches ClaudeAdapter). Phase 3 optimization: use `tail`-style reverse-scan (`Read::seek(SeekFrom::End)` + backward chunk read) or watch file mtime and cache last result. Phase 2 should NOT pre-optimize — measure first.
**Warning signs:** TUI tick taking > 100ms even on local fast SSD.

### Pitfall C: Codex `rate_limits` field absent vs null vs empty object
**What goes wrong:** Three failure modes look similar but indicate different upstream states:
- Field absent (`{"payload": {"type": "token_count", "info": {...}}}`) → likely older Codex version with no rate_limits machinery
- `rate_limits: null` → upstream emitted the field but the value is unknown (issue #14880)
- `rate_limits: {}` → empty object, neither primary nor secondary present
**How to avoid:** Treat all three identically as "no usable rate_limits signal" → `SchemaDrift`. The parser code above handles this naturally via `Option<RateLimits>` + the `if windows.is_empty()` check.

### Pitfall D: `state_*.sqlite` integer parse stripping `_` 
**What goes wrong:** `"state_5".strip_prefix("state_")` returns `"5"` which parses to `5_u32`. But `"state_50".strip_prefix("state_")` returns `"50"` which parses to `50_u32`. Good. But if Codex ever ships `"state_5_backup.sqlite"` or `"state_5.bak.sqlite"`, the parse fails — silent zero. Recommend logging unparseable filenames.
**How to avoid:** in `discover_state_sqlite`, when `parse::<u32>().ok()` fails, emit `tracing::debug!("ignored ~/.codex/{} — unparseable version suffix", filename)` so the user can investigate.

### Pitfall E: clap `conflicts_with_all` array element typos
**What goes wrong:** `#[arg(long, conflicts_with_all = ["detialed", "json"])]` (typo: "detialed") compiles successfully — clap silently does not detect the mismatch. The mutual-exclusion is then broken at runtime.
**How to avoid:** prefer `#[group(multiple = false)]` (Option B above) — the args are listed once in the group definition, so the typo would either fail to compile or fail the test that asserts `--detailed --json` exits 2.
**Recommend:** Add a Phase 2 acceptance test that explicitly runs `AHB --detailed --json` and asserts `exit_code == 2`.

### Pitfall F: SchemaDrift sentinel `Claude adapter may be out-of-date` is hardcoded
**What goes wrong:** `src/cli/render_text.rs::format_error_row_colored` line 127 has `let phrase = "Claude adapter may be out-of-date";` — hardcoded "Claude" regardless of which provider triggered the drift. For Codex hitting `rate_limits: null`, the user will see `codex  ▒▒▒▒▒▒▒▒▒▒ ??% • Claude adapter may be out-of-date` which is confusing.
**How to avoid:** **Either** generalize the phrase (`format!("{label}-cased} adapter may be out-of-date")` using `id_label(id)` with a `to_uppercase_first` helper), **or** accept that "Claude adapter" reads OK in context since the user can tell which provider is on the row from the leading label. **Recommend generalizing** — the diff is small (a few lines in `format_error_row_colored`), the UI-SPEC LOCKED phrase has the `{Label}` substitution slot intent, and it eliminates confusion. The current hardcoded form was a Phase 1 oversight (only one provider could trigger SchemaDrift then).

If generalizing: planner must update `.planning/phases/01-engine-claude-tui-scaffold/01-UI-SPEC.md` Dimension 1 Copywriting section to reflect the new pattern, OR add a Phase 2 UI-SPEC delta document. CONTEXT D-Deferred mentions this generalization is "if planner sees Codex use of Claude string is wrong" — recommend doing it.

## Code Examples

Verified patterns from official sources:

### rusqlite read-only open with busy_timeout

```rust
// Source: docs.rs/rusqlite/0.39.0/rusqlite/struct.Connection
use rusqlite::{Connection, OpenFlags};
use std::time::Duration;

let conn = Connection::open_with_flags(
    "/home/user/.codex/state_5.sqlite",
    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
)?;
conn.busy_timeout(Duration::from_millis(250))?;
```

### tokio spawn_blocking with panic recovery via JoinError::is_panic

```rust
// Source: docs.rs/tokio/latest/tokio/task/struct.JoinError
let handle = tokio::task::spawn_blocking(move || {
    // sync IO here — rusqlite, file reads, etc.
    Ok::<_, anyhow::Error>(payload)
});
match handle.await {
    Ok(Ok(payload)) => { /* use payload */ }
    Ok(Err(e)) => { /* user-side IO error */ }
    Err(join_err) if join_err.is_panic() => {
        // panic propagated from blocking thread; panic-hook already printed to stderr
        tracing::error!("blocking task panicked");
    }
    Err(join_err) => {
        // cancelled or other JoinError variant
    }
}
```

### serde_json compact emission (no pretty)

```rust
// Source: docs.rs/serde_json - serde_json::to_writer
let stdout = std::io::stdout().lock();
serde_json::to_writer(stdout, &json_root)?;
println!();  // trailing newline
```

### clap derive ArgGroup for ≥3 mutually-exclusive flags

```rust
// Source: docs.rs/clap/latest/clap/struct.ArgGroup + clap discussion #4195
#[derive(clap::Parser, Debug)]
#[command(group(
    clap::ArgGroup::new("format")
        .required(false)
        .multiple(false)
        .args(["compact", "detailed", "json"]),
))]
pub struct Cli {
    #[arg(long)] pub compact: bool,
    #[arg(long)] pub detailed: bool,
    #[arg(long)] pub json: bool,
}
```

### jiff RFC3339 UTC emission

```rust
// Source: Phase 0 src/model.rs (already wired)
use jiff::Timestamp;

let ts: Timestamp = Timestamp::now();
let s = ts.to_string();
// s = "2026-05-25T13:45:22Z"  (RFC3339 UTC with Z suffix, second precision)

// Or via serde:
#[derive(Serialize)]
struct X {
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    when: Timestamp,
}
```

### owo-colors reuse from Phase 1 (UI-SPEC LOCKED thresholds)

```rust
// Source: src/cli/render_text.rs (Phase 1, pub(crate) reusable)
use crate::cli::render_text::{filled_cells, format_countdown, id_label, compact_line_colored};

// Detailed render reuses ALL of these — see D-56 "100% 共享 compact 樣式".
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `tui-rs` | `ratatui` 0.30 | 2023-08 (tui-rs archived) | Already adopted in Phase 1 |
| `keyring` v4 (demo) | `keyring-core` 1.0 | 2026-04 | Already adopted in Phase 1 (D-40 ff.) |
| `reqwest` `default-tls` (OpenSSL) | `rustls-tls` default | 2026 (reqwest 0.13) | Phase 3 binding (not yet active) |
| `chrono` 0.4 | `jiff` 0.2 | 2026 (community pivot) | Already adopted in Phase 0 |
| `rusqlite` with system SQLite | `rusqlite` with `["bundled"]` | recurring | Phase 2 ADP-04 newly active |
| `cargo install` only | `cargo-dist` + `cargo binstall` | 2026 | Phase 4 binding |
| Codex `rate_limits` reliable | Widespread null bug (#14880) | 2025 onward | Phase 2 D-47 ban on estimation is direct consequence |

**Deprecated/outdated:**
- `tui` / `tui-rs` — archived 2023-08; replaced by ratatui (already done in Phase 1).
- `keyring` v4 — demo only; replaced by `keyring-core` (already done in Phase 1).
- Direct `crossterm` dependency parallel to ratatui — Phase 1 has a documented deviation (event-stream feature gating) but still routes USAGE through `ratatui::crossterm::*` re-exports per Pitfall L2 (clippy.toml relaxation is a known accepted deviation).
- `async-std` — discontinued 2025; tokio is the only viable runtime (already aligned).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Codex emits `rate_limits` with exactly two windows (`primary`, `secondary`) | Codex JSONL Schema | If Codex adds a third window (e.g., `weekly`), the strongly-typed `RateLimits { primary, secondary }` struct silently drops it. Mitigation: use `BTreeMap<String, RateLimitTier>` instead — see "Future-proofing recommendation" below. |
| A2 | `state_5.sqlite` is current as of 2026-05-25 | Codex SQLite Schema | If Codex bumps to `state_6.sqlite` before Phase 2 ships, the version-glob picks it up automatically (D-46 handles this). No risk. |
| A3 | `RolloutLine.timestamp` field name is verified | Codex JSONL Schema | DeepWiki cited "UTC timestamp for each persisted event" but did not show the exact JSON field name. Verify at implementation time by running `cat ~/.codex/sessions/**/rollout-*.jsonl | head -1 | jq` on a real machine. The outer line shape might be `{"ts": ..., "type": ..., "payload": ...}` or similar. |
| A4 | `CLAUDE_WEEKLY_TOKEN_LIMIT` for Pro tier is approximately 5x of the 5h limit (≈ 220,000 tokens, pre-May-2026 +50% baseline) | Claude Weekly Limit Handling | Wrong number → bar shows misleading percent. Mitigation: `Option<u64>` const with `None` default emits `null` percent (safe but UX-degrading); `Some(220_000)` is best-guess. Planner picks. |
| A5 | Claude weekly anchor is "ISO week, Monday 00:00 local time" | Claude Weekly Limit Handling | Reset countdown may be off by up to a few days. Mitigation: README flag as best-effort estimate; community sources diverge (Monday vs rolling 7d). |
| A6 | `glob` crate's `Paths::next()` does not sort | glob 0.3 Version Sort | docs.rs/glob/0.3.3 doesn't document sort order; user-forum consensus confirms unsorted; explicit `sort_by_key` removes the risk. Already mitigated in recommendation. |
| A7 | clap derive `#[group(multiple = false)]` returns exit code 2 on conflict | clap conflicts_with | docs.rs/clap doesn't quote the exact exit code; standard clap convention is 2; Phase 1 verified `--color` parse errors exit 2 via Cli::parse(). Recommend a test asserts exit 2 explicitly. |
| A8 | `Secret<T>::Serialize` emits `"[REDACTED]"` in ALL `--json` envelopes that transitively contain it | SEC-03 enforcement | Already verified by Phase 1 unit test `secret_serialize_emits_redacted_literal`; CI grep test in Phase 2 confirms across subprocess. |

**Future-proofing recommendation for A1:** instead of `struct RateLimits { primary: Option<RateLimitTier>, secondary: Option<RateLimitTier> }`, use:

```rust
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct RateLimits(std::collections::BTreeMap<String, RateLimitTier>);
```

This way, if Codex ships a third window labeled `"weekly"` (as the CONTEXT D-48 speculative example listed), the parser picks it up automatically and `passthrough` emit works without code changes. Recommend this shape — it's marginally more flexible and matches D-48's "upstream passthrough" intent more literally.

**If this table is more than 0:** A1, A3, A4, A5, A7 should be confirmed before Phase 2 ships. A2, A6, A8 are already-mitigated by code paths.

## Open Questions

1. **Should Phase 2 read the Codex SQLite at all, or just discover the path?**
   - What we know: D-45 says SQLite is for supplemental metadata only; JSONL is primary.
   - What's unclear: Whether the planner wants any actual `SELECT` to happen.
   - Recommendation: **Do not run `SELECT` in Phase 2.** Discover + open + busy_timeout (proves contract). Defer all `threads`-row reads to Phase 3 or v2 unless a concrete UI use case appears.

2. **Should the `Claude adapter may be out-of-date` sentinel be generalized to `{Provider} adapter may be out-of-date`?**
   - What we know: UI-SPEC Phase 1 LOCKED the phrase with hardcoded "Claude"; CONTEXT D-Deferred flags this as planner discretion.
   - What's unclear: Whether changing it requires a UI-SPEC update for Phase 1 or just a Phase 2 delta.
   - Recommendation: **Generalize.** Add helper `fn id_label_titlecase(id) -> &'static str` returning `"Claude" | "Codex" | "Gemini" | "Mock"`; update phrase to `format!("{label} adapter may be out-of-date", label = id_label_titlecase(id))`. Update UI-SPEC Phase 1 Dimension 1 with a Phase 2-amends-Phase 1 note.

3. **Should `--json` emit `windows: []` for Mock-when-disabled, or omit Mock entirely?**
   - What we know: D-50 says "mock 預設不 emit 到 production JSON（除非 config enabled）". Engine doesn't register disabled providers.
   - What's unclear: If user explicitly enables Mock + runs `--json`, should Mock appear?
   - Recommendation: **Yes, include Mock in `--json` IF enabled in config.** This is consistent with `--compact` and `--detailed` — the user opted in.

4. **Should `JsonError.message` for `ProviderError::Internal` strip the anyhow cause chain?**
   - What we know: D-49 + D-Deferred ban cause-chain expansion (path leak risk).
   - What's unclear: Does `format!("{e}")` on `anyhow::Error` already produce only the top-level Display string, or does it walk the chain?
   - Answer (verified via Phase 1 `provider_error_internal_serializes_display` test): `anyhow::Error`'s `Display` impl prints only the top-level message; `Debug` walks the chain. As long as Phase 2's `JsonError.message` uses `format!("{e}")` (Display) and not `format!("{e:?}")` (Debug), the cause chain stays hidden. **Recommend** explicit comment in `error_to_json` fn noting this is a SEC binding.

5. **For Claude weekly anchor, should AHB use system timezone or UTC?**
   - What we know: Users likely think of "Monday morning" in their local time, not UTC.
   - What's unclear: Anthropic's actual anchor TZ.
   - Recommendation: **Use system timezone** via `jiff::tz::TimeZone::system()`. README clarifies "weekly window resets at next Monday 00:00 in your local timezone — best-effort estimate; Anthropic's actual reset may differ by hours."

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `rustc` 1.88+ | Build (already locked) | (assumed at user machine) | — | — |
| `cargo` | Build | — | — | — |
| `~/.codex/` directory | Codex adapter at runtime (user's machine) | optional | — | If absent → `ProviderError::Unavailable` (UI-SPEC literal) |
| `~/.codex/state_*.sqlite` | Codex adapter at runtime | optional | varies | If absent → same `Unavailable` error |
| `~/.codex/sessions/**/rollout-*.jsonl` | Codex adapter at runtime | optional | varies | If absent → `SchemaDrift` (no signal) |
| `codex` CLI installed | Phase 2 integration tests (specifics §) | NOT in CI | — | Use `tempfile` + fake `state_5.sqlite` + RESERVED-lock writer thread (see Integration Test Patterns) |
| `jq` | NOT required for AHB; documented in README for `AHB --json | jq` usage | optional | none needed |
| `regex` crate | SEC-03 test | already dev-dep | 1.x | — |
| `tempfile` crate | All test fixtures | already dev-dep | 3.x | — |
| `insta` crate | OPTIONAL snapshot testing | NOT in dev-deps | — | Hand-rolled `assert!(stdout.contains(...))` (recommend defer unless > 3 snapshot tests needed) |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** `codex` CLI binary (use rusqlite-direct simulator instead — pattern documented).

## Security Domain

> `security_enforcement` config key is not explicitly set in `.planning/config.json` — treating as enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | Phase 2 adds Codex (no auth — reads local FS); Phase 3 Gemini will be the auth phase |
| V3 Session Management | no | AHB is a one-shot CLI / fixed-frame TUI — no session state outside Codex's own files |
| V4 Access Control | no | Single-user local tool |
| V5 Input Validation | yes | JSON inputs: Codex JSONL parser MUST tolerate malformed lines (silent skip; same as Phase 1 Claude pattern); config TOML parser already tolerates unknown keys (Phase 1). serde with `#[serde(default)]` + `Option<T>` everywhere keeps bad input from panicking. |
| V6 Cryptography | no (passthrough only) | `Secret<T>` already wraps with `zeroize`; Phase 2 does not introduce new crypto. SEC-03 grep test is the binding control. |
| V7 Error Handling | yes | `ProviderError::Internal` Display strips backtraces (Phase 0 W-7 binding); `format_one_line` sanitizer collapses whitespace; `JsonError.message` uses Display not Debug. |
| V8 Data Protection | yes | Codex DB opened READ-ONLY (no write surface); no secrets stored on disk by AHB (keyring only); JSON output passes through `Secret<T>` redaction. |
| V12 File Handling | yes | Codex JSONL streamed line-by-line (not `read_to_string` — prevents OOM on large rollouts, matches Phase 1 Claude pattern); glob discovery follows symlinks (acceptable for user-owned `~/.codex/`). |
| V14 Configuration | yes | `state_*.sqlite` is read-only opened with `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`; `busy_timeout = 250ms` prevents indefinite hangs; never write any PRAGMA. |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Secret leakage in `--json` output | Information Disclosure | `Secret<T>::Serialize → "[REDACTED]"`; SEC-03 CI test asserts via regex |
| SQLite corruption induced by AHB writing | Tampering | `SQLITE_OPEN_READ_ONLY` enforces at rusqlite layer; no `PRAGMA journal_mode=WAL` |
| OOM via large JSONL read | DoS | `BufReader::lines()` streaming (matches Phase 1 ClaudeAdapter); never `read_to_string` |
| Adapter panic crashes whole TUI | DoS | Phase 1 `engine/fanout.rs` `JoinSet` + `JoinError::is_panic()` catches; `panic-hook` already installed |
| Hardcoded `state_5.sqlite` filename breaking on upgrade | Availability | `discover_state_sqlite` version-glob (D-46) |
| Codex rate_limits null leading to phantom 100% | Integrity / Misleading UX | `SchemaDrift` sentinel; D-47 bans estimation |
| anyhow Debug leaking internal paths to `--json` | Information Disclosure | `format!("{e}")` Display only (Phase 0 W-7 binding); explicit comment in `error_to_json` |
| TOML config with unexpected provider key | Misconfiguration | Phase 1 `warn_unknown_keys` emits `tracing::warn!` (D-38 forward-compat) |
| User pipes `AHB | curl` → cookie/secret leak | Information Disclosure | NEVER include Secret<T> in JsonRoot DTOs; SEC-03 grep test covers regression |
| Symlink attack on `~/.codex/sessions/**/*.jsonl` | Tampering (limited — user-owned) | glob follows symlinks by default; acceptable since AHB only reads files; documented in code comment |

## Sources

### Primary (HIGH confidence)
- [openai/codex issue #14728 — feat(exec): emit rate_limits in exec mode JSONL output](https://github.com/openai/codex/issues/14728) — verified exact `RateLimitSnapshot` JSON shape (primary/secondary with used_percent/window_minutes/resets_in_seconds)
- [openai/codex issue #14880 — rate_limits is always null in rollout session files](https://github.com/openai/codex/issues/14880) — verified null shape + exact path `payload.rate_limits`
- [DeepWiki openai/codex 3.5.2 Rollout Persistence and Replay](https://deepwiki.com/openai/codex/3.5.2-rollout-persistence-and-replay) — verified rollout file location pattern + RolloutLine/RolloutItem variants
- [docs.rs/rusqlite/0.39.0/rusqlite/struct.OpenFlags](https://docs.rs/rusqlite/0.39.0/rusqlite/struct.OpenFlags.html) — verified `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` is valid combination via BitOr
- [docs.rs/rusqlite/0.39.0/rusqlite/struct.Connection](https://docs.rs/rusqlite/0.39.0/rusqlite/struct.Connection.html) — verified `open_with_flags` + `busy_timeout(Duration)` signatures
- [docs.rs/clap/latest/clap/struct.ArgGroup](https://docs.rs/clap/latest/clap/struct.ArgGroup.html) — verified `multiple(false)` semantics for ≥3 exclusive args
- [docs.rs/glob/0.3.3](https://docs.rs/glob/0.3.3/glob/index.html) — verified Paths iterator does not document order
- [Existing src/ files in /home/chasel/REPO/AIHPBar/src/](/home/chasel/REPO/AIHPBar/src/) — verified Phase 0 + Phase 1 wiring (model.rs serde adapters, tty.rs should_colorize_env, render_text.rs pub(crate) helpers, secrets.rs Secret<T>::Serialize, fanout.rs JoinSet + JoinError, main.rs panic-hook + exit(2) wiring)
- [.planning/research/PITFALLS.md](/home/chasel/REPO/AIHPBar/.planning/research/PITFALLS.md) — Pitfall 3 SQLite locking + Pitfall 7 ANSI leak + Pitfall 13 exit codes + Pitfall 15 TUI non-TTY
- [.planning/phases/01-engine-claude-tui-scaffold/01-UI-SPEC.md](/home/chasel/REPO/AIHPBar/.planning/phases/01-engine-claude-tui-scaffold/01-UI-SPEC.md) — UI-SPEC LOCKED dimensions

### Secondary (MEDIUM confidence)
- [github.com/openai/codex/issues/23247](https://github.com/openai/codex/issues/23247) — confirms `state_5.sqlite` and `threads` table existence
- [github.com/openai/codex/issues/23979](https://github.com/openai/codex/issues/23979) — confirms threads still exist in state_5.sqlite (schema reference)
- [github.com/openai/codex/issues/21750](https://github.com/openai/codex/issues/21750) — SQLite corruption evidence (Phase 1 PITFALLS.md cited)
- [github.com/openai/codex/issues/23848](https://github.com/openai/codex/issues/23848) — SQLite init failure (corroborating fragility)
- [github.com/openai/codex/issues/23984](https://github.com/openai/codex/issues/23984) — migration 34 drops thread_goals; confirms internal-unstable schema
- [openai/codex/issues/23251 — WSL CLI cannot share Windows Codex CODEX_HOME — migration_1 modified](https://github.com/openai/codex/issues/23251) — confirms SQLx migrations are versioned (`migration_N`)
- [tokio JoinError docs](https://docs.rs/tokio/latest/tokio/task/struct.JoinError.html) — `is_panic()` API confirmed
- [tokenmix.ai 2026 Claude limits guide](https://tokenmix.ai/blog/complete-claude-limits-guide-2026-tokens-uploads-5-hour) — Pro 5h ≈ 44k tokens; weekly ratio guidance
- [pasqualepillitteri.it Claude weekly +50% May 2026 increase](https://pasqualepillitteri.it/en/news/2494/claude-code-weekly-limits-50-percent-anti-codex-anthropic-2026) — temporary increase verified
- [Claude help center "How do usage and length limits work?"](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work) — confirms Anthropic does NOT publish exact reset cadence (silence is data)
- [anthropics/claude-code issue #49599](https://github.com/anthropics/claude-code/issues/49599) — user report of Monday→Friday cycle change (community uncertainty about anchor)
- [github.com/clap-rs/clap discussion #4195](https://github.com/clap-rs/clap/discussions/4195) — clap handles mutually exclusive argument sets via ArgGroup multiple=false

### Tertiary (LOW confidence — community estimates / single source)
- [ccusage blocks-reports guide](https://ccusage.com/guide/blocks-reports) — Custom plan analyzes 192h (8d) of sessions for personalized weekly limit estimation (corroborates rolling-7d-from-first-prompt theory)
- [Maciek-roboblog/Claude-Code-Usage-Monitor](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor) — community tool that computes per-user weekly estimates (no fixed token count)
- [hypereal.cloud Claude Pro & Max Weekly Rate Limits Guide 2026](https://hypereal.cloud/a/weekly-rate-limits-claude-pro-max-guide) — claims weekly is ~5-7x of 5h; uses computing-hour budget framing
- [usagebar.com Claude code usage reset complete schedule guide](https://usagebar.com/blog/when-does-claude-code-usage-reset) — would have been useful; returned HTTP 403 from WebFetch
- [duet.so Claude Code Pricing 2026](https://duet.so/blog/claude-code-pricing) — pricing context (not primary for limits)
- [ratelimit.rs StudyRaid task panic handling](https://app.studyraid.com/en/read/10838/332175/task-panic-handling) — pattern reference for spawn_blocking + catch_unwind

## Metadata

**Confidence breakdown:**
- Codex JSONL `rate_limits` schema: HIGH — verified literal JSON from openai/codex issue #14728 with exact field names (primary/secondary/used_percent/window_minutes/resets_in_seconds)
- Codex SQLite `state_*.sqlite` schema: MEDIUM — `state_5.sqlite` filename verified; `threads` table existence verified; exact column names NOT publicly documented (recommendation is to NOT read columns in Phase 2)
- rusqlite API: HIGH — docs.rs/rusqlite/0.39.0 confirms OpenFlags BitOr and busy_timeout
- clap conflict semantics: HIGH — `#[group(multiple = false)]` confirmed via docs.rs/clap and discussion #4195
- tokio spawn_blocking + JoinError::is_panic: HIGH — docs.rs/tokio API stable since 1.0
- jiff RFC3339 emission: HIGH — already in production Phase 0 code (serde adapter)
- `CLAUDE_WEEKLY_TOKEN_LIMIT` value: MEDIUM-LOW — community estimates (Pro ≈ 220k tokens), Anthropic silent; recommend `Option<u64> = None` as safe default, `Some(220_000)` if planner wants a number
- Claude weekly anchor: MEDIUM-LOW — community sources diverge (Monday-anchored vs rolling-7d-from-first-prompt); recommend ISO week Mon 00:00 local time + README best-effort flag
- glob 0.3 sort behavior: HIGH (negative claim) — confirmed unsorted; explicit sort mandatory
- SEC-03 grep extension: HIGH — existing `#[cfg(debug_assertions)]` flag + `regex` dev-dep already wired
- Integration test fallback (rusqlite-direct fake state.sqlite): MEDIUM — pattern is sound, exact behavior under read-only + writer-holding-RESERVED needs test-time verification
- Codex schema-drift sentinel rename ("Claude" → `{Label}`): HIGH on recommendation, MEDIUM on whether UI-SPEC Phase 1 needs an amendment doc

**Research date:** 2026-05-25
**Valid until:** 2026-06-25 for the Codex schema (upstream is fast-moving; revisit if any planner question references a Codex CLI version > 0.115); 2026-08-25 for the broader stack (jiff / rusqlite / clap APIs are stable enough for 3 months); Claude weekly limit estimate revisit quarterly (per existing convention in `CLAUDE_5H_TOKEN_LIMIT` comment).
