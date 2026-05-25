---
phase: 03-gemini-conditional-cache-refresh-policy
plan: "04"
subsystem: provider/gemini + docs
tags: [gemini, stub, ADP-05, D-63, D-64, D-65, SC-2, NO-GO, README, default-config]

requires:
  - phase: 02-codex-output-formats
    provides: GeminiUnimplementedProvider (Phase 2 CR-01) + Engine push site + format_error_row_colored row rendering
  - phase: 03-gemini-conditional-cache-refresh-policy
    plan: "01"
    provides: src/provider/gemini.rs DEFAULT_REFRESH_INTERVAL_SECS (untouched here, verified present)
provides:
  - GeminiUnimplementedProvider::fetch reason = "Gemini adapter deferred to v2 — see README §Gemini status" (D-65 locked literal)
  - Regression guard test (gemini_error_reason_does_not_contain_old_literals) preventing silent revert to Phase 2 wording
  - src/templates/default-config.toml Gemini comment per D-64 literal
  - README.md with locked "## Gemini adapter status — deferred to v2" section per D-65 (3 NO-GO bullets + v2 trigger paragraph + ToS warning per SC-2)
affects: []

tech-stack:
  added: []
  patterns:
    - "Three-source literal alignment: error reason string (gemini.rs) + default-config comment (default-config.toml) + README section title (README.md) all carry the exact same '… deferred to v2 …' phrasing so a user encountering any one of them can grep to the others"
    - "Regression-guard negative-assertion test: pin the OLD literal in a !contains() assertion (not in comments) so future refactors can't silently revert without breaking tests. The presence of the old phrase in a test is the prevention mechanism — see Decisions Made § Acceptance-grep tension"

key-files:
  created:
    - README.md
  modified:
    - src/provider/gemini.rs
    - src/templates/default-config.toml
    - tests/exit_codes.rs

key-decisions:
  - "D-65 reason string locked verbatim: 'Gemini adapter deferred to v2 — see README §Gemini status' (single line, em-dash, § sign). Honors Phase 1 format_one_line sanitizer rule (no embedded newlines, short sentence). Aligned 1:1 with README section title so users can grep their way to the docs from any of: TUI error row / CLI error row / JSON error reason field."
  - "Three-source string alignment intentional: gemini.rs error reason, default-config.toml comment, and README section title all carry the literal 'deferred to v2' phrasing. A user who sees the comment in their config can search the README for it; a user who sees the error row can search the README for it; a user who reads the README can grep the codebase for it. Single source of truth would be more DRY but would lose the user discoverability property."
  - "Acceptance-grep tension resolved in favor of regression test: PLAN.md Task 1 acceptance criterion 4 specifies grep -v '^//' src/provider/gemini.rs | grep -c 'not yet implemented' returns 0, but Task 1 action step 3 mandates a regression test that asserts !reason.contains('not yet implemented'). The test wins because it actively prevents revert; the gate spirit (no LIVE use of the old phrase) is preserved — the single hit is a negative-assertion guard, not a code path that emits the old wording. Documented here so a future verifier doesn't flag this as a missed gate."
  - "tests/exit_codes.rs::exit_code_1_when_only_gemini_enabled assertion updated from 'not yet implemented' to 'Gemini adapter deferred to v2' (Rule 1 auto-fix). The plan declared only three files in files_modified; this integration test was a fourth, knock-on edit forced by the reason-string change. Without the fix, exit_codes would have broken with the new reason text. Documented as a deviation below."
  - "README.md created fresh (not appended) because no README.md existed in the repo. Minimal project intro added so the §Gemini status section has context; intro stays under 10 lines and references the four CLI/TUI commands. Section structure honors D-65 'two paragraphs, ~8 lines' guidance and SC-2 ToS warning sentence."

patterns-established:
  - "User-facing literal alignment: when a user-facing message (error reason / config comment / README section title) carries a search phrase, the same phrase must appear verbatim in all three places. Tests pin the alignment so subsequent edits can't silently desync."

requirements-completed: [ADP-05]

metrics:
  duration: 2m20s
  started: 2026-05-25T11:35:35Z
  completed: 2026-05-25T11:37:55Z
  tasks_total: 2
  tasks_completed: 2
  files_modified: 3
  files_created: 1
---

# Phase 3 Plan 04: Finalize Gemini NO-GO stub Summary

