---
phase: 03-gemini-conditional-cache-refresh-policy
plan: "05"
subsystem: testing
tags: [engine, cache, RowOutcome, ScriptedProvider, integration-tests, BL-01, TUI-03, ADP-05, D-71, SC-3]

requires:
  - phase: 03-gemini-conditional-cache-refresh-policy
    plan: "03"
    provides: Engine::refresh_all returns Vec<(ProviderId, RowOutcome)>; RowState::StaleOk wired in TUI
  - phase: 03-gemini-conditional-cache-refresh-policy
    plan: "02"
    provides: Engine owns moka cache + refresh_intervals + is_transient; CacheEntry + RowOutcome types
  - phase: 01-engine-claude-tui-scaffold
    provides: BL-01 clock-injection contract; tests/no_walltime_in_adapter.rs scan infrastructure
provides:
  - Engine::new_with_providers(providers, cfg, secrets) — #[doc(hidden)] pub fn test constructor reachable from integration test crates
  - tests/cache_stale_on_error.rs — 5 integration tests with ScriptedProvider locking the D-71 three-state timeline + SC-3 multi-provider cadence + non-transient bypass + no-cache-fail + TTL-hit-no-fetch
  - tests/no_walltime_in_adapter.rs extended scan covers src/engine/ (BL-01 guardrail)
affects: [04-distribution-release]

tech-stack:
  added: []
  patterns:
    - "Integration-test access to test affordances via #[doc(hidden)] pub fn (canonical Rust idiom). #[cfg(test)] gates a constructor out of integration test crates because Rust's cfg(test) is per-crate-build (only flips on for the crate currently being built as a --test target — integration tests link the lib without --cfg test). #[doc(hidden)] + pub is the equivalent for cross-crate test helpers."
    - "ScriptedProvider pattern with Vec<Result<(), ProviderError>> script + AtomicU64 call_count for behavioral testing of cache + TTL gating. Ok(()) means 'build state from ctx.now' so the cache's fetched_at remains in the controlled test clock domain (BL-01 invariant honored end-to-end)."
    - "Per-Engine refresh_intervals derived from Config — test constructor walks the providers vec and consults cfg.providers.<id> per provider id, threading through the existing Self::resolve_interval helper so the ≥5s clamp + tracing::warn! (D-72) fires identically in both production and test paths."
    - "no_walltime_in_adapter scan_dirs widening pattern (Plan 04 → Plan 03-05): the BL-01 guardrail scan list grows phase-by-phase as new wall-clock-sensitive subtrees come online. Each addition documents WHY it was added (Plan 04: TUI widgets; Plan 03-05: engine cache write site)."

key-files:
  created:
    - tests/cache_stale_on_error.rs
  modified:
    - src/engine/mod.rs
    - tests/no_walltime_in_adapter.rs

key-decisions:
  - "#[doc(hidden)] pub fn over #[cfg(test)] pub fn for new_with_providers — plan literal called for #[cfg(test)] but that gate doesn't reach integration test crates. #[doc(hidden)] keeps the constructor off the public API surface while still being accessible from tests/. Rule 3 deviation documented inline."
  - "ScriptedProvider uses Result<(), ProviderError> rather than Result<ProviderState, ProviderError> as the script item — Ok(()) means 'succeed with synthetic state built from ctx.now'. This keeps fetched_at synchronized with the test's now clock so cache TTL math is fully controllable (BL-01 invariant)."
  - "SC-3 multi-provider test uses ProviderId::Codex as provider_b (TTL=600s, slow-changing) and ProviderId::Mock as provider_a (TTL=5s, fast-changing) per plan's explicit choice. Codex represents a 'local SQLite + JSONL' provider that doesn't need frequent polling; Mock is the canonical fault-injection / test provider. No conditional logic in test setup."
  - "Test 1 stale_age_secs assertion uses range [5, 8] (allowing ±3s slack around the 6s clock advance) rather than exact-equals to remain robust against sub-second math edge cases in jiff::Timestamp::since (the test's t1 = t0 + 6s exactly, so 6 is the canonical answer — the [5,8] range is purely defense in depth)."

patterns-established:
  - "Test constructor signature mirror: Engine::new_with_providers(providers, cfg, secrets) mirrors Engine::new(cfg, secrets) but takes providers as the first parameter — this matches the production signature's parameter order (cfg/secrets stay last) and makes the test seam visually distinct from the production constructor (different first parameter)."
  - "Per-provider config lookup in test constructor: walks providers.iter().map(p.id()) and threads each id through a match block to find the matching cfg.providers.<id> reference (claude/codex/gemini/mock). This avoids forcing tests to pre-build a HashMap<ProviderId, Duration> by hand — they declare cfg the same way production does and the constructor resolves the intervals via the shared resolve_interval helper."

