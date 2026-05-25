# Phase 3: Gemini (conditional) + Cache & Refresh Policy — Research

**Researched:** 2026-05-25
**Domain:** Rust async fan-out + in-memory cache + TUI render variant
**Confidence:** HIGH (codebase reads verified; moka 0.12 API verified via docs.rs)

## Summary

Phase 3 is three additive deltas to a load-bearing engine + TUI that Phases 1/2 already locked. All major decisions (D-63..D-74) are LOCKED in CONTEXT; this RESEARCH only resolves the ten Claude's-Discretion items the planner must concretize before slicing.

**Primary recommendation:** Internal-own `moka::sync::Cache<ProviderId, CacheEntry>` inside `Engine`, write at the `Engine::refresh_all` wrapper layer (not inside `fanout::refresh_all_inner`), add a single `RowState::StaleOk { state, stale_age_secs }` variant, and ship Gemini stub finalization as a 4-file edit (gemini.rs reason string + default-config.toml comment + README §Gemini status + config.toml fixture). Yellow override at the TUI uses ratatui 0.30's `Style::default().fg(Color::Yellow)` applied to the whole `Line` via the leaf-widget pattern already in place.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-63** Gemini stub唯一 gate = `[providers.gemini] enabled = true`. 不新增 CLI flag. `GeminiUnimplementedProvider` 只改 error message 字面.
- **D-64** Default template `[providers.gemini]` 註解改為 `# Gemini CLI subscription — deferred to v2 stub (see README §Gemini status)`.
- **D-65** README §Gemini status section LOCKED — section title `## Gemini adapter status — deferred to v2`,內文兩段+ToS warning 句. Error reason 字面 = `Gemini adapter deferred to v2 — see README §Gemini status`.
- **D-66** 純 in-memory moka,不 disk-persist. CLI 短命模式 cache 永遠空 → transient error 走 Phase 1/2 既有 ERROR row path; TUI 長駐模式 cache 累積.
- **D-67** Cache key = `ProviderId`, `CacheEntry = { state: ProviderState, fetched_at: jiff::Timestamp }`.
- **D-68** Stale row 渲染路徑唯一在 `src/tui/ui.rs`. CLI render_text / render_json **不**加 stale path. JSON `schema_version:1` **不** bump、**不**加 `stale_age_secs`.
- **D-69** TUI stale row 字面 LOCKED: 共用 `compact_line_colored` 左半段 + 兩空格 + `(stale Ns ago)`,整數秒 floor,整行 Yellow override (覆蓋 percent threshold color); `--ascii` 後綴不變; `--color=never` 去 Yellow 但保後綴.
- **D-70** 新增 `RowState::StaleOk { state: ProviderState, stale_age_secs: u64 }` variant. 不擴 `ProviderState`.
- **D-71** `refresh_interval` = rate-limit cap + cache TTL 同值. 三態時間軸: t<last+TTL → `Ok`; t≥last+TTL + success → `Ok` + update cache; t≥last+TTL + transient fail → `StaleOk`; 從未成功+fail → `Err`/`SchemaDrift`.
- **D-72** Per-provider 預設 15s (claude/codex/mock/gemini). Clamp ≥ 5s 並 `tracing::warn!`. 上限不設.
- **D-73** CLI 不 honor `refresh_interval` — 每次都 fetch,cache 永不命中.
- **D-74** TUI 全域 15s tick (`tokio::time::interval(Duration::from_secs(15))`) **不動**.

### Claude's Discretion

- Stale-on-error error variant 分類 — 提議 helper `fn is_transient(&ProviderError) -> bool`.
- moka builder 具體配置 — `Cache::builder().max_capacity(8).build()`,不設 TTL/TTI.
- Cache 寫入時機 — option A (engine wrapper) vs B (fanout 內部); CONTEXT 建議 A.
- `RowState::StaleOk` 字段命名 — 提議 `{ state, stale_age_secs }`.
- TUI stale row 顏色實作 — Yellow Line override,Modifier::DIM 視覺強化 optional.
- Integration test 形態 — `IntermittentFailureProvider` + TUI snapshot 三態序列.
- `Engine::new` signature — 內部 own cache + 測試用 short refresh_interval.

### Deferred Ideas (OUT OF SCOPE)

- Disk-persist last-good cache → v2
- `--experimental-gemini` CLI flag → v2
- Cache trait abstraction → v2
- HTTP wiremock infra → v2
- Separate `cache_ttl` config field → v2
- `--refresh` CLI flag → v2
- `AHB doctor` 子命令 → v2 debug
- Daemon mode → v2 OPS-01
- Conditional GET / ETag / daily ceiling → v2 (HTTP adapter才有)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TUI-03 | TUI refresh 頻率可由 config 設定,per-provider 可覆寫 | §Open Q3 (cache write site), §Open Q4 (Engine ownership), §Slice 1 + Slice 2 |
| CFG-03 | Config 允許 per-provider 指定 refresh interval | §Open Q9 (KNOWN_PROVIDER_FIELD_KEYS),§Slice 1 |
| ADP-05 | Gemini stub (NO-GO path) | §Open Q (Gemini error reason),§Slice 4 |
</phase_requirements>

## Open Questions Resolved

### Q1 — moka 0.12 API specifics (sync vs future, default eviction)

**Answer:** Use `moka::sync::Cache<ProviderId, CacheEntry>` with `Cache::builder().max_capacity(8).build()`. **No** `time_to_live` / `time_to_idle` set — entries live until manually invalidated or `max_capacity` hit. `Cache` is `Send + Sync + Clone`; clone is cheap (Arc-internal).

