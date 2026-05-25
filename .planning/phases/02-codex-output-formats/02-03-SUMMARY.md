---
phase: 02-codex-output-formats
plan: 03
subsystem: cli
tags: [json, schema_version, exit_codes, clap_arggroup, sec03, rust, dispatch_outcome, after_help]

requires:
  - phase: 00-spike-spine
    provides: ProviderState / HpWindow / ResetInfo / ProviderError serde shapes + jiff timestamp::second::required adapter
  - phase: 01-engine-claude-tui-scaffold
    provides: Cli/Command + run_compact dispatch surface + format_one_line sanitizer + Secret<T>::Serialize redaction + debug_emit_fake_secret_and_exit fixture pattern + cli/tty::should_colorize_env(json_mode=true) D-58 wiring
  - phase: 02-codex-output-formats (Plan 02-01)
    provides: CodexProvider end-to-end + id_label-titlecased SchemaDrift sentinel (generalizes per-provider)
  - phase: 02-codex-output-formats (Plan 02-02)
    provides: HpWindow.detailed_label additive field (Plan 02-03 mirrors as JsonWindow.detailed_label) + Cli.detailed flag in isolation (Plan 02-03 retroactively captures in ArgGroup) + run_detailed dispatch + cli.detailed main.rs branch
provides:
  - cli::render_json module — JsonRoot / JsonProvider / JsonWindow / JsonError DTOs (v1 wire shape) + to_json_root converter + run_json driver
  - cli::DispatchOutcome enum (AnySuccess / AllFailed) + from_results discriminant + exit_code mapper
  - Cli::compact + Cli::json flags wrapped in ArgGroup::new("format").multiple(false) (D-57)
  - Cli #[command(after_help = "Exit codes: ...")] (D-61)
  - debug_emit_fake_secret_and_exit(as_json: bool) — extended SEC-03 emitter (D-62)
  - main.rs DispatchOutcome -> std::process::exit(exit_code) wiring (D-59)
  - tests/secret_leak_subprocess.rs::subprocess_json_path_redacts_secret (SEC-03 grep over --json route)
  - tests/json_format_round_trip.rs (5 tests: schema_version + BL-02 + zero-ANSI + zero-providers + --ascii/color silently ignored + --help after_help)
  - tests/exit_codes.rs (6 tests: D-59 grid)
affects: [03-cache-refresh, 04-distribution, README CORE-04/CORE-06 documentation]

tech-stack:
  added: []  # No new deps — uses serde_json + jiff + clap + regex + assert_cmd already in tree
  patterns:
    - "Stable DTO decoupled from internal model via per-module render_json — refactor of ProviderState cannot regress the wire shape (D-49)"
    - "clap ArgGroup::new(...).multiple(false) for ≥3 mutually-exclusive flags — cleaner than per-arg conflicts_with_all chains; emits exit 2 + canonical 'cannot be used with' on conflict (Option B from RESEARCH)"
    - "DispatchOutcome::from_results discriminant — empty results collapse to AnySuccess (CFG-04), single Ok flips the whole grid, otherwise AllFailed; isolates exit-code logic from per-format dispatch"
    - "after_help block for end-matter docs (exit codes, environment vars) — cleaner than long_about which displaces the argument list"
    - "debug fixture branching on a runtime bool (as_json) — extends SEC-03 coverage to a second Serialize call site without adding a new test-only feature flag"
    - "Integer-Unix-epoch timestamp serialization via `jiff::fmt::serde::timestamp::second::required` — Phase 0 default; v1 schema commits to this representation (NOT RFC3339 string)"

key-files:
  created:
    - src/cli/render_json.rs
    - tests/json_format_round_trip.rs
    - tests/exit_codes.rs
  modified:
    - src/cli/mod.rs
    - src/cli/render_text.rs
    - src/main.rs
    - tests/secret_leak_subprocess.rs
    - tests/cli_walking_skeleton.rs
    - tests/schema_drift_sentinel.rs

