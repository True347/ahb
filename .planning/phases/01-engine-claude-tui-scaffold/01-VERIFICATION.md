---
phase: 01-engine-claude-tui-scaffold
verified: 2026-05-23T07:30:00Z
status: gaps_found
score: 4/5 must-haves verified
overrides_applied: 0
gaps:
  - truth: "AHB tui opens a fixed full-screen view that auto-refreshes every 15s, restores the terminal cleanly on q/Ctrl-C, and survives a deliberately-injected adapter panic() without leaving the shell in raw/altscreen mode (verified by integration test)"
    status: partial
    reason: "BL-01 (REVIEW): hp_row.rs line 73 calls jiff::Timestamp::now() directly inside the leaf render function build_ok_line, outside any authorized callsite. AppState has no 'now' field; the clock injection rule (enforced by no_walltime_in_adapter.rs) only scans src/provider/, so this violation escapes the acceptance grep. Additionally, BL-02: fanout::refresh_all_inner returns results in join_next() arrival order with no sort applied at the engine or consumer layer — row order is nondeterministic across invocations, violating the UI-SPEC fixed-row contract. BL-03: window.rs:51 uses `gap_hours >= 5` (hour-component comparison) while the doc says '> 5h'; get_hours() ignores sub-hour components, creating a ±1h boundary window around the correct split point."
    artifacts:
      - path: "src/tui/widgets/hp_row.rs"
        issue: "Line 73: jiff::Timestamp::now() called in leaf render fn — clock injection broken in TUI render path"
      - path: "src/engine/fanout.rs"
        issue: "No sort on results Vec; arrival order is nondeterministic across adapters"
      - path: "src/engine/mod.rs"
        issue: "refresh_all() does not sort results before returning to consumers"
      - path: "src/provider/claude/window.rs"
        issue: "Line 51: gap_hours >= 5 uses hour-component only (not total duration); doc/code mismatch (>= vs >)"
    missing:
      - "hp_row::build_ok_line must receive 'now: &jiff::Timestamp' from its caller (store on AppState.now, update on every render tick in tui_loop)"
      - "no_walltime_in_adapter.rs scope must be extended to include src/tui/widgets/"
      - "Engine::refresh_all() (or fanout) must sort results by canonical ProviderId order (Claude=0, Codex=1, Gemini=2, Mock=3)"
      - "window.rs gap check must use strict total-seconds comparison (gap_secs > 5*3600.0) to match the doc and handle sub-hour boundaries correctly"
---

# Phase 01: Engine + Claude + TUI Scaffold — Verification Report

**Phase Goal:** Make `AHB` and `AHB tui` work end-to-end against a real Claude Code subscription, with keyring-backed secrets, panic-safe terminal restore, and per-adapter error isolation wired in BEFORE feature code so the foundation is correct from day one.
**Verified:** 2026-05-23T07:30:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | `AHB` prints real Claude HP bar from `~/.claude/projects/**/*.jsonl`; `AHB \| cat` emits no ANSI | ✓ VERIFIED | `cli_walking_skeleton.rs` tests synthetic JSONL with `cache_creation_input_tokens`; `should_colorize_env` wired in `run_compact`; `!stdout.contains("\x1b[")` assertion at line 97 |
| 2 | `AHB tui` opens fixed-frame, auto-refreshes 15s, restores terminal on quit/panic (integration test) | ✗ PARTIAL | `tui_panic_safe_restore.rs` verifies alt-screen restore (1049h/1049l); `tui_non_tty_refusal.rs` verifies TUI-05; BUT BL-01: `hp_row.rs:73` calls `Timestamp::now()` bypassing clock injection; BL-02: provider row order is nondeterministic (no sort in fanout/engine); BL-03: gap check uses `>= 5h` (hour-component only) |
| 3 | Broken provider → ERROR row; Claude stays healthy; TUI non-TTY → exit 2 with literal | ✓ VERIFIED | `panic_isolation.rs` asserts claude row + `mock  ERROR:` + Phase 0 `ahb panicked:` prefix; `tui_non_tty_refusal.rs` asserts exit 2 + verbatim UI-SPEC literal |
| 4 | TOML config cross-OS; un-configured providers silently skipped; `Secret<T>` redacts in Debug/Serialize; CI grep test | ✓ VERIFIED | `ProjectDirs::from("", "", "ahb")` in `config.rs:78`; `LoadOutcome::Initialized` on first run; `Secret<T>::Debug` → `"***"`, `Serialize` → `"[REDACTED]"`; `tests/secret_leak.rs` + `tests/secret_leak_subprocess.rs` double-assert; `keyring-core = "1"` in Cargo.toml; no `keyring = ` v4 |
| 5 | Claude JSONL schema drift → visible "Claude adapter may be out-of-date" sentinel | ✓ VERIFIED | `detect_drift()` in `jsonl.rs` (raw `serde_json::Value` re-parse, typed `u64` schema unchanged); `fetch()` calls it before cluster math; `format_error_row_colored` emits U+2592 sentinel via `id_label(id)`; `schema_drift_sentinel.rs` integration test |

