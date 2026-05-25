---
phase: 02-codex-output-formats
plan: 01
subsystem: adapter
tags: [codex, sqlite, jsonl, rusqlite, spawn_blocking, schema_drift, rust]

requires:
  - phase: 00-spike-spine
    provides: ProviderState / HpWindow / ResetInfo / ProviderError model contract
  - phase: 01-engine-claude-tui-scaffold
    provides: Engine fan-out, JoinSet panic isolation, Secret<T>, format_error_row sentinel, id_label helper
provides:
  - CodexProvider end-to-end (rusqlite read-only + JSONL rate_limits parse + spawn_blocking)
  - state_*.sqlite version-glob discovery with multi-version warn (D-46)
  - rate_limits null/missing → SchemaDrift mapping (D-47)
  - HpWindow passthrough labels primary/secondary (D-48)
  - Generalized SchemaDrift sentinel `{Label} adapter may be out-of-date` (formerly hardcoded Claude)
  - CLI compact row label now sourced from ProviderId (was windows[0].label)
  - Pitfall 3 RESERVED-lock integration test
affects: [02-02-detailed-block, 02-03-json-schema, 03-cache-refresh, future codex window expansion]

tech-stack:
  added: ["rusqlite 0.39 with bundled feature"]
  patterns:
    - "spawn_blocking narrow-scope wrap around sync IO (sqlite open + JSONL scan)"
    - "Per-provider Title-cased label via id_label_titlecase for sentinel phrases"
    - "Version-glob with integer-suffix sort (avoids lexicographic state_10 < state_5 inversion)"

key-files:
  created:
    - src/provider/codex/mod.rs
    - src/provider/codex/jsonl.rs
    - src/provider/codex/sqlite.rs
    - src/provider/codex/window.rs
    - tests/codex_sqlite_lock_resilience.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - src/provider/mod.rs
    - src/engine/mod.rs
    - src/cli/render_text.rs
    - src/tui/widgets/hp_row.rs
    - src/templates/default-config.toml
    - .planning/phases/01-engine-claude-tui-scaffold/01-UI-SPEC.md
    - .planning/phases/02-codex-output-formats/02-RESEARCH.md

key-decisions:
  - "Phase 2 opens state_*.sqlite read-only with busy_timeout=250ms and runs ZERO SELECT queries (RESEARCH Q1 RESOLVED) — schema reads deferred to Phase 3"
  - "SchemaDrift sentinel generalized to per-provider Title-cased label via id_label_titlecase (RESEARCH Q2 RESOLVED) — Claude rendering byte-identical to Phase 1"
  - "spawn_blocking wraps the sync IO segment only (sqlite open + JSONL scan), not the full async fetch — keeps lifetime + error-mapping clean per RESEARCH §spawn_blocking Pattern"
  - "Rule 2 [Missing Critical]: compact_line row label now sourced from id_label(state.id), not windows[0].label — UI-SPEC line 141 binding"
  - "Codex resets_at anchored on rollout LINE timestamp (NOT ctx.now) — RESEARCH §Codex JSONL Schema bullet 3; no_walltime test still passes (arithmetic on parsed timestamps allowed)"

patterns-established:
  - "Codex submodule layout mirrors Claude (mod/jsonl/window) + adds sqlite/ for the DB discovery layer"
  - "rusqlite OpenFlags = READ_ONLY | NO_MUTEX; busy_timeout(250ms) immediately after open"
  - "Per-provider lock-resilience integration test pattern (tempdir + writer-thread RESERVED lock + < 1.5s deadline)"

requirements-completed: [ADP-04]

duration: ~25min
completed: 2026-05-25
---

# Phase 2 Plan 01: Codex Adapter (rusqlite + JSONL + SchemaDrift sentinel generalization) Summary

**Codex adapter ships end-to-end: rusqlite read-only state-DB discovery + JSONL rate_limits parser + Codex-named SchemaDrift sentinel + Pitfall 3 lock-resilience guard, all behind a narrow-scope `spawn_blocking` wrap.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-05-25T03:00:00Z (approximate — captured at executor spawn)
- **Completed:** 2026-05-25T03:09:06Z
- **Tasks:** 2 (both completed atomically)
- **Files modified:** 9 modified, 5 created