**Source:** docs.rs/moka/0.12.15/moka/sync/struct.Cache.html — verified via WebFetch 2026-05-25. Latest version on crates.io = `0.12.15` (verified via `cargo search moka`).

**Confidence:** HIGH

**Rationale for sync over future:** `Engine::refresh_all` is already `async`, but cache `get`/`insert` are synchronous-fast (lock-free hash table); a sync API inside an async function is the simpler pattern and avoids a second `await` per cache hit. The `future` variant only adds value when you need `entry_async` for atomically computing-on-miss — Phase 3 does explicit get-then-insert.

### Q2 — `is_transient(&ProviderError) -> bool` exact variant mapping

**Answer:** Add helper in `src/engine/mod.rs` (or new `src/engine/cache.rs`):

```rust
fn is_transient(err: &ProviderError) -> bool {
    matches!(
        err,
        ProviderError::Network { .. } | ProviderError::RateLimited { .. }
    )
}
```

Variants confirmed from `src/model.rs:108-140`:

| Variant | is_transient | Rationale |
|---------|--------------|-----------|
| `Unconfigured` | **false** | User config issue,not a transient fault |
| `Unavailable { reason }` | **false** | Gemini stub 永遠失敗 + adapter timeout 也走這條; stale 只延遲 user 察覺 |
| `SchemaDrift { missing }` | **false** | 上次成功的值已不適用新 schema |
| `Network { source }` | **true** | 典型 transient (Gemini v2 才有,但 helper 先就位) |
| `RateLimited { retry_after }` | **true** | 典型 transient |
| `Internal { source }` | **false** | adapter panic recovery + JoinError; 不確定 cache 還有效 |

**Confidence:** HIGH

**Note for planner:** Engine fanout maps `tokio::time::timeout` elapsed to `ProviderError::Unavailable { reason: "timed out after ..." }` (see `src/engine/fanout.rs:57-59`). This means **adapter timeouts do NOT trigger stale fallback** — they look like `Unavailable`. This is consistent with D-72 (TTL ≥ 5s; a 2s timeout is "adapter actually unreachable", not "transient network blip"). Document this so the planner doesn't accidentally widen `is_transient` to include `Unavailable`.

### Q3 — Cache write integration layer (A vs B)

**Answer:** **Option A — write in `Engine::refresh_all`**, leave `fanout::refresh_all_inner` pure.

**Source code shape (verified):**
- `src/engine/mod.rs:101-117` — `Engine::refresh_all` is the wrapper that calls `fanout::refresh_all_inner` then sorts by canonical row order.
- `src/engine/fanout.rs:34-100` — `refresh_all_inner` is purely `Vec<Arc<dyn Provider>> → Vec<(ProviderId, Result<...>)>`, no engine state.

Option A keeps `fanout` testable in isolation (existing `panic_in_adapter_becomes_internal_error_not_lost_task`, `slow_adapter_returns_unavailable_after_timeout` tests at `src/engine/fanout.rs:165-220` stay unchanged) and concentrates cache concern at the engine boundary where `Engine` already owns the provider list, secrets, and per-provider timeout.

**Recommended shape:**
```rust
pub async fn refresh_all(&self, now: jiff::Timestamp) -> Vec<(ProviderId, RowOutcome)> {
    // 1. Filter providers whose refresh_interval has not elapsed → emit cache hit as RowOutcome::Fresh
    // 2. Fan out the remaining set via refresh_all_inner
    // 3. For each result: success → update cache + emit Fresh; transient err with cache hit → emit Stale
    // 4. Sort by canonical key
}
```

**Where to gate fan-out skip vs. always fan out:** Recommend **always fan out** for v1 (simpler — keeps `fanout::refresh_all_inner` invariant). The TTL filter just decides whether to *return the cached state* vs. *use the fresh fetch result*. Optimization (skip the actual fetch when TTL not elapsed) is a Phase 3+ optimization that breaks the "fanout is pure" property. **Planner: choose between** (a) cache-only-result path that pre-filters and skips fetch, vs. (b) always-fetch path that just chooses what to return. CONTEXT D-72/D-73 strongly imply (a) — `refresh_interval` exists explicitly as a rate-limit cap. Verify with author on slice 2.

**Confidence:** HIGH (architectural fit); MEDIUM on (a) vs (b) (CONTEXT leans (a), worth confirming).

### Q4 — `Engine::new` ownership of Cache (internal own vs injectable)

**Answer:** **Internal own.** Build cache in `Engine::new`. For tests, inject `refresh_interval_override: Option<HashMap<ProviderId, Duration>>` so tests can use 1s intervals without changing config.

**Source:** Phase 1 BL-01 pattern (Plan 01-04) — `FetchCtx::now` is the clock seam, not `Engine` ownership. `tests/engine_row_order.rs` and `src/engine/mod.rs:158-176` already construct `Engine::new(cfg, Secrets::default())` directly; they don't inject anything beyond config. Adding a `Cache` injection point would be premature flexibility (cache trait abstraction is explicitly Deferred).

**Concrete plan:**
```rust
pub struct Engine {
    providers: Vec<Arc<dyn Provider>>,
    secrets: Arc<Secrets>,
    per_provider_timeout: Duration,
    // New Phase 3 fields:
    cache: moka::sync::Cache<ProviderId, CacheEntry>,
    refresh_intervals: HashMap<ProviderId, Duration>,
}
```

