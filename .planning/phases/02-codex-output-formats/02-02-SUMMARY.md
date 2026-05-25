---
phase: 02-codex-output-formats
plan: 02
subsystem: cli
tags: [detailed, claude_weekly, render_text, clap, rust, hpwindow_detailed_label, jiff, iso_week_anchor]

requires:
  - phase: 00-spike-spine
    provides: ProviderState / HpWindow / ResetInfo model contract (D-09 — extended additively in this plan with `detailed_label`)
  - phase: 01-engine-claude-tui-scaffold
    provides: ClaudeProvider 5h cluster + percent_remaining + compact_line_colored + id_label + format_countdown + Cli/run_compact dispatch surface
  - phase: 02-codex-output-formats (Plan 02-01)
    provides: id_label_titlecase generalized sentinel + compact row label sourced from id_label(state.id) (the deviation that made the Phase 1 `claude  ` prefix detached from `windows[0].label`)
provides:
  - HpWindow.detailed_label additive field (D-52 schema_version policy)
  - CLAUDE_WEEKLY_TOKEN_LIMIT (locked None for Phase 2) + WeekAnchor enum + CLAUDE_WEEKLY_ANCHOR const
  - next_iso_week_anchor(now) jiff helper (best-effort Monday 00:00 LOCAL anchor)
  - compute_weekly_window(sorted_msgs, now) builder (passthrough order [5h, weekly])
  - ClaudeProvider emits 2 HpWindows (windows[0].label = "claude" PRESERVED; windows[0].detailed_label = Some("5h"); windows[1] = weekly + Some("weekly"))
  - cli::render_text::detailed_block + render_window_row + effective_label
  - cli::run_detailed dispatch fn + Cli.detailed flag
  - main.rs dispatch branches on cli.detailed
  - tests/detailed_format.rs integration coverage (D7 single-provider shape + D8 two-provider separator)
affects: [02-03-json-schema-and-argroup, 03-cache-refresh, README weekly-bar best-effort note]

tech-stack:
  added: []  # No new deps — uses jiff 0.2 already in tree, owo-colors already wired
  patterns:
    - "Additive serde field via `#[serde(skip_serializing_if = Option::is_none, default)]` (D-52 schema_version policy realized in code)"
    - "Per-row detailed_label override with effective_label() fallback to label (compact/detailed routing without breaking compact)"
    - "jiff Date-arithmetic via to_monday_one_offset() — Timestamp arithmetic rejects calendar units (days/months/years); use Date.checked_add(Span::days) on civil dates then re-zone, OR use Span::hours(N*24) for Timestamp"
    - "NaN-as-sentinel pattern for limit-unknown (distinct from SchemaDrift `??%` sentinel) — render layer detects `pct.is_nan()` and paints U+2592 + ??% + `(limit unknown)` footer"

key-files:
  created:
    - tests/detailed_format.rs
  modified:
    - src/model.rs
    - src/provider/claude/window.rs
    - src/provider/claude/mod.rs
    - src/provider/mock.rs
    - src/provider/codex/window.rs
    - src/cli/render_text.rs
    - src/cli/mod.rs
    - src/main.rs
    - src/tui/widgets/hp_row.rs
    - src/tui/app.rs
    - src/engine/fanout.rs

key-decisions:
  - "Phase 2 deviation from naive D-54: introduce additive `HpWindow.detailed_label` field (NOT rename `windows[0].label`). Preserves Phase 0 D-25 `mock-session` literal AND Phase 1 `claude  ` compact prefix at byte level. Plan's `<locked_decisions_deviation>` block captures the rationale verbatim."
  - "CLAUDE_WEEKLY_TOKEN_LIMIT locked at None for Phase 2 — Anthropic publishes no reliable estimate (May-Jul 2026 +50% complication; community estimates vary 5x-7x of 5h budget). Render layer paints NaN as `▒▒▒▒▒▒▒▒▒▒ ??% (limit unknown)`. Upgrade path = one-line edit to `Some(220_000)` or similar Pro-tier estimate."
  - "Claude weekly anchor = ISO-week Monday 00:00 LOCAL (jiff to_monday_one_offset + Date.checked_add(Span::days) + zone-back). Acknowledged best-effort — Anthropic does not document reset cadence. WeekAnchor enum locks the type shape so a future plan can add `FirstPrompt` without re-engineering."
  - "render_window_row bar-build duplicates compact_line_colored intentionally per plan. PATTERNS Pattern 6 flags `render_bar_segment` factoring as future Plan 03+ refactor — keeping duplication keeps Plan 02-02 narrow and risk-free."
  - "format_countdown reused as-is — weekly bar shows `156h34m` (no day decomposition), which differs from CONTEXT D-53's illustrative `4d06h` example. Format_countdown is Phase 1 LOCKED and shared with compact (D-56 binding). Future plan may add `Xd{HH}h` decomposition for spans > 24h; would need to be applied to compact too (would change a Phase 1 byte-locked literal — Rule 4 territory)."
  - "`run_detailed` empty-state path mirrors `run_compact`'s heading + body literal pair (shared Phase 1 LOCKED literal). tmux user switching modes sees consistent empty-state copy."
  - "Mock provider's `detailed_label = None` — the detailed renderer's effective_label fallback uses `label = `mock-session``, so the indented Mock row reads `mock-session  …` (preserves D-25 intent at the *per-row* level; the compact row's `mock  …` prefix is from id_label per Plan 02-01)."