requirements: [TUI-03, ADP-05]
requirements-completed: [TUI-03, ADP-05]
# TUI-03 completion was substantively delivered by Plan 03-02 (engine TTL gating) +
# Plan 03-03 (TUI stale row rendering); Plan 03-05 closes the loop with
# end-to-end engine-layer integration coverage of SC-3 + SC-4 timelines.
# ADP-05 (Gemini stub) was substantively delivered by Plan 03-04; Plan 03-05's
# engine-cache scope doesn't touch the stub but the plan was frontmattered with
# both requirement IDs as the verification closer.

metrics:
  duration: 10m
  started: 2026-05-25T11:55:00Z
  completed: 2026-05-25T12:05:00Z
  tasks_total: 2
  tasks_completed: 2
  files_modified: 3
---

# Phase 3 Plan 05: cache + stale-on-error integration tests + BL-01 engine-scope extension Summary

**Five integration tests under `tests/cache_stale_on_error.rs` lock the D-71 three-state timeline (Fresh → Stale → Fresh) + SC-3 multi-provider polling cadence (Mock TTL=5s re-fetched, Codex TTL=600s served from cache) at the engine layer via a `#[doc(hidden)] pub` `Engine::new_with_providers` constructor; `tests/no_walltime_in_adapter.rs` widens its BL-01 grep scan to include `src/engine/` defensively.**

## Performance

- **Duration:** ~10m (Task 1 RED → Task 1 GREEN → Task 2)
- **Tasks:** 2 (Task 1 followed TDD RED → GREEN; Task 2 was additive)
- **Commits:** 3 (RED + GREEN + Task 2)
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments

- **`tests/cache_stale_on_error.rs` (new file, 5 integration tests)** drives `Engine::refresh_all` end-to-end via a `ScriptedProvider` that returns a programmed `Vec<Result<(), ProviderError>>` sequence:
  - `engine_fresh_stale_fresh_three_tick_sequence` — D-71 three-state timeline: Tick 1 Ok → Fresh; Tick 2 (+6s, TTL=5s elapsed) Network → Stale with `stale_age_secs ∈ [5,8]`; Tick 3 (+12s) Ok → Fresh (cache updated, `state.fetched_at == t2`).
  - `engine_non_transient_error_does_not_stale_despite_cache` — Q2 binding: SchemaDrift after TTL elapses MUST produce `RowOutcome::Failed`, NOT Stale, even with a valid cache hit. Verifies `is_transient` closed-set classifier reaches `Engine::refresh_all` correctly.
  - `engine_no_cache_with_transient_error_produces_failed` — first-ever call returns Network with no prior cache → Failed (Stale requires a prior successful fetch to fall back on).
  - `engine_cache_hit_within_ttl_returns_fresh_without_fetch` — two rapid calls at the same `now` produce `call_count == 1` (second call served from cache; provider.fetch never reached the second script entry). TTL gate works.
  - `engine_multi_provider_different_intervals_only_stale_provider_fetched` — SC-3 cadence: Mock with TTL=5s and Codex with TTL=600s, both fetched at `t0` (no cache yet, call_count_a=1, call_count_b=1). At `t0 + 6s`: Mock re-fetched (call_count_a=2); Codex served from cache (call_count_b=1). Both rows Fresh. Canonical BL-02 row order preserved (Codex=1 before Mock=3).
- **`Engine::new_with_providers(providers, cfg, secrets)`** added to `src/engine/mod.rs` as a `#[doc(hidden)] pub fn` — the cross-crate test seam. Honors per-provider `refresh_interval` from `cfg.providers.<id>` via the existing `Self::resolve_interval` helper, so the ≥5s clamp + `tracing::warn!` (D-72) fires identically in both production and test paths. Different from the pre-existing `Engine::new_for_test` (which is `pub(crate)` and accepts a pre-built `HashMap<ProviderId, Duration>`): the new constructor takes the production-shaped `Config` so tests configure refresh intervals the same way users do.
- **`tests/no_walltime_in_adapter.rs` scan widened** from `[src/provider, src/tui/widgets]` to `[src/provider, src/tui/widgets, src/engine]`. BL-01 guardrail now covers the cache write site introduced by Plan 03-02. Doc comment + assertion message document the new scope per 03-RESEARCH.md § Q8.
- **Full `cargo test` regression suite green**: 185 lib tests + 16 integration test binaries + 5 new tests in `cache_stale_on_error.rs`. Zero regressions. `cargo build` clean.

