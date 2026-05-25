---
phase: 02-codex-output-formats
reviewed: 2026-05-25T00:00:00Z
depth: standard
files_reviewed: 26
files_reviewed_list:
  - src/cli/mod.rs
  - src/cli/render_json.rs
  - src/cli/render_text.rs
  - src/engine/fanout.rs
  - src/engine/mod.rs
  - src/main.rs
  - src/model.rs
  - src/provider/claude/mod.rs
  - src/provider/claude/window.rs
  - src/provider/codex/jsonl.rs
  - src/provider/codex/mod.rs
  - src/provider/codex/sqlite.rs
  - src/provider/codex/window.rs
  - src/provider/mock.rs
  - src/provider/mod.rs
  - src/templates/default-config.toml
  - src/tui/app.rs
  - src/tui/widgets/hp_row.rs
  - tests/cli_walking_skeleton.rs
  - tests/codex_sqlite_lock_resilience.rs
  - tests/detailed_format.rs
  - tests/exit_codes.rs
  - tests/json_format_round_trip.rs
  - tests/schema_drift_sentinel.rs
  - tests/secret_leak_subprocess.rs
  - src/provider/claude/jsonl.rs
findings:
  critical: 1
  warning: 6
  info: 5
  total: 12
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-05-25
**Depth:** standard
**Files Reviewed:** 26
**Status:** issues_found

## Summary

Phase 02 ships `--detailed` (D-53/CORE-03), `--json schema_version=1` (D-49..D-52/CORE-04), exit-code grid (D-59/CORE-06), the Codex CLI adapter (ADP-04 with read-only SQLite + JSONL rollout parsing), and the Claude weekly-window addition (D-54). Test coverage is broad — every plan-level decision has unit and/or integration coverage and `tests/exit_codes.rs`, `tests/json_format_round_trip.rs`, and `tests/detailed_format.rs` exercise the binary end-to-end.

Findings concentrate around three areas:

1. **One CRITICAL gap in Gemini config handling** — `cfg.providers.gemini.enabled = true` is silently dropped at engine construction. The user gets exit 0 + empty-state, indistinguishable from "nothing enabled". This is a silent failure that masks misconfiguration and breaks the D-59 exit-code grid (Gemini-only enabled = "no providers configured" instead of "configured but all failed").
2. **WARNINGS** cluster around quality-of-output edge cases: `compact_line_colored` does not defend against NaN `percent_remaining` from arbitrary adapters; `format_one_line` does not strip leading/trailing whitespace; the Codex `resets_at` arithmetic silently swallows overflow into a stale `line_ts`; and a few cross-module duplications (BAR_WIDTH and `pick_newest_file`) that risk drift.
3. **INFO** items capture cosmetic / micro-correctness items that don't affect ship-readiness.

The structural sound:
- `--json` envelope is decoupled from internal model (D-49 ✅), the v1 wire shape is asserted via round-trip tests, `Secret<T>::Serialize → "[REDACTED]"` is verified end-to-end via `tests/secret_leak_subprocess.rs::subprocess_json_path_redacts_secret`.
- `DispatchOutcome::from_results` correctly collapses empty → AnySuccess (CFG-04) and counts SchemaDrift as Err (D-60).
- Codex adapter correctly anchors `resets_at` on the rollout line timestamp (not `ctx.now`), the JSONL parser is D-35-tolerant (mid-file warn + skip, trailing silent), and the SQLite open path uses `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX` + `busy_timeout(250ms)` per D-45.

No structural pre-pass was provided.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01: Enabling Gemini silently produces empty-state, breaking the D-59 exit-code grid

**File:** `src/engine/mod.rs:69-73`
**Issue:** When the user sets `[providers.gemini] enabled = true` in `~/.config/ahb/config.toml`, `Engine::new` emits a `tracing::debug!` and pushes **nothing** into `self.providers`. Three downstream consequences:

1. If Gemini is the **only** enabled provider, `refresh_all` returns an empty `Vec`. `DispatchOutcome::from_results(&[])` returns `AnySuccess` (CFG-04 — "zero providers enabled = exit 0"). The user enabled a provider, AHB prints `no providers configured` + the README hint, then exits 0. There is no way for the user to discover that Gemini was their misconfiguration.
2. This breaks the D-61 exit-code contract documented in `--help`: "1 all configured providers failed". With Gemini enabled, the provider IS configured but it never gets a chance to fail.
3. The default config template (`src/templates/default-config.toml:11-12`) DOES advertise the Gemini key with `enabled = false`, so a user reading the template would naturally flip it to `true` to try the feature — and silently see nothing.

