---
phase: 00-spike-spine
plan: 03
subsystem: infra
tags: [rust, cli, mock, render, bar, skeleton, clap, tokio, async-trait, jiff, panic-hook]

requires:
  - phase: 00-spike-spine/01
    provides: "Cargo manifest with 9 pinned deps + Phase 0 lint floor + placeholder main.rs"
  - phase: 00-spike-spine/02
    provides: "ProviderState/HpWindow/ResetInfo/ProviderId/ProviderError contract types + Provider trait + FetchCtx + Secrets stub (Cow<'static, str> source field per W-2; struct-variant ProviderError::Internal { source } / ::Network { source } per Plan 02 deviation)"

provides:
  - "src/provider/mock.rs: MockProvider implementing Provider trait, returns the D-25 fixture (label=mock-session, percent=60.0, resets_at=ctx.now+2h, source=Cow::Borrowed(\"mock\")), uses injected clock, no wall-clock reads"
  - "src/cli/mod.rs + src/cli/render_text.rs: compact_line(&ProviderState, &Timestamp, ascii) -> String produces the byte-exact Phase 0 HP bar line; BAR_WIDTH=10 (D-16); Unicode (\\u{2588}/\\u{2591}) + U+2022 or ASCII (#/-/|) (D-15/D-18); B-1 binding (Span::new() fallback) honored"
  - "src/main.rs: panic-hook + clap parser (Cli with --ascii / --color) + tokio current_thread runtime + MockProvider invocation via Box<dyn Provider> path -> render -> println; W-4 honored (no owo_colors import in Phase 0)"
  - "Working release binary: cargo build --release && ./target/release/ahb prints exactly one line matching ^mock-session  ██████░░░░ 60% • resets in [0-9]+h[0-9]{2}m$"
  - "ASCII fallback: ./ahb --ascii prints ^mock-session  ######---- 60% \\| resets in [0-9]+h[0-9]{2}m$"
  - "Phase 0 panic-hook contract installed in correct order for Phase 1 ratatui::init() composition (D-27 + Pitfall 5)"

affects:
  - 00-spike-spine/04 (Gemini spike memo — informational only; no source dependency)
  - 01-engine-claude-tui (will replace MockProvider with real Claude adapter via the same Provider trait; will wrap install_phase0_panic_hook via ratatui::init)
  - 02-codex-output (will add Codex adapter via the same trait; will reuse render_text or its successor)
  - 03-gemini-cache (Gemini stub-or-real per spike memo; trait path proven by this plan)
  - 04-distribution (release binary verified to build and exit 0; cargo-dist will package the same path)

tech-stack:
  added: []  # All deps were already pinned by Plan 01; this plan only consumes them.
  patterns:
    - "Pattern A (trait-first runtime spine): MockProvider invoked through Box<dyn Provider>-compatible call (`mock.fetch(&ctx).await`); B-2 guard ensures no bypass-the-trait println shortcut"
    - "Pattern B (clock injection at the entry boundary): src/main.rs reads jiff::Timestamp::now() exactly once into FetchCtx.now; every downstream call uses ctx.now. MockProvider is forbidden from calling Timestamp::now() (acceptance grep enforces)"
    - "Pattern C (Phase-0 panic-hook contract): take_hook + set_hook composition placed as the FIRST line of main, before Cli::parse / runtime / provider. Documented in code comment + RESEARCH Pitfall 5 + D-27 so Phase 1 ratatui::init wraps cleanly"
    - "Pattern D (ASCII-clean source for Unicode output): bar source uses `\\u{2588}` / `\\u{2591}` escapes, not literal block chars. Tests verify byte sequence (e2 96 88 / e2 96 91 / e2 80 a2) via .as_bytes().windows(3).any(...)"
    - "Pattern E (jiff Span decomposition via since((Unit::Hour, now))): Timestamp::since defaults to seconds-largest; passing (Unit::Hour, now) yields a Span with .get_hours() + .get_minutes() decomposed for `Xh00m` formatting"

