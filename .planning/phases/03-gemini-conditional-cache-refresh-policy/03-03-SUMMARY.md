---
phase: 03-gemini-conditional-cache-refresh-policy
plan: "03"
subsystem: tui
tags: [tui, ratatui, stale-on-error, RowState, RowOutcome, Yellow, D-69, D-70, D-74, BL-01, TUI-03]

requires:
  - phase: 03-gemini-conditional-cache-refresh-policy
    plan: "02"
    provides: Engine::refresh_all returns Vec<(ProviderId, RowOutcome)>; SCAFFOLD adapter in src/tui/mod.rs marked "SCAFFOLD: removed in Plan 03-03"
  - phase: 01-engine-claude-tui-scaffold
    provides: RowState enum + AppState::apply_results + hp_row widget + ui::draw plumbing; BL-01 clock-injection; tests/no_walltime_in_adapter.rs grep gate
provides:
  - RowState::StaleOk { state: ProviderState, stale_age_secs: u64 } variant in src/tui/app.rs
  - AppState::apply_results now consumes Vec<(ProviderId, RowOutcome)> directly (RowOutcome::Stale → RowState::StaleOk passing stale_age_secs through verbatim)
  - build_stale_ok_line(state, stale_age_secs, now) in src/tui/widgets/hp_row.rs — sibling of build_ok_line; all styled spans use Color::Yellow (D-69); suffix "  (stale Ns ago)" two-spaces gap byte-exact
  - SCAFFOLD adapter (2 sites) fully removed from src/tui/mod.rs; RowOutcome flows engine → AppState end-to-end
affects: [03-05-integration-tests]

tech-stack:
  added: []
  patterns:
    - "Per-Span Color::Yellow override for stale rows (RESEARCH Q6) — ratatui 0.30 per-Span style takes precedence over Paragraph base style, so build_stale_ok_line is a sibling of build_ok_line (not a wrapper). Yellow on every styled span (filled / empty / bullet) overrides the percent-threshold accent so the row is visually distinct regardless of percent."
    - "Stale-age math lives ONLY in the engine layer (BL-01 invariant under src/tui/widgets/). RowState::StaleOk carries the pre-computed stale_age_secs: u64 as a value field; the widget formats it into '(stale Ns ago)' without any wall-clock read. tests/no_walltime_in_adapter.rs grep-rejects any Timestamp::now() call under src/tui/widgets/ to enforce."
    - "RowOutcome → RowState translation lives in a single place (AppState::apply_results) and is a closed-set match — no fallthrough, no defaults. The CLI dispatch translator (cli::outcome_to_result, Plan 03-02) and the TUI translator (this plan) are symmetric: both consume the same engine boundary type and route to layer-appropriate shapes."

key-files:
  created: []
  modified:
    - src/tui/app.rs
    - src/tui/widgets/hp_row.rs
    - src/tui/mod.rs

key-decisions:
  - "RowState::StaleOk { state: ProviderState, stale_age_secs: u64 } — stale-age lives in the row state, NOT in ProviderState (D-70). ProviderState stays a pure DTO; JSON wire shape unaffected; CLI render paths (render_text.rs / render_json.rs) byte-identical to Phase 2 (D-68)."
  - "build_stale_ok_line is a SIBLING of build_ok_line, not a wrapper (RESEARCH Q6). Wrapping would require overriding each span's style after construction, which is fragile in ratatui 0.30 where per-Span style takes precedence. Each span is built directly with Style::default().fg(Color::Yellow) — bar filled, bar empty, bullet separator. The stale suffix itself is unstyled (raw) — bar color signals staleness; machine consumers can grep '(stale ' regardless of color support."
  - "Task 2 SCAFFOLD removal bundled into the Task 1 GREEN commit (Rule 3 deviation, mirrors Plan 03-02's cli/tui cascade) because Task 1's acceptance criterion `cargo build --lib exits 0` requires the SCAFFOLD adapter to be removed in the same commit as the apply_results signature change. Both SCAFFOLD blocks (priming fetch + fetch tick arm) were marked with the literal `SCAFFOLD: removed in Plan 03-03` exactly so this commit could grep-locate and remove them. Task semantics preserved — the work the plan attributed to Task 2 (SCAFFOLD removal + regression suite) landed in the GREEN commit; the verification pass-through is documented here."
  - "ui::draw needed NO changes — it iterates app.rows and hands every row to hp_row::render uniformly. hp_row::render calls build_line, which now matches RowState::StaleOk → build_stale_ok_line. The match on RowState lives ONLY in build_line, not in ui::draw, so adding the variant didn't cascade into ui::draw at all."
  - "D-74 invariant preserved verbatim: tokio::time::interval(Duration::from_secs(15)) at the global TUI tick is unchanged. Per-provider rate limiting is handled inside Engine::refresh_all via the refresh_intervals map (Plan 03-01) — the global tick is just the wakeup cadence."