## Accomplishments

- **Codex adapter ships end-to-end (ADP-04).** Discovery → `spawn_blocking { sqlite::open_readonly → drop → jsonl::parse }` → `Vec<HpWindow>` (passthrough primary/secondary). `~/.codex/state_*.sqlite` opens with `OpenFlags::SQLITE_OPEN_READ_ONLY | NO_MUTEX` + `busy_timeout(250ms)` and runs zero SELECT queries (Phase 2 contract — D-45). JSONL parsing handles the verified `RateLimitSnapshot` schema (issue #14728) — `primary`/`secondary` tiers, `used_percent → 100-used` for remaining, anchor on rollout line timestamp.
- **Schema-drift sentinel generalized.** Pre-Phase-2 hardcoded `Claude adapter may be out-of-date` is now `{Label} adapter may be out-of-date` via a new `id_label_titlecase(ProviderId) -> &'static str` helper. Claude rendering is byte-identical to Phase 1 (`tests/schema_drift_sentinel.rs` continues to pass unchanged); Codex `rate_limits: null` now correctly renders `codex  ▒▒▒▒▒▒▒▒▒▒ ??% • Codex adapter may be out-of-date`. The TUI widget (`src/tui/widgets/hp_row.rs`) was also updated to fix a pre-existing bug where Codex schema drift would falsely claim "Claude adapter…".
- **Pitfall 3 lock-resilience integration test.** New `tests/codex_sqlite_lock_resilience.rs` spawns a writer thread that holds a RESERVED lock on `state_5.sqlite` via `BEGIN IMMEDIATE`, then runs `AHB` as a subprocess. Asserts: process exits 0, elapsed < 1500 ms, first row starts with `codex  ` (Ok row or SchemaDrift sentinel — both acceptable), no `database is locked` substring in stdout. Test passes deterministically in ~60 ms on the local machine.

## Task Commits

1. **Task 1: Scaffold Codex submodule + rusqlite dep + RESEARCH RESOLVED markers** — `119388d` (feat)
2. **Task 2: Wire engine + generalize sentinel + lock-resilience test** — `cb90f56` (feat)

## Files Created/Modified

**Created:**
- `src/provider/codex/mod.rs` — `CodexProvider` struct + `Provider` impl wrapping sync IO in `spawn_blocking`; surfaces `Unavailable` for missing sqlite/rollouts and `SchemaDrift` for null rate_limits
- `src/provider/codex/jsonl.rs` — `discover_rollouts`, `pick_newest_file`, `parse_codex_rollout_windows`; serde structs (`RolloutLine`, `RolloutPayload`, `TokenCountPayload`, `RateLimits`, `RateLimitTier`)
- `src/provider/codex/sqlite.rs` — `discover_state_sqlite` (D-46 integer version sort + multi-version warn), `open_readonly` (250 ms busy_timeout, zero SELECT)
- `src/provider/codex/window.rs` — `to_hp_windows(&RateLimits, line_ts)` with D-48 passthrough order and `used_percent → 100-used` remaining math
- `tests/codex_sqlite_lock_resilience.rs` — Pitfall 3 guard

**Modified:**
- `Cargo.toml` — added `rusqlite = { version = "0.39", features = ["bundled"] }` with Phase 2 provenance comment
- `Cargo.lock` — automatic update
- `src/provider/mod.rs` — added `pub mod codex;`
- `src/engine/mod.rs` — replaced Phase-2 stub branch with real `CodexProvider::new(&home)` registration
- `src/cli/render_text.rs` — added `id_label_titlecase`; generalized `format_error_row_colored` SchemaDrift phrase; `compact_line_colored` now uses `id_label(state.id)` for the row label (Rule 2 deviation)
- `src/tui/widgets/hp_row.rs` — mirrored the sentinel generalization; fixed Codex-schema-drift test that previously asserted the Claude phrase
- `src/templates/default-config.toml` — updated `[providers.codex]` comment to reflect adapter is now wired
- `.planning/phases/01-engine-claude-tui-scaffold/01-UI-SPEC.md` — Phase 2 amendment footnote on the SchemaDrift sentinel row
- `.planning/phases/02-codex-output-formats/02-RESEARCH.md` — Open Questions heading → `## Open Questions (RESOLVED)`; Q1..Q5 per-question RESOLVED markers

## Decisions Made

- **rusqlite 0.39 with `bundled` feature** — confirms STACK.md recommendation; bundled SQLite avoids system-dep variance with Codex's own SQLite version. `cargo tree -i libsqlite3-sys` shows exactly one version (rusqlite-bundled is the only consumer; no sqlx ghost).
- **No `SELECT` queries in Phase 2** — Codex's `threads` table schema is internal-unstable (migration #34 dropped `thread_goals`; #23984 documents post-drop reads breaking). Opening + busy_timeout proves the D-45 "supplemental metadata" contract without exposing schema-drift surface. Reading any rows is deferred to Phase 3 if/when a concrete use case appears.
- **`Cow::Borrowed("codex-jsonl")` source string** — drops the `+sqlite` suffix because SQLite contributes no row data in Phase 2. Plan 02-02 / 02-03 may add a suffix when SQLite metadata first surfaces.
- **SchemaDrift sentinel generalization scope** — generalized in BOTH `cli/render_text.rs` (CLI compact + error path) AND `tui/widgets/hp_row.rs` (TUI render path). Pre-existing TUI test that asserted "Claude adapter…" for Codex schema drift was actually wrong; corrected.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] CLI compact row label sourced from `id_label(state.id)`, not `windows[0].label`**
- **Found during:** Task 2 (running the new `codex_sqlite_lock_resilience` integration test)
- **Issue:** The plan's integration test asserts `first row starts with "codex  "`, but Phase 1's `compact_line_colored` rendered `windows[0].label` directly. Claude's window label was `"claude"` so this coincidentally aligned with the UI-SPEC binding (line 141: "Provider labels in output use the lowercase provider name as it appears in `ProviderId`'s `snake_case` serialization"). Codex's D-48 passthrough labels `"primary"` / `"secondary"` broke that coincidence — compact output would say `primary  ████░░ 75% …` for Codex, not `codex  …`.
- **Fix:** Changed `compact_line_colored` to use `id_label(state.id)` for the row label (the per-window label still lives in the internal model — Plan 02-02 `--detailed` surfaces it). Updated 2 Mock-based unit tests (`compact_line_unicode_byte_exact`, `compact_line_ascii_byte_exact`) that asserted `mock-session  …` to now expect `mock  …`, aligning Mock compact with the UI-SPEC rule.
- **Files modified:** `src/cli/render_text.rs`
- **Verification:** All inline tests pass (135 tests across all binaries); the integration test `codex_sqlite_busy_does_not_crash_adapter` now asserts the literal `codex  ` prefix correctly. Claude row continues to render `claude  ` byte-identical (CLI walking-skeleton test continues to pass).
- **Committed in:** `cb90f56` (Task 2 commit)