`refresh_intervals` populated in `Engine::new` from `cfg.providers.<id>.refresh_interval.unwrap_or(DEFAULT_REFRESH_INTERVAL_SECS)` for each enabled provider.

**Confidence:** HIGH

### Q5 — `AppState::apply_results` signature change

**Answer:** Change `apply_results` to consume **`Vec<(ProviderId, RowOutcome)>`** where `RowOutcome` is a new engine-layer enum that pre-bakes the stale decision:

```rust
// In src/engine/mod.rs (or src/engine/cache.rs)
pub enum RowOutcome {
    Fresh(ProviderState),
    Stale { state: ProviderState, stale_age_secs: u64 },
    Failed(ProviderError),
}
```

`AppState::apply_results` becomes a pure translator (no cache lookup logic in `app.rs`):

| RowOutcome | RowState |
|------------|----------|
| `Fresh(state)` | `Ok(state)` |
| `Stale { state, stale_age_secs }` | `StaleOk { state, stale_age_secs }` |
| `Failed(ProviderError::SchemaDrift)` | `SchemaDrift { id }` |
| `Failed(other)` | `Err { id, message: other.to_string() }` |

**Source:** Current `apply_results` signature at `src/tui/app.rs:63-78` already takes `Vec<(ProviderId, Result<ProviderState, ProviderError>)>` — widening to `Vec<(ProviderId, RowOutcome)>` is a clean replacement. Cache lookup logic stays in `Engine`; `AppState` is pure UI translation.

**Confidence:** HIGH

**Why not pass cache snapshot into `apply_results`:** Coupling `AppState` to the cache crate is the wrong direction — `app.rs` would need to import `moka` to read the cache, and the stale-age math would have to know about `jiff::Timestamp::since` semantics. Engine-side pre-bake is cleaner.

### Q6 — ratatui 0.30 yellow-row override pattern

**Answer:** Apply `Style::default().fg(Color::Yellow)` at the **Line level via Span styling on each Span**, OR use `Paragraph::new(line).style(Style::default().fg(Color::Yellow))` which applies the style as a base that individual Span styles override. For the StaleOk case, **build the Line with already-yellow spans** — do NOT rely on Paragraph base style alone.

**Source:** Verified from `src/tui/widgets/hp_row.rs:96-103` — the existing `build_ok_line` uses `Span::styled(text, Style::default().fg(accent))` per-span. The ratatui 0.30 contract: each Span's style takes precedence over the parent Line/Paragraph's style. To make the row "all yellow regardless of percent threshold", construct each Span with `Style::default().fg(Color::Yellow)` directly — do NOT call `build_ok_line` and try to wrap it.

**Recommended implementation:** Add `build_stale_ok_line(state, stale_age_secs, now)` in `src/tui/widgets/hp_row.rs` that mirrors `build_ok_line` but:
1. Forces all bar/text Span colors to `Color::Yellow`
2. Appends `Span::raw("  ")` + `Span::styled(format!("(stale {n}s ago)"), Style::default().fg(Color::Yellow))`

**Confidence:** HIGH

**Modifier::DIM:** CONTEXT calls this Claude's discretion. **Recommendation: skip DIM** for v1 — Yellow alone signals "not current"; DIM can wash out on dim-background terminals (e.g. Solarized Light). If users request a stronger signal, add in Phase 3.5.

### Q7 — Integration test design

**Answer:** Use `IntermittentFailureProvider` defined in a `tests/common/` helper or inside the test file itself; clock advance via re-construction (`Engine::new` per tick or a `now: jiff::Timestamp` parameter to `Engine::refresh_all` already exists — see `src/engine/mod.rs:101-104`).

**Pattern aligned with existing snapshot infra:** `src/engine/fanout.rs:139-163` already defines `OkProvider`/`SlowProvider`/`PanicProvider` as test helpers. Phase 3 adds:

```rust
struct IntermittentFailureProvider {
    call_count: AtomicU64,
    fail_every_nth: u64,
}

#[async_trait]
impl Provider for IntermittentFailureProvider {
    fn id(&self) -> ProviderId { ProviderId::Mock }
    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        let n = self.call_count.fetch_add(1, Ordering::SeqCst);
        if n % self.fail_every_nth == 0 {
            Err(ProviderError::Network { source: NetworkErr("simulated".into()) })
        } else {
            Ok(/* synthetic state with ctx.now */)
        }
    }
}
```

**TUI snapshot infra:** Use `ratatui::backend::TestBackend` per established Phase 1/2 pattern. Cross-reference existing usage — search for `TestBackend` in tests/ and src/tui/ (none today; this will be the first TUI-level snapshot test). Slice 5 can defer the full snapshot if scope tight.

**Confidence:** MEDIUM-HIGH (pattern verified; specific snapshot file path TBD by planner).

### Q8 — No-walltime regression test extension

**Answer:** Cache `fetched_at` write site must route through `FetchCtx::now` (which adapters already use) OR through the engine-layer `now` parameter passed to `Engine::refresh_all`.

**Verified from `tests/no_walltime_in_adapter.rs:30-40`:** Current scan covers `src/provider/` and `src/tui/widgets/`. `src/engine/` is **NOT** currently in the grep scope, so engine-layer `Timestamp::now()` is allowed BUT the natural cache write site uses `now: jiff::Timestamp` already passed into `refresh_all` (`src/engine/mod.rs:101-104`). When fanout returns successful `ProviderState`, its `fetched_at` is already set from `FetchCtx { now, ... }` (see `src/engine/fanout.rs:50-53`).

