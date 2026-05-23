---
phase: 01-engine-claude-tui-scaffold
plan: 04
subsystem: correctness-gap-closure
tags: [rust, gap-closure, correctness, clock-injection, deterministic-order, gap-boundary, tui, engine, claude-adapter, cross-os-path, dead-code-removal]

requires:
  - phase: 01-engine-claude-tui-scaffold
    plan: 01
    provides: "Engine::refresh_all fanout + ClaudeProvider.find_active_cluster + cli::render_text::format_countdown + tests/no_walltime_in_adapter.rs (src/provider/ scope) + config::default_path() cross-OS resolution"
  - phase: 01-engine-claude-tui-scaffold
    plan: 02
    provides: "secrets::init InitOutcome::Unavailable D-41 hard-error branch in main.rs"
  - phase: 01-engine-claude-tui-scaffold
    plan: 03
    provides: "AppState + RowState + ui::draw + widgets::hp_row::{render, build_line, build_ok_line}; tui_loop sync-bridge + render-tick arm (BL-01 violation lived here); src/tui/widgets/ subtree introduced (now grep-forbidden)"
provides:
  - "AppState.now: pub jiff::Timestamp — snapshot of wall clock at most-recent render tick, set ONLY by tui_loop, consumed by widgets via &app.now (BL-01 fix data path)"
  - "tui_loop wall-clock authorization: render-tick arm is the SINGLE authorized site that updates app.now via jiff::Timestamp::now() before each terminal.draw"
  - "widgets::hp_row::render / build_line / build_ok_line: all carry now: &jiff::Timestamp parameter; leaf widget no longer reads wall clock"
  - "Engine::refresh_all canonical sort: sort_by_key(Self::sort_key(id)) with Claude=0, Codex=1, Gemini=2, Mock=3 — single source of truth at engine boundary; fanout still advertises arrival order, CLI/TUI consumers do not re-sort"
  - "src/provider/claude/window.rs FIVE_HOURS_SECS const + strict-greater total-seconds gap comparison: gap.total(Unit::Second) > FIVE_HOURS_SECS replaces hour-component-only non-strict path"
  - "tests/no_walltime_in_adapter.rs extended scope: scans BOTH src/provider/ AND src/tui/widgets/; renamed fn no_timestamp_now_in_provider_or_tui_widgets_subtree advertises dual scope"
  - "tests/engine_row_order.rs new integration test: enables claude+mock, seeds ~/.claude/projects fixture, asserts Claude row before Mock row (without sort, Mock would arrive first because its fetch has zero await points)"
  - "Three new boundary-locking unit tests in src/provider/claude/window.rs: gap_just_under_5h_does_not_split (4h59m30s), gap_exactly_5h_does_not_split (boundary), gap_just_over_5h_does_split (5h0m30s)"
  - "src/main.rs D-41 cross-OS error message: config::default_path().ok().map_or_else(...) resolves the right path on macOS/Windows/Linux with 'your AHB config file' fallback; TODO(future-phase) doc preserves [secrets].storage = 'file' escape-hatch contract"
  - "src/cli/mod.rs free of run_tui_stub dead code (zero callers; grep clean across src/ and tests/)"
affects: ["02-codex-output", "03-gemini-cache", "04-distribution"]

tech-stack:
  added: []  # ZERO new dependencies — gap-closure modifies only existing source files
  patterns:
    - "Clock injection through AppState.now: the render-tick arm is the single authorized wall-clock site; leaf widgets receive `now: &jiff::Timestamp` as a parameter, mirroring the adapter ctx.now rule established in Phase 0"
    - "Canonical row ordering at the engine boundary (NOT at fanout, NOT at consumers): sort_by_key(sort_key(id)) inside Engine::refresh_all after the fanout await; ProviderId discriminant order IS the canonical row order"
    - "Strict-greater total-seconds gap comparison: jiff::Span::total(Unit::Second) > BOUNDARY_SECS. Avoid Span::get_hours() (drops sub-hour) and `since((Unit::Hour, _))` largest-unit constraints when downstream comparison needs sub-hour precision"
    - "Cross-OS path messages: ALWAYS resolve via config::default_path() / directories::ProjectDirs at message-construction time; never hardcode `~/.config/...` because macOS uses ~/Library/Application Support and Windows uses %APPDATA%"
    - "Static grep acceptance gates require comment hygiene: when removing a code pattern, also rephrase doc comments that mentioned the literal so the regression-prevention grep stays at 0 hits without losing the historical context"