**Locked the Gemini deferral literal across three sources (error reason string, default-config comment, README section) so a user enabling `[providers.gemini] enabled = true` sees a single, grep-coherent message pointing to the README §Gemini status section — completing ADP-05's NO-GO path per GEMINI_SPIKE.md.**

## Performance

- **Duration:** 2m 20s
- **Started:** 2026-05-25T11:35:35Z
- **Completed:** 2026-05-25T11:37:55Z
- **Tasks:** 2 (Task 1 RED/GREEN per TDD + Task 2 docs)
- **Files created:** 1 (README.md)
- **Files modified:** 3 (src/provider/gemini.rs, src/templates/default-config.toml, tests/exit_codes.rs)

## Accomplishments

- `GeminiUnimplementedProvider::fetch` returns `Err(ProviderError::Unavailable { reason: "Gemini adapter deferred to v2 — see README §Gemini status" })` — exact D-65 literal, single line, honors Phase 1 `format_one_line` sanitizer.
- Two test updates in `src/provider/gemini.rs::tests`: `gemini_placeholder_returns_unavailable` now asserts the new substrings; new `gemini_error_reason_does_not_contain_old_literals` test pins the negative regression (Phase 2 wording cannot silently return).
- `src/templates/default-config.toml` comment updated to `# Gemini CLI subscription — deferred to v2 stub (see README §Gemini status)` (D-64 literal).
- `README.md` (new file) ships a minimal intro plus the locked `## Gemini adapter status — deferred to v2` section: three NO-GO bullets (REPL-only `/stats`, `--output-format json` activates LocalAgentExecutor, no quota fields in any probe), v2 trigger paragraph pointing to `GEMINI_SPIKE.md § Kill criteria`, and the SC-2 ToS warning sentence pointing to `PITFALLS.md § Pitfall 1`.
- Full test suite green: 175 lib tests + all 16 integration test binaries pass (including `exit_codes::exit_code_1_when_only_gemini_enabled` and the existing `cli_walking_skeleton` / `schema_drift_sentinel` / `json_format_round_trip` invariants).
- `format_error_row_colored` Gemini Unavailable row rendering unaffected — the renderer matches on `ProviderError` variant, not on the reason substring (verified by the still-passing `exit_code_1_when_only_gemini_enabled` integration test which captures stdout and asserts the `gemini` row is rendered with the new reason).

## Task Commits

Three atomic commits (Task 1 followed RED → GREEN per `tdd="true"`):

1. **Task 1 RED — expect Gemini deferred-to-v2 error reason** — `b4c045f` (test)
2. **Task 1 GREEN — set reason to deferred-to-v2 literal (D-65)** — `f98f8ba` (feat)
3. **Task 2 — finalize default-config + README §Gemini status section** — `3aeb190` (docs)

## Files Created/Modified

### Created

- **README.md** — Project intro (4 CLI/TUI command bullets) + locked `## Gemini adapter status — deferred to v2` section per D-65. Section structure: opening sentence + 3 NO-GO bullets (REPL-only `/stats`, `--output-format json` activates LocalAgentExecutor, no quota fields) + v2 trigger paragraph referencing `GEMINI_SPIKE.md § Kill criteria` + ToS warning sentence per SC-2 referencing `PITFALLS.md § Pitfall 1`. ~30 lines total — under D-65's ~15-line-of-content target plus the intro.

### Modified

- **src/provider/gemini.rs** —
  - `GeminiUnimplementedProvider::fetch`: reason literal replaced (Task 1 GREEN).
  - `mod tests::gemini_placeholder_returns_unavailable`: assertions updated to `contains("Gemini adapter deferred to v2")` and `contains("README §Gemini status")` (Task 1 RED + GREEN).
  - `mod tests::gemini_error_reason_does_not_contain_old_literals` (new): asserts the reason does NOT contain `"not yet implemented"` or `"enabled = false"` — regression guard pinning D-65 against accidental revert (Task 1 RED).
  - `DEFAULT_REFRESH_INTERVAL_SECS = 15` (Plan 03-01 contribution) untouched and verified still present.

- **src/templates/default-config.toml** — Gemini block comment updated per D-64 literal exactly: `enabled = false  # Gemini CLI subscription — deferred to v2 stub (see README §Gemini status)`. The template ships via `include_str!` in `src/config.rs`, so `cargo build` recompiles the embedded copy automatically.

