---
phase: 02-codex-output-formats
verified: 2026-05-25T12:30:00Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
---

# Phase 2: Codex + Output Formats — Verification Report

**Phase Goal:** Add a second real provider (Codex) using the new `spawn_blocking` + read-only SQLite pattern, and lock down all CLI output formats — `--compact`, `--detailed`, `--json` with stable `schema_version: 1`, plus exit codes — before any tmux / Starship user builds on AHB's output.

**Verified:** 2026-05-25T12:30:00Z
**Status:** passed
**Re-verification:** No — initial verification (post REVIEW fix landing)

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Codex adapter reads `~/.codex/state_*.sqlite` read-only with `busy_timeout`, prefers JSONL rollouts, `rate_limits: null` → unknown; verified by integration test that doesn't crash on locked DB | VERIFIED | `src/provider/codex/sqlite.rs:89-102` uses `OpenFlags::SQLITE_OPEN_READ_ONLY \| OpenFlags::SQLITE_OPEN_NO_MUTEX` + `busy_timeout(250ms)` + zero `SELECT` queries. `src/provider/codex/jsonl.rs:140-219` parses rollouts and maps `rate_limits: null` → `ProviderError::SchemaDrift { missing: ["rate_limits"] }` (D-47). `tests/codex_sqlite_lock_resilience.rs` (1 test) passed in 0.06s — confirms adapter doesn't crash, hang > 1.5s, or report "database is locked" when a parallel writer holds RESERVED lock. |
| 2 | `AHB --compact` forces single-line; `AHB --detailed` prints multi-line per-provider rows showing session+weekly bars; both work whether stdout TTY or piped | VERIFIED | `--compact` flag wired at `src/cli/mod.rs:89-90` (no-op alias to default per design). `--detailed` flag at `src/cli/mod.rs:95-96`, dispatched via `run_detailed` at `src/cli/mod.rs:196-240`. `src/cli/render_text.rs::detailed_block` emits header + indented per-window rows. Claude emits 2 windows (`5h` + `weekly`) via `src/provider/claude/mod.rs` + `src/provider/claude/window.rs::compute_weekly_window`. Integration: `tests/detailed_format.rs` (2 tests) green; `tests/cli_walking_skeleton.rs` proves piped + NO_COLOR strips ANSI. |
| 3 | `AHB --json` emits `schema_version: 1` document; round-trips through `jq` (no ANSI, no escape leakage); secret-shape strings never appear regardless of input | VERIFIED | `src/cli/render_json.rs` defines locked v1 DTO (JsonRoot/JsonProvider/JsonWindow/JsonError) with `SCHEMA_VERSION: u8 = 1` at line 60. `run_json` at line 252 emits compact JSON via `serde_json::to_writer`. Direct smoke test produced `{"schema_version":1,"generated_at":1779689505,"providers":[]}` exit 0. `tests/json_format_round_trip.rs` (5 tests) green — schema_version, BL-02 ordering, zero-ANSI assertion `!stdout.contains("\x1b[")`, zero-providers branch, ascii/color silently ignored. `tests/secret_leak_subprocess.rs::subprocess_json_path_redacts_secret` passes (fixture absent, no 20+-char alnum run, `[REDACTED]` present). |
| 4 | Exit codes 0/1/2 work: 0 ≥1 provider Ok, 1 all fail, 2 config/secrets unloadable; `--help` documents them; `NO_COLOR` + `--color=auto\|always\|never` honored | VERIFIED | `DispatchOutcome` enum at `src/cli/mod.rs:34-61` with `from_results` discriminant and `exit_code()` mapper. `src/main.rs:119` calls `std::process::exit(outcome.exit_code())`. clap `ArgGroup` at `src/cli/mod.rs:77-82` rejects flag conflicts with exit 2. `tests/exit_codes.rs` (7 tests, includes CR-01 regression) green: C1 (any Ok→0), C2 (all err→1), C3 (zero providers→0), C4-C6 (flag conflicts→2), C7 (Gemini-only→1 post-CR-01). `--help` after_help block at `src/cli/mod.rs:76` documents all 3 exit codes — verified via `target/debug/ahb --help` output showing `Exit codes:\n  0  …\n  1  …\n  2  …`. `--color=auto\|always\|never` enum at `src/cli/mod.rs:110-111`; NO_COLOR honored via `src/cli/tty.rs::should_colorize_env`. |