The Claude / Codex branches push a real provider (which then returns `Unavailable` if the dependency is missing). The Gemini branch should mirror that pattern with a `GeminiUnimplementedProvider` that returns `Err(ProviderError::Unavailable { reason: "Gemini provider is Phase 3 — not yet implemented; set enabled = false to suppress this row" })`. That preserves the exit-code grid AND gives the user a visible signal.

Failing-closed (returning a configured Err row) is the same approach Claude / Codex take when their dependency is missing — there's no reason Gemini's "not implemented" state should be downgraded to "not configured".

**Fix:**
```rust
// In src/engine/mod.rs::Engine::new — replace lines 69-73:
if cfg.providers.gemini.enabled {
    // Mirror the Claude / Codex pattern: push a placeholder provider that
    // returns Err(Unavailable) so (a) the user sees a row, (b) the exit-code
    // grid stays honest, (c) Phase 3 only swaps the impl, not the wiring.
    providers.push(Arc::new(crate::provider::gemini::GeminiUnimplementedProvider));
}
```
Add a one-file `src/provider/gemini.rs` (sibling to `mock.rs`) whose `fetch` returns `Err(ProviderError::Unavailable { reason: "Gemini provider is not yet implemented (Phase 3)".into() })`. Compact and detailed rendering already handle this via `format_error_row_colored`; the JSON envelope handles it via `error_to_json`.

Add a regression test in `tests/exit_codes.rs`:
```rust
#[test]
fn exit_code_1_when_only_gemini_enabled() {
    // gemini-only config must exit 1 (configured-but-failed), NOT 0 (no-providers).
    // Renders "gemini  ERROR: Gemini provider is not yet implemented (Phase 3)".
}
```

## Warnings

### WR-01: `compact_line_colored` and `render_window_row` cast NaN `percent_remaining` to a misleading bar

**File:** `src/cli/render_text.rs:64`, `src/cli/render_text.rs:337`
**Issue:** `let pct = w.percent_remaining.clamp(0.0, 100.0);` returns NaN when `w.percent_remaining` is NaN. Then `filled_cells(NaN)` evaluates `(NaN * 10.0 / 100.0).round() as usize`. The `as usize` cast on NaN is **implementation-defined** to 0 on current Rust (saturating-to-zero on x86-64), but this is brittle. Worse, the subsequent `match pct { p if p >= 30.0 => Green, p if p >= 10.0 => Yellow, _ => Red }` falls through to Red because all NaN comparisons return false — so a NaN-percent row renders as a fully empty, RED bar with "0%" displayed. That's a misleading "exhausted" signal for what is actually "unknown".

Today this only affects compact mode if an adapter ever puts NaN into `windows[0]`. Claude puts NaN into `windows[1]` (weekly), so the compact path is currently safe by virtue of indexing. But the contract is fragile — a future adapter that emits NaN in its primary window would silently produce a wrong-looking bar. The `--detailed` path (`render_window_row`) handles NaN explicitly via `if w.percent_remaining.is_nan() { ... }`; the compact path should do the same.

**Fix:**
```rust
// src/cli/render_text.rs::compact_line_colored — before the clamp:
let raw = w.percent_remaining;
if raw.is_nan() {
    // Mirror the detailed-mode NaN sentinel: 10 U+2592 cells + ??% + (limit unknown)
    let bar = "\u{2592}".repeat(BAR_WIDTH);
    let countdown = format_countdown(now, &w.reset.resets_at);
    let sep = if ascii { '|' } else { '\u{2022}' };
    return format!(
        "{label}  {bar} ??% {sep} resets in {countdown} (limit unknown)",
        label = id_label(state.id),
    );
}
let pct = raw.clamp(0.0, 100.0);
```
Also add a unit test that constructs a NaN-percent state and asserts the sentinel rendering instead of a red 0% bar.

### WR-02: Codex `tier_to_window` silently overflows large `resets_in_seconds` to `line_ts`

**File:** `src/provider/codex/window.rs:43-46`
**Issue:** `resets_in_seconds: u64` → `i64::try_from(...).unwrap_or(i64::MAX)`, then `line_ts.checked_add(Span::seconds(secs)).unwrap_or(line_ts)`. If `checked_add` overflows (e.g. seconds near `i64::MAX`), the fallback uses `line_ts` — a stale timestamp from the rollout file. The render layer then prints `resets in 0h00m` (or a negative-clamped countdown). The user sees a "just reset" signal when in fact the data is "way out of range".