key-files:
  created:
    - "src/provider/mock.rs (88 lines): MockProvider unit struct + #[async_trait] impl Provider + 4 inline tests + 2 assert_impl_all! lines"
    - "src/cli/mod.rs (1 line): `pub mod render_text;`"
    - "src/cli/render_text.rs (190 lines): BAR_WIDTH const, compact_line public fn, filled_cells / pct_int / format_countdown helpers, 5 inline tests"
  modified:
    - "src/provider/mod.rs: added `pub mod mock;` line after FetchCtx use statements"
    - "src/lib.rs: added `pub mod cli;` to expose the renderer module"
    - "src/main.rs: rewritten — installs Phase 0 panic hook, parses Cli (clap derive), constructs MockProvider, fetches via trait, renders via compact_line, prints"

key-decisions:
  - "Use jiff::Timestamp::since((Unit::Hour, now)) — not the default since() — so the returned Span decomposes into hours+minutes. The plan's verbatim transcription from RESEARCH (`target.since(*now).unwrap_or_else(|_| jiff::Span::new())`) returns a seconds-largest Span where get_hours() reports 0; this fails the `2h00m` byte-exact test. Adding the `Unit::Hour` largest-unit hint preserves the B-1 binding (still unwrap_or_else, still Span::new() fallback) and is functionally equivalent for the spec."
  - "Scoped clippy allows over filled_cells / pct_int helpers — extract f32→usize / f32→u32 casts into two tiny `#[allow(...)]`-decorated helpers so the cast-precision lints stay local. Alternative was a file-level #[allow] which would have hidden future cast bugs."
  - "Negative-span guard in format_countdown — if target < now (e.g. wall-clock drift across reset boundary in a long-running TUI), Timestamp::since returns negative h/m. Tests assert that `now - 1h` collapses to `0h00m` via .max(0). Phase 1 will probably want a different rendering (e.g. `RESET NOW`) but Phase 0 keeps it laconic."
  - "Plan 02 deviation absorbed without code impact — ProviderError::Internal { source } and ::Network { source } are struct variants (not newtype). MockProvider never constructs a ProviderError (always returns Ok), so the deviation has zero call-site cost in this plan."
  - "W-4 honored — no `use owo_colors` import in any Phase 0 source file. The crate is still pinned in Cargo.toml for Phase 1, but Phase 0 ships uncolored. Clippy's unused-import check enforces this automatically."

patterns-established:
  - "Pattern A: end-to-end Phase-0 spine. main.rs reads wall clock once, constructs FetchCtx, dispatches MockProvider via Provider trait, renders via compact_line, prints. Phase 1+ adapters drop into the same flow by replacing MockProvider with a Vec<Box<dyn Provider>>."
  - "Pattern B: jiff Span hour-largest formatting via since((Unit::Hour, now)). Any future countdown rendering should use this idiom rather than the default since() to get balanced hours+minutes."
  - "Pattern C: ASCII-clean Rust source for Unicode output — literal U+2588 / U+2591 / U+2022 never appear in source; all are `\\u{...}` escapes. Tests verify the byte sequence post-format."
  - "Pattern D: scoped #[allow(clippy::cast_*)] over tiny helper fns instead of file-level allows. Keeps cast-precision lints honest for future cast sites."

requirements-completed: [ADP-00]

duration: 6m
completed: 2026-05-22
---

# Phase 00 Plan 03: Runtime Spine Wiring Summary

**Phase 0 spine wired end-to-end — `cargo run --release` prints exactly one HP-bar line (`mock-session  ██████░░░░ 60% • resets in 2h00m`) through `MockProvider::fetch` -> `ProviderState` -> `compact_line` -> `println!`, with Phase 0 panic-hook installed in the correct order for Phase 1's `ratatui::init()` to wrap.**

## Performance

- **Duration:** ~6 min (351 seconds wall clock)
- **Started:** 2026-05-22T13:25:10Z
- **Completed:** 2026-05-22T13:31:01Z
- **Tasks:** 3 / 3
- **Files created:** 3 (src/provider/mock.rs, src/cli/mod.rs, src/cli/render_text.rs)
- **Files modified:** 3 (src/provider/mod.rs, src/lib.rs, src/main.rs)

## Accomplishments