key-decisions:
  - "DTO timestamp serialization is Unix epoch seconds (integer), NOT RFC3339 string. The locked DTO uses `#[serde(with = \"jiff::fmt::serde::timestamp::second::required\")]` matching Phase 0's existing adapter for `fetched_at` / `resets_at`. CONTEXT D-50's `2026-05-25T13:45:22Z` example was illustrative — actual emission is `1779686233`. Documented in C7 test comment. Consumers using jq `from_unixtime` get RFC3339 cheaply; this representation is part of the v1 contract."
  - "DispatchOutcome lives in cli/mod.rs (not main.rs) because both run_compact / run_detailed / run_json need to construct it from their `engine.refresh_all` results. main.rs is just the exit() caller; the discriminant logic stays with the dispatch fns."
  - "ArgGroup uses `multiple = false` (Option B from RESEARCH) instead of per-arg `conflicts_with_all` chains (Option A from CONTEXT D-57). Functionally identical (same exit 2, same clap error message); declarative form is the single source of truth and survives field reordering without resync risk."
  - "Two Phase 1 integration tests had pre-Phase-2 assumptions that 'any AHB invocation should exit 0'. Per D-59/D-60 those are now exit 1 (all-providers-fail and SchemaDrift-as-fail respectively). Updated assertions; the user-visible row literals + UI-SPEC invariants are preserved. The Rule 1 fix is documented inline in each test's comment."
  - "`as_json: bool` parameter on debug_emit_fake_secret_and_exit (per D-62 / RESEARCH SEC-03 recommendation) instead of a new `--debug-emit-fake-secret-json` flag or a test-only Cargo feature. Smaller diff, no new conditional-compilation surface; both branches share the Secret<String> fixture so the [REDACTED] proof is parallel."
  - "Resume protocol: a prior executor terminated with substantial Task 1 work uncommitted (render_json.rs new, mod.rs + render_text.rs + main.rs modified). Inspected diff, ran `cargo test --lib cli::render_json` (8/8 pass) + `cli::tests::dispatch_outcome` (4/4 pass) + full suite (148/148 pass) BEFORE committing — confirmed the partial state correctly implements Task 1 as written and is safe to land atomically. No revisions needed; the in-progress code matched the plan's <action> block verbatim."

patterns-established:
  - "Pattern J1: per-format dispatch fn signature `async fn run_X(engine, ...) -> anyhow::Result<DispatchOutcome>` — every output-format dispatch returns the same discriminant so main.rs can apply the exit-code mapping uniformly. Future Phase 3+ formats (--json-pretty, etc.) follow this signature."
  - "Pattern J2: SEC binding rustdoc on every fn that emits a String into JSON — `/// SEC binding: e.to_string() invokes Display (NOT Debug); anyhow chain stays hidden per D-49 + Claude's Discretion #7`. Grep gate (no `format!(...,e:?}...)` in render_json.rs) backs the rustdoc."
  - "Pattern J3: integration tests use a helper `run_X(home, xdg, args)` returning `std::process::Output` so assertions can examine exit code + stdout + stderr independently. Phase 1's `assert_cmd::Command...assert().success()` shortcut hides the exit code — replaced with `.output()` for any test that needs to assert specific codes."
  - "Pattern J4: when test scenario sets up exactly one provider that returns Err, the new D-59 exit-1 contract means the test must assert `output.status.code() == Some(1)`, NOT `.success()`. Future Phase 3 tests adding new scenarios must follow this rule."

requirements-completed: [CORE-02, CORE-04, CORE-06, SEC-03]

duration: 18min
completed: 2026-05-25
---

# Phase 2 Plan 02-03: --json schema_version 1 + clap ArgGroup + Exit Codes + SEC-03 Extension Summary

**Locked the v1 JSON wire shape (CORE-04), wired the `--compact / --detailed / --json` ArgGroup interlock (CORE-02 + D-57), shipped the D-59 exit-code grid via DispatchOutcome (CORE-06), and extended the SEC-03 grep test to cover the `--json` route (D-62). Phase 2 closes: all 4 of its ROADMAP success criteria are verifiable end-to-end.**

## Performance