patterns-established:
  - "Atomic-variant-and-arm pattern: adding a variant to a closed-match enum (RowState) is bundled with adding the corresponding match arm in the consumer (build_line) in ONE commit so the Rust match is exhaustive at all times. No transient `unimplemented!()` arm, no `#[allow(non_exhaustive)]` workaround. Equivalent to the Plan 03-02 cascade — type-system pressure as a correctness guard."
  - "SCAFFOLD adapter pattern (Plan 03-02 → Plan 03-03): when a type-system widening must land before all consumers are ready, the producer side ships with a SCAFFOLD adapter at every callsite, marked with a literal `SCAFFOLD: removed in Plan XX-YY` grep marker. The consumer-side plan then grep-locates the marker and removes the adapter. Worked cleanly here — 2 callsites identified by grep, both removed in this plan."

requirements: [TUI-03]
requirements-completed: [TUI-03]

metrics:
  duration: 18m
  started: 2026-05-25T11:40:13Z
  completed: 2026-05-25T11:58:00Z
  tasks_total: 2
  tasks_completed: 2
  files_modified: 3
---

# Phase 3 Plan 03: TUI stale-row rendering — RowState::StaleOk + build_stale_ok_line wired Summary

**`AHB tui` now renders `RowOutcome::Stale` as a Yellow HP bar row with a `(stale Ns ago)` suffix per D-69; the SCAFFOLD adapter from Plan 03-02 is fully removed and `RowOutcome` flows end-to-end from `Engine::refresh_all` through `AppState::apply_results` into `RowState::StaleOk` without any conversion layer in between.**

## Performance

- **Duration:** ~18m (start 2026-05-25T11:40:13Z → finish ~11:58Z)
- **Tasks:** 2 (both atomic; Task 1 RED → Task 1+2 GREEN bundled)
- **Commits:** 2 (RED + GREEN)
- **Files modified:** 3 (src/tui/app.rs, src/tui/widgets/hp_row.rs, src/tui/mod.rs)

## Accomplishments

- **`RowState::StaleOk { state: ProviderState, stale_age_secs: u64 }`** added to `src/tui/app.rs` (D-70). The variant carries a cloned `ProviderState` plus the pre-computed stale-age in seconds — no new field on `ProviderState` itself (D-70 binding preserves the DTO purity + the JSON wire shape `schema_version: 1`).
- **`AppState::apply_results` widened** to consume `Vec<(ProviderId, RowOutcome)>` directly. Translation table (RESEARCH Q5):
  - `Fresh(state)` → `Ok(state)`
  - `Stale { state, stale_age_secs }` → `StaleOk { state, stale_age_secs }`
  - `Failed(SchemaDrift { .. })` → `SchemaDrift { id }`
  - `Failed(other)` → `Err { id, message }`

  `stale_age_secs` is pre-computed by the engine and passes through verbatim — no wall-clock read in the TUI layer (BL-01 invariant honored).