patterns-established:
  - "Pattern A1: additive HpWindow field with serde skip_if + default — proves D-52 policy lands cleanly. Future window-level fields (bar_color_hint, last_active_at) should follow this template."
  - "Pattern A2: effective_label(w) helper for per-row label resolution — detailed-mode reads w.detailed_label.as_deref().unwrap_or(&w.label). Future renderers (TUI detailed view, JSON DTO) should reuse the same fallback pattern."
  - "Pattern A3: NaN-percent-as-limit-unknown sentinel + (limit unknown) suffix in renderer — distinct from SchemaDrift `out-of-date`. Future adapters that can't compute a meaningful percent (rate-limited window with no upstream signal, etc.) can re-use the NaN sentinel."
  - "Pattern A4: jiff Date-arithmetic for calendar units (days/months/years); Timestamp-arithmetic restricted to hours-or-smaller. Adapter Span::days() on a Timestamp panics at runtime — use Span::hours(N*24) or route through civil::Date.checked_add."
  - "Pattern A5: provider-block separator `if i < last_idx { println!(); }` — emits N-1 blank lines for N blocks, no trailing blank. Re-use for any future multi-provider output mode."

requirements-completed: [CORE-03]

duration: 12min
completed: 2026-05-25
---

# Phase 2 Plan 02-02: --detailed Output + Claude Weekly Bar Summary

**Additive `HpWindow.detailed_label` field + `compute_weekly_window` (NaN sentinel when limit unknown) + `cli::run_detailed` dispatch + 2 integration tests — Claude shows both `5h` and `weekly` rows under `--detailed` while compact stays byte-identical to Phase 1.**

## Performance

- **Duration:** ~12 min (2 atomic commits)
- **Started:** 2026-05-25T03:14:58Z
- **Completed:** 2026-05-25T03:26:27Z
- **Tasks:** 2 (both completed atomically; one Rule 1 bug fix inline)
- **Files modified:** 10 modified, 1 created

## Accomplishments

- **`AHB --detailed` ships end-to-end (CORE-03).** Smoke output:
  ```
  claude
    5h      █████████░ 90% • resets in 12h34m
    weekly  ▒▒▒▒▒▒▒▒▒▒ ??% • resets in 156h34m (limit unknown)
  ```
  Per-row labels come from `effective_label(w) = w.detailed_label.unwrap_or(&w.label)`, label-padding aligns dynamically to the longest effective label across windows, bar styling mirrors compact 1:1 (D-56 binding), `--ascii` substitutes glyphs, `--color=never` produces zero ANSI bytes (D-58).
- **Additive `HpWindow.detailed_label` field (D-52).** `Option<Cow<'static, str>>` with `#[serde(skip_serializing_if = "Option::is_none", default)]` — existing JSON consumers see byte-identical shape; new consumers can read `"detailed_label":"5h"` when present. Touches every `HpWindow {}` construction site (10 in total — see audit below).
- **Claude weekly window infrastructure.** `CLAUDE_WEEKLY_TOKEN_LIMIT: Option<u64> = None` (Phase 2 safe path — Anthropic publishes no reliable estimate); `WeekAnchor::Iso` enum + `CLAUDE_WEEKLY_ANCHOR` const lock the type shape; `next_iso_week_anchor(now)` returns the next Monday 00:00 LOCAL converted to UTC; `compute_weekly_window(sorted_msgs, now)` sums `cache_creation_input_tokens` across `[reset_at - 7d, now]` and builds the HpWindow with NaN sentinel when limit is None. ClaudeProvider emits the weekly window in passthrough order `[5h, weekly]` (D-55).
- **Phase 0 D-25 + Phase 1 compact literals BYTE-IDENTICAL.** `mock-session` model literal preserved (mock.rs `label: Cow::Borrowed("mock-session")`). `^claude  ` compact prefix preserved — Plan 02-01 already routed the compact row label through `id_label(state.id)` so detaching `windows[0].label` from the compact prefix is no-op here; new `compact_prefix_preserves_phase1_literal` test pins the invariant at the model→renderer boundary.

