---
phase: 03-gemini-conditional-cache-refresh-policy
plan: "02"
subsystem: engine
tags: [engine, cache, moka, RowOutcome, refresh_interval, TUI-03, ADP-05, D-66, D-67, D-71, D-72]

requires:
  - phase: 03-gemini-conditional-cache-refresh-policy
    plan: "01"
    provides: ProviderConfig.refresh_interval Option<u64> + DEFAULT_REFRESH_INTERVAL_SECS per provider
  - phase: 02-codex-output-formats
    provides: Stable CLI dispatch surface (run_compact / run_detailed / run_json) ready for RowOutcome translator
  - phase: 01-engine-claude-tui-scaffold
    provides: Engine + fanout::refresh_all_inner contract (Pitfall L4 panic recovery preserved); BL-01 clock-injection; BL-02 canonical row order
provides:
  - Engine::refresh_all return type widened to Vec<(ProviderId, RowOutcome)>
  - moka::sync::Cache<ProviderId, CacheEntry> owned by Engine (D-66 / D-67 — no TTL/TTI, manual stale semantics per D-71)
  - Engine::refresh_intervals: HashMap<ProviderId, Duration> populated from cfg + per-provider DEFAULT_REFRESH_INTERVAL_SECS with ≥5s clamp + tracing::warn! (D-72)
  - is_transient(&ProviderError) -> bool — closed-set transient classifier (Network + RateLimited only, Q2)
  - CacheEntry { state, fetched_at } + RowOutcome { Fresh, Stale, Failed } enum (cache.rs)
  - cli::outcome_to_result() pub(crate) translator — RowOutcome → Result<ProviderState, ProviderError> with unreachable!() on Stale (D-66 + D-73)
  - SCAFFOLD adapter in src/tui/mod.rs (2 sites, marked "SCAFFOLD: removed in Plan 03-03") so the full binary compiles until Plan 03-03 wires RowState::StaleOk
affects: [03-03-tui-stale-row, 03-04-gemini-stub-finalization, 03-05-integration-tests]

tech-stack:
  added:
    - "moka 0.12.15 (default-features = false, features = ['sync']) — in-memory stale-on-error cache (D-66). Verified provenance: crates.io repository = github.com/moka-rs/moka. No time_to_live / time_to_idle set — manual eviction per D-71 / Pitfall 1."
  patterns:
    - "Engine owns cache internally (Q4 internal-own, no injection point); tests construct via #[cfg(test)] pub(crate) Engine::new_for_test(providers, secrets, refresh_intervals) to plug in stateful ScriptedProvider helpers without going through Config"
    - "Q3 Option A pre-filter: Engine::refresh_all partitions providers into needs_fetch vs from_cache before calling fanout::refresh_all_inner; skip fan-out entirely when all providers are TTL-hit (avoids one await + preserves fanout purity)"
    - "RowOutcome is the engine boundary's verdict shape (Q5); AppState::apply_results (Plan 03-03) and CLI dispatch (this plan) both consume it via a thin translator — neither layer imports moka or computes stale-age math"
    - "BL-01 invariant preserved: cache write uses state.fetched_at (which itself comes from FetchCtx::now per Phase 1); no new jiff::Timestamp::now() callsite under src/provider/ or src/tui/widgets/"

key-files:
  created:
    - src/engine/cache.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - src/engine/mod.rs
    - src/cli/mod.rs
    - src/cli/render_json.rs
    - src/tui/mod.rs
    - tests/engine_row_order.rs

