//! Compact one-line HP-bar renderer for the CLI front-end.
//!
//! Phase 0 output literal (D-25 + CONTEXT specifics first bullet) is six U+2588
//! blocks + four U+2591 blocks + " 60% " + U+2022 + " resets in 2h00m". Source
//! file stays ASCII-clean by using `\u{...}` escapes; tests assert the byte form.
//!
//! Contract:
//! - Fixed width 10 cells (D-16) for snapshot stability + multi-provider alignment.
//! - Unicode block chars `\u{2588}` (filled) / `\u{2591}` (empty) (D-15), or `#` / `-` ASCII fallback (D-18).
//! - Label followed by two spaces, then bar, then `pct%`, then U+2022 (or `|` in ASCII) separator.
//! - Countdown format `Xh00m` (B-1: uses `Span::new()` fallback, not `unwrap_or_default()`).
//!
//! Phase 1 extensions (Task 3):
//! - `compact_line` relaxes the Phase 0 single-window `debug_assert_eq!` to
//!   `debug_assert!(!windows.is_empty())` and renders `windows[0]` for now.
//! - `filled_cells` + `format_countdown` are promoted to `pub(crate) fn` so Plan 03's
//!   TUI widget can re-import them without duplication or scoped-clippy drift.
//! - `id_label(id)` helper plus `format_error_row(id, err, ascii)` for the
//!   per-row error rendering (UI-SPEC LOCKED `{label}  ERROR: {reason}`).
//! - Color application via `owo-colors` at the bar segment when `color_on` is true
//!   (UI-SPEC thresholds: pct ≥ 30 → Green, 10 ≤ pct < 30 → Yellow, < 10 → Red;
//!   empty cells `DarkGray`).

use owo_colors::OwoColorize;

use crate::model::{ProviderError, ProviderId, ProviderState};

/// CONTEXT D-16 -- fixed bar width for snapshot stability + multi-provider alignment in compact mode.
pub const BAR_WIDTH: usize = 10;

/// Empty-state heading printed when no providers are configured / enabled (UI-SPEC).
pub const EMPTY_STATE_HEADING: &str = "no providers configured";
/// Empty-state body printed below the heading (UI-SPEC).
pub const EMPTY_STATE_BODY: &str =
    "add at least one provider to ~/.config/ahb/config.toml — see README";

/// Render a compact HP-bar line through the locked `Provider` trait output (`ProviderState`).
///
/// Phase 1: relaxed from Phase 0's `debug_assert_eq!(windows.len(), 1)` to
/// `debug_assert!(!windows.is_empty())`. Still renders only `windows[0]`; Phase 2's
/// `--detailed` will iterate all windows.
///
/// `ascii=true` substitutes `#`/`-` for the bar cells and `|` for the U+2022 separator
/// per D-18 (deterministic opt-in, no auto-detection).
#[must_use]
pub fn compact_line(state: &ProviderState, now: &jiff::Timestamp, ascii: bool) -> String {
    compact_line_colored(state, now, ascii, false)
}