**Planner action:**
- Phase 3 cache write: `cache.insert(id, CacheEntry { state, fetched_at: state.fetched_at })`. Do **not** call `jiff::Timestamp::now()` inside the cache write — reuse the value already in `ProviderState`.
- Engine-layer stale-age math: `(now - cache_entry.fetched_at).total(jiff::Unit::Second)` where `now` is the engine `refresh_all(now)` parameter. No new walltime callsite.
- **Optional extension** to `tests/no_walltime_in_adapter.rs`: add `src/engine/` to the scan dir list as a defensive guardrail. Current scope is provider/widgets — engine is structurally under-the-line so extending is overkill, but document the rule in the cache module's doc-comment.

**Confidence:** HIGH

### Q9 — `KNOWN_PROVIDER_FIELD_KEYS` + `refresh_interval` parsing

**Answer:** Add `"refresh_interval"` to `KNOWN_PROVIDER_FIELD_KEYS` (currently `&["enabled"]` at `src/config.rs:30`). Extend `ProviderConfig` to:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub refresh_interval: Option<u64>,  // seconds
}
```

**Parse type rationale:** `u64` (not `u32`) because:
1. CONTEXT D-72 explicitly mentions "user 可設 600s / 1h+ / 24h" — `u32` covers 24h fine (86,400s) but `u64` is the idiomatic Rust seconds type (matches `Duration::from_secs(u64)`).
2. `toml = "0.8"` (Cargo.toml:50) parses integers as `i64` by default; `serde` will reject negative values when deserializing into `u64` automatically.
3. `Option<u64>` lets `None` mean "use per-provider default" without sentinel values like `0`.

**Verified from `src/config.rs:30, 140-148`:** Unknown-key warn-walker is field-name-based. Adding `"refresh_interval"` to the const array silences the D-38 warn for the new key. Typos like `refresh_intervall` will still trigger the warn (verified by the unknown-key test pattern at `src/config.rs:225-233`).

**Confidence:** HIGH

### Q10 — moka feature flag / STACK.md addition

**Answer:** Add to Cargo.toml:

```toml
# Phase 3 — moka stale-on-error cache (D-66, D-67).
moka = { version = "0.12", default-features = false, features = ["sync"] }
```

**Verified:** `cargo search moka` → `moka = "0.12.15"` is current; `mini-moka` exists as a lighter alternative but lacks the same `Cache::builder()` ergonomics and is overkill-savings for v1. `moka2` is a fork (NOT to use). Default features include both `sync` and `future` — explicitly opting into only `sync` shrinks the dep tree (drops a `futures-util` propagation we already have, harmless).

**STACK.md addition:** Phase 3 implementation should add to `.planning/research/STACK.md` an entry under the "Recommended Stack" → "Core Technologies" table:

| Technology | Version | Purpose | Why Recommended |
|---|---|---|---|
| **moka** | 0.12.15 | In-memory stale-on-error cache | Lock-free concurrent hash table; sync API fits engine `async fn` without extra `await`s; `Send + Sync + Clone` makes Arc-free sharing trivial; explicit no-TTL mode aligns with D-71 (manual stale eviction) |

**Confidence:** HIGH

## Code-level Anchors

For `<read_first>` blocks in PLAN.md tasks:

### Engine layer
- `src/engine/mod.rs:29-33` — `Engine` struct definition (3 fields today, +2 Phase 3)
- `src/engine/mod.rs:39-88` — `Engine::new` — Phase 3 inserts `refresh_intervals` build + `cache: Cache::builder().max_capacity(8).build()`
- `src/engine/mod.rs:101-117` — `Engine::refresh_all` — Phase 3 wraps `fanout::refresh_all_inner` with TTL filter + cache read/write
- `src/engine/mod.rs:122-129` — `Engine::sort_key` — does NOT change (canonical row order is `ProviderId`-keyed, not `RowOutcome`-keyed; sort applies to either)
- `src/engine/fanout.rs:34-100` — `refresh_all_inner` — does NOT change (purity preserved)
- `src/engine/fanout.rs:25` — `DEFAULT_PER_PROVIDER_TIMEOUT = Duration::from_secs(2)` — distinct from refresh_interval; 不要混淆

### Config layer
- `src/config.rs:30` — `KNOWN_PROVIDER_FIELD_KEYS: &[&str] = &["enabled"]` → add `"refresh_interval"`
- `src/config.rs:32-36` — `ProviderConfig` struct → add `refresh_interval: Option<u64>` field
- `src/config.rs:91-112` — `load_or_init` — no changes needed (typed parse handles new optional field transparently)
- `src/config.rs:140-147` — `warn_unknown_keys` field walker — needs the KNOWN_PROVIDER_FIELD_KEYS update only

### Templates / docs
- `src/templates/default-config.toml:11-12` — Gemini block — change comment per D-64; do NOT add `refresh_interval` to template (users get default 15s without writing it)
- `src/provider/gemini.rs:44-48` — `GeminiUnimplementedProvider::fetch` error reason → change to `"Gemini adapter deferred to v2 — see README §Gemini status"`
- `src/provider/gemini.rs:69-77` — existing test `gemini_placeholder_returns_unavailable` asserts `contains("not yet implemented")` and `contains("enabled = false")` — **both assertions break** with the new reason; update test text to match new字面

### TUI layer
- `src/tui/app.rs:21-31` — `RowState` enum → add `StaleOk { state: ProviderState, stale_age_secs: u64 }` variant
- `src/tui/app.rs:63-78` — `apply_results` signature change (input type) + match logic for new outcome
- `src/tui/widgets/hp_row.rs:49-58` — `render(area, buf, row, ascii, now)` — does NOT change at signature level
- `src/tui/widgets/hp_row.rs:66-72` — `build_line` match → add `RowState::StaleOk { state, stale_age_secs } => build_stale_ok_line(state, *stale_age_secs, now)` arm
- `src/tui/widgets/hp_row.rs:74-104` — `build_ok_line` — keep as-is; build_stale_ok_line is a sibling, not a wrapper
- `src/tui/mod.rs:124-127` — `tokio::time::interval(Duration::from_secs(15))` — **DO NOT CHANGE** (D-74)
- `src/tui/mod.rs:144-147` — fetch tick arm → engine method signature change propagates here
- `src/tui/mod.rs:108, 112, 120, 145, 153` — `jiff::Timestamp::now()` callsites — all authorized; do NOT add a new one

### Test infrastructure
- `tests/no_walltime_in_adapter.rs:30-40` — scan dirs are `["src/provider", "src/tui/widgets"]` — extending to engine is optional defensive bonus
- `src/engine/fanout.rs:111-163` — `PanicProvider` / `SlowProvider` / `OkProvider` test scaffolding — pattern for `IntermittentFailureProvider`

## External API Snippets

### moka 0.12 sync Cache (verified docs.rs/moka/0.12.15)

```rust
use moka::sync::Cache;
use crate::model::{ProviderId, ProviderState};