key-files:
  created:
    - "tests/engine_row_order.rs — in-process integration test for BL-02 canonical sort (claude + mock enabled, asserts Claude appears first)"
  modified:
    - "src/tui/app.rs — AppState carries pub now: jiff::Timestamp; constructor takes the seed snapshot; Default removed (Timestamp has no Default); 4 existing tests updated to new ::new(fixture_now()) signature"
    - "src/tui/mod.rs — tui_loop seeds app.now at startup; render-tick arm updates app.now immediately before terminal.draw (single authorized wall-clock site for the render path); module doc extended with BL-01 binding"
    - "src/tui/widgets/hp_row.rs — render / build_line / build_ok_line all carry now: &jiff::Timestamp; leaf widget no longer calls jiff::Timestamp::now(); existing tests updated to pass &fixture_now() to build_line"
    - "src/tui/ui.rs — draw plumbs &app.now into hp_row::render call site"
    - "src/engine/mod.rs — refresh_all binds the fanout result, sorts via sort_by_key(Self::sort_key(id)), then returns; new private sort_key fn maps Claude=0/Codex=1/Gemini=2/Mock=3; new unit test refresh_all_returns_canonical_order_with_mock_only confirms the sort code path runs"
    - "src/provider/claude/window.rs — FIVE_HOURS_SECS const = 5.0 * 3600.0; gap-comparison rewritten as gap.total(Unit::Second) > FIVE_HOURS_SECS; three new boundary-locking tests; module doc updated"
    - "src/main.rs — D-41 InitOutcome::Unavailable branch uses config::default_path().ok().map_or_else(...) for cross-OS path display; TODO(future-phase) doc comment preserves [secrets].storage = 'file' escape-hatch contract"
    - "src/cli/mod.rs — deleted run_tui_stub() and its doc comment (zero callers); module doc updated to reference Plan 03 ratatui surface + Plan 04 WR-08 cleanup without re-introducing the symbol name (keeps the static grep gate at 0)"
    - "tests/no_walltime_in_adapter.rs — scope expanded from src/provider/ only to BOTH src/provider/ AND src/tui/widgets/; test renamed no_timestamp_now_in_provider_or_tui_widgets_subtree to advertise dual scope so future agents do not re-narrow"

decisions:
  - "[Plan 01-04 BL-01] Clock injection extended from src/provider/ to src/tui/widgets/. AppState carries pub now: jiff::Timestamp; tui_loop is the single authorized TUI wall-clock site (renders updates app.now immediately before terminal.draw, AND constructor seeds it at startup). The render-leaf widget MUST receive now via parameter — tests/no_walltime_in_adapter.rs grep-rejects any Timestamp::now under src/tui/widgets/."
  - "[Plan 01-04 BL-02] Canonical ProviderId row order locked at the engine boundary (NOT fanout, NOT consumers). Mapping: Claude=0, Codex=1, Gemini=2, Mock=3. Mock is last because it is debug/fault-injection only, not a user-facing provider. Phase 2's Codex adapter lands in row 1 between Claude and Gemini without rediscovering the rule. CLI/TUI consumers never re-sort."
  - "[Plan 01-04 BL-03] Cluster gap comparison uses jiff::Span::total(Unit::Second) > FIVE_HOURS_SECS (strict-greater on total seconds). The prior hour-component-only non-strict comparator (get_hours() >= 5) double-misclassified the 5h boundary: exactly-5h INCORRECTLY split, and any sub-hour precision was discarded. Three boundary tests lock the contract: 4h59m30s no-split, exactly-5h no-split, 5h0m30s split."
  - "[Plan 01-04 WR-06] D-41 keyring-unavailable error message resolves the config path via config::default_path().ok().map_or_else(...) at message-construction time. macOS users see ~/Library/Application Support/ahb/config.toml; Windows users see %APPDATA%\\ahb\\config.toml; Linux users see ~/.config/ahb/config.toml. Fallback literal: 'your AHB config file' if directories::ProjectDirs cannot resolve. A TODO(future-phase) doc comment preserves the [secrets].storage = 'file' escape-hatch contract for the future plan that will extend Config with a [secrets] table + secrets::init file-fallback wiring."
  - "[Plan 01-04 WR-08] run_tui_stub() deleted from src/cli/mod.rs (zero callers since Plan 03 wired ahb::tui::run). Module doc updated to reference Plan 03 + Plan 04 history WITHOUT re-introducing the symbol name, so the static grep gate (grep -rn run_tui_stub src/ tests/ | wc -l == 0) stays clean."