**Score:** 4/4 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/provider/codex/mod.rs` | CodexProvider + Provider impl + spawn_blocking wrap | ✓ VERIFIED | 245 lines; `impl Provider for CodexProvider` at line 56; spawn_blocking wraps SQLite+JSONL parse at line 90; 4 Provider tests (test_a..test_d) cover empty dir / sqlite-only / valid rollout / null rate_limits |
| `src/provider/codex/jsonl.rs` | parse_codex_rollout_windows + serde structs | ✓ VERIFIED | `parse_codex_rollout_windows` at line 140; full serde struct ladder (RolloutLine/RolloutPayload/TokenCountPayload/RateLimits/RateLimitTier); inline tests cover skip-malformed (D-35), latest-wins, null→SchemaDrift |
| `src/provider/codex/sqlite.rs` | discover_state_sqlite (D-46 version sort) + open_readonly | ✓ VERIFIED | `discover_state_sqlite` at line 33 with integer suffix sort + multi-version warn; `open_readonly` at line 89 with READ_ONLY+NO_MUTEX+busy_timeout(250ms); tests prove highest-N pick + state_10>state_5 ordering |
| `src/provider/codex/window.rs` | RateLimitTier → HpWindow with line_ts anchor (WR-02 fixed) | ✓ VERIFIED | `to_hp_windows` at line 26 passthrough-order primary/secondary; line_ts anchor (NOT ctx.now); post-WR-02 fix clamps absurd `resets_in_seconds` to `JIFF_SECONDS_MAX` with `tracing::warn!` to prevent panic |
| `src/provider/gemini.rs` | GeminiUnimplementedProvider (CR-01 fix) | ✓ VERIFIED | 88 lines; returns `Err(Unavailable { reason: "Gemini provider is not yet implemented (Phase 3) — set [providers.gemini].enabled = false to suppress this row" })`; tests verify id + next-step hint |
| `src/cli/render_json.rs` | JsonRoot/JsonProvider/JsonWindow/JsonError DTOs + to_json_root + run_json | ✓ VERIFIED | 499 lines; locked v1 DTOs at lines 62-118; `to_json_root` at line 129; `run_json` at line 252; 8 inline tests (J1-J8) cover empty/Ok/Err/round-trip/Internal-Display-only/RateLimited-seconds/Mock-id/detailed_label-skip |
| `src/cli/render_text.rs::detailed_block` | Multi-line per-provider block | ✓ VERIFIED | `detailed_block` exists; `effective_label` helper uses `detailed_label.unwrap_or(label)`; NaN sentinel branch + `(limit unknown)` suffix; compact_line_colored ALSO got NaN guard per WR-01 fix |
| `src/cli/mod.rs::Cli` + ArgGroup + DispatchOutcome | Full clap interlock + exit-code enum | ✓ VERIFIED | `Cli` struct (lines 84-124) with all three `compact/detailed/json` `pub bool` fields wrapped in `ArgGroup::new("format").required(false).multiple(false)`; `after_help` documents exit codes 0/1/2; `DispatchOutcome` enum + `from_results` + `exit_code()` at lines 34-61; `debug_emit_fake_secret_and_exit(as_json: bool)` at line 275 (D-62 SEC-03 extension) |
| `src/main.rs` | Dispatch with std::process::exit per D-59 | ✓ VERIFIED | `cli.json` branch at line 110; `cli.detailed` at 112; `std::process::exit(outcome.exit_code())` at line 119; config/secrets unloadable exits 2 at line 87 (Phase 1 unchanged) |
| `tests/codex_sqlite_lock_resilience.rs` | Pitfall 3 RESERVED-lock guard | ✓ VERIFIED | 1 test passing in 0.06s — well under 1.5s ceiling |
| `tests/detailed_format.rs` | --detailed shape integration | ✓ VERIFIED | 2 tests passing |
| `tests/exit_codes.rs` | D-59 grid coverage | ✓ VERIFIED | 7 tests passing (C1-C6 + C7 CR-01 regression) |
| `tests/json_format_round_trip.rs` | CORE-04 + after_help docs | ✓ VERIFIED | 5 tests passing (schema_version, zero-ANSI, zero-providers, ascii/color-ignored, help block) |
| `tests/secret_leak_subprocess.rs::subprocess_json_path_redacts_secret` | SEC-03 over --json route | ✓ VERIFIED | Both `subprocess_secret_does_not_leak` and `subprocess_json_path_redacts_secret` pass |
| `Cargo.toml` rusqlite 0.39 bundled | New dep added | ✓ VERIFIED | Present; `cargo tree -i libsqlite3-sys` returns single version v0.37.0 — only rusqlite-bundled consumer |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|----|--------|---------|
| `src/engine/mod.rs` | `CodexProvider` | registration when `cfg.providers.codex.enabled` | ✓ WIRED | Line 67: `providers.push(Arc::new(crate::provider::codex::CodexProvider::new(&home)))` mirrors Claude branch's HOME resolution |
| `src/engine/mod.rs` | `GeminiUnimplementedProvider` | registration when `cfg.providers.gemini.enabled` (CR-01) | ✓ WIRED | Lines 69-78: post-CR-01 fix pushes Arc<GeminiUnimplementedProvider> instead of silent debug log |
| `src/provider/codex/mod.rs` | `ctx.now` / `line.timestamp` | anchor for resets_at math | ✓ WIRED | `fetched_at: ctx.now` at line 117; `to_hp_windows` uses `line_ts` not ctx.now per RESEARCH §Codex JSONL Schema bullet 3; `tests/no_walltime_in_adapter` continues to pass |
| `src/cli/render_text.rs::format_error_row_colored` | `id_label_titlecase(id)` | generalized SchemaDrift sentinel | ✓ WIRED | Line 189-192: `format!("{label_titlecased} adapter may be out-of-date", label_titlecased = id_label_titlecase(id))`; Claude byte-identical to Phase 1 per `tests/schema_drift_sentinel.rs` |
| `src/cli/render_json.rs::to_json_root` | engine `refresh_all` results | (ProviderId, Result<...>) iteration | ✓ WIRED | Lines 129-159: iterates `results` in caller-supplied (BL-02) order, builds JsonProvider per Ok/Err arm |
| `src/cli/mod.rs::Cli` | clap `ArgGroup` `multiple=false` | format flag interlock | ✓ WIRED | Lines 77-82: `ArgGroup::new("format").required(false).multiple(false).args(["compact", "detailed", "json"])` — exit-2 on conflict verified by C4/C5/C6 |
| `src/main.rs` | `std::process::exit` | DispatchOutcome mapping per D-59 | ✓ WIRED | Line 119: `std::process::exit(outcome.exit_code())` |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `--help` shows exit code section | `target/debug/ahb --help` | Output ends with `Exit codes:\n  0  at least one provider returned data (or no providers configured)\n  1  all configured providers failed\n  2  config / secrets unloadable, or invalid command-line usage` | ✓ PASS |
| `--json` with zero providers emits empty array | `HOME=<tmp> XDG_CONFIG_HOME=<tmp>/cfg AHB_SECRETS_MOCK=1 NO_COLOR=1 target/debug/ahb --json` | `{"schema_version":1,"generated_at":1779689505,"providers":[]}` exit=0 | ✓ PASS |
| Gemini-only config exits 1 with error row (CR-01) | Same env with `[providers.gemini] enabled=true` | `{"schema_version":1,...,"providers":[{"id":"gemini","status":"error",...,"error":{"kind":"unavailable","message":"Gemini provider is not yet implemented (Phase 3) ..."}}]}` exit=1 | ✓ PASS |
| Full lib test suite | `cargo test --lib` | `156 passed; 0 failed` | ✓ PASS |
| Phase 2 integration tests | `cargo test --test exit_codes --test json_format_round_trip --test secret_leak_subprocess --test codex_sqlite_lock_resilience --test detailed_format` | `7 + 5 + 2 + 1 + 2 = 17 passed; 0 failed` | ✓ PASS |
| Full test suite (all integration + lib) | `cargo test` | All test binaries: `ok` with 0 failures | ✓ PASS |
| No walltime in adapters | `grep -rn 'Timestamp::now' src/provider/ src/tui/widgets/` | 3 matches all in `///` doc comments (no code) | ✓ PASS |
| Single libsqlite3-sys version | `cargo tree -i libsqlite3-sys` | `libsqlite3-sys v0.37.0` → rusqlite v0.39.0 → ahb (only consumer) | ✓ PASS |

