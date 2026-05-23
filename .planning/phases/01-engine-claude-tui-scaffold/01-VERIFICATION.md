---
phase: 01-engine-claude-tui-scaffold
verified: 2026-05-23T10:00:00Z
status: human_needed
score: 5/5 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 4/5
  gaps_closed:
    - "BL-01: hp_row.rs::build_ok_line no longer calls jiff::Timestamp::now() directly; AppState.now field carries the snapshot; tui_loop render-tick arm is the single authorized wall-clock site; no_walltime_in_adapter.rs scope extended to src/tui/widgets/"
    - "BL-02: Engine::refresh_all sorts results by canonical ProviderId order via sort_by_key(Self::sort_key(id)) — Claude=0, Codex=1, Gemini=2, Mock=3; tests/engine_row_order.rs proves Claude appears before Mock even when Mock's fetch completes first"
    - "BL-03: window.rs::find_active_cluster uses gap.total(jiff::Unit::Second) > FIVE_HOURS_SECS strict-greater total-seconds comparison; three boundary-locking tests (4h59m30s no-split, exactly-5h no-split, 5h0m30s split) all pass"
    - "WR-06: D-41 error message resolves cross-OS config path via config::default_path().ok().map_or_else(...) with 'your AHB config file' fallback; TODO(future-phase) doc comment preserves [secrets].storage = 'file' contract"
    - "WR-08: run_tui_stub fully removed from src/cli/mod.rs; grep across src/ and tests/ returns zero callers"
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "Run `./target/release/ahb tui` (with AHB_SECRETS_MOCK=1 on backend-less hosts) with `[providers.claude] enabled = true` in config. Observe: bordered frame titled ` AHB ` (leading + trailing space); one claude row with 10 bar cells + percent + bullet + `resets in Xh{MM}m`; quit hint `q quit  ·  ctrl-c quit` in DarkGray at bottom; q/Ctrl-C returns terminal cleanly."
    expected: "Bordered full-screen frame with correct layout, provider row, and quit behavior matching UI-SPEC."
    why_human: "Color threshold validation (Green/Yellow/Red per 30/10 thresholds), visual alignment, interactive quit feel, and stty -a terminal state verification require a real TTY and human judgment."
  - test: "Observe bar fill color across percent ranges: >=30% green, 10-30% yellow, <10% red; empty cells always DarkGray."
    expected: "UI-SPEC 60/30/10 color thresholds rendered correctly."
    why_human: "Color rendering requires visual inspection in a real terminal."
  - test: "Run `cargo test --test keyring_init_sanity` on macOS and Windows (not just Linux)."
    expected: "InitOutcome::Ready(_) — OS keyring backend registers successfully; first macOS run may show Keychain access prompt."
    why_human: "Dev machine is Linux with dbus unavailable; AHB_SECRETS_MOCK=1 masks the real backend path in CI; platform-specific keyring behavior requires manual testing on each OS."
---

# Phase 01: Engine + Claude + TUI Scaffold — Verification Report (Re-verification)