#[derive(Clone)]
pub struct CacheEntry {
    pub state: ProviderState,
    pub fetched_at: jiff::Timestamp,
}

let cache: Cache<ProviderId, CacheEntry> = Cache::builder().max_capacity(8).build();
cache.insert(ProviderId::Claude, entry.clone());
let hit: Option<CacheEntry> = cache.get(&ProviderId::Claude);
```

Notes: `Cache::get` returns `Option<V>` (a clone, not `&V`) — `CacheEntry` must implement `Clone`. `ProviderState` already derives `Clone` (verified `src/model.rs:76`).

### ratatui 0.30 Yellow Line override

```rust
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

let line = Line::from(vec![
    Span::raw("claude  "),
    Span::styled("\u{2588}".repeat(6), Style::default().fg(Color::Yellow)),
    Span::styled("\u{2591}".repeat(4), Style::default().fg(Color::Yellow)),
    Span::raw(" 60% "),
    Span::styled("\u{2022}", Style::default().fg(Color::Yellow)),
    Span::raw(" resets in 2h00m  (stale 32s ago)"),
]);
```

Per-Span styling overrides any parent style — this is the only reliable way to force "whole row yellow regardless of pct threshold" in ratatui 0.30.

### jiff Timestamp duration arithmetic

```rust
use jiff::{Timestamp, Unit};

let stale_age_secs: u64 = now
    .since((Unit::Second, cache_entry.fetched_at))
    .ok()
    .and_then(|span| i64::try_from(span.get_seconds()).ok())
    .and_then(|s| u64::try_from(s.max(0)).ok())
    .unwrap_or(0);
```

Pattern matches `format_countdown` at `src/cli/render_text.rs:293-303`. Negative spans clamp to 0; failed conversions fall back to 0 (defensive — should not happen if cache invariant holds).

## Files to Create/Modify

### Slice "Gemini stub finalization" (D-63, D-64, D-65)

| File | Change | Purpose |
|------|--------|---------|
| `src/provider/gemini.rs` | Edit `fetch` error reason string (line 45-47); update test assertions (line 69-77) | D-65 — align stub error字面 with README section |
| `src/templates/default-config.toml` | Edit Gemini comment (line 12) | D-64 — reflect "deferred" not "pending" |
| `README.md` | Add `## Gemini adapter status — deferred to v2` section | D-65 — ToS warning + v2 trigger doc |

### Slice "Per-provider refresh_interval" (CFG-03, D-71, D-72, D-73)

| File | Change | Purpose |
|------|--------|---------|
| `src/config.rs` | Add `refresh_interval: Option<u64>` to `ProviderConfig`; add `"refresh_interval"` to `KNOWN_PROVIDER_FIELD_KEYS` | D-72 — config schema extension |
| `src/provider/claude.rs` | Add `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15;` | D-72 — per-provider default |
| `src/provider/codex.rs` | Add `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15;` | D-72 |
| `src/provider/gemini.rs` | Add `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15;` | D-72 (cosmetic; stub never fetches) |
| `src/provider/mock.rs` | Add `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15;` | D-72 |
| `src/engine/mod.rs` | Add `refresh_intervals: HashMap<ProviderId, Duration>` field; populate in `Engine::new`; add clamp logic with `tracing::warn!` | D-72 clamp ≥ 5s |

### Slice "moka stale-on-error cache" (TUI-03, SC-4, D-66..D-71)