/// Color-aware variant. `color_on=true` wraps the bar segments in ANSI fg color escapes
/// per UI-SPEC thresholds (Green/Yellow/Red for filled, `DarkGray` for empty).
#[must_use]
pub fn compact_line_colored(
    state: &ProviderState,
    now: &jiff::Timestamp,
    ascii: bool,
    color_on: bool,
) -> String {
    debug_assert!(
        !state.windows.is_empty(),
        "ProviderState must have at least one window"
    );
    let w = &state.windows[0];
    let pct = w.percent_remaining.clamp(0.0, 100.0);
    let filled = filled_cells(pct);

    let (filled_glyph, empty_glyph): (&str, &str) = if ascii {
        ("#", "-")
    } else {
        ("\u{2588}", "\u{2591}")
    };
    let filled_str = filled_glyph.repeat(filled);
    let empty_str = empty_glyph.repeat(BAR_WIDTH - filled);

    let bar = if color_on {
        // UI-SPEC threshold: Green ≥ 30, Yellow 10..30, Red < 10. Empty cells DarkGray.
        match pct {
            p if p >= 30.0 => format!("{}{}", filled_str.green(), empty_str.bright_black()),
            p if p >= 10.0 => format!("{}{}", filled_str.yellow(), empty_str.bright_black()),
            _ => format!("{}{}", filled_str.red(), empty_str.bright_black()),
        }
    } else {
        format!("{filled_str}{empty_str}")
    };

    let countdown = format_countdown(now, &w.reset.resets_at);
    let sep = if ascii { '|' } else { '\u{2022}' };

    // Phase 2 [Rule 2]: row label is the provider id (UI-SPEC LOCKED line 141 —
    // "Provider labels in output use the lowercase provider name as it appears
    // in `ProviderId`'s `snake_case` serialization"). Pre-Phase-2 this happened
    // to align with `windows[0].label` because Claude's window label IS "claude";
    // Codex breaks that coincidence (D-48 passthrough labels "primary" /
    // "secondary") so we now route the row label through `id_label` explicitly.
    // Mock compact output also flips from `mock-session  …` to `mock  …`; this
    // aligns the Mock label with the UI-SPEC binding (the per-window label
    // remains `"mock-session"` for the internal model and is what `--detailed`
    // surfaces in Plan 02-02).
    format!(
        "{label}  {bar} {pct}% {sep} resets in {countdown}",
        label = id_label(state.id),
        pct = pct_int(pct)
    )
}

/// Format an error row per UI-SPEC LOCKED: `{label}  ERROR: {one-line reason}`.
/// `reason` is `err.to_string()` (the Display impl already enforces one-line because
/// `ProviderError`'s `#[error(...)]` strings contain no `\n`).
///
/// Plan 02 special case: `ProviderError::SchemaDrift` returns the verbatim UI-SPEC
/// sentinel `{label}  ▒▒▒▒▒▒▒▒▒▒ ??% • Claude adapter may be out-of-date` using
/// U+2592 (medium-shade, NOT U+2591 light-shade — UI-SPEC distinguishes). The label
/// comes from `id_label(id)` so a future non-Claude adapter triggering `SchemaDrift`
/// renders cleanly (WARNING #5 resolution — no hard-coded "claude").
///
/// `_ascii` is reserved for symmetry with `compact_line` — Phase 2 may use it.
#[must_use]
pub fn format_error_row(id: ProviderId, err: &ProviderError, ascii: bool) -> String {
    format_error_row_colored(id, err, ascii, false)
}

/// Color-aware variant. `color_on=true` paints the bar cells + `??%` `DarkGray`
/// (Secondary role per UI-SPEC — "unknown, not critical") and the trailing phrase
/// `Claude adapter may be out-of-date` Bold + Red (Destructive role).
#[must_use]
pub fn format_error_row_colored(
    id: ProviderId,
    err: &ProviderError,
    _ascii: bool,
    color_on: bool,
) -> String {
    let label = id_label(id);
    if let ProviderError::SchemaDrift { .. } = err {
        // UI-SPEC LOCKED sentinel: 10× U+2592 medium-shade, " ??% ", U+2022, then phrase.
        // Phase 2 amendment: the phrase is now per-provider Title-cased
        // (`{Label} adapter may be out-of-date`) so Codex / Gemini / Mock render
        // correctly when triggering SchemaDrift. Claude rendering is byte-identical
        // to Phase 1 — `tests/schema_drift_sentinel.rs` continues to pass unchanged.
        let bar = "\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}";
        let pct = "??%";
        let phrase_owned = format!(
            "{label_titlecased} adapter may be out-of-date",
            label_titlecased = id_label_titlecase(id)
        );
        let phrase = phrase_owned.as_str();
        if color_on {
            return format!(
                "{label}  {bar} {pct} \u{2022} {phrase}",
                bar = bar.bright_black(),
                pct = pct.bright_black(),
                phrase = phrase.red().bold(),
            );
        }
        return format!("{label}  {bar} {pct} \u{2022} {phrase}");
    }
    let reason = format_one_line(&err.to_string());
    format!("{label}  ERROR: {reason}")
}