- **Duration:** ~18 min (2 atomic commits, resumed from a partially-completed state)
- **Started (Task 1 resume):** 2026-05-25T03:31:00Z
- **Completed:** 2026-05-25T03:49:00Z (approx)
- **Tasks:** 2 (Task 1 resumed from in-progress; Task 2 fresh)
- **Files modified:** 6 modified, 3 created

## Accomplishments

- **`AHB --json` ships end-to-end (CORE-04).** Sample stdout (zero providers enabled):
  ```json
  {"schema_version":1,"generated_at":1779686233,"providers":[]}
  ```
  Sample stdout (claude OK + codex error):
  ```json
  {"schema_version":1,"generated_at":...,"providers":[{"id":"claude","status":"ok","source":"claude-jsonl","fetched_at":...,"windows":[{"label":"claude","percent_remaining":90.0,"reset_at":...,"detailed_label":"5h"},{"label":"weekly","percent_remaining":null,"reset_at":...,"detailed_label":"weekly"}]},{"id":"codex","status":"error","windows":[],"error":{"kind":"unavailable","message":"no ~/.codex/state_*.sqlite found — is Codex CLI installed?"}}]}
  ```
  Round-trips through `jq` cleanly; `jq '.providers[].id'` emits `"claude"` / `"codex"` per BL-02 order; `--json` short-circuits ANSI styling regardless of `--color=always` (D-58 silent ignore verified by C10 byte-comparison test).
- **`--compact / --detailed / --json` ArgGroup interlock (CORE-02 + D-57).** All three flags wrapped in `ArgGroup::new("format").required(false).multiple(false)`. Clap auto-rejects any two with the canonical `"the argument '--X' cannot be used with '--Y'"` on stderr + exit 2. Verified by C4/C5/C6 + manual `cargo run -p ahb -- --compact --json` smoke. `--compact` alone has no special dispatch — it falls through to `run_compact` (semantically equivalent to no flag, but grep-friendly for tmux integrations that want to assert "single-line").
- **D-59 exit-code grid (CORE-06).** `DispatchOutcome::{AnySuccess, AllFailed}` is the single discriminant — derived from `Engine::refresh_all` results by `from_results`: empty (CFG-04) or any Ok → AnySuccess (0); all Err (including SchemaDrift per D-60) → AllFailed (1). `main.rs` calls `std::process::exit(outcome.exit_code())` after dispatch. Config / secrets unloadable → exit 2 (unchanged from Phase 1 main.rs line 85). Clap usage error → exit 2 (automatic). Verified by C1-C6.
- **D-61 `--help` exit-code documentation.** `after_help` block lists 0 / 1 / 2 with the verbatim D-59 phrasing. Verified by C12.
- **D-62 SEC-03 extension.** `debug_emit_fake_secret_and_exit` now takes `as_json: bool`. When `true`, emits a `JsonRoot`-shaped envelope containing a `Secret<String>` field via `serde_json::to_writer` — same `Serialize` path that production `run_json` uses. The new `subprocess_json_path_redacts_secret` test asserts (a) fixture absent, (b) no 20+-char alphanumeric run, (c) `[REDACTED]` present on stdout. The existing `subprocess_secret_does_not_leak` test continues to cover the non-JSON path (byte-identical Plan 02 emission preserved).

## Locked v1 JSON Schema Shape