The path is practically unreachable for sane Codex data (rollouts emit `resets_in_seconds` in the 17000s range), but the swallowed-overflow + stale-anchor combination is a real silent corruption. At minimum, log a `tracing::warn!` so an operator can see when the upstream schema starts emitting absurd values.

**Fix:**
```rust
let resets_at = line_ts
    .checked_add(jiff::Span::new().seconds(secs))
    .unwrap_or_else(|| {
        tracing::warn!(
            "codex resets_in_seconds={} overflowed Timestamp arithmetic; falling back to line_ts (countdown will read 0h00m)",
            tier.resets_in_seconds
        );
        line_ts
    });
```
Or return `Err(SchemaDrift)` from the parser when overflow happens — surfacing the issue is better than masking it.

### WR-03: `format_one_line` does not strip leading/trailing whitespace, leaking " message " into JSON `error.message`

**File:** `src/cli/render_text.rs:166-183`
**Issue:** The collapsing loop initializes `prev_space = false`, so a leading `\n` / `\t` / `\r` becomes a leading space in the output. Similarly, a trailing whitespace char becomes a trailing space. Two concrete impacts:

1. `error_to_json` invokes this to build `JsonError.message`. A `ProviderError::Unavailable { reason: "\n   something broke\n" }` produces JSON like `"message":" something broke "` — leading/trailing whitespace inside the JSON string is wire-noise that downstream consumers may treat as significant.
2. The Phase 1 sanitizer doc says "collapse any newline / CR to a space" — the actual behavior also _emits_ that space at column 0 / end, which differs from typical sanitizer semantics ("normalize internal whitespace, trim ends").

**Fix:**
```rust
pub(crate) fn format_one_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true; // start as if we just saw whitespace -> drops leading ws
    for ch in s.chars() {
        let is_ws = ch == '\n' || ch == '\r' || ch == '\t' || ch == ' ';
        if is_ws {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    // Trim trailing space if any.
    if out.ends_with(' ') {
        out.pop();
    }
    out
}
```
Add a unit test:
```rust
#[test]
fn format_one_line_strips_leading_and_trailing_whitespace() {
    assert_eq!(format_one_line("\n  foo  \n"), "foo");
}
```

### WR-04: `BAR_WIDTH` duplicated between `cli/render_text.rs` and `tui/widgets/hp_row.rs`

**File:** `src/cli/render_text.rs:29`, `src/tui/widgets/hp_row.rs:32`
**Issue:** Both modules define `const BAR_WIDTH: usize = 10;`. If a future phase changes the bar width in one location (e.g., to accommodate a wider snapshot), the other surface silently keeps the old width and the TUI / CLI start showing different bar lengths — a visual regression with no compile error. The CLI module already publishes `pub const BAR_WIDTH`; the TUI should re-import that constant instead of redefining it.

**Fix:**
```rust
// In src/tui/widgets/hp_row.rs — replace line 31-32:
use crate::cli::render_text::BAR_WIDTH;
```
Delete the local const. Compile will catch any width drift at the single source of truth.

### WR-05: `format_error_row_colored` ignores `_ascii` for the SchemaDrift sentinel

**File:** `src/cli/render_text.rs:118-158`
**Issue:** The public signature takes `ascii: bool` and the colored variant takes `_ascii: bool` (note the underscore). For the SchemaDrift path the function unconditionally emits U+2592 (medium-shade) and U+2022 (middle dot) regardless of the `--ascii` flag. Users who pass `--ascii` for tmux / Starship integration get a mixed output: ASCII rows everywhere EXCEPT the schema-drift sentinel, which then contains 3 non-ASCII bytes per cell. tmux clients running on a terminal that can't render U+2592 will see replacement glyphs and the row width will misalign.

The detailed-mode NaN sentinel (`src/cli/render_text.rs:324`) has the same issue and its inline comment explicitly acknowledges it ("Always 10 U+2592 medium-shade cells regardless of `ascii` ..."). That's a deliberate design choice for the NaN path, but for the SchemaDrift sentinel reachable via `--ascii` the user expectation is "no Unicode in my output stream".

**Fix:** In the SchemaDrift branch, choose an ASCII fallback when `ascii=true`:
```rust
let bar = if ascii {
    "??????????".to_string() // or "X".repeat(10) — pick something visually distinct
} else {
    "\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}".to_string()
};
let sep_char = if ascii { '|' } else { '\u{2022}' };
```
And rename `_ascii` → `ascii` in `format_error_row_colored` so the param is actually used. Add an integration regression in `tests/schema_drift_sentinel.rs` that runs AHB with `--ascii` and asserts zero non-ASCII bytes in stdout.