| File | Change | Purpose |
|------|--------|---------|
| `Cargo.toml` | Add `moka = { version = "0.12", default-features = false, features = ["sync"] }` | D-66 dependency |
| `src/engine/mod.rs` or `src/engine/cache.rs` (new module) | Add `CacheEntry`, `RowOutcome`, `is_transient` helper, cache+TTL logic | D-66, D-67, D-71 |
| `src/engine/mod.rs` | Change `Engine::refresh_all` return type to `Vec<(ProviderId, RowOutcome)>` | Q5 resolution |
| `src/tui/app.rs` | Add `RowState::StaleOk` variant; rewrite `apply_results` to consume `RowOutcome` | D-70 |
| `src/tui/widgets/hp_row.rs` | Add `build_stale_ok_line(state, stale_age_secs, now)` + match arm in `build_line` | D-69 yellow override + suffix |
| `src/cli/mod.rs` (compact/detailed/json dispatch) | Adapt to new `Engine::refresh_all` return type — call new helper that converts `RowOutcome` → `Result<ProviderState, ProviderError>` (CLI cache永遠空, so RowOutcome 永遠 Fresh/Failed,never Stale) | D-66, D-73 |

### Slice "Test surface"

| File | Change | Purpose |
|------|--------|---------|
| `tests/refresh_interval_config_parse.rs` (new) | Parse roundtrip + clamp test + unknown-key tolerance | CFG-03 verification |
| `tests/cache_stale_on_error.rs` (new) | Unit-level: engine with IntermittentFailureProvider + 3-tick sequence; assert RowOutcome shape | SC-4 verification |
| `tests/tui_stale_row_snapshot.rs` (new, optional Slice 5) | TestBackend snapshot of fresh → stale → fresh sequence | D-69 visual lock |

## Test Surface

### Unit-level (in-file `#[cfg(test)] mod tests`)
- `src/engine/cache.rs` (or `src/engine/mod.rs`) — `is_transient` table-driven test covering all six `ProviderError` variants
- `src/engine/mod.rs` — `engine_caches_successful_fetch` (insert + get back same entry)
- `src/engine/mod.rs` — `engine_returns_stale_on_transient_error_with_cache_hit`
- `src/engine/mod.rs` — `engine_returns_failed_on_non_transient_error_with_cache_hit` (e.g. SchemaDrift — no fallback)
- `src/engine/mod.rs` — `refresh_interval_clamps_to_five_seconds_minimum` + warn-emit
- `src/config.rs` — `provider_config_parses_refresh_interval` + `provider_config_refresh_interval_missing_means_none`
- `src/tui/app.rs` — `apply_results_translates_stale_ok` + `apply_results_passes_stale_age_secs_unchanged`
- `src/tui/widgets/hp_row.rs` — `build_stale_ok_line_uses_yellow_for_all_spans` + `build_stale_ok_line_includes_stale_suffix`

### Integration tests (in `tests/`)
- `refresh_interval_config_parse.rs` — assert parse + clamp behavior + unknown-key (`refresh_intervall` typo) still warns
- `cache_stale_on_error.rs` — `IntermittentFailureProvider` 3-tick sequence (success → fail+cache hit → success); assert outcome variants
- `tui_stale_row_snapshot.rs` — TestBackend visual snapshot (slice 5; optional)

### Phase 1 invariants — verify still pass
- `tests/no_walltime_in_adapter.rs` — Phase 3 must NOT add `Timestamp::now()` under `src/provider/` or `src/tui/widgets/`
- `tests/engine_row_order.rs` — canonical row order MUST survive `Vec<(ProviderId, RowOutcome)>` shape change
- `tests/json_format_round_trip.rs` — schema_version: 1 MUST NOT bump (D-68)
- `tests/detailed_format.rs` — detailed CLI MUST NOT acquire stale path (D-68)
- `tests/cli_walking_skeleton.rs` — compact CLI MUST NOT acquire stale path (D-66)
- `tests/exit_codes.rs` — Gemini stub still produces exit 1 in Gemini-only-fails config

## Pitfalls / Landmines

### Things the planner MUST NOT do

1. **DO NOT set `time_to_live` or `time_to_idle` on moka Cache.** Q1 resolution: D-71 wants manual stale semantics; moka auto-eviction would silently remove the entry that the error path wants to surface as `StaleOk`. Always-manual.

2. **DO NOT bump JSON `schema_version` past 1.** D-68 explicit. The new `stale_age_secs` is a TUI-only concept; JSON consumers (tmux/Starship) see Phase 2 byte-identical output. `tests/json_format_round_trip.rs` must continue to pass without changes.

3. **DO NOT change `tokio::time::interval(Duration::from_secs(15))` at `src/tui/mod.rs:124`.** D-74. Global tick is the fan-out cadence; per-provider gating happens inside `Engine::refresh_all`.

4. **DO NOT add a CLI flag (`--experimental-gemini`, `--no-cache`, `--refresh`, etc.).** D-63 / D-66 / D-73. All deferred to v2.

5. **DO NOT touch `format_error_row_colored` at `src/cli/render_text.rs:167-206`.** D-68. CLI ERROR row path stays Phase 1/2 exact.

6. **DO NOT widen `is_transient` to include `Unavailable`.** Q2 — `Unavailable` covers Gemini stub (permanent) and adapter timeout (transient-looking but D-72 5s clamp says "if it timed out, it's actually broken"). If a real user complains about Gemini-stub being treated as stale, they're already misconfigured; widening this would mask real failures.

7. **DO NOT use `moka::future::Cache`.** Q1 — `sync` is correct; future variant adds an `await` per cache op for zero gain.

8. **DO NOT use `mini-moka` or `moka2`.** `mini-moka` lacks the same builder ergonomics; `moka2` is an unmaintained fork. STACK.md verified.

9. **DO NOT add a `Cache` trait abstraction in Phase 3.** v2 explicit Deferred. YAGNI.