## Task Commits

Each task was committed atomically:

1. **Task 1: HpWindow.detailed_label + Claude weekly window infrastructure + dual-window emit** — `8c93606` (feat)
2. **Task 2: render_text::detailed_block + run_detailed dispatch + --detailed flag + integration test** — `c7f6ff8` (feat)

## Files Created/Modified

**Created:**
- `tests/detailed_format.rs` — D7 (single-provider shape: header + 5h row regex + weekly NaN footer + zero ANSI bytes) + D8 (two-provider blank-line separator contract, no trailing blank)

**Modified:**
- `src/model.rs` — Added `pub detailed_label: Option<Cow<'static, str>>` to `HpWindow` with `skip_serializing_if = "Option::is_none", default` attribute. Added M1/M2 inline tests. Updated existing `provider_state_serde_roundtrip` HpWindow literal with `detailed_label: None`.
- `src/provider/claude/window.rs` — Added `CLAUDE_WEEKLY_TOKEN_LIMIT`, `WeekAnchor` enum, `CLAUDE_WEEKLY_ANCHOR` const, `next_iso_week_anchor`, `compute_weekly_window`. Added W1-W6 inline tests + `make_entry_ts` helper.
- `src/provider/claude/mod.rs` — `ClaudeProvider::fetch` now builds `vec![win_5h]` then `if let Some(w) = weekly { windows.push(w); }`. `windows[0].label = "claude"` PRESERVED. Added `detailed_label = Some("5h")` on win_5h. Updated existing `fetch_against_tempdir_with_one_assistant_entry` test to assert windows.len()==2 + both labels + NaN weekly. Added new `compact_prefix_preserves_phase1_literal` test.
- `src/provider/mock.rs` — Added `detailed_label: None` to the existing HpWindow literal (D-25 LOCKED `mock-session` label preserved).
- `src/provider/codex/window.rs` — Added `detailed_label: None` to the existing `tier_to_window` HpWindow literal. Codex passes through `label = "primary"` / `"secondary"` which already serves as a meaningful per-row label; explicit None documents intent.
- `src/cli/render_text.rs` — Imported `HpWindow`. Added `fn effective_label(w)`, `pub(crate) fn detailed_block`, `fn render_window_row`. **`compact_line_colored` UNCHANGED**. Added 6 inline tests D1-D6. Added `detailed_label: None` to the existing `make_state` test fixture.
- `src/cli/mod.rs` — Added `pub detailed: bool` field on `Cli` (`#[arg(long)]`, NO `conflicts_with`). Added `pub async fn run_detailed`. Added 2 new mod-level tests (`run_detailed_with_empty_engine_prints_empty_state` + `run_detailed_with_mock_provider_succeeds`).
- `src/main.rs` — Dispatch match now branches on `cli.detailed` for the no-subcommand path; explicit comment notes Plan 02-03 will introduce the full ArgGroup.
- `src/tui/widgets/hp_row.rs` — Added `detailed_label: None` to the existing `ok_row` test fixture HpWindow literal (TUI unchanged in this plan; field-add was a compile-only change).
- `src/tui/app.rs` — Same `detailed_label: None` compile-only patch on test fixture.
- `src/engine/fanout.rs` — Same `detailed_label: None` compile-only patch on test fixture.

## HpWindow Construction Site Audit