/// Sanitize a reason string into one line (collapse any newline / CR to a space).
/// Defensive: `ProviderError`'s Display strings should already be one line.
fn format_one_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        let is_ws = ch == '\n' || ch == '\r' || ch == '\t';
        let ch = if is_ws { ' ' } else { ch };
        if ch == ' ' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

/// Map `ProviderId` to its UI label (matches `ProviderId`'s `snake_case` serde repr).
/// Used by `format_error_row` and by Plan 02's `SchemaDrift` sentinel.
#[must_use]
pub(crate) fn id_label(id: ProviderId) -> &'static str {
    match id {
        ProviderId::Claude => "claude",
        ProviderId::Codex => "codex",
        ProviderId::Gemini => "gemini",
        ProviderId::Mock => "mock",
    }
}

/// Title-cased provider label for the SchemaDrift sentinel phrase
/// (`{Label} adapter may be out-of-date`). Phase 2 generalization (D-Deferred
/// folded — see RESEARCH §Open Questions Q2 RESOLVED): the Claude rendering is
/// byte-identical to Phase 1; Codex / Gemini / Mock now render correctly.
#[must_use]
pub(crate) fn id_label_titlecase(id: ProviderId) -> &'static str {
    match id {
        ProviderId::Claude => "Claude",
        ProviderId::Codex => "Codex",
        ProviderId::Gemini => "Gemini",
        ProviderId::Mock => "Mock",
    }
}

/// Convert a clamped `0.0..=100.0` percent into an integer in `0..=10`.
/// Pulled out so the f32→usize cast lints can be scoped to one tiny function.
/// Phase 1: promoted to `pub(crate)` so Plan 03's TUI widget can re-import (WARNING #3).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
pub(crate) fn filled_cells(pct: f32) -> usize {
    // pct is pre-clamped 0..=100 by caller; product is 0..=BAR_WIDTH; round to nearest cell.
    (pct * BAR_WIDTH as f32 / 100.0).round() as usize
}

/// Convert clamped percent to integer for display (`0..=100`).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pct_int(pct: f32) -> u32 {
    pct.round() as u32
}