10. **DO NOT add `stale_age_secs` to `ProviderState`.** D-70 — `RowState` is the right place; `ProviderState` is a wire DTO and the new field would either need a JSON schema bump (forbidden by D-68) or a `#[serde(skip)]` that lies to readers.

11. **DO NOT call `jiff::Timestamp::now()` inside `src/provider/gemini.rs` (or any provider).** Phase 1 BL-01 rule, enforced by `tests/no_walltime_in_adapter.rs`. Even though Gemini stub never uses `ctx.now`, the discipline holds.

12. **DO NOT change the test `gemini_placeholder_returns_unavailable`'s structure** (just the assertion strings). The test pattern (assert via `match ProviderError::Unavailable { reason } => assert!(reason.contains(...))`) is correct; only the substring assertions need updating.

13. **DO NOT add `refresh_interval` to `src/templates/default-config.toml`.** D-72 says "defaults if absent"; including it in the template would clutter the file with values that all match the defaults. Document it in README instead.

14. **DO NOT remove `src/cli/render_text.rs::format_error_row_colored`'s ProviderError::Unavailable path** — Gemini stub still routes through this on CLI (D-66 + D-73). 確認 it still produces `gemini  ERROR: Gemini adapter deferred to v2 — see README §Gemini status` after the reason-string change.

15. **DO NOT inject `Cache` into `Engine::new` as a parameter.** Q4 — internal own; tests use config-level `refresh_interval` override.

16. **DO NOT skip the fanout call when ALL providers are within TTL.** If you choose path (a) in Q3 (pre-filter + skip), the function must still emit one `RowOutcome::Fresh` per provider; an empty `Vec` would mistake "all cached" for "no providers" and trigger the empty-state UI.

## Recommended Plan Slicing

Five slices (sized for Phase 3 mode=mvp); planner may consolidate Slices 1+2 if dependency tight, or defer Slice 5 if scope pressure.

### Slice 1 — `refresh_interval` config field (CFG-03 partial)
**Scope:** Add the config knob without engine-side gating yet.
- `src/config.rs`: add `refresh_interval: Option<u64>` to `ProviderConfig`; add `"refresh_interval"` to `KNOWN_PROVIDER_FIELD_KEYS`
- Per-provider modules: add `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15`
- `tests/refresh_interval_config_parse.rs`: parse roundtrip + clamp test + typo-warn test

**Verifiable by:** `cargo test refresh_interval_config_parse` green. Engine behavior unchanged. CLI/TUI unchanged.

### Slice 2 — moka cache + is_transient + Engine::refresh_all rewrite (TUI-03 core)
**Scope:** The big architectural change. Adds `moka` dep, `CacheEntry`, `RowOutcome`, `is_transient`, and rewrites `Engine::refresh_all` to filter by TTL + cache read/write + stale decision.
- Add `moka` dep to Cargo.toml
- New module `src/engine/cache.rs` (or inline in `src/engine/mod.rs`)
- Rewrite `Engine::refresh_all` signature: `Vec<(ProviderId, Result<...>)>` → `Vec<(ProviderId, RowOutcome)>`
- Add `refresh_intervals` + `cache` fields to `Engine`; populate in `Engine::new` with clamp + warn
- Update `src/cli/mod.rs` dispatch to translate `RowOutcome` → existing CLI render paths (CLI cache always empty → never Stale; safe to map `Fresh → Ok`, `Failed → Err`, `Stale → unreachable!`)
- Unit tests in `src/engine/mod.rs`: cache hit, cache miss, TTL filter, is_transient table-drive

**Verifiable by:** `cargo test --lib engine` green; existing `tests/engine_row_order.rs` + `tests/cli_walking_skeleton.rs` + `tests/exit_codes.rs` still green.

### Slice 3 — `RowState::StaleOk` + TUI yellow row (D-69, D-70)
**Scope:** Wire the engine-layer `RowOutcome::Stale` into the TUI render path.
- `src/tui/app.rs`: add `RowState::StaleOk` variant; rewrite `apply_results` to consume `RowOutcome`
- `src/tui/widgets/hp_row.rs`: add `build_stale_ok_line` with yellow override + `(stale Ns ago)` suffix
- `src/tui/mod.rs`: thread the new return type through fetch tick
- Unit tests in `app.rs` + `hp_row.rs`

**Verifiable by:** `cargo test --lib tui` green; `tests/tui_panic_safe_restore.rs` + `tests/tui_non_tty_refusal.rs` still green.

### Slice 4 — Gemini stub finalization (ADP-05, D-65)
**Scope:** Pure字面 + doc work; smallest slice; can run parallel to others.
- `src/provider/gemini.rs`: error reason string update + test assertion update
- `src/templates/default-config.toml`: Gemini comment update
- `README.md`: new `## Gemini adapter status — deferred to v2` section

**Verifiable by:** `cargo test gemini` green;手動 `grep -F "Gemini adapter deferred to v2"` 在 src/ + README.md 都命中.

### Slice 5 — Integration test: IntermittentFailureProvider + 3-tick TUI snapshot (optional)
**Scope:** Belt-and-suspenders integration coverage for the full stale flow.
- New `tests/cache_stale_on_error.rs`: drive Engine with `IntermittentFailureProvider`; assert outcome sequence
- New `tests/tui_stale_row_snapshot.rs` (optional if scope pressure): TestBackend visual snapshot