## Task Commits

Three atomic commits:

1. **Task 1 RED: failing integration tests** — `6132415` (test)
   - `tests/cache_stale_on_error.rs` created with 5 tests + `ScriptedProvider` helper.
   - Confirmed RED via `cargo build --tests --test cache_stale_on_error` → 9 compile errors (E0599 `new_with_providers` not found + cascading E0282/E0308).
2. **Task 1 GREEN: Engine::new_with_providers** — `1080a4b` (feat)
   - `src/engine/mod.rs` gains `#[doc(hidden)] pub fn new_with_providers(providers, cfg, secrets)` that walks the provider list and resolves per-provider refresh intervals from `cfg.providers.<id>` via `Self::resolve_interval`.
   - All 5 integration tests pass.
3. **Task 2: BL-01 scan extension** — `64aa5dd` (test)
   - `tests/no_walltime_in_adapter.rs` widens `scan_dirs` to include `src/engine`.
   - Doc comment + assertion message updated.
   - Full `cargo test` suite green; zero regressions.

## Files Created/Modified

### Created

- **`tests/cache_stale_on_error.rs`** — 5 integration tests + `ScriptedProvider` helper + `synthetic_state` builder. Drives `Engine::refresh_all` against deterministic scripted sequences to lock the D-71 timeline + SC-3 cadence + non-transient bypass + no-cache failure + TTL-hit-no-fetch invariants.

### Modified

- **`src/engine/mod.rs`** — Adds `#[doc(hidden)] pub fn new_with_providers(providers, cfg, secrets)` (65 new lines incl. inline doc comment). Walks `providers.iter()`, matches each `p.id()` against the `ProviderId` enum, looks up the matching `cfg.providers.<id>` reference, and threads everything through `Self::resolve_interval(id_str, &ProviderConfig, default_secs)`. Inserts the resolved `Duration` into the `refresh_intervals: HashMap<ProviderId, Duration>` map. Builds the moka cache identically to `Engine::new` (`Cache::builder().max_capacity(8).build()` — no TTL/TTI per D-66 / Pitfall 1). Production code path (`Engine::new`) untouched.
- **`tests/no_walltime_in_adapter.rs`** — `scan_dirs` array widened from `[src/provider, src/tui/widgets]` to `[src/provider, src/tui/widgets, src/engine]`. Doc comment + assertion message + inline scan-loop comment document why `src/engine` was added (Plan 03-02 cache write site; 03-RESEARCH.md § Q8 binding).

## Decisions Made

- **`#[doc(hidden)] pub fn` instead of `#[cfg(test)] pub fn`** for `new_with_providers` — Plan literal said `#[cfg(test)] pub fn`, but Rust's `#[cfg(test)]` flag is per-crate-build: it flips on only for the crate currently being built as a `--test` target. When `cargo test` builds the lib for an integration test crate, the lib does NOT see `--cfg test`. So `#[cfg(test)] pub fn` would gate the constructor out of `tests/cache_stale_on_error.rs` entirely. `#[doc(hidden)] pub fn` is the canonical Rust idiom for "test helper that must cross the crate boundary but is not part of the public contract." Inline doc comment explains the gate choice + Rust language rule.
- **`ScriptedProvider` script shape: `Vec<Result<(), ProviderError>>`** — not `Vec<Result<ProviderState, ProviderError>>`. The `Ok(())` variant signals "succeed with synthetic state built from `ctx.now`", so the cache's `fetched_at` always tracks the test's controlled clock (`t0`, `t0+6s`, etc.). The alternative (Ok-with-state) would force tests to pre-compute `ProviderState` snapshots before the test knows what `ctx.now` will be, which breaks the clock-injection contract. Errors come from the script verbatim — they don't interact with `ctx.now`.
- **`stale_age_secs ∈ [5, 8]` range assertion in Test 1** — `t1 = t0 + 6s` exactly, so the canonical answer is 6. The `[5, 8]` range is defense in depth against any sub-second math edge case in `jiff::Timestamp::since` (e.g., if `Timestamp` arithmetic ever introduces nanosecond drift). The lower bound (5) matches the TTL boundary; the upper bound (8) catches up-to-2s of drift before flagging a real bug.
- **SC-3 test uses canonical `ProviderId::Codex` (TTL=600s) and `ProviderId::Mock` (TTL=5s)** — plan-mandated provider ids. No conditional logic in test setup: both providers always succeed in their scripts; the assertion delta is purely the `call_count` difference (Mock=2, Codex=1 after the 2nd refresh). Engine BL-02 sort lands them in canonical order (Codex=1 before Mock=3) on both calls.
- **Task 2 (BL-01 scan extension) ran AFTER Task 1 (RED+GREEN bundle), not as part of Task 1** — Task 2's scan widening is independent of the cache test infrastructure and doesn't share a TDD cycle. Atomic commit per task: Task 2 = single `test(03-05)` commit; Task 1 split into RED + GREEN per `tdd="true"` attribute.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `#[cfg(test)] pub fn new_with_providers` would gate the constructor out of integration test crates**