### Probe Execution

N/A — phase has no `scripts/*/tests/probe-*.sh` files declared in PLANs or shipped in repo (project uses `cargo test` as canonical probe).

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| ADP-04 | 02-01-PLAN | Codex CLI adapter — read-only state_*.sqlite + busy_timeout + JSONL rollouts; rate_limits null = unknown | ✓ SATISFIED | `src/provider/codex/*` end-to-end; tests test_a..test_d + lock-resilience |
| CORE-03 | 02-02-PLAN | `AHB --detailed` multi-line per provider, session+weekly bars | ✓ SATISFIED | `run_detailed` + `detailed_block` + Claude weekly window infrastructure; `tests/detailed_format.rs` |
| CORE-02 | 02-03-PLAN | `AHB --compact` forces compact output | ✓ SATISFIED | `Cli.compact: bool` flag; falls through to `run_compact` (same as default); ArgGroup interlock |
| CORE-04 | 02-03-PLAN | `AHB --json` schema_version:1, safe for tmux/Starship | ✓ SATISFIED | `src/cli/render_json.rs` v1 DTO; `tests/json_format_round_trip.rs` (5 tests) |
| CORE-06 | 02-03-PLAN | Exit codes 0/1/2 | ✓ SATISFIED | `DispatchOutcome` + `main.rs::std::process::exit`; `tests/exit_codes.rs` (7 tests) |
| SEC-03 | 02-03-PLAN | `--json`/log/error never expose secrets, CI grep test | ✓ SATISFIED | `tests/secret_leak_subprocess.rs::subprocess_json_path_redacts_secret` + sibling test |