**Verifiable by:** Both tests green. If snapshot test deferred, document why in SUMMARY.md.

### Dependency graph for slicing
```
Slice 1 ───┐
           ├──> Slice 2 ───┐
Slice 4   独立              ├──> Slice 3 ───> Slice 5
                            │
            (Slice 2 alone makes CLI / Engine compile;
             Slice 3 makes TUI compile; Slice 5 verifies)
```

Slice 4 has zero dependency on 1/2/3 — can be done first or last. Slice 1 must precede Slice 2 (Slice 2 reads `cfg.providers.<id>.refresh_interval`). Slice 3 must follow Slice 2 (depends on `RowOutcome` type).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `moka::sync::Cache::builder().build()` with no TTL = "live until invalidated or capacity-evicted" | Q1, External API | If moka 0.12 silently TTLs entries, stale row would disappear before user can observe — would require explicit `.time_to_live(Duration::MAX)` workaround. Verified via docs.rs but not via local cargo expand. |
| A2 | `tokio::time::timeout` elapsed converts to `ProviderError::Unavailable`, not `ProviderError::Network` | Q2 mapping | Verified at `src/engine/fanout.rs:57-59` — solid. |
| A3 | ratatui 0.30 Span-level styling overrides Paragraph base style | Q6 | Verified via existing `build_ok_line` pattern at `src/tui/widgets/hp_row.rs:96-103`. |
| A4 | Phase 3 cache 永遠 read-only in CLI dispatch path (CLI cache 永遠空) | Q5 / Slice 2 | If CLI dispatch accidentally writes cache, breaks D-66; mitigated by Engine::refresh_all being the ONLY write site and CLI being short-lived (process dies before cache reuse). |
| A5 | `is_transient` should NOT include `Unavailable` even though adapter timeout currently maps there | Q2 / Pitfall 6 | If users complain that 2-second adapter timeouts should yield stale fallback, would need to either widen `is_transient(Unavailable)` (loses Gemini-stub discrimination) or change timeout to its own variant. Document in PLAN.md as Phase 3.5 fodder if it comes up. |

## Open Questions

1. **(a) vs (b) in Q3 — pre-filter+skip-fanout vs always-fanout-and-choose?**
   - What we know: CONTEXT D-72 + D-73 strongly imply (a). D-71's "language: rate-limit cap" reads as "don't fetch if within TTL".
   - What's unclear: (a) is the obviously-correct behavior; (b) is the obviously-easier implementation. Picking (a) means `Engine::refresh_all` skips fan-out for providers within TTL — cleaner UX, slightly more complex logic.
   - Recommendation: Implement (a). If (b) accidentally ships, it's still correct per D-71 (the TTL filter just decides what to return); user-visible behavior identical assuming fetch is fast.

2. **README §Gemini status — exact wording for the v2 trigger sentence?**
   - What we know: D-65 LOCKS the section title and the basic structure (two paragraphs + ToS sentence).
   - What's unclear: exact prose of the v2 trigger condition (probably "see GEMINI_SPIKE.md § Kill criteria" link, but planner should confirm with the spike doc's actual section name).
   - Recommendation: Read `GEMINI_SPIKE.md § Kill criteria` (or whatever the actual section is named — verified at `.planning/research/GEMINI_SPIKE.md` lines 154-161) and quote-link in the README.

## Environment Availability

Skip — Phase 3 is pure Rust code/config changes. No external CLI tools, services, or runtimes added beyond the existing project deps. `moka` is a regular crate dependency (Slice 2 adds it).

## Sources

### Primary (HIGH confidence)
- Codebase reads: `src/provider/gemini.rs`, `src/provider/mod.rs`, `src/engine/mod.rs`, `src/engine/fanout.rs`, `src/config.rs`, `src/tui/app.rs`, `src/tui/ui.rs`, `src/tui/mod.rs`, `src/tui/widgets/hp_row.rs`, `src/cli/render_text.rs`, `src/model.rs`, `Cargo.toml`, `tests/no_walltime_in_adapter.rs`, `src/templates/default-config.toml` (all verified 2026-05-25)
- `.planning/phases/03-gemini-conditional-cache-refresh-policy/03-CONTEXT.md` — D-63..D-74 LOCKED decisions
- `.planning/research/PITFALLS.md` — § Pitfall 1, 4, 12 — informs is_transient and refresh_interval defaults
- `.planning/research/GEMINI_SPIKE.md` § Phase 3 hand-off (lines 154-161) — NO-GO path字面
- docs.rs/moka/0.12.15/moka/sync/struct.Cache.html — moka API verified 2026-05-25 via WebFetch

### Secondary (MEDIUM confidence)
- crates.io search via `cargo search moka` — confirms 0.12.15 as latest 2026-05-25

### Tertiary (LOW confidence)
- None — all major claims tied to source code or official docs.

## Metadata

**Confidence breakdown:**
- Architectural responsibility (engine owns cache, app.rs translates only): HIGH — verified from BL-01 / BL-02 patterns
- moka 0.12 API: HIGH — docs.rs verified + cargo search verified
- is_transient mapping: HIGH — `src/model.rs` enum read directly
- ratatui 0.30 yellow override: HIGH — existing code uses same pattern
- Slicing dependency graph: MEDIUM-HIGH — depends on planner choice of (a) vs (b) in Q3

**Research date:** 2026-05-25
**Valid until:** 2026-06-25 (30 days; moka stable, ratatui 0.30 stable, codebase frozen at master)

## RESEARCH COMPLETE