**Score:** 4/5 truths verified (SC2 is PARTIAL — functional but has 3 correctness defects from code review)

### Deferred Items

None — all 3 blockers from 01-REVIEW.md are correctness defects in Phase 1 code, not work explicitly scheduled for later phases. ROADMAP Phase 2–4 success criteria make no reference to row ordering, clock injection in the TUI render layer, or the 5h gap boundary semantics.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/engine/mod.rs` | Engine struct + refresh_all + new | ✓ VERIFIED | `ClaudeProvider::new` wired on `cfg.providers.claude.enabled` |
| `src/engine/fanout.rs` | JoinSet fan-out + timeout + panic-to-Internal | ✓ VERIFIED | `JoinSet`, `tokio::time::timeout`, `is_panic()`, `HashMap<task::Id, ProviderId>` all present |
| `src/engine/events.rs` | EngineEvent enum + EVENT_BUFFER | ✓ VERIFIED | IN-03 (info): module doc claims Plan 03 uses mpsc but it doesn't — doc is stale, not a blocker |
| `src/provider/claude/mod.rs` | ClaudeProvider implementing Provider | ✓ VERIFIED | `pub struct ClaudeProvider`, `impl Provider`, `detect_drift` called before cluster math |
| `src/provider/claude/jsonl.rs` | Streaming parser + drift detector | ✓ VERIFIED | `read_assistant_entries`, `discover_session_files`, `read_recent_raw`, `detect_drift` |
| `src/provider/claude/window.rs` | 5h cluster anchor + percent math | ✓ VERIFIED (with caveat) | `CLAUDE_5H_TOKEN_LIMIT = 44_000`; `find_active_cluster` exists; BL-03: gap comparison uses `>= 5` hour-component not total-seconds strict-greater |
| `src/config.rs` | Config + load_or_init + default_path | ✓ VERIFIED | `LoadOutcome` enum, `ProjectDirs::from("", "", "ahb")`, D-37 literal |
| `src/templates/default-config.toml` | Embedded TOML with 3 providers disabled | ✓ VERIFIED | `[providers.claude]`, `enabled = false` for all 3 |
| `src/cli/tty.rs` | ColorMode + should_colorize | ✓ VERIFIED | Pure fn with 6-path truth table + env wrapper |
| `src/secrets.rs` | `Secret<T>` + `init()` + `InitOutcome` | ✓ VERIFIED | Drop/Debug/Serialize impls present; no Deserialize impl (comments at lines 67-68 confirm intent; `grep -E 'impl[^{]*Deserialize[^{]*for Secret'` returns empty); `set_default_store` present |
| `src/tui/mod.rs` | `pub async fn run` + non-TTY refusal + spawn_blocking bridge | ✓ VERIFIED | `ratatui::run` (NOT init/restore), `spawn_blocking`, `Handle::current`, `is_terminal` gate, UI-SPEC verbatim literal |
| `src/tui/app.rs` | AppState + RowState + handle_event | ✓ VERIFIED | `pub struct AppState`, `RowState` enum (Ok/SchemaDrift/Err), `apply_results`, `handle_event` |
| `src/tui/ui.rs` | draw() with 4-chunk layout | ✓ VERIFIED | `Borders::ALL`, `" AHB "` title, `q quit  ·  ctrl-c quit` hint |
| `src/tui/widgets/hp_row.rs` | Per-row render with color thresholds | ✓ VERIFIED (with caveat) | `use crate::cli::render_text::{...}` re-import; U+2592 sentinel; UI-SPEC colors; BL-01: `Timestamp::now()` called at line 73 |
| `tests/tui_non_tty_refusal.rs` | TUI-05 integration test | ✓ VERIFIED | Exit code 2 + verbatim UI-SPEC literal assertion |
| `tests/tui_panic_safe_restore.rs` | TUI-04 real-pty test (WARNING #6) | ✓ VERIFIED | `portable_pty`, alt-screen enter/leave byte assertions, `#[cfg(unix)]`-gated |
| `tests/secret_leak.rs` | D-43 unit-tier double-assert | ✓ VERIFIED | Literal absent + 20-char regex absent on Debug + Serialize |
| `tests/secret_leak_subprocess.rs` | D-43 integration-tier subprocess | ✓ VERIFIED | `--debug-emit-fake-secret` flag, `[REDACTED]` positive assertion |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `src/provider/claude/window.rs` | `cache_creation_input_tokens` | u64 summation across cluster | ✓ WIRED | `grep -c 'cache_creation_input_tokens' window.rs` → 5 hits |
| `src/engine/fanout.rs` | `tokio::task::JoinSet` | spawn + join_next_with_id + HashMap<task::Id, ProviderId> | ✓ WIRED | `JoinSet::new`, `is_panic()` present |
| `src/main.rs` | `ahb::engine::Engine::refresh_all` | single .await before render | ✓ WIRED | `engine.refresh_all(now).await` in `run_compact`; `ahb::tui::run(engine).await` for TUI |
| `src/config.rs` | `directories::ProjectDirs::from` | `("", "", "ahb")` three-arg form | ✓ WIRED | `ProjectDirs::from("", "", "ahb")` at line 78 |
| `src/secrets.rs` | `keyring_core::set_default_store` | cfg-gated `make_default_store` in `init()` | ✓ WIRED | `set_default_store` present 6 times |
| `src/tui/mod.rs` | `ratatui::run` (SYNC) | `spawn_blocking` → `Handle::block_on` | ✓ WIRED | `spawn_blocking`, `Handle::current()`, `ratatui::run` all present; `ratatui::init/restore` = 0 |
| `src/main.rs` | `secrets::init` | replaces `Secrets::default()` stub | ✓ WIRED | `match ahb::secrets::init()? { InitOutcome::Ready(s) => s, ...` |
| `hp_row.rs` | `cli::render_text::{filled_cells, format_countdown, id_label}` | `use crate::cli::render_text::` | ✓ WIRED | Re-import confirmed, no duplication |
| `hp_row.rs::build_ok_line` | `jiff::Timestamp::now()` | FORBIDDEN direct call at line 73 | ✗ NOT_WIRED_CORRECTLY | Clock injection broken; `now` must flow from `AppState` through `ui::draw` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `src/tui/widgets/hp_row.rs` | `countdown` (via `format_countdown`) | `jiff::Timestamp::now()` at line 73 | Yes, but from wall-clock singleton | ✗ HOLLOW — not injectable; `now` should come from `AppState.now` |
| `src/provider/claude/mod.rs` | `used_tokens` | `cache_creation_input_tokens` from JSONL | Yes — real file data | ✓ FLOWING |
| `src/cli/mod.rs::run_compact` | `results` Vec | `engine.refresh_all(now).await` | Yes — real engine results | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| AHB \| cat emits zero ANSI bytes | `tests/cli_walking_skeleton.rs::ahb_piped_stdout_contains_no_ansi_escapes` | assertion on `!stdout.contains("\x1b[")` | ✓ PASS (test exists + asserts correctly) |
| Schema-drift sentinel renders U+2592 | `tests/schema_drift_sentinel.rs` | `≥10 U+2592 bytes` assertion | ✓ PASS |
| Panic isolation: claude healthy, mock ERROR | `tests/panic_isolation.rs` | claude row present, mock ERROR row present, `ahb panicked:` in stderr | ✓ PASS |
| TUI non-TTY refusal | `tests/tui_non_tty_refusal.rs` | exit 2 + verbatim UI-SPEC literal | ✓ PASS |
| TUI panic-safe restore (real-pty) | `tests/tui_panic_safe_restore.rs` | 1049h enter + 1049l leave before exit | ✓ PASS (Unix-gated) |
| Secret<T> does not leak in Debug or Serialize | `tests/secret_leak.rs` + `tests/secret_leak_subprocess.rs` | literal absent + 20-char regex absent | ✓ PASS |