| Field | Path | Type | Optional? | Notes |
|-------|------|------|-----------|-------|
| `schema_version` | `root.schema_version` | `u8` | required | Locked to `1`; bump only on breaking change (D-52) |
| `generated_at` | `root.generated_at` | `i64` (Unix epoch seconds) | required | jiff `timestamp::second::required` adapter — matches Phase 0 `fetched_at` / `resets_at`. NOT RFC3339 string. |
| `providers` | `root.providers` | array | required (may be empty) | BL-02 order: claude=0, codex=1, gemini=2, mock=3 |
| `id` | `providers[].id` | `&'static str` | required | One of `"claude"` / `"codex"` / `"gemini"` / `"mock"` |
| `status` | `providers[].status` | `&'static str` | required | `"ok"` or `"error"` — binary discriminant per D-51 |
| `source` | `providers[].source` | string | iff `status == "ok"` | adapter-controlled (`"claude-jsonl"` / `"codex-jsonl"` / `"mock"`) |
| `fetched_at` | `providers[].fetched_at` | `i64` (Unix epoch seconds) | iff `status == "ok"` | Same encoding as `generated_at` |
| `windows` | `providers[].windows` | array | required (may be empty) | empty when `status == "error"` |
| `error` | `providers[].error` | object | iff `status == "error"` | Per-kind sub-fields below |
| `label` | `providers[].windows[].label` | string | required | Compact-mode label; preserves Phase 1 byte-locked `"claude"` |
| `percent_remaining` | `providers[].windows[].percent_remaining` | `f32 | null` | required | `null` when NaN (Claude weekly limit-unknown sentinel) |
| `reset_at` | `providers[].windows[].reset_at` | `i64` (Unix epoch seconds) | required | When this window's quota resets |
| `detailed_label` | `providers[].windows[].detailed_label` | string | optional | Plan 02-02 additive; present iff adapter set distinct detailed-mode label (Claude `"5h"` / `"weekly"`) |
| `kind` | `providers[].error.kind` | `&'static str` | required | One of `"unconfigured"` / `"unavailable"` / `"schema_drift"` / `"network"` / `"rate_limited"` / `"internal"` |
| `message` | `providers[].error.message` | string | required | one-line via `format_one_line(e.to_string())` — Display only, NO Debug expansion |
| `missing` | `providers[].error.missing` | `array<string>` | iff `kind == "schema_drift"` | The drifted field names |
| `retry_after_seconds` | `providers[].error.retry_after_seconds` | `u64` | iff `kind == "rate_limited"` AND upstream supplied a span | Converted from `jiff::Span` via `total(Unit::Second)` |

## DispatchOutcome + Exit-Code Mapping (no deviations from D-59)

| Scenario | DispatchOutcome | Exit Code | Source |
|---|---|---|---|
| ≥1 provider returned `Ok` | `AnySuccess` | 0 | `DispatchOutcome::from_results` |
| All providers returned `Err` (incl. SchemaDrift per D-60) | `AllFailed` | 1 | `DispatchOutcome::from_results` |
| Zero providers enabled (CFG-04 empty results) | `AnySuccess` | 0 | `DispatchOutcome::from_results` |
| Config / secrets unloadable | n/a (early return) | 2 | `src/main.rs` lines 53-87 (Phase 1 wiring, unchanged) |
| Clap parse error (flag conflict, unknown flag) | n/a (clap exits before dispatch) | 2 | clap automatic via `ArgGroup` + `ErrorKind::ArgumentConflict` |
| Panic | n/a (panic-hook catches) | OS default (non-0) | Phase 0 D-27 panic-hook |
| TUI subcommand | not used | 0 (explicit `return Ok(())`) | `main.rs` Tui arm bypasses DispatchOutcome |

## clap ArgGroup Configuration

```rust
#[command(
    version,
    about = "...",
    after_help = "Exit codes:\n  0  ...\n  1  ...\n  2  ...",
    group(
        clap::ArgGroup::new("format")
            .required(false)
            .multiple(false)
            .args(["compact", "detailed", "json"]),
    ),
)]
pub struct Cli {
    #[arg(long)] pub compact: bool,
    #[arg(long)] pub detailed: bool,
    #[arg(long)] pub json: bool,
    // ... ascii / color / debug_emit_fake_secret / command unchanged
}
```

`required = false` — no flag falls through to compact default. `multiple = false` — at most one of the three. `args = [...]` — single source of truth (no per-field `conflicts_with_all` chains).

## SEC-03 Test Extension

Test `subprocess_json_path_redacts_secret` lives in `tests/secret_leak_subprocess.rs` next to the existing `subprocess_secret_does_not_leak`. Both:

- Run on every CI runner that the Phase 1 test runs on (`#[cfg(debug_assertions)]` gate is identical).
- Use `assert_cmd::Command::cargo_bin("ahb")` (same crate-relative path).
- Apply the same three assertions: literal fixture absent, no 20+-char alphanumeric run, `[REDACTED]` marker present.