- **Found during:** Task 1 GREEN (`cargo test --test cache_stale_on_error` still failed with `E0599 no function or associated item named 'new_with_providers' found for struct 'Engine'` after the initial `#[cfg(test)] pub fn` implementation landed).
- **Issue:** Rust's `#[cfg(test)]` flag flips on per-crate-build, only for the crate currently being built as a `--test` target. `cargo test` builds the integration test crate (`tests/cache_stale_on_error.rs`) as its own `--test` target, but the `ahb` lib it links is built **without** `--cfg test`. So `#[cfg(test)] pub fn new_with_providers` was invisible from inside the integration test crate.
- **Fix:** Replaced `#[cfg(test)]` with `#[doc(hidden)]` (canonical Rust idiom for cross-crate test helpers). `pub fn` makes it reachable from integration tests; `#[doc(hidden)]` hides it from `cargo doc` output so it doesn't pollute the public API surface. Documented inline with a paragraph explaining the language rule.
- **Files modified:** `src/engine/mod.rs` (same file as the original GREEN edit — the gate change landed in the same commit because the GREEN test run wouldn't pass without it).
- **Verification:** `cargo test --test cache_stale_on_error` → 5/5 tests pass.
- **Committed in:** `1080a4b` (Task 1 GREEN — gate change bundled in).

---

**Total deviations:** 1 auto-fixed (Rule 3 — Rust language-rule blocker).
**Impact on plan:** No scope creep. The deviation is purely a gate-attribute choice (`#[cfg(test)]` → `#[doc(hidden)]`); the function body, signature, and behavior are identical to the plan's prescription. Inline documentation captures the language rule so future agents don't re-narrow back to `#[cfg(test)]`.

## Issues Encountered

None beyond the Rule 3 deviation above. The plan was very precisely scoped (1 test file + 1 constructor + 1 scan-dir extension), and the only friction was the `#[cfg(test)]`-vs-`#[doc(hidden)]` gate distinction, which was resolved immediately when the GREEN test run flagged the missing symbol.

## TDD Gate Compliance

Task 1 followed RED → GREEN per `tdd="true"`:

- **RED gate (commit `6132415`):** `test(03-05): add failing integration tests for engine cache + stale-on-error (RED)` — confirmed RED via `cargo build --tests --test cache_stale_on_error` showing 9 compile errors (E0599 `new_with_providers` not found + cascading E0282/E0308 on the result-type inference). The compile-time failure is the strongest form of RED.
- **GREEN gate (commit `1080a4b`):** `feat(03-05): add Engine::new_with_providers test constructor (GREEN)` — confirmed GREEN via `cargo test --test cache_stale_on_error` → `5 passed; 0 failed`.
- **REFACTOR gate:** Not needed. The `new_with_providers` implementation is minimal (a single match over the closed `ProviderId` set + a `for` loop populating one HashMap entry per provider) and shares all production logic via `Self::resolve_interval`.

Task 2 (`test(03-05)` commit `64aa5dd`) is not a TDD cycle — it's a defensive extension of an existing grep guardrail. The pattern matches Plan 03-03 Task 2 (SCAFFOLD removal also used `test` commit type for a non-TDD additive change).

## Plan-Level Verification Results

All 7 verification steps from PLAN.md `<verification>` passed:

1. ✅ `cargo test cache_stale_on_error` passes (5 tests: 3-tick sequence, non-transient bypass, no-cache fail, TTL-hit no-fetch, multi-provider cadence)
2. ✅ `cargo test no_walltime_in_adapter` passes with `src/engine/` in scan scope
3. ✅ `cargo test` full suite passes (185 lib + 16 integration binaries + 5 new = zero regressions across all Phase 0/1/2/3 tests)
4. ✅ `grep -rn "Timestamp::now()" src/engine/` returns 3 matches, ALL in comment / doc lines (the BL-01 scan in `tests/no_walltime_in_adapter.rs` filters comments and returns 0 actual code matches under `src/engine/`)
5. ✅ Phase 3 SC-3 verified — `engine_multi_provider_different_intervals_only_stale_provider_fetched` proves `ProviderId::Codex` with TTL=600s is NOT called on the second tick at t+6s while `ProviderId::Mock` with TTL=5s IS called.
6. ✅ Phase 3 SC-4 verified — `engine_fresh_stale_fresh_three_tick_sequence` produces `RowOutcome::Stale` during the transient Network error at tick 2 with the cache hit from tick 1 (stale fallback verified at the engine layer; Plan 03-03 already verified the TUI rendering side).
7. ✅ D-71 three-state timeline verified programmatically across 3 ticks via the same test (Fresh at tick 1, Stale at tick 2, Fresh-with-cache-updated at tick 3).

Acceptance-criteria spot checks:

- ✅ `grep -F "new_with_providers" src/engine/mod.rs` → 1 match (the `pub fn` declaration itself).
- ✅ `grep -F "ProviderId::Codex" tests/cache_stale_on_error.rs` → 3 matches (SC-3 setup + per-id assertion lookups).
- ✅ `grep -F "ProviderId::Mock" tests/cache_stale_on_error.rs` → 3 matches (SC-3 setup + assertions).
- ✅ `grep -F "src/engine" tests/no_walltime_in_adapter.rs` → 4 matches (scan_dirs entry + doc comment + inline-loop comment + module-docstring extension).

## User Setup Required

None — this plan is pure test infrastructure + a test affordance constructor; no external service configuration, env vars, or manual steps.

## Next Phase Readiness

**Phase 3 is now complete** (5/5 plans landed):

- Plan 03-01: `refresh_interval` config field + per-provider defaults
- Plan 03-02: moka cache + `RowOutcome` + `is_transient` + `Engine::refresh_all` rewrite
- Plan 03-03: `RowState::StaleOk` + `build_stale_ok_line` Yellow override + SCAFFOLD removed
- Plan 03-04: Gemini stub error reason finalized (D-65) + README §Gemini status section + default-config comment
- Plan 03-05 (this plan): end-to-end engine-layer integration coverage of D-71 + SC-3 + BL-01 engine-scope extension

**Phase 4 readiness:**

- `cargo build --release` still produces a clean single binary (verified).
- All 185 lib tests + 16 integration binaries green — zero regressions accumulated across Phase 3.
- The `#[doc(hidden)] pub fn new_with_providers` constructor pattern is in place for any future integration tests Phase 4 might add (cargo-dist installer smoke tests, etc.). It does NOT appear in `cargo doc` output.
- BL-01 grep guardrail now covers `src/provider/`, `src/tui/widgets/`, AND `src/engine/` — wall-clock injection contract is fully fenced for all clock-sensitive subtrees.

## Self-Check: PASSED

- ✅ `tests/cache_stale_on_error.rs` created (verified: file exists, contains `ScriptedProvider` + 5 `#[tokio::test]` functions)
- ✅ `src/engine/mod.rs` modified (verified: `grep -F "new_with_providers" src/engine/mod.rs` returns 1 match; `#[doc(hidden)]` attribute present)
- ✅ `tests/no_walltime_in_adapter.rs` modified (verified: `grep -F "src/engine" tests/no_walltime_in_adapter.rs` returns 4 matches; `scan_dirs: [&str; 3]` array contains `"src/engine"`)
- ✅ Commit `6132415` (Task 1 RED — failing integration tests) — verified in `git log`
- ✅ Commit `1080a4b` (Task 1 GREEN — `Engine::new_with_providers`) — verified in `git log`
- ✅ Commit `64aa5dd` (Task 2 — BL-01 scan extension) — verified in `git log`
- ✅ All 5 cache_stale_on_error tests pass: `cargo test --test cache_stale_on_error` → `5 passed; 0 failed`
- ✅ Full `cargo test` suite green: zero regressions across all Phase 0/1/2/3 tests + the 5 new tests
- ✅ BL-01 engine-scope guardrail in place: `grep -rn "Timestamp::now()" src/engine/` returns only comment / doc lines (0 actual code matches)

---
*Phase: 03-gemini-conditional-cache-refresh-policy*
*Completed: 2026-05-25*