All 6 declared requirement IDs satisfied. REQUIREMENTS.md traceability table confirms Phase 2 maps to CORE-02/03/04/06 + SEC-03 + ADP-04 — all marked Complete. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | — | — | — | No debt markers (TBD/FIXME/XXX), unreferenced TODOs, or empty stubs in Phase-2 modified files. The codex `// TODO Phase 3: consider BTreeMap…` in `jsonl.rs:69` is a forward-pointing design note (Phase 3 scope), not a debt marker. The pre-existing REVIEW report flagged 1 Critical + 6 Warnings; all 7 are fixed in commits b834b57..6d074d4 (CR-01 + WR-01..WR-06). |

The REVIEW INFO items (IN-01..IN-05) are explicitly cosmetic / non-blocking per the REVIEW classification — code duplication awaiting third caller (Rule of Three), comment freshness, etc. No blocker or warning anti-patterns introduced.

### Human Verification Required

None — every must-have is verified programmatically:
- Behavior is verified via running tests (175+ tests, 0 failures) and smoke commands (`--help`, `--json`, gemini-only exit-1 regression)
- Output format invariants are verified by grep/round-trip assertions in integration tests
- No visual / TTY / real-time / external-service test paths remain unproven

### Gaps Summary

None. All 4 ROADMAP success criteria and all 6 declared requirements are satisfied with running test evidence. The 1 Critical (CR-01) + 6 Warnings (WR-01..WR-06) flagged by the standalone REVIEW were closed in commits b834b57..6d074d4 BEFORE this verification ran, and the corresponding fixed code paths are exercised by the integration tests (Gemini-only exit-1 by `tests/exit_codes.rs::exit_code_1_when_only_gemini_enabled`; NaN compact guard by render_text test path; format_one_line trim, BAR_WIDTH dedup, ASCII sentinel honor, no-op color_ignored removal by cargo test passing).

---

_Verified: 2026-05-25T12:30:00Z_
_Verifier: Claude (gsd-verifier)_