Differences:

- The new test invokes `--json --debug-emit-fake-secret` (vs `--debug-emit-fake-secret` alone).
- The non-JSON branch exercises the Plan 02 emission `{"fake_secret":"[REDACTED]"}`; the JSON branch exercises `{"schema_version":1,"fake_secret_in_label":"[REDACTED]"}` (mimicking the JsonRoot top-level shape).
- Both grep paths converge on `Secret<T>::Serialize → "[REDACTED]"` — the same impl in `src/secrets.rs`.

Both tests passed in the same `cargo test` invocation that ran the full suite (148 lib + 30 integration = 178 total, 0 failed).

## Final Phase 2 ROADMAP Success-Criteria Checklist

- [x] **#1 — Codex adapter ships end-to-end.** Verified by Plan 02-01 SUMMARY + integration tests (test_a/b/c/d in `src/provider/codex/tests`).
- [x] **#2 — `AHB --compact` and `AHB --detailed` show single-line + multi-line formats.** `--compact` explicit flag added in this plan (CORE-02); `--detailed` shipped in Plan 02-02 (CORE-03). Both verified by `tests/cli_walking_skeleton.rs` + `tests/detailed_format.rs`.
- [x] **#3 — `AHB --json` round-trips through jq with schema_version=1, no ANSI, SEC-03 grep clean.** Verified by `tests/json_format_round_trip.rs` (5 tests) + `tests/secret_leak_subprocess.rs::subprocess_json_path_redacts_secret`.
- [x] **#4 — Exit codes 0/1/2; --help docs them; NO_COLOR + --color honored.** Verified by `tests/exit_codes.rs` (6 tests) + `tests/json_format_round_trip.rs::help_after_help_exposes_exit_codes` + the Phase 1 / Plan 02 NO_COLOR + --color tests (zero ANSI in pipe + `--color=always` in pipe paths).

**Phase 2 closes** — all 4 success criteria green.

## Task Commits

Each task was committed atomically:

1. **Task 1: render_json DTOs + DispatchOutcome** — `bdf9583` (feat)
2. **Task 2: ArgGroup + exit-code wiring + SEC-03 --json extension + integration tests** — `09549da` (feat)

## Files Created/Modified

**Created:**
- `src/cli/render_json.rs` — DTOs (JsonRoot/JsonProvider/JsonWindow/JsonError) + to_json_root + window_to_json + error_to_json + run_json; 8 inline tests J1-J8 (all passing).
- `tests/json_format_round_trip.rs` — 5 tests (C7 schema_version+BL-02, C8 zero-ANSI, C9 zero-providers, C10 --ascii/color silently ignored, C12 --help after_help).
- `tests/exit_codes.rs` — 6 tests (C1 ≥1 Ok=0, C2 all-Err=1, C3 CFG-04=0, C4/C5/C6 clap conflict=2).

**Modified:**
- `src/cli/mod.rs` — Cli gains `compact` + `json` flags; ArgGroup + after_help applied; DispatchOutcome enum + impl added; run_compact + run_detailed return `Result<DispatchOutcome>`; debug_emit_fake_secret_and_exit gains `as_json: bool` parameter.
- `src/cli/render_text.rs` — `format_one_line` promoted private → `pub(crate)` so render_json can reuse the sanitizer.
- `src/main.rs` — debug-flag dispatch passes `cli.json`; main dispatch reshaped to `let outcome = ... ; std::process::exit(outcome.exit_code())`; TUI arm returns early with explicit Ok(()) (exit 0).
- `tests/secret_leak_subprocess.rs` — adds `subprocess_json_path_redacts_secret` for SEC-03 over --json route.
- `tests/cli_walking_skeleton.rs` — `ahb_with_broken_claude_config_prints_error_row_not_crash` updated to assert exit 1 per D-59 (Claude-only Err = AllFailed). Row shape + next-step-hint invariants preserved.
- `tests/schema_drift_sentinel.rs` — `drift_in_recent_assistants_triggers_sentinel_literal` updated to assert exit 1 per D-60 (SchemaDrift counts as Err). UI sentinel literal + U+2592 byte count invariants preserved.