metrics:
  duration_minutes: 22
  completed_date: "2026-05-23"
  tasks_completed: 4
  files_created: 1
  files_modified: 9
  lines_added: ~280
  lines_removed: ~50
  tests_added: 5  # 1 engine unit + 1 engine integration + 3 window boundary
  new_dependencies: 0
---

# Phase 01 Plan 04: Gap Closure (BL-01/BL-02/BL-03/WR-06/WR-08) Summary

Gap-closure plan that closes the three BLOCKER-tier correctness defects (BL-01 clock injection, BL-02 deterministic row order, BL-03 gap-boundary precision) and two WARNING-tier hygiene items (WR-06 cross-OS D-41 path, WR-08 dead `run_tui_stub` removal) surfaced by Phase 1 code review (`01-REVIEW.md`) and confirmed by `01-VERIFICATION.md` as the reason SC2 (TUI fixed-frame contract) shipped PARTIAL. All five fixes are surgical, file-disjoint, and touch only `src/*` and `tests/*` — no prior PLAN.md/SUMMARY.md/CONTEXT.md/ROADMAP.md/STATE.md artifacts were modified.

## What Shipped

**BL-01 (clock injection extended to TUI widgets).** `AppState` now carries `pub now: jiff::Timestamp`; `tui_loop` is the SINGLE authorized wall-clock site in the TUI render path — it seeds `app.now` at startup and updates it immediately before each `terminal.draw` in the render-tick arm. The leaf widget `widgets::hp_row::build_ok_line` no longer calls `jiff::Timestamp::now()` itself; it receives `now: &jiff::Timestamp` from `ui::draw`, which plumbs `&app.now` from the cached `AppState`. `tests/no_walltime_in_adapter.rs` extends its scanned-paths list to include `src/tui/widgets/` — `src/tui/mod.rs` (canonical site) remains authorized but is NOT scanned, while `src/tui/widgets/` and `src/provider/` are both grep-rejected for `Timestamp::now`.

**BL-02 (canonical row order at the engine boundary).** `Engine::refresh_all` now binds the fanout `Vec`, sorts it by `Self::sort_key(id)`, then returns. The new private `sort_key` fn maps `Claude=0, Codex=1, Gemini=2, Mock=3` (Mock last because it is debug/fault-injection only, not a user-facing provider). The sort lives EXCLUSIVELY at the engine boundary — `fanout::refresh_all_inner` still advertises arrival order, and CLI/TUI consumers do not re-sort. A new integration test `tests/engine_row_order.rs` enables both `claude` + `mock`, seeds a synthetic `~/.claude/projects/proj-a/session.jsonl` fixture, and asserts `Claude` appears in row 0 even though `MockProvider`'s zero-await `fetch` would otherwise arrive first via `join_next`.

**BL-03 (strict total-seconds gap comparison).** `src/provider/claude/window.rs::find_active_cluster` now uses `gap.total(jiff::Unit::Second) > FIVE_HOURS_SECS` (strict-greater on total seconds) where `const FIVE_HOURS_SECS: f64 = 5.0 * 3600.0`. The prior code did `since((jiff::Unit::Hour, prev.timestamp))` + `gap.get_hours() >= 5`, which dropped sub-hour precision AND used non-strict inequality — double-misclassifying the boundary. Removing the `(Unit::Hour, _)` largest-unit constraint lets jiff auto-pick a unit that preserves sub-hour precision when `.total(Unit::Second)` reduces to a scalar. Three new boundary-locking tests prevent regression: 4h59m30s no-split, exactly-5h no-split, 5h0m30s split.