### WR-06: Cosmetic-only `_color_ignored` binding in `run_json` is a foot-gun

**File:** `src/cli/render_json.rs:258`
**Issue:** `let _color_ignored = tty::should_colorize_env(color_flag, true);` is a side-effect-free call whose return value is intentionally discarded. The inline comment justifies this as "documenting the contract at the call site". But:

1. `should_colorize_env(_, true)` always returns `false` per its contract — calling it does not exercise any side effect (no env-var probe is meaningful here since the result is discarded).
2. A reader skimming `run_json` sees a meaningless binding and may either delete it (regressing the documentary intent the original author had) or convince themselves it does something it doesn't.
3. clippy `let_underscore_must_use` etc. catch real foot-guns here in newer toolchains.

Documenting D-58 in a comment alone, without the dead call, is cleaner.

**Fix:** Delete the `let _color_ignored = ...` line and the surrounding comment block. The function still satisfies the D-58 binding because it makes NO ANSI calls — that IS the compile-time guarantee. Keep a one-line `// D-58: --color is silently ignored in --json mode; this module emits zero ANSI bytes.` if documentary intent matters.

## Info

### IN-01: `pick_newest_file` is duplicated between `provider/claude/mod.rs` and `provider/codex/jsonl.rs`

**File:** `src/provider/claude/mod.rs:54-64`, `src/provider/codex/jsonl.rs:117-127`
**Issue:** Two byte-equivalent implementations. The codex copy has a docstring acknowledging the duplication and referencing PATTERNS Pattern 1 §225 ("keep the duplication until a third caller appears"). That's a valid Rule-of-Three deferral — flagging only to note that a third caller in Phase 3 (Gemini) should trigger consolidation. No fix required now; track in PATTERNS.md.

### IN-02: `discover_session_files` (Claude) silently ignores symlink-loop errors

**File:** `src/provider/claude/jsonl.rs:179-197`
**Issue:** `glob::glob_with` with `require_literal_separator: false` follows symlinks. If `~/.claude/projects/` contains a symlink loop (unusual but possible), glob emits an error per iteration. `paths.filter_map(Result::ok).collect()` swallows these errors silently. A `tracing::debug!("symlink loop or permission error: {e}")` on the dropped errors would help diagnostics without changing user-facing behavior.

**Fix:**
```rust
match glob::glob_with(&pattern_str, opts) {
    Ok(paths) => paths
        .filter_map(|r| match r {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::debug!("glob entry error in {pattern_str}: {e}");
                None
            }
        })
        .collect(),
    Err(e) => { tracing::warn!("glob error for {}: {e}", pattern_str); Vec::new() }
}
```

### IN-03: `JsonRoot.providers: Vec<JsonProvider<'a>>` allocates per-provider — minor for v1 scope

**File:** `src/cli/render_json.rs:131-151`
**Issue:** Building `Vec<JsonProvider>` for typically 1-4 providers and serializing it is fine. Mentioned only because v1's "perf out of scope" rule (per `<review_scope>`) covers this — but the `.iter().map(...).collect()` pattern is idiomatic and not flagged as a quality issue. Drop this item if you prefer brevity.

### IN-04: `compact_line_unicode_byte_exact` test docstring still references the old `mock-session` assumption

**File:** `src/cli/render_text.rs:387-402`
**Issue:** The test comment describes the Phase 1 → Phase 2 transition ("Pre-Phase-2 this test asserted `mock-session  …` because ...") — informative but adds noise once Phase 2 settles. Suggest condensing to a one-line "row label is `id_label(state.id)`" reference after this phase ships.

### IN-05: `tests/codex_sqlite_lock_resilience.rs` rollout uses `jiff::Timestamp::now()` for fixture timestamps

**File:** `tests/codex_sqlite_lock_resilience.rs:61`
**Issue:** `let one_h_ago = jiff::Timestamp::now() - jiff::Span::new().hours(1);` introduces a wall-clock dependency in a test fixture. The test is robust because it only requires the resulting timestamp to be parser-valid (no value assertions on the bar countdown). But it does break the testing-seam discipline established for adapter code (BL-01: "renderer must NEVER call `jiff::Timestamp::now()`"). The discipline is correctly scoped to renderer/adapter source, not test code — flagging for symmetry. Suggest using a fixed `"2026-05-25T11:00:00Z".parse().unwrap()` timestamp instead.

---

_Reviewed: 2026-05-25_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