## Deviations from Plan

### Resume Decisions (no Rule trigger — opening protocol)

**1. [Resume - inspect partial state] In-progress Task 1 verified safe to commit as-is**
- **Found during:** Resume protocol Step 1 (pre-staging inspection per `<safe_resume_context>`).
- **State:** Prior executor left `src/cli/render_json.rs` (~492 lines new) + `src/cli/mod.rs` (~130 lines added) + `src/cli/render_text.rs` (~6 lines) + `src/main.rs` (~8 lines) uncommitted but `cargo check` passing.
- **Inspection:** Read all four file diffs; cross-referenced against `<action>` block of Task 1; ran `cargo test --lib cli::render_json` (8 passing) + `cargo test --lib cli::tests::dispatch_outcome` (4 passing) + full `cargo test` (no regressions). Confirmed the partial work implements Task 1 verbatim per plan.
- **Action:** Staged + committed the four files atomically as Task 1 (`bdf9583`). No revisions needed.

### Auto-fixed Issues

**1. [Rule 1 - Bug] Test fixture `generated_at must be a string` assertion contradicted the locked DTO encoding**
- **Found during:** Task 2 first integration-test run (`cargo test --test json_format_round_trip`).
- **Issue:** The C7 test asserted `obj["generated_at"].as_str().is_some()` expecting an RFC3339 string. But the locked DTO uses `#[serde(with = "jiff::fmt::serde::timestamp::second::required")]` which emits a Unix epoch integer (e.g. `1779686233`). The CONTEXT D-50 example showed RFC3339 illustratively but the actual code matches Phase 0's existing `fetched_at` / `resets_at` adapter. This is the LOCKED v1 contract.
- **Fix:** Test now asserts `obj["generated_at"].as_u64().is_some()` with a doc-comment explaining the integer-epoch contract per D-52. Documented in C7 test comment so future readers don't re-trip on the CONTEXT vs RESEARCH discrepancy.
- **Files modified:** `tests/json_format_round_trip.rs` (one assertion + 5 lines of comment).
- **Verification:** C7 + other 4 json_format tests all pass.
- **Committed in:** `09549da` (Task 2 commit).

**2. [Rule 1 - Bug] Two pre-Phase-2 integration tests asserted ".success()" on scenarios that are now exit-1 per the locked D-59 grid**
- **Found during:** Task 2 full `cargo test` run.
- **Issue:**
  - `tests/cli_walking_skeleton.rs::ahb_with_broken_claude_config_prints_error_row_not_crash` set up Claude-only-enabled + no `~/.claude/projects` dir → Claude returns `Unavailable` → AllFailed → exit 1. Test asserted `.success()`.
  - `tests/schema_drift_sentinel.rs::drift_in_recent_assistants_triggers_sentinel_literal` set up Claude-only-enabled with drifted JSONL → Claude returns `SchemaDrift` (Err per D-60) → AllFailed → exit 1. Test asserted `.success()`.
- **Fix:** Both tests reworked to assert `output.status.code() == Some(1)`. All other invariants (row shape, sentinel literal, U+2592 byte count, next-step-hint) preserved. Each test gains an inline comment explaining the D-59 / D-60 binding.
- **Files modified:** `tests/cli_walking_skeleton.rs`, `tests/schema_drift_sentinel.rs`.
- **Verification:** Both tests pass; the row shape / sentinel literal assertions still fire.
- **Committed in:** `09549da` (Task 2 commit).
- **Rationale (not a scope-creep):** The new D-59 exit-code grid is a Phase 2 deliverable that necessarily changes existing test assumptions. Without updating these two tests, the new contract cannot ship cleanly. The Rule 1 fix is narrowly scoped — only the exit-code assertion is touched; the user-visible row contracts are preserved byte-identically.

---

**Total deviations:** 1 resume inspection + 2 Rule 1 fixes (1 test-fixture vs locked DTO + 1 cross-test exit-code-contract update).
**Impact on plan:** None — both Rule 1 fixes were necessary correctness adjustments and stayed narrowly scoped. No architectural decisions required user input (no Rule 4 trigger). The plan executed end-to-end as written.