### Probe Execution

Step 7c: SKIPPED — no conventional `scripts/*/tests/probe-*.sh` probes exist in this project.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| CORE-01 | Plan 01 | `AHB` no-args prints compact HP bar | ✓ SATISFIED | `run_compact` iterates engine results; `cli_walking_skeleton.rs` integration test |
| CORE-05 | Plan 01 | Non-TTY / NO_COLOR → no ANSI | ✓ SATISFIED | `should_colorize_env` wired in `run_compact`; `cli_walking_skeleton.rs` ANSI assertion |
| TUI-01 | Plan 03 | `AHB tui` fixed-screen per-provider HP bar | ✓ SATISFIED | Full TUI module group; `ui::draw` with `Borders::ALL` + provider rows |
| TUI-02 | Plan 03 | 15s auto-refresh | ✓ SATISFIED | `Duration::from_secs(15)` fetch_tick in `tui_loop` |
| TUI-04 | Plan 03 | panic-safe terminal restore (integration test) | ✓ SATISFIED (with BL-01 caveat) | `tests/tui_panic_safe_restore.rs` real-pty test asserts 1049h/1049l; `ratatui::run` auto-installs panic hook |
| TUI-05 | Plan 03 | TUI non-TTY → clear error + exit | ✓ SATISFIED | `is_terminal` gate; verbatim UI-SPEC literal; `tests/tui_non_tty_refusal.rs` exit 2 |
| CFG-01 | Plan 01 | TOML config with provider enable/disable | ✓ SATISFIED | `Config/Providers/ProviderConfig` structs; embedded template |
| CFG-02 | Plan 01 | Cross-platform config path via `directories` | ✓ SATISFIED | `ProjectDirs::from("", "", "ahb")` in `config.rs` |
| CFG-04 | Plan 01 | Unconfigured providers silently skipped | ✓ SATISFIED | Engine only pushes providers with `enabled = true`; `LoadOutcome` enum |
| SEC-01 | Plan 02 | OS keyring via `keyring-core` 1.0 | ✓ SATISFIED | `set_default_store` wired in `secrets::init()`; `keyring-core = "1"` in Cargo.toml; `keyring = ` v4 absent |
| SEC-02 | Plan 02 | `Secret<T>` newtype with Debug redact | ✓ SATISFIED | `Debug` → `"***"`, `Serialize` → `"[REDACTED]"`, no `Deserialize`; double-assert tests |
| SEC-04 | Plan 01 | No-secret provider uses same interface contract | ✓ SATISFIED | `ClaudeProvider::fetch` receives `&FetchCtx` with `&Secrets`; `_ = ctx.secrets` pattern preserved |
| ADP-01 | Plan 01+02 | Adapter failure isolated; no crash or blank | ✓ SATISFIED | JoinSet + `is_panic()` panic recovery; `panic_isolation.rs` verifies end-to-end |
| ADP-02 | Plan 01 | Claude JSONL adapter with 5h cluster | ✓ SATISFIED | `discover_session_files`, `read_assistant_entries`, `find_active_cluster`, `cache_creation_input_tokens` summation |
| ADP-03 | Plan 02 | Schema-drift sentinel | ✓ SATISFIED | `detect_drift` (raw Value re-parse), `ProviderError::SchemaDrift`, U+2592 sentinel in CLI + TUI |