key-decisions:
  - "Engine owns moka::sync::Cache internally (Q4) — no injection point because the Plan 03-02 test surface is in-file via pub(crate) Engine::new_for_test, and a cache abstraction is explicitly Deferred to v2 (Q1 + Pitfall 9). This keeps Engine's public surface unchanged externally (Engine::new(cfg, secrets) is still the only public constructor)."
  - "Q3 Option A picked (pre-filter + skip fanout for TTL-hit providers) per CONTEXT D-72/D-73's strong implication that refresh_interval is a rate-limit cap, not just a 'what to return' filter. Skipping the fanout call when all providers are within TTL avoids waking up adapters unnecessarily — important for the future Gemini HTTP path (v2) and good hygiene now."
  - "cli::outcome_to_result() Stale arm uses unreachable!() (D-66 + D-73 binding). CLI cache is always empty because the CLI process is short-lived — a Stale verdict requires a previous successful fetch + a transient error within the same process, which CLI dispatch never observes. The unreachable!() is the correctness assertion + #[should_panic] test pins it."
  - "Task 2 GREEN commit bundled the cli/tui scaffold work (which the plan attributed to Task 3) because cargo build --lib is a Task 2 acceptance criterion AND the engine return-type change cascades structurally into cli/cli_render_json/tui callers. Task 3 commit then formally adds the outcome_to_result unit tests that explicitly belong to Task 3. The plan's task split is semantically faithful even if the file-touches landed in commit cb18343."
  - "moka cache built with Cache::builder().max_capacity(8).build() — no time_to_live / time_to_idle (D-66 / Pitfall 1). max_capacity=8 comfortably exceeds the 4-variant ProviderId enum (closed set today). LRU eviction at capacity is fine because we always re-insert on the next successful fetch; nothing would land in eviction territory during normal operation."
  - "SCAFFOLD adapter in src/tui/mod.rs uses literal 'SCAFFOLD: removed in Plan 03-03' marker comment at both call sites (priming fetch + fetch tick arm) so Plan 03-03 can grep-locate and remove. Pattern: RowOutcome::Fresh(state) | RowOutcome::Stale { state, .. } => Ok(state); RowOutcome::Failed(err) => Err(err). Until Plan 03-03 lands, TUI shows stale data as a normal Ok row (no visible regression because Phase 2 had no cache layer)."

patterns-established:
  - "Engine-layer verdict types live in their own module (src/engine/cache.rs) instead of being inlined in engine/mod.rs — keeps the orchestration layer focused on fan-out logic, lets the cache concerns evolve independently (e.g., a future v2 DiskCache won't churn engine/mod.rs)"
  - "Test affordances live next to production code under #[cfg(test)] pub(crate) attributes (e.g., Engine::new_for_test, Engine::refresh_interval_for) — same pattern as Phase 1's FetchCtx + the clock-injection seam. Keeps tests honest (no special-cased prod path) while not leaking test affordances into the public API"
  - "CLI dispatch translates engine-layer verdict types to the legacy shape via a single private helper (cli::outcome_to_result) immediately after engine.refresh_all(). The render layer + DispatchOutcome stay byte-identical to Phase 2 — no render_text.rs or render_json.rs surface changes (D-68 invariant)"

requirements-completed: [TUI-03]
# Note: PLAN.md frontmatter declared requirements: [TUI-03, ADP-05] but
# Plan 03-02 does not touch Gemini code — ADP-05 work belongs to Plan
# 03-04 (Gemini stub finalization). TUI-03 is delivered here via the
# engine-side TTL gating (refresh_interval honored in Engine::refresh_all);
# the user-visible TUI refresh-per-provider behavior fully lands when
# Plan 03-03 wires RowState::StaleOk and Plan 03-04 finalizes the stub.

metrics:
  duration: 11m
  started: 2026-05-25T11:18:49Z
  completed: 2026-05-25T11:30:00Z
  tasks_total: 3
  tasks_completed: 3
  files_modified: 8
---

# Phase 3 Plan 02: moka stale-on-error cache + Engine::refresh_all rewrite Summary

**Engine now owns a `moka::sync::Cache<ProviderId, CacheEntry>` and a per-provider `refresh_intervals: HashMap<ProviderId, Duration>`; `Engine::refresh_all` returns `Vec<(ProviderId, RowOutcome)>` where `RowOutcome` is `Fresh | Stale | Failed`. CLI dispatch translates RowOutcome → legacy `Result` (Stale unreachable per D-66); TUI uses a temporary SCAFFOLD adapter until Plan 03-03 wires `RowState::StaleOk`.**

## Performance

- **Duration:** 11m 11s (1779708600 - 1779707929)
- **Started:** 2026-05-25T11:18:49Z
- **Completed:** 2026-05-25T11:30:00Z
- **Tasks:** 3 (Task 1 + Task 2 RED/GREEN + Task 3)
- **Files modified:** 7 + 1 created (src/engine/cache.rs)

## Accomplishments