## Issues Encountered

- **CONTEXT D-50 example shows `generated_at` as an RFC3339 string (`"2026-05-25T13:45:22Z"`), but the locked DTO uses Phase 0's integer-epoch jiff adapter.** Surfaced during C7 test failure. The locked code matches the verbatim DTO definitions from RESEARCH §--json schema_version: 1 lines 583-633 (which use the integer-epoch adapter consistent with the rest of the model). The CONTEXT example was illustrative pseudo-JSON, not a binding shape. Test assertion updated; v1 schema commits to integer epoch — consumers using `jq` get RFC3339 cheaply via `from_unixtime`. Tracked in Decisions Made #1.
- **No human checkpoints triggered.** Both tasks were autonomous-mode-eligible per the plan's `<task type="auto">` markers; no Rule 4 architectural decisions arose.

## Threat Flags

No new security-relevant surface introduced. The threat register entries T-02-01 (Secret leak via run_json) and T-02-02 (anyhow chain expansion in JsonError.message) are both verified as `mitigate` by the inline SEC binding rustdoc + the new `subprocess_json_path_redacts_secret` test (T-02-01) and by the grep gate on `format!(...:?})` patterns in `src/cli/render_json.rs` (T-02-02).

## Known Stubs

None. Every plan deliverable is wired end-to-end — no placeholders, no TODOs, no empty-data UI components.

## Compact-Line + Detailed Phase 1/2-02 Byte-Identity Proof

The new ArgGroup wraps the existing `Cli.detailed` field; the no-flag default path still routes through `run_compact` (`else` arm in main.rs dispatch). Plan 01 `tests/cli_walking_skeleton.rs::ahb_default_run_emits_one_claude_row_with_real_numbers` continues to pass (the test does not supply any of the three format flags, so the default compact path is exercised). Plan 02-02 `tests/detailed_format.rs` continues to pass (D7 + D8 still green). The 178-test full-suite run is the canonical proof of zero regression.

## Self-Check: PASSED

**Files created — verified exist:**
- `src/cli/render_json.rs` — FOUND
- `tests/json_format_round_trip.rs` — FOUND
- `tests/exit_codes.rs` — FOUND

**Files modified — verified exist:**
- `src/cli/mod.rs` — FOUND (DispatchOutcome at line ~34; compact/json fields + ArgGroup + after_help; debug_emit_fake_secret_and_exit gains as_json: bool)
- `src/cli/render_text.rs` — FOUND (format_one_line promoted to pub(crate))
- `src/main.rs` — FOUND (cli.json branch + std::process::exit(outcome.exit_code()))
- `tests/secret_leak_subprocess.rs` — FOUND (new subprocess_json_path_redacts_secret)
- `tests/cli_walking_skeleton.rs` — FOUND (exit-1 assertion in broken-config test)
- `tests/schema_drift_sentinel.rs` — FOUND (exit-1 assertion per D-60)

**Commits — verified in git log:**
- bdf9583 (Task 1: `feat(02-03): add render_json DTOs + DispatchOutcome (Task 1)`) — FOUND
- 09549da (Task 2: `feat(02-03): wire ArgGroup + exit codes + SEC-03 --json route (Task 2)`) — FOUND

**Final cargo test count:** 178 passing (148 lib + 30 integration), 0 failing.
**Final smoke proof:**
- `AHB --help` shows the `Exit codes:` block listing 0 / 1 / 2.
- `AHB --compact --json` / `AHB --compact --detailed` / `AHB --detailed --json` all exit 2 with `error: the argument '--X' cannot be used with '--Y'` on stderr.
- `AHB --json` with zero providers prints `{"schema_version":1,"generated_at":<epoch>,"providers":[]}` and exits 0.
- `--json` route SEC-03 test (`subprocess_json_path_redacts_secret`) passes — `[REDACTED]` present, fixture absent, no 20+-char run.

---

*Phase: 02-codex-output-formats*
*Plan: 03 — JSON schema, ArgGroup, exit codes, SEC-03 extension*
*Completed: 2026-05-25*
