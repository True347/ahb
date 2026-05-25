//! `RateLimits` → `Vec<HpWindow>` pure transform for the Codex adapter (REQ ADP-04).
//!
//! Conversion rules per RESEARCH §Codex JSONL Schema bullets 1-3:
//! 1. Codex reports `used_percent` (0..=100); AHB renders REMAINING percent.
//!    `percent_remaining = (100.0 - used_percent).clamp(0.0, 100.0)`.
//! 2. Window labels are passthrough literals `"primary"` / `"secondary"` (D-48).
//!    No reordering, no merging, no synthesizing.
//! 3. `resets_at` is anchored on the rollout LINE's own timestamp + `Span::seconds(resets_in_seconds)`
//!    — NOT `ctx.now`. The rollout `resets_in_seconds` field is relative to the
//!    moment the event was persisted; using `ctx.now` would skew the countdown by
//!    however stale the rollout is.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::borrow::Cow;

use crate::model::{HpWindow, ResetInfo};
use crate::provider::codex::jsonl::{RateLimits, RateLimitTier};

/// Convert one `RateLimits` snapshot into the corresponding `HpWindow`s in
/// passthrough order: primary first if present, secondary second if present.
/// Empty input (both tiers `None`) yields an empty Vec — the caller decides
/// whether that's a schema-drift condition (parser already filters this case).
#[must_use]
pub(crate) fn to_hp_windows(rate_limits: &RateLimits, line_ts: jiff::Timestamp) -> Vec<HpWindow> {
    let mut out: Vec<HpWindow> = Vec::with_capacity(2);
    if let Some(tier) = &rate_limits.primary {
        out.push(tier_to_window("primary", tier, line_ts));
    }
    if let Some(tier) = &rate_limits.secondary {
        out.push(tier_to_window("secondary", tier, line_ts));
    }
    out
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn tier_to_window(label: &'static str, tier: &RateLimitTier, line_ts: jiff::Timestamp) -> HpWindow {
    let percent_remaining = (100.0 - tier.used_percent as f32).clamp(0.0, 100.0);
    // WR-02: `Span::seconds(i64::MAX)` itself panics — the valid range per
    // jiff 0.2 is roughly ±631_107_417_600s (~20_000 years). Clamp BEFORE
    // we hand the value to `Span::seconds`, otherwise an absurd upstream
    // value crashes the adapter instead of degrading. `JIFF_SECONDS_MAX` is
    // a generous ceiling (centuries) — practically unreachable for sane
    // Codex data (rollouts emit values in the 17000s range), but the
    // clamp + warn means we surface drift instead of panicking.
    //
    // After the clamp, `checked_add` can still legitimately overflow when
    // `line_ts` is itself near the representable Timestamp boundary; that
    // path remains a warn + fall back to `line_ts` (countdown renders
    // `0h00m`, which is consistent with the existing "render something
    // rather than crash" stance for adapter edge cases).
    const JIFF_SECONDS_MAX: i64 = 631_107_417_600;
    let raw_secs = tier.resets_in_seconds;
    #[allow(clippy::cast_possible_wrap)]
    let clamped_secs = i64::try_from(raw_secs).unwrap_or(JIFF_SECONDS_MAX).min(JIFF_SECONDS_MAX);
    if i128::from(clamped_secs) < i128::from(raw_secs) {
        tracing::warn!(
            "codex rate_limits.{label}.resets_in_seconds={raw_secs} exceeds jiff::Span \
             representable range; clamping to {JIFF_SECONDS_MAX}s before anchor \
             arithmetic (countdown will render as the clamped value)"
        );
    }
    let resets_at = line_ts
        .checked_add(jiff::Span::new().seconds(clamped_secs))
        .unwrap_or_else(|err| {
            tracing::warn!(
                "codex rate_limits.{label}.resets_in_seconds={raw_secs} overflowed Timestamp \
                 arithmetic from line_ts={line_ts} ({err}); falling back to line_ts \
                 (countdown will read 0h00m)"
            );
            line_ts
        });
    HpWindow {
        label: Cow::Borrowed(label),
        percent_remaining,
        reset: ResetInfo { resets_at },
        bar_color: None,
        // Phase 2 D-52 additive: Codex passes through the same string for the
        // detailed-mode row label — `label` is already a meaningful, user-facing
        // identifier ("primary" / "secondary"). Leaving as None would still
        // render correctly via the renderer's `unwrap_or(&label)` fallback;
        // setting it explicitly documents intent and matches Claude's pattern.
        detailed_label: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier(used_percent: f64, resets_in_seconds: u64) -> RateLimitTier {
        RateLimitTier {
            used_percent,
            window_minutes: 299,
            resets_in_seconds,
        }
    }

    #[test]
    fn test_7_both_tiers_compute_correct_reset_anchors() {
        let line_ts: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let rl = RateLimits {
            primary: Some(tier(0.0, 3600)),
            secondary: Some(tier(6.0, 7200)),
        };
        let windows = to_hp_windows(&rl, line_ts);
        assert_eq!(windows.len(), 2);
        // primary at index 0
        assert_eq!(windows[0].label, "primary");
        assert!((windows[0].percent_remaining - 100.0).abs() < 0.01);
        let expected_primary: jiff::Timestamp = "2026-05-25T13:00:00Z".parse().unwrap();
        assert_eq!(windows[0].reset.resets_at, expected_primary);
        // secondary at index 1
        assert_eq!(windows[1].label, "secondary");
        assert!((windows[1].percent_remaining - 94.0).abs() < 0.01);
        let expected_secondary: jiff::Timestamp = "2026-05-25T14:00:00Z".parse().unwrap();
        assert_eq!(windows[1].reset.resets_at, expected_secondary);
    }

    #[test]
    fn primary_only_returns_single_window() {
        let line_ts: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let rl = RateLimits {
            primary: Some(tier(40.0, 60)),
            secondary: None,
        };
        let windows = to_hp_windows(&rl, line_ts);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].label, "primary");
        assert!((windows[0].percent_remaining - 60.0).abs() < 0.01);
    }

    #[test]
    fn secondary_only_returns_single_window_with_correct_label() {
        let line_ts: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let rl = RateLimits {
            primary: None,
            secondary: Some(tier(0.0, 60)),
        };
        let windows = to_hp_windows(&rl, line_ts);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].label, "secondary");
    }

    #[test]
    fn empty_rate_limits_returns_empty_vec() {
        let line_ts: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let rl = RateLimits {
            primary: None,
            secondary: None,
        };
        let windows = to_hp_windows(&rl, line_ts);
        assert!(windows.is_empty());
    }

    #[test]
    fn used_percent_above_100_clamps_to_zero_remaining() {
        let line_ts: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let rl = RateLimits {
            primary: Some(tier(150.0, 60)),
            secondary: None,
        };
        let windows = to_hp_windows(&rl, line_ts);
        assert_eq!(windows.len(), 1);
        assert!(
            (windows[0].percent_remaining - 0.0).abs() < 0.01,
            "used_percent > 100 should clamp to 0% remaining"
        );
    }

    #[test]
    fn negative_used_percent_clamps_to_100_remaining() {
        let line_ts: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let rl = RateLimits {
            primary: Some(tier(-5.0, 60)),
            secondary: None,
        };
        let windows = to_hp_windows(&rl, line_ts);
        assert_eq!(windows.len(), 1);
        assert!(
            (windows[0].percent_remaining - 100.0).abs() < 0.01,
            "negative used_percent should clamp to 100% remaining"
        );
    }

    // WR-02: absurdly large `resets_in_seconds` must NOT panic. Pre-fix
    // `Span::seconds(i64::MAX)` itself panics inside jiff (valid range is
    // ±631_107_417_600s ≈ ±20_000 years). Post-fix the adapter clamps to
    // `JIFF_SECONDS_MAX` BEFORE handing the value to `Span::seconds`, then
    // proceeds with `checked_add` as the second line of defense. The
    // accompanying `tracing::warn!` is observability-only and not asserted
    // here (no tracing-test machinery wired up for this crate).
    #[test]
    fn absurd_resets_in_seconds_does_not_panic_and_yields_sane_window() {
        let line_ts: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let rl = RateLimits {
            primary: Some(tier(50.0, u64::MAX)),
            secondary: None,
        };
        // The key invariant: this must not panic. (Pre-fix it did.)
        let windows = to_hp_windows(&rl, line_ts);
        assert_eq!(windows.len(), 1);
        // Percent and label are unaffected by the clamp.
        assert_eq!(windows[0].label, "primary");
        assert!((windows[0].percent_remaining - 50.0).abs() < 0.01);
        // resets_at is either `line_ts + ~20_000 years` (clamp succeeded +
        // checked_add succeeded) or `line_ts` (checked_add fell back).
        // Both are acceptable per the fix; the test asserts only that the
        // anchor is >= line_ts (never goes backwards in time).
        assert!(
            windows[0].reset.resets_at >= line_ts,
            "anchor must not regress past line_ts on overflow: got {}",
            windows[0].reset.resets_at
        );
    }
}