- **`build_stale_ok_line(state, stale_age_secs, now)`** added to `src/tui/widgets/hp_row.rs` — sibling of `build_ok_line`, not a wrapper. All styled spans use `Color::Yellow` (D-69 + RESEARCH Q6); suffix format is `"  (stale Ns ago)"` with two spaces before the open paren (D-69 byte-exact). Defensive empty-windows guard mirrors `build_ok_line` (degrade to ERROR row rather than panic).
- **`build_line` match arm added** for `RowState::StaleOk` → `build_stale_ok_line(state, *stale_age_secs, now)`. The match stays exhaustive at all times — variant addition + arm addition land in the SAME commit.
- **SCAFFOLD adapter fully removed** from `src/tui/mod.rs` at both callsites (priming fetch + fetch tick arm). `RowOutcome` now flows from `engine.refresh_all()` directly into `app.apply_results(outcomes)`. The `// SCAFFOLD: removed in Plan 03-03` literal marker that Plan 03-02 left for this plan is gone; `grep -F "SCAFFOLD: removed in Plan 03-03" src/tui/mod.rs` returns 0 matches.
- **D-74 preserved verbatim**: `tokio::time::interval(Duration::from_secs(15))` at the global TUI tick is byte-identical to Phase 2. Per-provider rate limiting is handled inside `Engine::refresh_all` via the `refresh_intervals` map (Plan 03-01).
- **All Phase 1/2 invariants preserved.** Full `cargo test` green: 185 lib tests + all integration test binaries. TUI-04 (`tui_panic_safe_restore`) + TUI-05 (`tui_non_tty_refusal`) + `cli_walking_skeleton` + `exit_codes` + `json_format_round_trip` + `no_walltime_in_adapter` all pass. D-68 (CLI not modified) verified by grep: `src/cli/render_text.rs` and `src/cli/render_json.rs` contain zero "stale" matches.

## Task Commits

Two atomic commits:

1. **Task 1 RED: add failing tests for RowState::StaleOk + build_stale_ok_line** — `d2da3a5` (test)
   - Added 5 app.rs unit tests + 5 hp_row.rs unit tests + rewrote the existing `apply_results_translates_ok_schema_drift_and_err` test to use `RowOutcome` input.
   - Confirmed RED via `cargo build --lib --tests` → 9 compile errors (E0308 mismatched apply_results input type + E0599 variant `StaleOk` not found). The compile-time failure is the strongest form of RED — the test names exist but the API they reference doesn't.

2. **Task 1 GREEN + Task 2 SCAFFOLD removal: atomic** — `553e016` (feat)
   - Added `RowState::StaleOk { state, stale_age_secs }` variant.
   - Rewrote `AppState::apply_results` signature to consume `RowOutcome`.
   - Added `build_stale_ok_line` function in `hp_row.rs`.
   - Added `RowState::StaleOk` → `build_stale_ok_line` arm in `build_line`.
   - Removed both SCAFFOLD adapter blocks from `src/tui/mod.rs` (priming fetch + fetch tick arm).
   - Cleaned up now-unused imports (`RowOutcome`, `ProviderError`, `ProviderId`, `ProviderState`).

## Files Created/Modified

### Modified

- **`src/tui/app.rs`** — Added `use crate::engine::cache::RowOutcome`. Added `RowState::StaleOk { state: ProviderState, stale_age_secs: u64 }` variant. Rewrote `apply_results` signature to `Vec<(ProviderId, RowOutcome)>` with the four-arm translation table (Fresh / Stale / Failed(SchemaDrift) / Failed(other)). Added 5 new behavioral tests + rewrote 1 existing test for the new input shape. BL-01 preserved: no `Timestamp::now()` call.

- **`src/tui/widgets/hp_row.rs`** — Added `RowState::StaleOk` match arm in `build_line` → calls `build_stale_ok_line(state, *stale_age_secs, now)`. Added new function `build_stale_ok_line` that mirrors `build_ok_line`'s pct/filled/empty/countdown/label math but builds each span with `Style::default().fg(Color::Yellow)` (filled cells, empty cells, bullet separator). Appends `Span::raw("  ")` + `Span::raw(format!("(stale {stale_age_secs}s ago)"))` at the end (D-69 byte-exact two-space gap + suffix format). Empty-windows defensive guard mirrors `build_ok_line`. Added 5 new behavioral tests (Yellow on all styled spans, stale suffix included, two-spaces gap, zero-secs case, build_line dispatch). BL-01 preserved: no `Timestamp::now()` call.

