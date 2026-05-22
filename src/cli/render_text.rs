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

use crate::model::ProviderState;

/// CONTEXT D-16 -- fixed bar width for snapshot stability + multi-provider alignment in compact mode.
pub const BAR_WIDTH: usize = 10;

/// Render a compact HP-bar line through the locked `Provider` trait output (`ProviderState`).
///
/// Phase 0 only renders one window; Phase 1 will extend to multi-window rendering.
///
/// `ascii=true` substitutes `#`/`-` for the bar cells and `|` for the U+2022 separator
/// per D-18 (deterministic opt-in, no auto-detection).
#[must_use]
pub fn compact_line(state: &ProviderState, now: &jiff::Timestamp, ascii: bool) -> String {
    debug_assert_eq!(
        state.windows.len(),
        1,
        "Phase 0 mock returns exactly one window; multi-window rendering is Phase 1"
    );
    let w = &state.windows[0];
    let pct = w.percent_remaining.clamp(0.0, 100.0);
    let filled = filled_cells(pct);

    let bar = if ascii {
        format!(
            "{}{}",
            "#".repeat(filled),
            "-".repeat(BAR_WIDTH - filled)
        )
    } else {
        format!(
            "{}{}",
            "\u{2588}".repeat(filled),
            "\u{2591}".repeat(BAR_WIDTH - filled)
        )
    };

    let countdown = format_countdown(now, &w.reset.resets_at);
    let sep = if ascii { '|' } else { '\u{2022}' };

    format!(
        "{label}  {bar} {pct}% {sep} resets in {countdown}",
        label = w.label,
        pct = pct_int(pct)
    )
}

/// Convert a clamped `0.0..=100.0` percent into an integer in `0..=10`.
/// Pulled out so the f32→usize cast lints can be scoped to one tiny function.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn filled_cells(pct: f32) -> usize {
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
fn format_countdown(now: &jiff::Timestamp, target: &jiff::Timestamp) -> String {
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
            }],
            fetched_at: resets_at - jiff::Span::new().hours(2),
            source: Cow::Borrowed("mock"),
        }
    }

    // Test 1: Unicode byte-exact line for the D-25 fixture.
    #[test]
    fn compact_line_unicode_byte_exact() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let resets_at = now + jiff::Span::new().hours(2);
        let state = make_state(60.0, resets_at);

        let line = compact_line(&state, &now, false);
        assert_eq!(
            line,
            "mock-session  \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\
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
        assert_eq!(line, "mock-session  ######---- 60% | resets in 2h00m");
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
}