**2. [Rule 1 - Bug] TUI schema-drift line hardcoded "Claude adapter may be out-of-date" for ALL providers**
- **Found during:** Task 2 (generalizing the CLI sentinel surfaced the parallel TUI hardcoding)
- **Issue:** `src/tui/widgets/hp_row.rs::build_schema_drift_line` had a `Span::styled("Claude adapter may be out-of-date", …)` hardcoded literal. Worse, the pre-existing TUI test `schema_drift_row_uses_id_label_not_hardcoded_claude` asserted `plain.contains("Claude adapter may be out-of-date")` *even when the provider id was Codex* — meaning a Codex schema drift in the TUI would falsely claim Claude was out of date.
- **Fix:** Added `id_label_titlecase` to the import; built `phrase = format!("{label_titlecased} adapter may be out-of-date", …)` per-call; updated the test to assert the correct per-provider phrase + added a second test (`schema_drift_row_for_claude_stays_byte_identical_to_phase_1_phrase`) to lock the Phase 1 byte-identity for Claude.
- **Files modified:** `src/tui/widgets/hp_row.rs`
- **Verification:** TUI tests pass; no regression on the CLI sentinel path.
- **Committed in:** `cb90f56` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 missing critical, 1 bug)
**Impact on plan:** Both deviations were direct consequences of Codex being the first multi-window provider AND the first non-`Claude` schema-drift case. The plan anticipated the sentinel generalization (Task 2 explicitly listed it) but did not anticipate the compact-row-label drift or the parallel TUI hardcoding. Both fixes preserve Phase 1 byte-identity for Claude. No scope creep — both directly support the plan's `<done>` criterion "Running `AHB` … prints both a `claude` row and a `codex` row".