- **moka 0.12.15 in Cargo.lock** with `default-features = false, features = ["sync"]`. Phase 3 dep budget honored (one new crate added, justified by D-66).
- **src/engine/cache.rs (new module)** publishes `CacheEntry`, `RowOutcome`, and a `pub(crate) is_transient()` helper — closed-set classifier covering all 6 `ProviderError` variants (only `Network` + `RateLimited` are transient per Q2).
- **Engine struct gains 2 fields** (`cache`, `refresh_intervals`). `Engine::new` populates `refresh_intervals` per enabled provider, falling back to `crate::provider::<id>::DEFAULT_REFRESH_INTERVAL_SECS` (from Plan 01) and clamping `< 5s` to 5s with `tracing::warn!` (D-72).
- **`Engine::refresh_all` rewritten** to Q3 Option A: pre-filter providers into needs_fetch vs from_cache, call `fanout::refresh_all_inner` only on the elapsed-TTL subset, map fanout results to `RowOutcome` (Fresh on Ok + cache.insert; Stale on transient Err + cache hit; Failed otherwise). Pitfall 16 honored — all-cache pass still emits one row per provider, not empty Vec. Sort by canonical `ProviderId` order preserved (BL-02).
- **CLI dispatch (run_compact / run_detailed / run_json)** translates `RowOutcome` → `Result<ProviderState, ProviderError>` via the `cli::outcome_to_result()` helper immediately after `engine.refresh_all()`. Stale arm is `unreachable!()` (D-66 + D-73 binding). render_text.rs and render_json.rs unchanged — JSON `schema_version: 1` preserved (D-68); compact / detailed output byte-identical to Phase 2.
- **TUI SCAFFOLD adapter** at both `engine.refresh_all` call sites in `src/tui/mod.rs` (priming fetch + fetch-tick arm). Marked with literal `// SCAFFOLD: removed in Plan 03-03` so Plan 03-03 can grep-locate. Pattern: `Fresh | Stale → Ok(state); Failed → Err(err)`. Until Plan 03-03 lands, stale data shows as a normal Ok row (no visible regression vs Phase 2).
- **All Phase 2 invariants preserved.** Full test suite green: 174 lib tests + 16 integration test binaries. cli_walking_skeleton + exit_codes + json_format_round_trip pass byte-identical to Phase 2. D-74 (TUI 15s tick) untouched. BL-01 (no new `Timestamp::now()` under `src/provider/` or `src/tui/widgets/`) verified.

## Task Commits

Three atomic commits:

1. **Task 1: moka dep + engine::cache module** — `3f338d4` (feat)
   - Cargo.toml + Cargo.lock + src/engine/cache.rs + src/engine/mod.rs
   - 6 is_transient unit tests pass.
2. **Task 2 RED: failing tests for engine cache + TTL + RowOutcome** — `c2d89f9` (test)
   - src/engine/mod.rs gains 5 new behavioral tests + ScriptedProvider helper.
   - Confirmed RED (tests fail to compile — `Engine::new_for_test` / `Engine::refresh_interval_for` undefined; existing tests reference `Result::is_ok` on what becomes `RowOutcome`).
3. **Task 2 GREEN: Engine rewrite + cli/tui scaffold** — `cb18343` (feat)
   - src/engine/mod.rs (Engine struct + new + refresh_all + helpers)
   - src/cli/mod.rs + src/cli/render_json.rs (RowOutcome → Result translator)
   - src/tui/mod.rs (SCAFFOLD adapter at 2 sites)
   - tests/engine_row_order.rs (assertion updated to RowOutcome::Fresh)
4. **Task 3: outcome_to_result unit tests** — `0732130` (test)
   - src/cli/mod.rs gains 3 new unit tests covering Fresh / Failed / Stale arms.

## Files Created/Modified

### Created

- **src/engine/cache.rs** — `CacheEntry { state, fetched_at }`, `RowOutcome { Fresh, Stale, Failed }`, `pub(crate) is_transient(&ProviderError) -> bool`. 6 unit tests covering the closed-set transient mapping (Network + RateLimited true; Unavailable + SchemaDrift + Internal + Unconfigured false).

### Modified