**Phase Goal:** Make `AHB` and `AHB tui` work end-to-end against a real Claude Code subscription, with keyring-backed secrets, panic-safe terminal restore, and per-adapter error isolation wired in BEFORE feature code so the foundation is correct from day one.
**Verified:** 2026-05-23T10:00:00Z
**Status:** human_needed
**Re-verification:** Yes — after gap closure (Plan 01-04 closed BL-01/BL-02/BL-03/WR-06/WR-08; CR-01 and IN-01 also landed)

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | `AHB` prints real Claude HP bar from `~/.claude/projects/**/*.jsonl`; `AHB \| cat` emits no ANSI | ✓ VERIFIED | `cli_walking_skeleton.rs` tests synthetic JSONL with `cache_creation_input_tokens`; `should_colorize_env` wired in `run_compact`; `!stdout.contains("\x1b[")` assertion passes |
| 2 | `AHB tui` opens fixed-frame, auto-refreshes 15s, restores terminal on quit/panic (integration test) | ✓ VERIFIED | BL-01: `hp_row.rs` no longer calls `Timestamp::now()` (0 hits grep); `AppState.now` field at line 46; `tui_loop` sets `app.now` at lines 120 and 153; BL-02: `Engine::refresh_all` sorts via `sort_by_key(Self::sort_key(id))` at line 103; `tests/engine_row_order.rs` proves canonical ordering; BL-03: `gap.total(jiff::Unit::Second) > FIVE_HOURS_SECS` strict-greater at line 67; three boundary tests pass; `tui_panic_safe_restore.rs` and `tui_non_tty_refusal.rs` still green |
| 3 | Broken provider -> ERROR row; Claude stays healthy; TUI non-TTY -> exit 2 with literal | ✓ VERIFIED | `panic_isolation.rs` asserts claude row + mock ERROR row + `ahb panicked:` prefix; `tui_non_tty_refusal.rs` asserts exit 2 + verbatim UI-SPEC literal; CR-01: mock.rs panic block wrapped in `#[cfg(debug_assertions)]` at line 34 |
| 4 | TOML config cross-OS; un-configured providers silently skipped; `Secret<T>` redacts in Debug/Serialize; CI grep test | ✓ VERIFIED | `ProjectDirs::from("", "", "ahb")` in `config.rs`; `LoadOutcome::Initialized` on first run; `Secret<T>::Debug` -> `"***"`, `Serialize` -> `"[REDACTED]"`; `tests/secret_leak.rs` + `tests/secret_leak_subprocess.rs` double-assert; `keyring-core = "1"` in Cargo.toml |
| 5 | Claude JSONL schema drift -> visible "Claude adapter may be out-of-date" sentinel | ✓ VERIFIED | `detect_drift()` in `jsonl.rs`; `fetch()` calls it before cluster math; `format_error_row_colored` emits U+2592 sentinel; `schema_drift_sentinel.rs` integration test |

**Score:** 5/5 truths verified

### Required Artifacts (Gap-closure files — re-verified at all levels)

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/tui/app.rs` | `pub now: jiff::Timestamp` field on AppState | ✓ VERIFIED | Line 46: `pub now: jiff::Timestamp` — `Default` removed; constructor takes seed snapshot |
| `src/tui/mod.rs` | `app.now = jiff::Timestamp::now()` at priming draw (IN-01) AND render-tick arm | ✓ VERIFIED | Line 120: priming draw update (IN-01 fix); line 153: render-tick arm update — both BEFORE `terminal.draw(...)` |
| `src/tui/widgets/hp_row.rs` | `now: &jiff::Timestamp` on `build_ok_line` / `build_line` / `render`; zero `Timestamp::now()` calls | ✓ VERIFIED | 3 occurrences of `now: &jiff::Timestamp` confirmed; `grep -n 'jiff::Timestamp::now' src/tui/widgets/hp_row.rs` returns 0 lines |
| `src/tui/ui.rs` | `&app.now` plumbed into `hp_row::render` call | ✓ VERIFIED | Line 97: `hp_row::render(*area, f.buffer_mut(), &app.rows[i], false, &app.now)` |
| `src/engine/mod.rs` | `sort_by_key(Self::sort_key(id))` in `refresh_all`; `sort_key` fn with canonical mapping | ✓ VERIFIED | Line 103: `results.sort_by_key(...)` ; line 110-115: `sort_key` fn with Claude=0, Codex=1, Gemini=2, Mock=3 |
| `src/provider/claude/window.rs` | `FIVE_HOURS_SECS` const; `gap.total(Unit::Second) > FIVE_HOURS_SECS` strict-greater; no `get_hours()` | ✓ VERIFIED | 3 hits for `FIVE_HOURS_SECS` (doc, const def, use); line 67: `if gap_secs > FIVE_HOURS_SECS`; 0 hits for `get_hours()`, `>= 5`, `jiff::Unit::Hour` |
| `src/main.rs` | `config::default_path()` in D-41 error message; `"your AHB config file"` fallback; `TODO(future-phase)` | ✓ VERIFIED | Line 78-79: `config::default_path().ok().map_or_else(...)` with fallback; line 68: `TODO(future-phase)` doc comment present |
| `src/cli/mod.rs` | `run_tui_stub` fully deleted; 0 callers | ✓ VERIFIED | `grep -c 'run_tui_stub' src/cli/mod.rs` = 0; `grep -rn 'run_tui_stub' src/ tests/` = 0 callers |
| `tests/no_walltime_in_adapter.rs` | Scans BOTH `src/provider/` AND `src/tui/widgets/`; `src/tui/mod.rs` remains authorized | ✓ VERIFIED | Line 33: `let scan_dirs: [&str; 2] = ["src/provider", "src/tui/widgets"]`; both subtrees in grep output |
| `tests/engine_row_order.rs` | Integration test proving Claude appears before Mock when both enabled | ✓ VERIFIED | File exists; contains `ProviderId::Claude` assertion at line 87; test passes in full suite |
| `src/provider/mock.rs` | `AHB_DEBUG_PANIC` block wrapped in `#[cfg(debug_assertions)]` (CR-01) | ✓ VERIFIED | Line 34: `#[cfg(debug_assertions)]` gate on panic injection block |