- **Phase 0 ROADMAP success criterion #2 met:** `./target/release/ahb` prints `mock-session  ██████░░░░ 60% • resets in 2h00m` and exits 0. Verified byte-by-byte via `xxd` — 6 × U+2588, 4 × U+2591, 1 × U+2022.
- **The bar value flows through the locked `Provider` trait**, NOT a hardcoded `println!`. B-2 guards prove this: `grep -q 'mock.fetch(&ctx).await' src/main.rs` succeeds; `grep -qE 'println!\([^)]*mock.session' src/main.rs` fails (no hardcoded label-string println).
- **`--ascii` works:** `./ahb --ascii` prints `mock-session  ######---- 60% | resets in 2h00m` honoring D-18.
- **`--color` and `--version` and `--help` work:** clap-derived CLI surface accepts `--color=auto|always|never` (parsed but not applied — Phase 0); `--version` prints `ahb 0.0.1`; `--help` shows the D-01 description line.
- **Phase 0 panic hook installed correctly:** `install_phase0_panic_hook()` is the first line of `main`, composes via `std::panic::take_hook()` + `set_hook()` so Phase 1's `ratatui::init()` chains correctly (D-27 + RESEARCH Pitfall 5). Documented in code with a contract comment.
- **Clock-injection contract honored:** `src/main.rs` reads `jiff::Timestamp::now()` exactly once (the only wall-clock read in the entire binary); `MockProvider` uses `ctx.now`, never wall-clock. Acceptance grep `! grep -E 'jiff::Timestamp::now\(\)' src/provider/mock.rs` succeeds.
- **W-4 honored:** No `use owo_colors` import anywhere in Phase 0 source. The crate stays in Cargo.toml for Phase 1, but Phase 0 ships uncolored.
- **`cargo build --release` ✓, `cargo test --all-targets` ✓ (15/15 pass — 4 model + 2 provider + 4 mock + 5 render_text), `cargo clippy --all-targets -- -D warnings` ✓ — all exit 0.**

## Task Commits

Each task was committed atomically on `master`:

1. **Task 1: MockProvider returning the D-25 fixture** — `63be300` (feat)
2. **Task 2: compact_line renderer + cli module** — `d148678` (feat)
3. **Task 3: main.rs rewrite — panic hook + clap + MockProvider wire-up** — `60666e5` (feat)

_Plan metadata commit follows this SUMMARY._

## Files Created/Modified

### Created

- **`src/provider/mock.rs`** (88 lines) — `MockProvider` unit struct + `#[async_trait] impl Provider`. Constructs `ProviderState { id: Mock, windows: [HpWindow { label: Cow::Borrowed("mock-session"), percent_remaining: 60.0, reset: ResetInfo { resets_at: ctx.now + 2h }, bar_color: None }], fetched_at: ctx.now, source: Cow::Borrowed("mock") }`. Four inline tests + two `assert_impl_all!` compile-time proofs (`MockProvider: Send + Sync` and `Box<dyn Provider>: Send + Sync`).
- **`src/cli/mod.rs`** (1 line) — `pub mod render_text;`. Exposes the renderer to both `main.rs` and any future integration test.
- **`src/cli/render_text.rs`** (190 lines) — `pub const BAR_WIDTH: usize = 10;` + `pub fn compact_line(&ProviderState, &Timestamp, bool) -> String` + `filled_cells` / `pct_int` / `format_countdown` helpers. Uses `\u{2588}` / `\u{2591}` escapes (no literal block chars in source). Five inline tests: unicode byte-exact, ASCII byte-exact, bar-width-fixed-at-10, countdown zero-pads minutes, byte-sequence (e2 96 88 / e2 96 91 / e2 80 a2) present.

### Modified

- **`src/provider/mod.rs`** — added `pub mod mock;` line between the `use` statements and `FetchCtx`. Submodule reachable as `ahb::provider::mock::MockProvider`.
- **`src/lib.rs`** — added `pub mod cli;` so binary entry and integration tests can reach `ahb::cli::render_text`.
- **`src/main.rs`** — rewritten from the Plan-01 placeholder. New content: file-level `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)] #![warn(clippy::pedantic)]` (preserved); imports of `ahb::cli::render_text`, `ahb::provider::{Provider, FetchCtx, mock::MockProvider}`, `ahb::secrets::Secrets`; `clap::Parser`-derived `Cli` struct with `--ascii` (D-18) and `--color` (D-17); `ColorMode` enum (Auto/Always/Never); `install_phase0_panic_hook()` (take_hook+set_hook composition per D-27 + Pitfall 5); `#[tokio::main(flavor = "current_thread")]` async main reading `jiff::Timestamp::now()` once, constructing `FetchCtx`, calling `mock.fetch(&ctx).await`, rendering via `compact_line`, printing.

## Decisions Made