- **Cargo.toml** — Added `moka = { version = "0.12", default-features = false, features = ["sync"] }` with a Phase 3 comment block citing D-66 + D-71.
- **Cargo.lock** — moka 0.12.15 + transitive deps (crossbeam-channel, crossbeam-epoch, crossbeam-utils, tagptr) added.
- **src/engine/mod.rs** — Engine struct gains `cache: Cache<ProviderId, CacheEntry>` + `refresh_intervals: HashMap<ProviderId, Duration>`. Engine::new populates both (refresh_intervals via new `Self::resolve_interval(id_str, &ProviderConfig, default_secs)` helper that handles the ≥5s clamp + tracing::warn!). Engine::refresh_all rewritten with Q3 Option A pre-filter (partition into from_cache + needs_fetch; skip fanout when needs_fetch is empty). Added `duration_since(now, earlier) -> Duration` helper (clamp-to-zero pattern matching `format_countdown`). Added `#[cfg(test)] pub(crate)` test affordances `new_for_test` + `refresh_interval_for`. 5 new behavioral tests + ScriptedProvider helper in `#[cfg(test)] mod tests`.
- **src/cli/mod.rs** — Imports `RowOutcome` + `ProviderError` + `ProviderState`. Added `pub(crate) fn outcome_to_result(outcome: RowOutcome) -> Result<ProviderState, ProviderError>` (Stale arm = `unreachable!()` with explicit D-66 + D-73 message). run_compact + run_detailed each insert the translator immediately after `engine.refresh_all()`. 3 new unit tests covering Fresh / Failed / Stale arms (Stale via `#[should_panic]`).
- **src/cli/render_json.rs** — Imports `ProviderError` + `ProviderId` + `ProviderState`. run_json inserts the `crate::cli::outcome_to_result` translator immediately after `engine.refresh_all()`. to_json_root + DispatchOutcome::from_results stay byte-identical to Phase 2.
- **src/tui/mod.rs** — Imports `RowOutcome` + `ProviderError` + `ProviderId` + `ProviderState`. SCAFFOLD adapter (marked with literal `// SCAFFOLD: removed in Plan 03-03`) at both call sites: priming fetch (~line 113) and fetch tick arm (~line 153). Both sites collapse Fresh | Stale → Ok(state); Failed → Err(err) so AppState::apply_results (Phase 1/2 signature) keeps compiling.
- **tests/engine_row_order.rs** — Imports `ahb::engine::cache::RowOutcome`. Assertion updated: `results[0].1.is_ok()` → `matches!(results[0].1, RowOutcome::Fresh(_))` (BL-02 invariant preserved through the new shape).

## Decisions Made

- **Engine owns cache internally (Q4)** — No injection point. Tests use `pub(crate) #[cfg(test)] Engine::new_for_test(providers, secrets, refresh_intervals)` to plug in stateful providers without going through Config. This matches the Phase 1 `FetchCtx::now` clock-injection pattern (test affordance lives next to prod code, not in a separate trait abstraction). Cache trait abstraction is explicitly Deferred to v2.
- **Q3 Option A picked** (pre-filter + skip fanout for TTL-hit providers) — CONTEXT D-72/D-73 strongly imply this. Skipping the fan-out call when needs_fetch is empty avoids waking up `JoinSet::spawn` + a `join_next` loop when nothing needs to happen, which is good hygiene and important for the v2 Gemini HTTP path. Pitfall 16 honored: even when all providers are within TTL, we still emit one row per provider (the from_cache accumulator does this), not an empty Vec.
- **`cli::outcome_to_result` Stale arm = `unreachable!()`** with explicit `#[allow(clippy::unreachable)]` (clippy::unreachable is pedantic, the lib.rs deny only covers panic/unwrap/expect). The `#[should_panic]` test pins the invariant: if a future refactor wires CLI through a persistent engine instance, the panic will fire and force the refactor to address the cache-state divergence.
- **No moka TTL / TTI** — `Cache::builder().max_capacity(8).build()` only. D-66 / Pitfall 1: manual stale semantics require moka to never silently evict an entry the error path wants to surface as `RowOutcome::Stale`. max_capacity=8 comfortably exceeds the ProviderId closed set (4 variants).
- **Cache write uses `state.fetched_at`** (not a fresh `jiff::Timestamp::now()` call) — BL-01 / Q8 binding. `state.fetched_at` already comes from `FetchCtx::now` (set in `fanout::refresh_all_inner`), so the cache's `fetched_at` is the same wall-clock snapshot the adapter saw. No new `Timestamp::now()` callsite added under `src/provider/` or `src/tui/widgets/`.
- **Task 2 GREEN bundles cli + tui scaffold** — Necessary because `cargo build --lib` is a Task 2 acceptance criterion AND the engine return-type change cascades structurally into cli/render_json/tui callers. Task 3 commit then formally adds the `outcome_to_result` unit-test coverage. The semantic split between Task 2 (engine) and Task 3 (cli/tui translator) is preserved even though the file touches landed in commit cb18343.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] cargo build --lib requires cli + tui changes alongside Engine return-type change**