### Key Link Verification (Gap-closure re-verification)

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `src/tui/mod.rs::tui_loop` | `jiff::Timestamp::now()` | priming draw (line 120) AND render-tick arm (line 153) — ONLY authorized TUI wall-clock sites | ✓ WIRED | Both hits confirmed; no other `Timestamp::now` calls in TUI layer |
| `src/tui/widgets/hp_row.rs::build_ok_line` | `format_countdown` | `now: &jiff::Timestamp` parameter from caller (NOT local `Timestamp::now` call) | ✓ WIRED | `format_countdown(now, &w.reset.resets_at)` confirmed; 0 forbidden `Timestamp::now` calls |
| `src/tui/ui.rs::draw` | `widgets::hp_row::render` | `&app.now` passed as 5th argument | ✓ WIRED | Line 97 confirmed |
| `src/engine/mod.rs::refresh_all` | `ProviderId` discriminant ordering | `sort_by_key(|(id, _)| Self::sort_key(*id))` after fanout await | ✓ WIRED | Line 103; sort is ONLY at engine boundary — fanout, CLI, TUI do not re-sort (all three verified at 0 sort calls) |
| `src/provider/claude/window.rs::find_active_cluster` | `jiff::Span::total(Unit::Second)` | `gap.total(jiff::Unit::Second) > FIVE_HOURS_SECS` strict-greater comparison | ✓ WIRED | Line 64 and 67 confirmed; `(Unit::Hour, _)` constraint removed |
| `src/main.rs` | `config::default_path` | D-41 error message path resolution with graceful fallback | ✓ WIRED | Lines 78-79 confirmed; hardcoded `~/.config/ahb/config.toml` absent |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `src/tui/widgets/hp_row.rs` | `countdown` (via `format_countdown`) | `now: &jiff::Timestamp` parameter from `AppState.now`, set by `tui_loop` before each draw | Yes — injectable; snapshot from authorized callsite | ✓ FLOWING |
| `src/provider/claude/mod.rs` | `used_tokens` | `cache_creation_input_tokens` from JSONL | Yes — real file data | ✓ FLOWING |
| `src/cli/mod.rs::run_compact` | `results` Vec | `engine.refresh_all(now).await` returning sorted canonical-order Vec | Yes — real engine results in canonical order | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `AHB \| cat` emits zero ANSI bytes | `tests/cli_walking_skeleton.rs::ahb_piped_stdout_contains_no_ansi_escapes` | `!stdout.contains("\x1b[")` assertion — test passes | ✓ PASS |
| Schema-drift sentinel renders U+2592 | `tests/schema_drift_sentinel.rs` | `>=10 U+2592 bytes` assertion — test passes | ✓ PASS |
| Panic isolation: claude healthy, mock ERROR | `tests/panic_isolation.rs` | claude row present, mock ERROR row present, `ahb panicked:` in stderr — test passes | ✓ PASS |
| TUI non-TTY refusal | `tests/tui_non_tty_refusal.rs` | exit 2 + verbatim UI-SPEC literal — test passes | ✓ PASS |
| TUI panic-safe restore (real-pty) | `tests/tui_panic_safe_restore.rs` | 1049h enter + 1049l leave before exit — test passes (Unix-gated) | ✓ PASS |
| Secret<T> does not leak in Debug or Serialize | `tests/secret_leak.rs` + `tests/secret_leak_subprocess.rs` | literal absent + 20-char regex absent — tests pass | ✓ PASS |
| Engine row order: Claude before Mock regardless of completion order | `tests/engine_row_order.rs` | `results[0].0 == ProviderId::Claude`, `results[1].0 == ProviderId::Mock` — test passes | ✓ PASS |
| BL-03 boundary: 4h59m30s no-split, exactly-5h no-split, 5h0m30s split | `window::tests::gap_just_under/exactly/just_over_5h` | all three boundary assertions pass | ✓ PASS |
| Full suite | `cargo test --workspace` | 103 passed, 0 failed, 0 ignored | ✓ PASS |
| Release build | `cargo build --release` | exit 0 | ✓ PASS |
| Clippy clean | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 | ✓ PASS |