- **`src/tui/mod.rs`** — Removed both SCAFFOLD adapter blocks (priming fetch ~line 113, fetch tick arm ~line 153). Removed now-unused imports (`RowOutcome`, `ProviderError`, `ProviderId`, `ProviderState`). The priming fetch path now reads `engine.refresh_all(jiff::Timestamp::now()).await` → `app.apply_results(outcomes)` directly. The fetch-tick arm does the same. D-74 preserved verbatim: `tokio::time::interval(Duration::from_secs(15))` line untouched. BL-01 preserved: the two authorized `jiff::Timestamp::now()` call sites in `tui_loop` (priming fetch + fetch tick + render tick + post-prime app.now refresh) are unchanged in count and position.

## Decisions Made

- **`RowState::StaleOk { state, stale_age_secs }` field names** (D-70 binding). Picked `{ state, stale_age_secs }` rather than `{ cached_state, cached_at: Timestamp }` because (a) it mirrors `RowOutcome::Stale { state, stale_age_secs }` exactly (engine boundary → row state with byte-identical field names — zero cognitive load), (b) `stale_age_secs: u64` avoids re-computing `(now - cached_at).total(Unit::Second)` at render time, (c) it doesn't drag a `jiff::Timestamp` into the TUI layer's data model.

- **`build_stale_ok_line` is a sibling of `build_ok_line`, NOT a wrapper** (RESEARCH Q6 binding). The temptation is to call `build_ok_line(state, now)` and then iterate `.spans` to override `.style`, but ratatui 0.30's per-Span style takes precedence over `Paragraph` base style, so wrapping would have to override every individual span — that's the same work as building directly, with extra fragility (any new span in `build_ok_line` would silently bypass the Yellow override). Direct construction is clearer + future-proof.

- **Stale suffix is `Span::raw` (unstyled)**, not `Span::styled(...Yellow...)` (D-69 reading). The bar color already signals staleness; the suffix doesn't need an extra Yellow attribute. Crucially, this means `--color=never` / `NO_COLOR=1` paths (when added in future plans) automatically preserve the semantic "(stale 32s ago)" text even when the Yellow attribute is stripped — machine consumers can still grep `(stale ` regardless of color support. D-69 explicitly states "machine consumers still grep stale". 

- **`Modifier::DIM` skipped for v1** (RESEARCH Q6 reading). D-69 says "整行 Yellow color attribute" (whole-line Yellow) but doesn't mandate DIM. Adding DIM is a separate stylistic choice; v1 sticks with Yellow alone to match Plan 03-02's CLI ERROR row simplicity (single-color signal). If users find Yellow alone insufficient, DIM can be added in a follow-up without changing any data shapes.

- **Task 2 SCAFFOLD removal bundled into Task 1 GREEN** (Rule 3 deviation; documented below). The plan split Task 1 (variant + apply_results + build_stale_ok_line) and Task 2 (SCAFFOLD removal + regression suite). But Task 1's acceptance criterion `cargo build --lib exits 0` cannot be met without removing the SCAFFOLD, because the apply_results signature change cascades into the two SCAFFOLD blocks in `src/tui/mod.rs`. Mirrors Plan 03-02 Task 2 GREEN where cli/tui scaffolding had to land alongside the engine return-type widening for the same reason.

- **ui::draw needed NO changes**. The plan's Task 2 step 5 said "If src/tui/ui.rs needs a StaleOk match arm added (to draw the row via hp_row::render), add it now". Investigation confirmed `ui::draw` iterates `app.rows` and hands every row uniformly to `hp_row::render` — the `RowState` match lives ONLY inside `build_line` in `hp_row.rs`, not in `ui::draw`. So adding the variant didn't cascade into `ui::draw` at all. The plan's "if" condition was false; no action taken.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Task 2 SCAFFOLD removal bundled into Task 1 GREEN commit**