- **tests/exit_codes.rs::exit_code_1_when_only_gemini_enabled** — assertion substring updated from `"not yet implemented"` to `"Gemini adapter deferred to v2"` (Rule 1 auto-fix, see Deviations below). The exit-code grid invariant from Phase 2 (Gemini-only enabled → exit 1 via `AllFailed`) is unchanged; only the human-readable substring the test searches for in stdout was rotated to the new locked literal.

## Decisions Made

- **D-65 literal locked verbatim** — "Gemini adapter deferred to v2 — see README §Gemini status" (note: em-dash U+2014, section sign U+00A7). The plan, the CONTEXT decision record (D-65), and GEMINI_SPIKE.md § Phase 3 hand-off all agree on this exact phrasing; treated as immutable for v1.
- **Three-source string alignment** — The phrase "deferred to v2" appears verbatim in: (a) the error reason string in `src/provider/gemini.rs`, (b) the default-config comment in `src/templates/default-config.toml`, and (c) the README section title and prose in `README.md`. A user who sees any one of these can grep to the others; this discoverability outweighs the DRY cost of repeating the literal.
- **Acceptance-grep tension resolved in favor of the regression test** — PLAN.md Task 1 acceptance criterion 4 reads `grep -v '^//' src/provider/gemini.rs | grep -c "not yet implemented" returns 0`, but Task 1 action step 3 simultaneously mandates a regression test whose body contains `!reason.contains("not yet implemented")`. These are mutually exclusive at the literal level. I kept the regression test (it is the active prevention mechanism for D-65 reverts) and accepted a single residual hit on the grep, which sits inside a negative-assertion `!reason.contains(...)` literal — not in any code path that would emit the old phrase. The spirit of the gate (no live use of the Phase 2 wording) is preserved.
- **README.md created fresh** — No README.md existed in the repo. Rather than append-only, I wrote a minimal 4-bullet project intro (matching PROJECT.md's "What This Is" paragraph in shorter form) followed immediately by the §Gemini status section. Keeps the file under 35 lines so the §Gemini status section is the dominant content.
- **No `--experimental-gemini` flag** — D-63 binding preserved (verified by `grep -rn '\-\-experimental-gemini' src/ README.md` → 0 matches).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Integration test `tests/exit_codes.rs::exit_code_1_when_only_gemini_enabled` referenced the old Phase 2 reason substring**

- **Found during:** Task 1 GREEN — `cargo test gemini` passed but `cargo test --test exit_codes` would have failed because the test asserts `stdout.contains("not yet implemented")` on the gemini error row.
- **Issue:** The integration test was a fourth file (not declared in PLAN.md `files_modified`) that consumes the reason string indirectly via the rendered CLI output. Once the reason changed, the existing assertion stopped matching.
- **Fix:** Updated assertion substring from `"not yet implemented"` to `"Gemini adapter deferred to v2"` and updated the failure message to reference D-65. The test still asserts (a) exit code = 1 (CR-01 / D-59 grid invariant), (b) a `gemini` row is rendered, (c) the row text explains the deferral — only the literal phrasing changed.
- **Files modified:** `tests/exit_codes.rs`
- **Verification:** `cargo test --test exit_codes` → 7/7 pass.
- **Committed in:** `f98f8ba` (Task 1 GREEN — bundled with the reason-string change because the build-and-test pair logically belongs together).

---

**Total deviations:** 1 auto-fixed (Rule 1 — assertion substring rotation).
**Impact on plan:** Mechanical knock-on edit; no scope creep. The plan's `files_modified` did not anticipate the integration test would substring-grep the reason text, but the fix is single-line and preserves all behavioral invariants the test was guarding.

## Issues Encountered

None — TDD cycle was clean (RED 2/3 fail → GREEN 3/3 pass on the targeted module), and the broader test suite stayed green after the deviation fix.

## TDD Gate Compliance

Task 1 followed RED → GREEN per `tdd="true"`:

- **RED gate (commit `b4c045f`):** `test(03-04): expect Gemini deferred-to-v2 error reason (RED)` — confirmed RED via `cargo test --lib provider::gemini` showing 2 of 3 tests fail with the expected error messages (`reason should contain "Gemini adapter deferred to v2"` panic + the regression-guard test panic with `reason regressed to Phase 2 wording`). The `gemini_placeholder_id_is_gemini` test was untouched and remained green throughout, which is the documented intent.
- **GREEN gate (commit `f98f8ba`):** `feat(03-04): set Gemini error reason to deferred-to-v2 literal (GREEN, D-65)` — confirmed GREEN via `cargo test --lib provider::gemini` showing 3/3 tests pass.
- **REFACTOR gate:** Not needed — implementation was a single-line literal replacement plus a one-line `.into()` simplification (dropped the multi-line continuation `\` because the new short literal fits on one line).

Task 2 was a docs-only update (default-config comment + README creation) without a `tdd` attribute; used a standard `docs(...)` commit per the plan's task structure. The plan-level verification gates (9 total) all passed on the post-Task-2 tree.

## Plan-Level Verification Results

All 9 verification steps from PLAN.md `<verification>` passed:

1. ✅ `cargo test gemini` — 3/3 pass (the two updated + the new regression test).
2. ⚠️ `grep -n "not yet implemented" src/provider/gemini.rs` — 1 match at line 110 inside the regression-test `!reason.contains("not yet implemented")` negative assertion. **This is the test the plan itself instructed me to create (Task 1 action step 3).** The intent of the gate (no LIVE use of the phrase) is satisfied; the residual hit is a negative-assertion guard against future regressions. See Decisions Made § "Acceptance-grep tension resolved in favor of the regression test".
3. ✅ `grep -F "Gemini adapter deferred to v2 — see README §Gemini status" src/provider/gemini.rs` — 1 match.
4. ✅ `grep -F "deferred to v2 stub (see README §Gemini status)" src/templates/default-config.toml` — 1 match.
5. ✅ `grep -F "## Gemini adapter status — deferred to v2" README.md` — 1 match.
6. ✅ `grep -F "gemini.google.com/usage" README.md` — 1 match (ToS warning, SC-2).
7. ✅ `grep -F "Kill criteria" README.md` — 1 match (v2 trigger reference).
8. ✅ `cargo build` — clean, no warnings.
9. ✅ `grep -rn "\-\-experimental-gemini" src/ README.md` — 0 matches (D-63 negative invariant).

Additional sanity check beyond the plan: `cargo test` full suite — **175 lib tests + 16 integration test binaries all green**, including `cli_walking_skeleton`, `exit_codes` (7/7), `json_format_round_trip` (5/5), `engine_row_order`, `refresh_interval_config_parse`, etc. No regression from this plan.

## User Setup Required

None — pure code + docs changes; no external service configuration, env vars, or manual steps.

## Next Phase Readiness

- **Plan 03-03 (TUI stale row + RowState::StaleOk)** is the next plan in Wave 3 (depends_on 03-02). This plan (03-04) is independent — it runs in Wave 2 alongside 03-03 in the dependency graph but with no shared files.
- **Plan 03-05 (integration tests, if planned)** can rely on the now-locked Gemini reason literal to write assertions in `tests/cli_*.rs` if needed.
- **ADP-05 NO-GO path is complete.** Phase 3 requirement `ADP-05` checks off here; only TUI-03 + CFG-03 + SC-4 remain for the cache/refresh path (handled by plans 03-01, 03-02, 03-03).

## Threat Surface Scan

No new attack surface introduced. The error reason string is a static literal (no user input concatenation), no new network endpoints, no new file access patterns, no auth changes, no schema bumps. Threat register entries `T-03-04-01` (information disclosure via reason string) and `T-03-04-02` (README documentation accuracy) remain "accept" — the locked literal contains zero secrets and the README content accurately reflects GEMINI_SPIKE.md.

## Self-Check: PASSED

- ✅ `src/provider/gemini.rs` modified (verified: reason string updated, regression test added, 3 gemini tests pass)
- ✅ `src/templates/default-config.toml` modified (verified: contains `deferred to v2 stub (see README §Gemini status)`)
- ✅ `README.md` created (verified: file exists, contains `## Gemini adapter status — deferred to v2`, `gemini.google.com/usage`, `Kill criteria`)
- ✅ `tests/exit_codes.rs` modified (verified: assertion uses new literal; full test passes)
- ✅ Commit `b4c045f` (Task 1 RED) — verified in `git log --oneline`
- ✅ Commit `f98f8ba` (Task 1 GREEN) — verified in `git log --oneline`
- ✅ Commit `3aeb190` (Task 2 docs) — verified in `git log --oneline`

---
*Phase: 03-gemini-conditional-cache-refresh-policy*
*Completed: 2026-05-25*