### Probe Execution

Step 7c: SKIPPED — no conventional `scripts/*/tests/probe-*.sh` probes exist in this project.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| CORE-01 | Plan 01-01 | `AHB` no-args prints compact HP bar | ✓ SATISFIED | `run_compact` iterates sorted engine results; `cli_walking_skeleton.rs` integration test |
| CORE-05 | Plan 01-01 | Non-TTY / NO_COLOR -> no ANSI | ✓ SATISFIED | `should_colorize_env` wired in `run_compact`; ANSI assertion passes |
| TUI-01 | Plan 01-03 | `AHB tui` fixed-screen per-provider HP bar | ✓ SATISFIED | Full TUI module group; `ui::draw` with `Borders::ALL` + provider rows; BL-01 now clean |
| TUI-02 | Plan 01-03 | 15s auto-refresh | ✓ SATISFIED | `Duration::from_secs(15)` fetch_tick in `tui_loop` |
| TUI-04 | Plan 01-03 | panic-safe terminal restore (integration test) | ✓ SATISFIED | `tests/tui_panic_safe_restore.rs` real-pty test asserts 1049h/1049l; `ratatui::run` auto-installs panic hook |
| TUI-05 | Plan 01-03 | TUI non-TTY -> clear error + exit | ✓ SATISFIED | `is_terminal` gate; verbatim UI-SPEC literal; `tests/tui_non_tty_refusal.rs` exit 2 |
| CFG-01 | Plan 01-01 | TOML config with provider enable/disable | ✓ SATISFIED | `Config/Providers/ProviderConfig` structs; embedded template |
| CFG-02 | Plan 01-01 | Cross-platform config path via `directories` | ✓ SATISFIED | `ProjectDirs::from("", "", "ahb")` in `config.rs`; WR-06 also fixed D-41 cross-OS error message |
| CFG-04 | Plan 01-01 | Unconfigured providers silently skipped | ✓ SATISFIED | Engine only pushes providers with `enabled = true`; `LoadOutcome` enum |
| SEC-01 | Plan 01-02 | OS keyring via `keyring-core` 1.0 | ✓ SATISFIED | `set_default_store` wired in `secrets::init()`; `keyring-core = "1"` in Cargo.toml; `keyring = ` v4 absent |
| SEC-02 | Plan 01-02 | `Secret<T>` newtype with Debug redact | ✓ SATISFIED | `Debug` -> `"***"`, `Serialize` -> `"[REDACTED]"`, no `Deserialize`; double-assert tests |
| SEC-04 | Plan 01-01 | No-secret provider uses same interface contract | ✓ SATISFIED | `ClaudeProvider::fetch` receives `&FetchCtx` with `&Secrets`; `_ = ctx.secrets` pattern preserved |
| ADP-01 | Plan 01-01/02 | Adapter failure isolated; no crash or blank | ✓ SATISFIED | JoinSet + `is_panic()` panic recovery; `panic_isolation.rs` verifies end-to-end; CR-01 gates debug-panic behind `#[cfg(debug_assertions)]` |
| ADP-02 | Plan 01-01 | Claude JSONL adapter with 5h cluster | ✓ SATISFIED | `discover_session_files`, `read_assistant_entries`, `find_active_cluster`; BL-03 fixes boundary semantics to strict-greater total-seconds |
| ADP-03 | Plan 01-02 | Schema-drift sentinel | ✓ SATISFIED | `detect_drift` (raw Value re-parse), `ProviderError::SchemaDrift`, U+2592 sentinel in CLI + TUI |

All 15 Phase 1 requirements are satisfied. No orphaned requirements detected.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None | — | No debt markers (TBD/FIXME/XXX), no forbidden clock calls, no dead stubs, no hardcoded paths | — | — |