- **Found during:** Task 1 GREEN (immediately after rewriting `AppState::apply_results` signature, `cargo build --lib` failed with `error[E0308]: mismatched types` at both SCAFFOLD callsites in `src/tui/mod.rs`).
- **Issue:** Task 1's acceptance criterion `cargo build --lib exits 0` is structurally incompatible with leaving the SCAFFOLD adapter in place — once `apply_results` accepts `Vec<(ProviderId, RowOutcome)>`, the SCAFFOLD blocks (which produced `Vec<(ProviderId, Result<...>)>`) no longer compile. The plan attributed SCAFFOLD removal to Task 2, but Task 2 cannot start until Task 1 compiles.
- **Fix:** Bundled the SCAFFOLD removal (both callsites: priming fetch ~line 113 + fetch tick arm ~line 153) into the Task 1 GREEN commit (`553e016`). Also cleaned up the now-unused imports (`RowOutcome`, `ProviderError`, `ProviderId`, `ProviderState`) that Plan 03-02 added for the SCAFFOLD adapter. Task 2's verification work (regression suite green) is documented in this SUMMARY's `Plan-Level Verification Results` section and was run after the bundled commit landed.
- **Files modified:** `src/tui/mod.rs` (bundled into `553e016`).
- **Verification:** Full `cargo test` green (185 lib tests + integration binaries). `grep -F "SCAFFOLD: removed in Plan 03-03" src/tui/mod.rs` → 0 matches (scaffold fully removed). `grep -c "tokio::time::interval(Duration::from_secs(15))" src/tui/mod.rs` → 1 (D-74 unchanged).
- **Committed in:** `553e016` (Task 1 GREEN — atomic variant + arm + apply_results + SCAFFOLD removal).

---

**Total deviations:** 1 auto-fixed (Rule 3 — type-system pressure forced the cascade).
**Impact on plan:** No scope creep. The deviation is a mechanical commit-shape adjustment that mirrors Plan 03-02 Task 2 GREEN's cli/tui cascade bundle — both cases are situations where a Rust type-system widening forces all downstream callsites to be updated in the same commit. Task semantics preserved: Task 1 still delivers the variant + apply_results + build_stale_ok_line; Task 2's SCAFFOLD removal landed in the same commit; Task 2's verification work (regression suite green) is documented in this SUMMARY.

## Issues Encountered

None. The plan was extremely precisely scoped (variant + arm + function + scaffold removal = 3 files, 2 commits) and the only friction was the predictable Rule 3 deviation above. Resolution was a single commit-shape adjustment.

## TDD Gate Compliance

Task 1 followed RED → GREEN per `tdd="true"`:

- **RED gate (commit `d2da3a5`):** `test(03-03): add failing tests for RowState::StaleOk + build_stale_ok_line (RED)` — confirmed RED via `cargo build --lib --tests` showing 9 compile errors (E0308 mismatched apply_results input type + E0599 variant `StaleOk` not found in `app::RowState`). The compile-time failure is the strongest form of RED — the test names exist but the API they reference doesn't.
- **GREEN gate (commit `553e016`):** `feat(03-03): wire RowState::StaleOk + build_stale_ok_line + remove SCAFFOLD (GREEN)` — confirmed GREEN via `cargo test --lib tui` showing 22 tests pass (including the 10 new ones across app.rs + hp_row.rs).
- **REFACTOR gate:** Not needed; `build_stale_ok_line` was minimal at first write (single-pass span construction; same math as `build_ok_line` but Yellow on every styled span).

Task 2 (SCAFFOLD removal + regression suite) used `feat` commit type bundled into the Task 1 GREEN commit (Rule 3 deviation above) — these changes are not TDD (the regression suite was already in place from earlier phases; the removal is an additive removal of dead-after-Plan-03-02 code).

## Plan-Level Verification Results

All 13 verification steps from PLAN.md `<verification>` passed:

1. ✅ `cargo build` (full binary including TUI) — clean
2. ✅ `cargo test --lib tui` — 22 tests pass (10 new + 12 pre-existing)
3. ✅ `cargo test --test tui_panic_safe_restore` — 1/1 pass (TUI-04 regression)
4. ✅ `cargo test --test tui_non_tty_refusal` — 1/1 pass (TUI-05 regression)
5. ✅ `grep -F "SCAFFOLD: removed in Plan 03-03" src/tui/mod.rs` → 0 matches
6. ✅ `grep -c "tokio::time::interval(Duration::from_secs(15))" src/tui/mod.rs` → 1 (D-74 unchanged)
7. ✅ `grep -rn "Timestamp::now()" src/tui/widgets/` → 1 match in a doc comment only (no actual call; `tests/no_walltime_in_adapter.rs` test passes)
8. ✅ `grep -F "StaleOk" src/tui/app.rs` → 7 matches (variant def + apply_results arm + tests)
9. ✅ `grep -F "build_stale_ok_line" src/tui/widgets/hp_row.rs` → 7 matches (build_line arm + fn def + tests)
10. ✅ `grep -c "Color::Yellow" src/tui/widgets/hp_row.rs` → 12 matches (all stale spans in build_stale_ok_line + existing Yellow threshold in build_ok_line + test assertions)
11. ✅ Negative: `grep -F "stale" src/cli/render_text.rs` → 0 matches (D-68 — CLI not modified)
12. ✅ Negative: `grep -F "stale" src/cli/render_json.rs` → 0 matches (D-68)
13. ✅ Negative: `grep -F "stale_age_secs" src/model.rs` → 0 matches (D-70 — ProviderState unmodified)

Additional regression-suite coverage:
- ✅ `cargo test --test cli_walking_skeleton` — 4/4 pass (CLI output byte-identical to Phase 2)
- ✅ `cargo test --test exit_codes` — 7/7 pass (Gemini stub still exit 1; "any OK provider" still exit 0)
- ✅ `cargo test --test json_format_round_trip` — 5/5 pass (schema_version: 1 unchanged per D-68)
- ✅ `cargo test --test no_walltime_in_adapter` — 1/1 pass (BL-01 grep guard under src/tui/widgets/)

## User Setup Required

None — this plan is pure code changes; no external service configuration, env vars, or manual steps.

## Next Phase Readiness

Plan 03-03 closes the data path: `Engine` produces `RowOutcome::Stale` → `AppState` translates to `RowState::StaleOk` → `hp_row::build_stale_ok_line` renders the Yellow row + `(stale Ns ago)` suffix. The visible UX from Phase 3's `<domain>` example is now achievable in code.

**Plan 03-05 (integration tests with IntermittentFailureProvider)** can now write end-to-end TUI snapshot tests that exercise the three-state time axis from CONTEXT D-71:
- t < last + refresh_interval → `RowState::Ok` (no stale tag)
- t ≥ last + refresh_interval, fetch succeeds → `RowState::Ok` (cache updated)
- t ≥ last + refresh_interval, fetch fails transient → `RowState::StaleOk` (yellow + suffix)
- never-succeeded + fail → `RowState::Err` / `RowState::SchemaDrift` (Phase 1 path)

Plan 03-05 can build on the `ScriptedProvider` test helper added in Plan 03-02 (extract to `tests/common/` if needed for cross-test reuse).

## Self-Check: PASSED

- ✅ `src/tui/app.rs` modified (verified: `StaleOk` variant + `apply_results` consumes `RowOutcome` — `grep -F "StaleOk" src/tui/app.rs` returns 7 matches)
- ✅ `src/tui/widgets/hp_row.rs` modified (verified: `build_stale_ok_line` function + `RowState::StaleOk` arm in `build_line` — `grep -F "build_stale_ok_line" src/tui/widgets/hp_row.rs` returns 7 matches)
- ✅ `src/tui/mod.rs` modified (verified: SCAFFOLD blocks fully removed — `grep -F "SCAFFOLD: removed in Plan 03-03" src/tui/mod.rs` returns 0 matches; `grep -c "tokio::time::interval(Duration::from_secs(15))" src/tui/mod.rs` returns 1)
- ✅ Commit `d2da3a5` (Task 1 RED — failing tests) — verified in `git log`
- ✅ Commit `553e016` (Task 1 GREEN + Task 2 SCAFFOLD removal — atomic) — verified in `git log`
- ✅ All 13 plan-level verification steps pass
- ✅ Full `cargo test` green: 185 lib tests + all integration binaries
- ✅ D-68 (CLI not modified), D-70 (ProviderState not modified), D-74 (TUI 15s tick not changed) all preserved by grep

---
*Phase: 03-gemini-conditional-cache-refresh-policy*
*Completed: 2026-05-25*