## Issues Encountered

- **rusqlite OpenFlags ABI.** `OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX` compiles cleanly in 0.39; no API surprises. The `Connection::open_with_flags(path, flags)` signature matches RESEARCH §Codex SQLite Schema verbatim.
- **`tokio::task::spawn_blocking` lifetimes.** Inside `CodexProvider::fetch`, the closure captures owned `PathBuf` clones (`sqlite_path.clone()`, `newest_rollout.clone()`) so the `move + 'static` bound is trivially satisfied. No fighting with `&FetchCtx<'_>` lifetimes (we don't need to capture `ctx.secrets` because Codex needs no secrets).
- **`window_minutes` field unused.** The serde struct deserializes it for forward-compat, but Phase 2 never reads it — added `#[allow(dead_code)]` with a comment noting Phase 3 may surface it in `--detailed`. Clippy `pedantic` still passes.

## BL-02 Row-Order Proof (claude + codex side-by-side)

`Engine::sort_key` locks Claude=0 / Codex=1 / Gemini=2 / Mock=3. The integration tests `tests/engine_row_order.rs` (Phase 1) continue to pass; combined with the new `tests/codex_sqlite_lock_resilience.rs` (which configures only codex and asserts the row appears), the engine boundary correctly emits codex at index 1 when both Claude and Codex are enabled. No new ordering test was added because Phase 1's `engine_row_order.rs` already exercises the multi-provider permutation and Phase 2 did not modify `Engine::sort_key`.

## Pitfall 3 Lock-Resilience Test Determinism

The `codex_sqlite_lock_resilience` test runs deterministically in ~60 ms on the local machine (well under the 1500 ms ceiling). The writer thread `BEGIN IMMEDIATE`s on `state_5.sqlite` and holds the RESERVED lock for the full duration of the AHB subprocess; AHB's `busy_timeout(250ms)` never even has to fire because the read-only `Connection::open_with_flags` path does not contend with the writer's RESERVED lock in practice — but the contract is exercised end-to-end (the test would catch a regression where AHB tried to take a write lock, used the wrong flags, or hung > 1500 ms).

## User Setup Required

None — this plan is a pure code change. Users opting into Codex set `[providers.codex] enabled = true` in `~/.config/ahb/config.toml` and run `AHB`.

## Next Phase Readiness

- Plan 02-02 (`--detailed` + Claude weekly window) can proceed: Codex's `windows: Vec<HpWindow>` shape already supports multi-window per provider (D-48); `detailed_block` will iterate `state.windows` and render one indented line per window.
- Plan 02-03 (`--json schema_version: 1`) can proceed: Codex now emits `ProviderState { id: Codex, windows, source: "codex-jsonl", fetched_at }` cleanly serializable through the upcoming `JsonProvider` DTO.
- No blockers for downstream plans. The sentinel generalization in `id_label_titlecase` is the only cross-cutting API surface added; Plan 02-02 / 02-03 may import it from `cli::render_text` without further changes.

---

## Self-Check: PASSED

**Files created — verified exist:**
- src/provider/codex/mod.rs — FOUND
- src/provider/codex/jsonl.rs — FOUND
- src/provider/codex/sqlite.rs — FOUND
- src/provider/codex/window.rs — FOUND
- tests/codex_sqlite_lock_resilience.rs — FOUND

**Commits — verified in `git log`:**
- 119388d (Task 1) — FOUND
- cb90f56 (Task 2) — FOUND

**Final cargo test count:** 135 passing, 0 failing across all test binaries.
**Final `cargo tree -i libsqlite3-sys`:** exactly one version (rusqlite-bundled is the only consumer).
**Final `grep '\*\*RESOLVED' 02-RESEARCH.md`:** 5 markers present.

---

*Phase: 02-codex-output-formats*
*Completed: 2026-05-25*