/// Format the gap between `now` and `target` as `Xh00m`. Negative or unrepresentable
/// spans collapse to `0h00m` via `Span::new()` fallback (B-1: explicit `Span::new()`,
/// not `unwrap_or_default()`).
///
/// `Timestamp::since` defaults to a seconds-largest span; we pass `(Unit::Hour, now)`
/// so the returned `Span` carries `hours()` + `minutes()` + `seconds()` decomposed
/// (e.g. a 2-hour gap returns `2h 0m 0s`, not `7200s 0m 0h`).
///
/// Phase 1: promoted to `pub(crate)` so Plan 03's TUI widget can re-import (WARNING #3).
pub(crate) fn format_countdown(now: &jiff::Timestamp, target: &jiff::Timestamp) -> String {
    let span = target
        .since((jiff::Unit::Hour, *now))
        .unwrap_or_else(|_| jiff::Span::new());
    let h = i64::from(span.get_hours());
    let m = span.get_minutes();
    // Negative spans (target < now) yield negative h/m; clamp to zero for display.
    let h = h.max(0);
    let m = m.max(0);
    format!("{h}h{m:02}m")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HpWindow, ProviderId, ResetInfo};
    use std::borrow::Cow;

    fn make_state(pct: f32, resets_at: jiff::Timestamp) -> ProviderState {
        ProviderState {
            id: ProviderId::Mock,
            windows: vec![HpWindow {
                label: Cow::Borrowed("mock-session"),
                percent_remaining: pct,
                reset: ResetInfo { resets_at },
                bar_color: None,
                detailed_label: None,
            }],
            fetched_at: resets_at - jiff::Span::new().hours(2),
            source: Cow::Borrowed("mock"),
        }
    }

    // Test 1: Unicode byte-exact line for the D-25 fixture.
    // Phase 2: row label is `mock` (provider id) — Phase 2 [Rule 2] applies the
    // UI-SPEC LOCKED rule that "provider labels in output use the lowercase
    // provider name". Pre-Phase-2 this test asserted `mock-session  …` because
    // the renderer happened to read `windows[0].label`; the renderer now reads
    // `id_label(state.id)` (Codex broke that coincidence — D-48 passthrough).
    #[test]
    fn compact_line_unicode_byte_exact() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let resets_at = now + jiff::Span::new().hours(2);
        let state = make_state(60.0, resets_at);

        let line = compact_line(&state, &now, false);
        assert_eq!(
            line,
            "mock  \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\
             \u{2591}\u{2591}\u{2591}\u{2591} 60% \u{2022} resets in 2h00m"
        );
    }

    // Test 2: ASCII byte-exact line for the D-25 fixture.
    #[test]
    fn compact_line_ascii_byte_exact() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let resets_at = now + jiff::Span::new().hours(2);
        let state = make_state(60.0, resets_at);

        let line = compact_line(&state, &now, true);
        assert_eq!(line, "mock  ######---- 60% | resets in 2h00m");
    }

    // Test 3: Bar width is fixed at 10 cells regardless of percent.
    #[test]
    fn bar_width_fixed_at_ten() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let resets_at = now + jiff::Span::new().hours(2);

        // 0% -> 0 filled / 10 empty
        let line = compact_line(&make_state(0.0, resets_at), &now, true);
        assert!(line.contains("---------- 0%"), "0% line: {line}");

        // 100% -> 10 filled / 0 empty
        let line = compact_line(&make_state(100.0, resets_at), &now, true);
        assert!(line.contains("########## 100%"), "100% line: {line}");

        // 60% -> 6 filled / 4 empty
        let line = compact_line(&make_state(60.0, resets_at), &now, true);
        assert!(line.contains("######---- 60%"), "60% line: {line}");

        // 67% rounds to 7 cells (0.67 * 10 = 6.7 -> rounds to 7)
        let line = compact_line(&make_state(67.0, resets_at), &now, true);
        assert!(line.contains("#######--- 67%"), "67% line: {line}");
    }

    // Test 4: Countdown format zero-pads minutes.
    #[test]
    fn countdown_zero_pads_minutes() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();

        assert_eq!(
            format_countdown(&now, &(now + jiff::Span::new().hours(2))),
            "2h00m"
        );
        assert_eq!(
            format_countdown(&now, &(now + jiff::Span::new().minutes(15))),
            "0h15m"
        );
        // Negative span clamps to zero.
        assert_eq!(
            format_countdown(&now, &(now - jiff::Span::new().hours(1))),
            "0h00m"
        );
    }

    // Test 5: Byte-level verification of the U+2588 / U+2591 / U+2022 sequences.
    #[test]
    fn unicode_bytes_present() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let resets_at = now + jiff::Span::new().hours(2);
        let line = compact_line(&make_state(60.0, resets_at), &now, false);
        let bytes = line.as_bytes();

        // U+2588 (filled block) = e2 96 88
        assert!(
            bytes.windows(3).any(|w| w == [0xe2, 0x96, 0x88]),
            "U+2588 bytes missing: {line}"
        );
        // U+2591 (empty block) = e2 96 91
        assert!(
            bytes.windows(3).any(|w| w == [0xe2, 0x96, 0x91]),
            "U+2591 bytes missing: {line}"
        );
        // U+2022 (middle dot) = e2 80 a2
        assert!(
            bytes.windows(3).any(|w| w == [0xe2, 0x80, 0xa2]),
            "U+2022 bytes missing: {line}"
        );
    }

    // Phase 1: id_label + format_error_row tests
    #[test]
    fn id_label_returns_snake_case_provider_names() {
        assert_eq!(id_label(ProviderId::Claude), "claude");
        assert_eq!(id_label(ProviderId::Codex), "codex");
        assert_eq!(id_label(ProviderId::Gemini), "gemini");
        assert_eq!(id_label(ProviderId::Mock), "mock");
    }

    #[test]
    fn format_error_row_uses_id_label_not_hardcoded_string() {
        let err = ProviderError::Unavailable {
            reason: "~/.claude/projects not found — is Claude Code installed?".into(),
        };
        let row = format_error_row(ProviderId::Claude, &err, false);
        assert!(row.starts_with("claude  ERROR:"), "row: {row}");
        assert!(
            row.ends_with("is Claude Code installed?"),
            "row: {row} — must end with next-step hint"
        );

        // Same fn handles other providers via id_label.
        let row2 = format_error_row(ProviderId::Codex, &err, false);
        assert!(row2.starts_with("codex  ERROR:"), "row: {row2}");
    }

    #[test]
    fn format_error_row_collapses_any_embedded_whitespace_into_single_spaces() {
        let err = ProviderError::Unavailable {
            reason: "line1\nline2".into(),
        };
        let row = format_error_row(ProviderId::Claude, &err, false);
        assert!(!row.contains('\n'), "row leaks newline: {row}");
    }

    // Phase 1: compact_line_colored emits ANSI escape bytes when color_on is true
    #[test]
    fn compact_line_colored_emits_ansi_when_color_on() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let resets_at = now + jiff::Span::new().hours(2);
        let state = make_state(60.0, resets_at);

        let line = compact_line_colored(&state, &now, true, true);
        assert!(line.contains("\x1b["), "colored line must contain ANSI escapes: {line:?}");
    }

    #[test]
    fn compact_line_colored_no_ansi_when_color_off() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let resets_at = now + jiff::Span::new().hours(2);
        let state = make_state(60.0, resets_at);

        let line = compact_line_colored(&state, &now, true, false);
        assert!(!line.contains("\x1b["), "uncolored line must not contain ANSI escapes: {line:?}");
    }

    #[test]
    fn empty_state_constants_match_ui_spec() {
        assert_eq!(EMPTY_STATE_HEADING, "no providers configured");
        assert!(EMPTY_STATE_BODY.contains("config.toml"));
        assert!(EMPTY_STATE_BODY.contains("README"));
    }

    // Phase 2 Test E: SchemaDrift sentinel for Codex uses "Codex adapter…"
    #[test]
    fn format_error_row_codex_uses_codex_label_in_schema_drift_sentinel() {
        let err = ProviderError::SchemaDrift {
            missing: vec!["rate_limits".into()],
        };
        let row = format_error_row_colored(ProviderId::Codex, &err, false, false);
        assert!(
            row.ends_with("Codex adapter may be out-of-date"),
            "Codex row should end with `Codex adapter may be out-of-date`, got: {row:?}"
        );
        assert!(
            row.starts_with("codex  "),
            "row should start with `codex  ` (lowercase id_label), got: {row:?}"
        );
    }

    // Phase 2 Test F: SchemaDrift sentinel for Claude stays byte-identical to Phase 1.
    #[test]
    fn format_error_row_claude_schema_drift_sentinel_byte_identical_to_phase_1() {
        let err = ProviderError::SchemaDrift {
            missing: vec!["cache_creation_input_tokens".into()],
        };
        let row = format_error_row_colored(ProviderId::Claude, &err, false, false);
        let expected = "claude  \u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592} ??% \u{2022} Claude adapter may be out-of-date";
        assert_eq!(row, expected, "Claude row must stay byte-identical to Phase 1");
    }

    #[test]
    fn id_label_titlecase_returns_titlecased_provider_names() {
        assert_eq!(id_label_titlecase(ProviderId::Claude), "Claude");
        assert_eq!(id_label_titlecase(ProviderId::Codex), "Codex");
        assert_eq!(id_label_titlecase(ProviderId::Gemini), "Gemini");
        assert_eq!(id_label_titlecase(ProviderId::Mock), "Mock");
    }
}