Full enumeration (per the plan's Task 1 Step 1 "CRITICAL: Audit every construction site"):

| File | Site | detailed_label value | Note |
|------|------|---------------------|------|
| `src/model.rs:163` | tests::provider_state_serde_roundtrip fixture | `None` | round-trip test |
| `src/provider/mock.rs:51` | `MockProvider::fetch` production HpWindow | `None` | D-25 `mock-session` preserved |
| `src/provider/claude/mod.rs:119` | `ClaudeProvider::fetch` 5h window (Task 1 setting!) | **`Some(Cow::Borrowed("5h"))`** | Override for detailed mode |
| `src/provider/claude/window.rs:215` | `compute_weekly_window` weekly window (Task 1 setting!) | **`Some(Cow::Borrowed("weekly"))`** | Override for detailed mode |
| `src/provider/claude/mod.rs:223` | tests::compact_prefix_preserves_phase1_literal fixture | `Some(Cow::Borrowed("5h"))` | mirrors production shape |
| `src/provider/codex/window.rs:47` | `tier_to_window` (Codex passthrough) | `None` | fallback to `label="primary"/"secondary"` |
| `src/cli/render_text.rs:252` | tests::make_state fixture | `None` | compact-line tests |
| `src/cli/render_text.rs::detailed_block_*` | D1..D6 fixtures inline | mix of `Some` / `None` | covers both paths |
| `src/tui/widgets/hp_row.rs:159` | tests::ok_row fixture | `None` | TUI unchanged |
| `src/tui/app.rs:109` | tests fixture | `None` | TUI unchanged |
| `src/engine/fanout.rs:150` | tests::OkProvider fixture | `None` | engine fan-out test |

**Total: 11 construction sites — all updated; `cargo build` clean on first try after the audit pass.**

## Decisions Made

See frontmatter `key-decisions` block for the substantive list. Quick recap:

1. **Additive field instead of rename** — preserves Phase 0 + Phase 1 byte-locked literals; net behavior change is the JSON-shape gain of `"detailed_label"` on Claude's two windows.
2. **`CLAUDE_WEEKLY_TOKEN_LIMIT = None`** — safest Phase 2 default; NaN sentinel + `(limit unknown)` footer is visually distinct from SchemaDrift.
3. **ISO-week Monday anchor** — community-consensus best guess; documented in const doc-comment as "Anthropic does not document reset cadence; revisit when upstream clarifies".
4. **Bar-build duplication acceptable** — per plan's `<detailed_block_layout>` note. Future Plan 03+ refactor candidate.
5. **format_countdown reused as-is** — weekly bar shows `156h34m` not `4d06h` from CONTEXT example. Plan to amend format_countdown is Rule 4 (would touch Phase 1 byte-locked compact literal).
6. **Empty-state mirrors compact** — tmux UX consistency.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `jiff::Timestamp::checked_sub(Span::days(N))` rejects calendar units at runtime**
- **Found during:** Task 1 (running W4 / W6 tests after writing `compute_weekly_window`)
- **Issue:** jiff 0.2 enforces `Timestamp` arithmetic to be limited to hours-or-smaller units — `Span::days/months/years` are rejected because they are time-zone-dependent. The pseudocode in RESEARCH §Claude Weekly Limit Handling (lines 418-424) and in the plan's `<compute_weekly_window_pattern>` used `Span::new().days(7)` on a `Timestamp`, which panics at `unwrap()` time with: *"operation can only be performed with units of hours or smaller, but found non-zero 'day' units (operations on `jiff::Timestamp`, `jiff::tz::Offset` and `jiff::civil::Time` don't support calendar units in a `jiff::Span`)"*.
- **Fix:** Replaced `Span::new().days(7)` with `Span::new().hours(7 * 24)` in both `compute_weekly_window` (the `week_start` computation) and the W4/W6 test fixtures (arithmetic relative to `week_start`). Note `next_iso_week_anchor`'s use of `Span::new().days(days_until_next_monday)` on a `civil::Date` (NOT a Timestamp) IS valid — `Date` supports calendar arithmetic; this is the correct surface for "next Monday" math.
- **Files modified:** `src/provider/claude/window.rs` (one production line + two test fixtures).
- **Verification:** W4 + W6 tests pass on re-run; W2 + W3 (which use only `next_iso_week_anchor` + Date arithmetic) pass unchanged.
- **Committed in:** `8c93606` (Task 1 commit).

---

**Total deviations:** 1 auto-fixed (1 bug).
**Impact on plan:** Rule 1 fix in scope; no scope creep. The plan's pseudocode was wrong on the same point; future plans referencing weekly arithmetic should use `Span::hours(N*24)` on Timestamps OR `Date.checked_add(Span::days(N))` on civil dates.

## Issues Encountered

- **CONTEXT D-53 example shows `4d06h` countdown for weekly, but `format_countdown` produces `156h34m`.** Not a regression — Phase 1's `format_countdown` is shared with compact (D-56 binding) and was Phase 1-LOCKED. Adding day decomposition would change a Phase 1 byte-locked compact literal (e.g. a 30h compact countdown would render `1d06h` instead of `30h00m`), which is Rule 4 territory (architectural change requiring user decision). Documented under "Decisions Made #5" as a known visual delta from CONTEXT's illustrative example. The functional contract (header + indented rows + NaN sentinel + color-off) is fully delivered.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| (none) | — | No new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries. JSON-shape gain (`detailed_label` field) is additive per D-52 and SEC-03's existing `format_one_line` sanitizer + Secret<T> Serialize redaction continue to apply unchanged. |