- **Found during:** Task 2 GREEN (cargo build --lib initially failed with 10+ type-mismatch errors in src/cli/mod.rs, src/cli/render_json.rs, src/tui/mod.rs because `Engine::refresh_all` return type changed from `Vec<(ProviderId, Result<...>)>` to `Vec<(ProviderId, RowOutcome)>`).
- **Issue:** Rust's type system propagates the return-type change to every callsite; src/cli/mod.rs (run_compact, run_detailed), src/cli/render_json.rs (run_json + to_json_root), and src/tui/mod.rs (priming + fetch tick) all consume the engine's return type. Without simultaneous updates, the lib does NOT compile, which fails Task 2's `cargo build --lib exits 0` acceptance criterion.
- **Fix:** Bundled the cli + tui scaffold work into the Task 2 GREEN commit (cb18343). The work is semantically Task 3's prescription (the `outcome_to_result` translator helper + SCAFFOLD adapter in tui/mod.rs) — both tasks landed in the same commit. Task 3 commit (0732130) then formally adds the `outcome_to_result` unit tests that explicitly belong to Task 3's gate.
- **Files modified:** src/cli/mod.rs, src/cli/render_json.rs, src/tui/mod.rs, tests/engine_row_order.rs (the integration test asserting BL-02 row order also had to update its `Result::is_ok()` assertion to `matches!(_, RowOutcome::Fresh(_))`).
- **Verification:** Full `cargo test` suite green (174 lib + integration). All Task 2 + Task 3 acceptance criteria met.
- **Committed in:** `cb18343` (Task 2 GREEN — engine + cascade fixes) + `0732130` (Task 3 — outcome_to_result unit tests).

**2. [Rule 3 - Blocking] tests/engine_row_order.rs broke on the new RowOutcome shape**

- **Found during:** Task 2 GREEN, full `cargo test` run.
- **Issue:** The integration test `engine_refresh_all_returns_canonical_order_with_claude_and_mock_enabled` asserts BL-02 (canonical row order) and checks both rows are `Result::is_ok()`. With the new `Vec<(ProviderId, RowOutcome)>` shape, `is_ok()` is no longer a method on the value — compile error.
- **Fix:** Replaced `results[0].1.is_ok()` with `matches!(results[0].1, RowOutcome::Fresh(_))`. BL-02 invariant (canonical row order) is preserved verbatim; only the success-discriminant predicate changed shape.
- **Files modified:** tests/engine_row_order.rs.
- **Verification:** `cargo test --test engine_row_order` green.
- **Committed in:** `cb18343` (Task 2 GREEN — bundled with the lib changes).

---