All three prior BLOCKER-tier defects (BL-01, BL-02, BL-03) and two WARNING-tier items (WR-06, WR-08) are fully closed. The additional fixes CR-01 (mock.rs debug-assertions gate) and IN-01 (priming-frame app.now refresh before first draw) are also confirmed in place.

### Human Verification Required

#### 1. TUI Visual Behavior (deferred from Plan 03 Task 2 checkpoint)

**Test:** Run `./target/release/ahb tui` (with `AHB_SECRETS_MOCK=1` on backend-less hosts) with `[providers.claude] enabled = true` in `~/.config/ahb/config.toml`
**Expected:** Bordered frame titled ` AHB ` (leading + trailing space); one `claude` row with 10 bar cells + percent + bullet + `resets in Xh{MM}m`; quit hint `q quit  ·  ctrl-c quit` in DarkGray at bottom; q/Ctrl-C return terminal cleanly
**Why human:** Color threshold validation (Green/Yellow/Red), visual alignment, interactive quit feel, and stty -a terminal state verification require a real TTY and human judgment

#### 2. Color Threshold Verification

**Test:** Observe bar fill color across percent ranges: >=30% green, 10-30% yellow, <10% red; empty cells always DarkGray
**Expected:** UI-SPEC 60/30/10 color thresholds rendered correctly
**Why human:** Color rendering requires visual inspection in a real terminal

#### 3. Keyring Backend on macOS / Windows (CI gap)

**Test:** Run `cargo test --test keyring_init_sanity` on macOS and Windows (not just Linux)
**Expected:** `InitOutcome::Ready(_)` — OS keyring backend registers successfully; first macOS run may show Keychain access prompt
**Why human:** Dev machine is Linux with dbus unavailable; `AHB_SECRETS_MOCK=1` masks the real backend path in CI; platform-specific keyring behavior requires manual testing on each OS

### Gaps Summary

No gaps remain. All five BLOCKER/WARNING items from the initial verification are closed:

- **BL-01 CLOSED:** `src/tui/widgets/hp_row.rs` has 0 `jiff::Timestamp::now()` calls (grep-verified). `AppState.now: jiff::Timestamp` field exists. `tui_loop` sets it at startup (priming draw, IN-01 fix) and in the render-tick arm — the ONLY authorized wall-clock sites. `tests/no_walltime_in_adapter.rs` now scans both `src/provider/` and `src/tui/widgets/` and passes.
- **BL-02 CLOSED:** `Engine::refresh_all` sorts by `sort_by_key(Self::sort_key(id))` with canonical ProviderId order (Claude=0, Codex=1, Gemini=2, Mock=3). `tests/engine_row_order.rs` proves Claude appears before Mock even when Mock's zero-await fetch arrives first. Fanout, CLI, and TUI consumers have 0 sort calls — single source of truth at the engine boundary.
- **BL-03 CLOSED:** `find_active_cluster` uses `gap.total(jiff::Unit::Second) > FIVE_HOURS_SECS` (strict-greater, total seconds). `get_hours()`, `>= 5`, and `jiff::Unit::Hour` constraints are all gone (all grep 0). Three boundary-locking tests pass: 4h59m30s no-split, exactly-5h no-split, 5h0m30s split.
- **WR-06 CLOSED:** D-41 error message resolves cross-OS path via `config::default_path().ok().map_or_else(...)`. Linux hardcoded path absent. `TODO(future-phase)` doc comment preserves `[secrets].storage = "file"` contract.
- **WR-08 CLOSED:** `run_tui_stub` is absent from `src/cli/mod.rs` and has 0 callers across `src/` and `tests/`.

Additionally:
- **CR-01 CLOSED:** `mock.rs` panic injection block is wrapped in `#[cfg(debug_assertions)]` — release builds cannot trigger the `AHB_DEBUG_PANIC` env path.
- **IN-01 CLOSED:** `app.now = jiff::Timestamp::now()` appears at line 120, immediately before the priming `terminal.draw()` at line 121, closing the stale-timestamp window between initial engine fetch and first frame render.

Three human verification items remain — visual/interactive/platform-specific behaviors that cannot be verified programmatically. These carry over unchanged from the initial verification.

---

_Verified: 2026-05-23T10:00:00Z_
_Verifier: Claude (gsd-verifier)_
_Re-verification: Yes — Plan 01-04 gap closure_