All 15 Phase 1 requirements are addressed. No orphaned requirements detected.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/tui/widgets/hp_row.rs` | 73 | `jiff::Timestamp::now()` direct call in leaf render fn | 🛑 Blocker | Breaks clock injection contract; render layer cannot be tested with frozen clock; `no_walltime_in_adapter.rs` does not catch it (scope = `src/provider/` only) |
| `src/engine/fanout.rs` / `src/engine/mod.rs` | — | No sort on `Vec<(ProviderId, Result<...>)>` from `join_next` | 🛑 Blocker | Provider row order is nondeterministic (arrival order); UI-SPEC fixed-row contract violated; user will see rows reorder on every invocation once Phase 2 adds Codex |
| `src/provider/claude/window.rs` | 50-51 | `gap_hours >= 5` using `get_hours()` (hour-component only, not total duration) | 🛑 Blocker | Off-by-one at the 5h boundary; a 4h59m30s gap reports `get_hours()==4` (no split) but 5h0m0s reports `get_hours()==5` (split); doc says `> 5h` but code uses `>= 5h`; boundary regression not covered by tests (all fixtures use hour-aligned timestamps) |
| `src/cli/mod.rs` | 97 | `pub fn run_tui_stub()` — dead code, never called (WR-08) | ⚠️ Warning | Dead public API; confuses readers about the TUI dispatch path |
| `src/main.rs` | 69 | D-41 error message hardcodes `~/.config/ahb/config.toml` (Linux path) regardless of OS (WR-06) | ⚠️ Warning | On macOS the path is `~/Library/Application Support/ahb/config.toml`; user told to edit a non-existent file |

Note: No `TBD`, `FIXME`, or `XXX` debt markers detected in phase-modified files. The 3 blockers are structural correctness issues, not unresolved debt markers.

### Human Verification Required

#### 1. TUI Visual Behavior (deferred from Plan 03 Task 2 checkpoint)

**Test:** Run `./target/release/ahb tui` (with `AHB_SECRETS_MOCK=1` on backend-less hosts) with `[providers.claude] enabled = true` in `~/.config/ahb/config.toml`
**Expected:** Bordered frame titled ` AHB ` (leading + trailing space); one `claude` row with 10 bar cells + percent + bullet + `resets in Xh{MM}m`; quit hint `q quit  ·  ctrl-c quit` in DarkGray at bottom; q/Ctrl-C return terminal cleanly
**Why human:** Color threshold validation (Green/Yellow/Red), visual alignment, interactive quit feel, and stty -a terminal state verification require a real TTY and human judgment

#### 2. Color Threshold Verification

**Test:** Observe bar fill color across percent ranges: ≥30% → green, 10–30% → yellow, <10% → red; empty cells always DarkGray
**Expected:** UI-SPEC 60/30/10 color thresholds rendered correctly
**Why human:** Color rendering requires visual inspection in a real terminal

#### 3. Keyring Backend on macOS / Windows (CI gap)

**Test:** Run `cargo test --test keyring_init_sanity` on macOS and Windows (not just Linux)
**Expected:** `InitOutcome::Ready(_)` — OS keyring backend registers successfully; first macOS run may show Keychain access prompt
**Why human:** Dev machine is Linux with dbus unavailable; `AHB_SECRETS_MOCK=1` masks the real backend path in CI; platform-specific keyring behavior requires manual testing on each OS

---

## Gaps Summary

Three BLOCKER-tier correctness defects discovered in code review (01-REVIEW.md BL-01, BL-02, BL-03) are unaddressed in the codebase. They do not prevent the binary from functioning in the common case (Phase 1 has only one real adapter, and test fixtures use hour-aligned timestamps), but they undermine contractual guarantees that Phase 1 explicitly made:

1. **BL-01 (Clock injection):** `hp_row.rs:73` calls `jiff::Timestamp::now()` directly in a leaf render function, bypassing the clock-injection architecture. The `no_walltime_in_adapter.rs` acceptance grep only scans `src/provider/`, so this violation is structurally invisible to the test suite. The TUI render layer cannot be tested with a frozen clock, which breaks the `format_countdown` output determinism that Phase 2 countdown tests will rely on.

2. **BL-02 (Nondeterministic row order):** `fanout::refresh_all_inner` returns results in `join_next` arrival order. Neither the engine nor the CLI/TUI consumers sort. Once Phase 2 adds Codex (with different fetch latency than Claude), rows will reorder on every invocation. The UI-SPEC mockup shows a fixed canonical order (claude / codex / gemini). A one-line `sort_by_key` fix in `Engine::refresh_all` would close this before Phase 2 ships.

3. **BL-03 (Gap boundary):** `window.rs:51` uses `gap_hours >= 5` where `gap_hours = gap.get_hours()`. This extracts only the hour component of the span (not total duration), creating a ±59m59s window around the correct 5h boundary. The module doc says `> 5h` but the code uses `>= 5h`. A 4h59m30s gap incorrectly triggers no split; a 5h0m0s gap triggers an early split. Test fixtures all use hour-aligned timestamps, so no existing test catches this. The cluster anchor directly determines the percent and reset countdown shown to the user.

All three are correctness defects with bounded Phase 1 visibility (hidden by single-provider and aligned-clock test conditions) that will manifest as observable bugs in Phase 2+ multi-provider scenarios.

Additionally, two warnings remain unaddressed: dead code `run_tui_stub` (WR-08, one-line delete) and the Linux-hardcoded D-41 error path (WR-06, affects macOS/Windows users who hit a missing keyring backend).

---

_Verified: 2026-05-23T07:30:00Z_
_Verifier: Claude (gsd-verifier)_