- **D-1 — `Timestamp::since((Unit::Hour, now))` instead of plain `Timestamp::since(*now)`.** The plan's verbatim transcription from RESEARCH Code Examples (lines 635-639) uses `target.since(*now)` which returns a seconds-largest `Span` (verified by reading the jiff source: `since` calls `until_with_largest_unit` whose default is `Unit::Second`). With that default, `Span::get_hours()` returns 0 even for a 2-hour gap, so the byte-exact `2h00m` test fails with `0h00m`. Fix: pass `(Unit::Hour, *now)` so the span decomposes into hours + minutes. B-1 binding preserved (still `unwrap_or_else(|_| Span::new())`, not `unwrap_or_default()`). This deviation is documented below.
- **D-2 — Scoped `#[allow(clippy::cast_*)]` over tiny helper fns.** Plan 01 / Plan 02 established the Phase 0 lint floor (`#![warn(clippy::pedantic)]` in `src/lib.rs`). The renderer's f32→usize / f32→u32 casts trigger `clippy::cast_possible_truncation` / `cast_sign_loss` / `cast_precision_loss`. Rather than a file-level `#[allow]` that would mask future cast bugs, I extracted `filled_cells(pct)` and `pct_int(pct)` as tiny helpers with the allows scoped just there. Plan 02 used the same approach for `Secrets::default()` test-side calls.
- **D-3 — Negative-span guard in `format_countdown`.** If `target < now` (clock drift across a reset boundary), `Timestamp::since` returns negative `get_hours()` / `get_minutes()`. Phase 0 collapses these to zero (`h.max(0)`, `m.max(0)`) so the bar line never prints `-1h59m`. Phase 1 may want a different presentation (e.g. `RESET NOW`), but Phase 0 keeps it laconic per CONTEXT specifics second bullet.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Plan's `Timestamp::since(*now)` produces seconds-largest Span, not hour-decomposed**

- **Found during:** Task 2 (`cargo test --lib cli::render_text` first run after writing the file verbatim from RESEARCH Code Examples)
- **Issue:** RESEARCH § "Code Examples — cli/render_text.rs — bar builder" (lines 611-641) writes `let span = target.since(*now).unwrap_or_default();` then reads `span.get_hours()` and `span.get_minutes()`. jiff's `Timestamp::since` defaults to `Unit::Second` as the largest unit, so the returned `Span` for a 2-hour gap has `get_seconds() == 7200`, `get_hours() == 0`, `get_minutes() == 0`. The byte-exact unicode test fails with `mock-session  ██████░░░░ 60% • resets in 0h00m` (countdown reads `0h00m` instead of `2h00m`).
- **Fix:** Changed call to `target.since((jiff::Unit::Hour, *now)).unwrap_or_else(|_| jiff::Span::new())`. The `(Unit, Timestamp)` tuple invokes `TimestampDifference::new(timestamp).largest(unit)` (per jiff docs), yielding a Span with hours+minutes+seconds decomposed. Tests now pass byte-exact. B-1 binding preserved (still `.unwrap_or_else(|_| jiff::Span::new())`, never `.unwrap_or_default()` — acceptance grep `grep -q 'unwrap_or_else' src/cli/render_text.rs && ! grep -q 'unwrap_or_default' src/cli/render_text.rs` holds).
- **Files modified:** `src/cli/render_text.rs` (`format_countdown` function)
- **Verification:** `cargo test --lib cli::render_text` — 5/5 tests pass; release binary output matches the W-3 regex.
- **Committed in:** `d148678` (Task 2 commit)

**2. [Rule 1 - Bug] clippy::pedantic lints fired on first-draft mock.rs and render_text.rs**

- **Found during:** Task 1 + Task 2 `cargo clippy --lib --all-targets -- -D warnings`
- **Issue:** Multiple pedantic warnings under `-D warnings`:
  - `clippy::doc_markdown` on doc comments mentioning `MockProvider`, `HpWindow`, `cfg(test)`, `HhMMm` (clippy wants backticks around `Type`-shaped identifiers in docs).
  - `clippy::default_constructed_unit_structs` on `Secrets::default()` calls in inline tests (clippy wants `Secrets {}` or just `Secrets` for unit structs — but the acceptance criteria of Plan 02 explicitly exercise the Default impl, so the call must stay).
  - `clippy::must_use_candidate` on `pub fn compact_line` (clippy wants `#[must_use]` on pure return-value-only public fns).
  - `clippy::cast_possible_truncation` / `cast_sign_loss` / `cast_precision_loss` on `(pct * BAR_WIDTH as f32 / 100.0).round() as usize` and `pct.round() as u32`.
  - `clippy::op_ref` on `bytes.windows(3).any(|w| w == &[0xe2, 0x96, 0x88])` (clippy wants `w == [0xe2, 0x96, 0x88]` without the `&`).