**WR-06 (cross-OS D-41 error message).** `src/main.rs` D-41 keyring-unavailable branch resolves the config path at message-construction time via `config::default_path().ok().map_or_else(|| "your AHB config file".to_string(), |p| p.display().to_string())`. macOS users see `~/Library/Application Support/ahb/config.toml`; Windows users see `%APPDATA%\ahb\config.toml`; Linux users see `~/.config/ahb/config.toml`; the literal fallback handles the rare case where `directories::ProjectDirs` cannot resolve. A `TODO(future-phase)` doc comment preserves the `[secrets].storage = "file"` escape-hatch contract (which `Config` does not yet support — a future plan must extend `Config` with a `[secrets]` table + `secrets::init` file-fallback wiring).

**WR-08 (dead `run_tui_stub` removed).** `src/cli/mod.rs` no longer exposes `run_tui_stub()` — it had zero callers since Plan 03 wired `Command::Tui → ahb::tui::run(engine).await` in `src/main.rs`. The module doc was updated to reference Plan 03 + Plan 04 history WITHOUT re-introducing the symbol name, so the static grep gate (`grep -rn 'run_tui_stub' src/ tests/ | wc -l == 0`) stays at zero hits while preserving the historical context.

## Pointers for Phase 2 and Beyond

These deliverables are intentionally durable so Phase 2's Codex adapter does not rediscover any of them:

(a) **Gaps closed:** BL-01, BL-02, BL-03, WR-06, WR-08 — all five recipes from `01-REVIEW.md` were applied verbatim (Rule 1/2/3 deviation patterns; no architectural escalations). The verifier should see `01-VERIFICATION.md` SC2 status flip from PARTIAL to PASS.

(b) **`tests/no_walltime_in_adapter.rs` scope:** the test scans BOTH `src/provider/` AND `src/tui/widgets/`. Future agents MUST NOT re-narrow this — the dual scope is the whole point. The test fn name (`no_timestamp_now_in_provider_or_tui_widgets_subtree`) advertises both subtrees. If a Phase 3+ widget legitimately needs the wall clock, it MUST plumb the snapshot through `AppState.now` (or a future equivalent) rather than adding a fourth authorized callsite.

(c) **Canonical ProviderId sort mapping:** `Claude=0, Codex=1, Gemini=2, Mock=3`. Phase 2's Codex adapter lands in row 1 — between Claude and Gemini — without needing to rediscover the rule. The mapping is a private `Engine::sort_key` fn in `src/engine/mod.rs`; if a Phase 4+ provider is added, extend the `match` exhaustively (Rust will refuse to compile otherwise because `ProviderId` is a closed enum). Mock stays last forever because it is debug/fault-injection only.

(d) **`FIVE_HOURS_SECS` strict-greater contract:** `const FIVE_HOURS_SECS: f64 = 5.0 * 3600.0` lives at module scope in `src/provider/claude/window.rs`. The three boundary tests (4h59m30s, exactly-5h, 5h0m30s) lock the contract — they will fail loudly if a future agent reverts to `get_hours()` OR flips the comparator to non-strict (`>=`). The same pattern applies to any future provider boundary computation: prefer `Span::total(Unit::Second)` over `Span::get_hours()` whenever sub-hour precision could matter.