**Total deviations:** 2 auto-fixed (both blocking, both Rule 3 — knock-on edits forced by the return-type widening).
**Impact on plan:** No scope creep. Both deviations are mechanical type-propagation fixes that the plan implicitly required (since Task 2's `cargo build --lib` acceptance criterion cannot be met without them). Task semantics preserved.

## Issues Encountered

None. The plan was very precisely scoped; the only friction was the predictable Rule 3 deviation above, where the return-type widening cascaded into cli/tui/integration-test callers. Resolution was straightforward (each callsite gained a thin translator).

## TDD Gate Compliance

Task 2 followed RED → GREEN per `tdd="true"`:

- **RED gate (commit `c2d89f9`):** `test(03-02): add failing tests for engine cache + TTL + RowOutcome (RED)` — confirmed RED via `cargo build --lib --tests` showing 14 compile errors (E0599 method not found, E0282 type annotations needed, E0425 unresolved name). The compile-time failure is the strongest form of RED — the test names exist but the engine API they reference doesn't.
- **GREEN gate (commit `cb18343`):** `feat(03-02): rewrite Engine with cache + TTL gating + RowOutcome (GREEN)` — confirmed GREEN via `cargo test --lib engine` showing 22 tests pass (including the 5 new behavioral tests).
- **REFACTOR gate:** Not needed; Engine::refresh_all implementation was already minimal (single-pass partition + single fanout call + single map+sort).

Task 1 (moka dep + cache module) used standard `feat` commit per the plan's task structure (no `tdd="true"` attribute). Task 3 (outcome_to_result tests) used `test` commit type — these are additive tests pinning an existing behavior, not a TDD cycle.

## Plan-Level Verification Results

All 11 verification steps from PLAN.md `<verification>` passed:

1. ✅ `cargo test --lib engine` — 22 tests green (Task 2 behavioral + Task 1 is_transient)
2. ✅ `cargo test --test refresh_interval_config_parse` — 5/5 pass (Plan 01 invariant preserved)
3. ✅ `cargo build` (full binary) — clean, no warnings
4. ✅ `cargo test --test cli_walking_skeleton` — 4/4 pass (CLI output byte-identical to Phase 2)
5. ✅ `cargo test --test exit_codes` — 7/7 pass (Gemini stub still exit 1 in Gemini-only config)
6. ✅ `cargo test --test json_format_round_trip` — 5/5 pass (schema_version: 1 unchanged per D-68)
7. ✅ `grep -c "tokio::time::interval(Duration::from_secs(15))" src/tui/mod.rs` → `1` (D-74 unchanged)
8. ✅ `grep -rn "Timestamp::now()" src/provider/ src/tui/widgets/` → only comments, no actual calls (BL-01)
9. ✅ `grep -rn "time_to_live\|time_to_idle" src/engine/` → only comments mentioning the rule, no code (D-66 / Pitfall 1)
10. ✅ `grep -F "SCAFFOLD: removed in Plan 03-03" src/tui/mod.rs` → 2 matches (priming + fetch tick)
11. ✅ `grep -F "schema_version.*2" tests/json_format_round_trip.rs` → 0 matches (D-68 negative)

## User Setup Required

None — this plan is pure code changes; no external service configuration, env vars, or manual steps.

## Next Phase Readiness

- **Plan 03-03 (TUI stale row + RowState::StaleOk)** can now consume `Vec<(ProviderId, RowOutcome)>` directly:
  - The SCAFFOLD adapter in src/tui/mod.rs is grep-locatable via `SCAFFOLD: removed in Plan 03-03` (2 sites).
  - Plan 03-03 Task 2 should: (a) widen `AppState::apply_results` signature to accept `Vec<(ProviderId, RowOutcome)>`, (b) add `RowState::StaleOk { state, stale_age_secs }` variant per D-70, (c) delete the SCAFFOLD adapter blocks in tui/mod.rs at both call sites, (d) wire `build_stale_ok_line` in `src/tui/widgets/hp_row.rs` per D-69.
- **Plan 03-04 (Gemini stub finalization)** is unaffected by this plan — it can run in parallel.
- **Plan 03-05 (integration tests with IntermittentFailureProvider)** can build on the `ScriptedProvider` test helper added in this plan's `#[cfg(test)] mod tests` block of src/engine/mod.rs (extract to `tests/common/` if needed).

## Self-Check: PASSED

- ✅ `src/engine/cache.rs` created (verified: file exists, defines `CacheEntry`, `RowOutcome`, `is_transient`)
- ✅ `Cargo.toml` modified (verified: `moka = { version = "0.12", default-features = false, features = ["sync"] }` line present)
- ✅ `Cargo.lock` includes moka (verified: `grep -F "moka" Cargo.lock` returns matches)
- ✅ `src/engine/mod.rs` modified (verified: contains `cache: Cache<ProviderId, CacheEntry>` + `refresh_intervals: HashMap<ProviderId, Duration>` fields + `Engine::new_for_test` + `Engine::refresh_interval_for` + new `refresh_all` body)
- ✅ `src/cli/mod.rs` modified (verified: contains `pub(crate) fn outcome_to_result` + 3 new unit tests)
- ✅ `src/cli/render_json.rs` modified (verified: contains `crate::cli::outcome_to_result(o)` call in run_json)
- ✅ `src/tui/mod.rs` modified (verified: 2 `SCAFFOLD: removed in Plan 03-03` matches)
- ✅ `tests/engine_row_order.rs` modified (verified: `RowOutcome::Fresh(_)` match in assertions)
- ✅ Commit `3f338d4` (Task 1) — verified in `git log`
- ✅ Commit `c2d89f9` (Task 2 RED) — verified in `git log`
- ✅ Commit `cb18343` (Task 2 GREEN) — verified in `git log`
- ✅ Commit `0732130` (Task 3) — verified in `git log`

---
*Phase: 03-gemini-conditional-cache-refresh-policy*
*Completed: 2026-05-25*