- **Fix:**
  - Added backticks to doc comments.
  - Scoped `#[allow(clippy::default_constructed_unit_structs)]` on the three mock.rs tests that use `Secrets::default()` (matching Plan 02's pattern).
  - Added `#[must_use]` to `compact_line`.
  - Extracted casts into `filled_cells(pct: f32) -> usize` + `pct_int(pct: f32) -> u32` with `#[allow(clippy::cast_*)]` scoped to those two helpers.
  - Removed the `&` from the byte-windows comparisons: `w == [0xe2, 0x96, 0x88]` (array deref vs `&[u8; 3]` is what clippy wants).
- **Files modified:** `src/provider/mock.rs`, `src/cli/render_text.rs`
- **Verification:** `cargo clippy --lib --all-targets -- -D warnings` exits 0 from the project root.
- **Committed in:** `63be300` (Task 1) and `d148678` (Task 2)

**3. [Rule 1 - Bug] clippy::doc_markdown + clippy::default_constructed_unit_structs in main.rs**

- **Found during:** Task 3 `cargo clippy --bin ahb --all-targets -- -D warnings`
- **Issue:** Two doc-markdown hits on the module docstring ("MockProvider" and "render_text::compact_line" needed backticks) and one `default_constructed_unit_structs` on `let secrets = Secrets::default();`.
- **Fix:** Wrapped both identifiers in backticks; added `#[allow(clippy::default_constructed_unit_structs)]` scoped to `async fn main`. The call must use `Secrets::default()` (rather than `Secrets`) because Plan 02's contract is that `Secrets` is constructable via `Default::default()` — Phase 1 will widen `Secrets` to a non-unit struct and the call site must already be using the Default-based constructor so Phase 1 doesn't have to touch `main.rs`.
- **Files modified:** `src/main.rs`
- **Verification:** `cargo clippy --all-targets -- -D warnings` exits 0.
- **Committed in:** `60666e5` (Task 3 commit)

**4. [Rule 1 - Bug] Acceptance grep `! grep -E 'jiff::Timestamp::now\(\)' src/provider/mock.rs` failed initially**

- **Found during:** Task 1 acceptance verification
- **Issue:** The first draft of `mock.rs` had a code comment that read `// ... CRITICAL: use ctx.now, NOT jiff::Timestamp::now() ...`. The grep guard is intended to catch any actual call to `Timestamp::now()` inside `fetch`, but it triggers on string-literal occurrences in comments too.
- **Fix:** Rewrote the comment to omit the literal token while preserving the intent: `// CRITICAL: use ctx.now (the injected clock), never a wall-clock read, per RESEARCH Anti-Patterns. Clock-injection contract for testability.`
- **Files modified:** `src/provider/mock.rs`
- **Verification:** `! grep -E 'jiff::Timestamp::now\(\)' src/provider/mock.rs` succeeds (no match).
- **Committed in:** `63be300` (Task 1)

**5. [Rule 1 - Bug] Acceptance check `! grep -P '[█░]' src/cli/render_text.rs` failed — module docstring contained literal Unicode blocks**

- **Found during:** Task 2 acceptance verification
- **Issue:** The first-draft module docstring of `render_text.rs` contained the literal phrase `mock-session  ██████░░░░ 60% • resets in 2h00m` with actual U+2588 / U+2591 / U+2022 bytes embedded. The plan's acceptance item said the source file must use `\u{2588}` / `\u{2591}` escapes for "grep/diff readability" — having literal block chars in docs defeats that.
- **Fix:** Rewrote the module docstring to describe the output structurally ("six U+2588 blocks + four U+2591 blocks + ...") and reference the escape form, without embedding literal Unicode block chars.
- **Files modified:** `src/cli/render_text.rs`
- **Verification:** `! grep -P '[█░]' src/cli/render_text.rs` succeeds (no match).
- **Committed in:** `d148678` (Task 2)

---

**Total deviations:** 5 auto-fixed (all Rule 1 — bugs / lint compliance / acceptance-grep correctness).

**Impact on plan:** Deviation 1 (`since((Unit::Hour, now))`) is structurally interesting — RESEARCH's verbatim code example would have shipped a binary that prints `0h00m` instead of `2h00m`, silently breaking Phase 0 success criterion #2. The fix preserves both the B-1 binding (Span::new() fallback, not unwrap_or_default) and the byte-exact output. Deviations 2-5 are mechanical lint compliance + acceptance-grep correctness under the Phase 0 lint floor that Plan 01/02 established; they don't change the contract surface.

## Issues Encountered

- **`Timestamp::since` default-largest-unit behavior surprised the plan.** The plan's RESEARCH section transcribed code that calls `target.since(*now)` directly. jiff's API defaults this to `Unit::Second` as the largest unit. Documented in Deviation 1 above. Future plans that need balanced-unit Span output should use `since((Unit::Hour, *now))` or wider.
- **No environmental issues** — `cargo build --release` worked first-attempt; tokio 1.52.3, jiff 0.2.24, clap 4.6.1 all resolved per Plan 01's `Cargo.lock`.

## User Setup Required

None — Phase 0 binary is self-contained. No env vars, no config files, no external services.

## Threat Flags

None. The Phase 0 plan's `<threat_model>` items (T-00-09 through T-00-13) were all `mitigate` or `accept` and the implementation matches: MockProvider has no panic surface (no unwrap/expect/panic in the fetch body); panic hook installed at the correct boundary per D-27; `--color` flag is parsed but unwired (Phase 0 doesn't colorize anything, so no ANSI-into-pipe risk yet — Phase 1 must respect `std::io::IsTerminal` + `NO_COLOR` when wiring color); no info disclosure (no secrets in scope).