(e) **`[secrets].storage = "file"` TODO marker:** `src/main.rs` line ~70 carries a `TODO(future-phase)` doc comment documenting that `Config` does not yet support a `[secrets]` table — the message still advertises the contract (per WR-06 disposition (a) of `01-REVIEW.md`), but the wiring is intentionally deferred. A future plan must (i) add `pub struct SecretsConfig { storage: Storage }` to `Config`, (ii) extend `secrets::init` to honor `SecretsConfig::storage == Storage::File` by writing/reading `~/.config/ahb/secrets.toml` at mode 0600, (iii) preserve the D-41 hard-error path for the default (keyring) case.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Clippy `map_unwrap_or` lint in `src/main.rs::D-41`**
- **Found during:** Task 4 first clippy run
- **Issue:** The plan's prescribed pattern `config::default_path().ok().map(|p| p.display().to_string()).unwrap_or_else(|| "your AHB config file".to_string())` trips the `clippy::map-unwrap-or` lint (denied via the workspace `-D warnings` floor).
- **Fix:** Collapsed to `config::default_path().ok().map_or_else(|| "your AHB config file".to_string(), |p| p.display().to_string())` — semantically identical, single-call form. Acceptance grep for `"your AHB config file"` still matches.
- **Files modified:** `src/main.rs`
- **Commit:** `0122e8f`

**2. [Rule 3 - Blocking] Clippy lints in `tests/engine_row_order.rs`**
- **Found during:** Task 2 first clippy run
- **Issue:** (a) `matches!(result, Ok(_))` tripped `clippy::redundant-pattern-matching`; (b) `Secrets::default()` tripped `clippy::default-constructed-unit-structs` because `Secrets` is a unit struct in the test path.
- **Fix:** (a) Switched to `result.is_ok()` — same semantics, lint-clean. (b) Added `#[allow(clippy::default_constructed_unit_structs)]` to the test fn matching the pattern used in `src/engine/mod.rs` and `tests/cli_walking_skeleton.rs`.
- **Files modified:** `tests/engine_row_order.rs`
- **Commit:** `a6f281c`

**3. [Rule 3 - Blocking] Static grep gates require comment hygiene**
- **Found during:** Task 1 (BL-01) acceptance verification and Task 3 (BL-03) acceptance verification
- **Issue:** Acceptance criteria specify static greps that count occurrences without distinguishing code from doc comments. Plan-prescribed doc comments contained the literal patterns being removed (`jiff::Timestamp::now()` in BL-01 docstring; `get_hours() >= 5` and `5 >= 5` in BL-03 historical context comments).
- **Fix:** Rephrased the doc comments to describe the prior code without using the literal pattern (e.g., "MUST NOT read wall clock" instead of "MUST NOT call `jiff::Timestamp::now()`"; "the prior hour-component-only code" instead of "the prior `get_hours() >= 5` code"). Historical context preserved; static grep gates stay at 0 hits.
- **Files modified:** `src/tui/widgets/hp_row.rs`, `src/provider/claude/window.rs`, `src/cli/mod.rs`
- **Commits:** `ca83523`, `d3f1c03`, `0122e8f`

No architectural escalations (Rule 4) were needed — all five gap-closure recipes from `01-REVIEW.md` applied verbatim. No authentication gates triggered (the plan touches no external services).

## Verification Evidence

- `cargo build --release` exits 0 (final run after Task 4)
- `cargo test --workspace` exits 0; **103 tests passed** total (88 lib + 11 integration test files, no failures, no ignored)
  - Lib: 88 (was 84 in Plan 03; +3 BL-03 boundary tests + 1 BL-02 unit test = +4; one test had a name change but no addition: the BL-01 grep test was renamed, not added)
  - Integration: 11 files, 1 new (`tests/engine_row_order.rs`)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` exits 0
- All static greps in `<verification>` block pass (see "Pointers for Phase 2" above for the canonical references).

## Self-Check: PASSED

- File `tests/engine_row_order.rs` exists at `/home/chasel/REPO/AIHPBar/tests/engine_row_order.rs`
- All four commits resolve under `git log`:
  - `ca83523` (Task 1, BL-01)
  - `a6f281c` (Task 2, BL-02)
  - `d3f1c03` (Task 3, BL-03)
  - `0122e8f` (Task 4, WR-06 + WR-08)
- All acceptance greps in `<verification>` block return the expected counts (0 for forbidden patterns, ≥1 for required patterns).
- `cargo test --workspace 2>&1 | grep -E 'FAILED' | wc -l` returns 0.