## Note for Plan 02-03

Plan 02-03's `JsonWindow` DTO needs an additive `pub detailed_label: Option<String>` field with `#[serde(skip_serializing_if = "Option::is_none")]` to round-trip the new model field cleanly. Per the plan's `<locked_decisions_deviation>` block: "Plan 02-03 should mirror this additive field as `pub detailed_label: Option<String>` with `#[serde(skip_serializing_if = "Option::is_none")]`. Plan 02-03 will add this to the DTO; the round-trip stays clean." Additive per D-52 — no schema_version bump.

## User Setup Required

None — pure code change. Users opting into Claude continue to set `[providers.claude] enabled = true` in `~/.config/ahb/config.toml` and run `AHB --detailed` to see the two-window view. The weekly bar will display `??% (limit unknown)` until a future plan locks `CLAUDE_WEEKLY_TOKEN_LIMIT` to a real `Some(N)` value (or until upstream documents the actual reset cadence and AHB adopts it).

## Next Phase Readiness

- **Plan 02-03 (`--json schema_version: 1` + clap ArgGroup) can proceed.** `Cli.detailed` flag lives in isolation right now; Plan 02-03 introduces the full `--compact / --detailed / --json` ArgGroup interlock (D-57). The additive `HpWindow.detailed_label` field rolls cleanly into the `JsonWindow` DTO per the note above.
- **Future polish (post-Phase-2):**
  - Add day-decomposition to `format_countdown` (would need to coordinate with Phase 1 compact literal — Rule 4 territory; consider a separate `format_long_countdown` for >24h spans).
  - Flip `CLAUDE_WEEKLY_TOKEN_LIMIT` to `Some(220_000)` (or a per-tier config knob via CFG-03) once Anthropic publishes a reliable estimate.
  - Surface `window_minutes` from Codex's rate_limits in detailed mode (RESEARCH §Codex JSONL Schema bullet 6 — "primary 5h window" annotation).

## Compact-Line Phase 1 Byte-Identity Proof

`tests/cli_walking_skeleton.rs::ahb_default_run_emits_one_claude_row_with_real_numbers` passed BEFORE Plan 02-02 AND continues to pass AFTER, asserting `^claude  \S{10}\s+\d{1,3}%\s+.\s+resets in \d+h\d{2}m$`. The new `compact_prefix_preserves_phase1_literal` unit test additionally pins the byte-level `claude  ` prefix at the model→renderer boundary so a future refactor of `compact_line_colored` can't silently regress it.

`src/provider/mock.rs::tests::mock_returns_expected_shape` continues to pass with the assertion `state.windows[0].label == "mock-session"` (D-25 LOCKED).

---

## Self-Check: PASSED

**Files created — verified exist:**
- `tests/detailed_format.rs` — FOUND

**Files modified — verified exist:**
- `src/model.rs` — FOUND (additive `detailed_label` field at line 69)
- `src/provider/claude/window.rs` — FOUND (CLAUDE_WEEKLY_TOKEN_LIMIT at line 41; compute_weekly_window at line 193; next_iso_week_anchor at line 159)
- `src/provider/claude/mod.rs` — FOUND (windows.push(weekly) at line 130; detailed_label: Some("5h") at line 125)
- `src/provider/mock.rs` — FOUND (detailed_label: None preserved D-25)
- `src/provider/codex/window.rs` — FOUND (detailed_label: None on Codex tier_to_window)
- `src/cli/render_text.rs` — FOUND (detailed_block at line 276; effective_label at line 249)
- `src/cli/mod.rs` — FOUND (Cli.detailed flag + run_detailed at line 116)
- `src/main.rs` — FOUND (cli.detailed branch at line 99)
- `src/tui/widgets/hp_row.rs` + `src/tui/app.rs` + `src/engine/fanout.rs` — all FOUND (compile-only field-add patches)

**Commits — verified in git log:**
- 8c93606 (Task 1: `feat(02-02): add HpWindow.detailed_label + Claude weekly window`) — FOUND
- c7f6ff8 (Task 2: `feat(02-02): render_text::detailed_block + --detailed dispatch`) — FOUND

**Final cargo test count:** 153 passing, 0 failing across all test binaries (128 lib + 25 integration including new 2 detailed_format tests).
**Final smoke output:** `--detailed --color=never` against a fake Claude JSONL produces the documented 3-line block (`claude` + `  5h …` + `  weekly … (limit unknown)`); compact mode unchanged.

---

*Phase: 02-codex-output-formats*
*Completed: 2026-05-25*