## Next Phase Readiness

**Plan 04 (Gemini spike memo)** has no Rust dependencies and is already complete (committed `6bde881`).

**Phase 1 (engine + Claude + TUI)** can now:
- Replace `MockProvider` in `main.rs` with `Vec<Box<dyn Provider>>` containing Claude / Codex / Gemini adapters via the same Provider trait. The construction site is one line.
- Wrap `install_phase0_panic_hook()` via `ratatui::init()` — the take_hook+set_hook composition documented in `src/main.rs` is the explicit contract Phase 1 should preserve.
- Reuse `compact_line` for `--compact` CLI output, or extend with multi-window rendering by relaxing the `debug_assert_eq!(state.windows.len(), 1, ...)` and iterating.
- Use `Timestamp::since((Unit::Hour, *now))` as the canonical countdown idiom — Phase 0's Deviation 1 documents the gotcha.

**No blockers or concerns.**

## Self-Check: PASSED

Verified post-write:

- **Files exist (all 6 final files):**
  - `src/provider/mock.rs` — FOUND
  - `src/cli/mod.rs` — FOUND
  - `src/cli/render_text.rs` — FOUND
  - `src/provider/mod.rs` — FOUND (modified)
  - `src/lib.rs` — FOUND (modified)
  - `src/main.rs` — FOUND (rewritten)
  - `.planning/phases/00-spike-spine/00-03-SUMMARY.md` — FOUND (this file)
- **Commit hashes verified in git log:**
  - `63be300` — Task 1 (feat: add MockProvider returning D-25 fixture)
  - `d148678` — Task 2 (feat: add cli::render_text::compact_line bar renderer)
  - `60666e5` — Task 3 (feat: wire panic-hook + clap + MockProvider into src/main.rs)
- **End-to-end verification:**
  - `cargo build --release` ✓ exit 0
  - `cargo test --all-targets` ✓ 15/15 passed
  - `cargo clippy --all-targets -- -D warnings` ✓ exit 0
  - `./target/release/ahb` prints `mock-session  ██████░░░░ 60% • resets in 2h00m`, exit 0
  - `./target/release/ahb --ascii` prints `mock-session  ######---- 60% | resets in 2h00m`, exit 0
  - `./target/release/ahb --version` prints `ahb 0.0.1`
  - `./target/release/ahb --help` shows the D-01 description + `--ascii` + `--color` flags
  - xxd byte counts: 6 × `e2 96 88` (U+2588), 4 × `e2 96 91` (U+2591), 1 × `e2 80 a2` (U+2022)

---

*Phase: 00-spike-spine*
*Completed: 2026-05-22*
