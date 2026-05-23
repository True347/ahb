//! 5h cluster anchor + percent computation for the Claude adapter (D-33 amended per L1).
//!
//! Cluster algorithm: sort assistant messages by `timestamp`, walk newest→oldest,
//! find the first gap > 5 h. Everything from that gap to the end is the active cluster.
//! `session_start` = first message in the cluster; `reset_at = session_start + 5h`.
//! `used_tokens` = SUM of `cache_creation_input_tokens` across cluster (L1: the
//! `input_tokens` / `output_tokens` fields are upstream-broken streaming placeholders;
//! `cache_creation_input_tokens` is the only reliable budget-aligned counter).
//!
//! Phase 1 uses `CLAUDE_5H_TOKEN_LIMIT = 44_000` (Pro-tier estimate; D-44).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use crate::provider::claude::jsonl::AssistantEntry;

/// Best-effort 5h budget estimate (Pro tier ~44k tokens / 5h window). Anthropic does not
/// publish exact numbers; revisit quarterly. Max5 / Max20 subscribers will see undercounted
/// bars — Phase 2 may add a `plan_tier` knob (CFG-03). Source: tokenmix.ai 2026 +
/// ccusage community measurements.
pub const CLAUDE_5H_TOKEN_LIMIT: u64 = 44_000;

/// One active 5h cluster. `session_start` anchors the window; `reset_at = session_start + 5h`.
#[derive(Debug, Clone)]
pub struct Cluster {
    pub session_start: jiff::Timestamp,
    pub reset_at: jiff::Timestamp,
    pub used_tokens: u64,
}

/// Find the active 5h cluster per D-33 (amended). Returns `None` for empty input.
///
/// Input is assumed sorted ascending by `timestamp` (the adapter sorts before calling).
#[must_use]
pub fn find_active_cluster(sorted_msgs: &[AssistantEntry]) -> Option<Cluster> {
    if sorted_msgs.is_empty() {
        return None;
    }
    // Walk newest→oldest looking for the first gap > 5h. Slice from there to end.
    let five_hours = jiff::Span::new().hours(5);
    let mut start_idx = 0_usize;
    for i in (1..sorted_msgs.len()).rev() {
        let prev = &sorted_msgs[i - 1];
        let curr = &sorted_msgs[i];
        // Gap = curr.timestamp - prev.timestamp (curr is newer because sorted ascending).
        let Ok(gap) = curr.timestamp.since((jiff::Unit::Hour, prev.timestamp)) else {
            continue;
        };
        // Compare in hours (Span is calendrical; compare directly).
        let gap_hours = gap.get_hours();
        if gap_hours >= 5 {
            // The cluster starts at `i` (curr is the first message after the gap).
            start_idx = i;
            break;
        }
    }
    let cluster = &sorted_msgs[start_idx..];
    let first = cluster.first()?;
    let session_start = first.timestamp;
    let reset_at = session_start.checked_add(five_hours).ok()?;
    let used_tokens: u64 = cluster
        .iter()
        .map(|m| {
            m.message
                .usage
                .as_ref()
                .map_or(0_u64, |u| u.cache_creation_input_tokens)
        })
        .sum();
    Some(Cluster {
        session_start,
        reset_at,
        used_tokens,
    })
}

/// Compute remaining-percentage from `used` vs `limit`. Clamps to `0.0..=100.0`.
/// Returns `0.0` for `limit == 0` (defensive against config misconfiguration).
#[must_use]
pub fn percent_remaining(used: u64, limit: u64) -> f32 {
    if limit == 0 {
        return 0.0;
    }
    let remaining = limit.saturating_sub(used);
    pct_from_ratio(remaining, limit)
}

/// Convert a saturating ratio into a clamped percent. Cast lints scoped to this fn
/// (mirrors `cli::render_text::filled_cells` pattern).
#[allow(clippy::cast_precision_loss)]
fn pct_from_ratio(remaining: u64, limit: u64) -> f32 {
    let raw = (remaining as f32) / (limit as f32) * 100.0;
    raw.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::claude::jsonl::{AssistantEntry, ClaudeMessage, Usage};

    fn make_entry(ts: &str, cache_creation: u64) -> AssistantEntry {
        AssistantEntry {
            timestamp: ts.parse().unwrap(),
            message: ClaudeMessage {
                usage: Some(Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_creation_input_tokens: cache_creation,
                    cache_read_input_tokens: 0,
                }),
            },
        }
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(find_active_cluster(&[]).is_none());
    }

    #[test]
    fn single_message_returns_single_cluster() {
        let msgs = vec![make_entry("2026-05-23T12:00:00Z", 41630)];
        let cluster = find_active_cluster(&msgs).unwrap();
        assert_eq!(
            cluster.session_start,
            "2026-05-23T12:00:00Z".parse::<jiff::Timestamp>().unwrap()
        );
        assert_eq!(cluster.used_tokens, 41630);
        let expected_reset: jiff::Timestamp = "2026-05-23T17:00:00Z".parse().unwrap();
        assert_eq!(cluster.reset_at, expected_reset);
    }

    #[test]
    fn used_tokens_sums_only_cache_creation_field() {
        // Specifically tests that ONLY cache_creation_input_tokens is summed
        // (D-33 amended; L1 binding — input_tokens + output_tokens are upstream-broken).
        let msgs = vec![
            make_entry("2026-05-23T12:00:00Z", 41630),
            make_entry("2026-05-23T13:00:00Z", 100),
        ];
        let cluster = find_active_cluster(&msgs).unwrap();
        assert_eq!(cluster.used_tokens, 41_730);
    }

    #[test]
    fn gap_over_5_hours_splits_cluster() {
        // Three messages: A at 00:00, B at 01:00, gap, C at 10:00, D at 11:00.
        // The 9h gap between B and C splits — cluster should be [C, D].
        let msgs = vec![
            make_entry("2026-05-23T00:00:00Z", 100),
            make_entry("2026-05-23T01:00:00Z", 200),
            make_entry("2026-05-23T10:00:00Z", 400),
            make_entry("2026-05-23T11:00:00Z", 800),
        ];
        let cluster = find_active_cluster(&msgs).unwrap();
        assert_eq!(
            cluster.session_start,
            "2026-05-23T10:00:00Z".parse::<jiff::Timestamp>().unwrap()
        );
        // Only C + D are summed.
        assert_eq!(cluster.used_tokens, 1200);
    }

    #[test]
    fn no_gap_means_all_messages_in_one_cluster() {
        let msgs = vec![
            make_entry("2026-05-23T00:00:00Z", 10),
            make_entry("2026-05-23T01:00:00Z", 20),
            make_entry("2026-05-23T02:00:00Z", 30),
            make_entry("2026-05-23T03:00:00Z", 40),
        ];
        let cluster = find_active_cluster(&msgs).unwrap();
        assert_eq!(
            cluster.session_start,
            "2026-05-23T00:00:00Z".parse::<jiff::Timestamp>().unwrap()
        );
        assert_eq!(cluster.used_tokens, 100);
    }

    #[test]
    fn percent_remaining_edge_cases() {
        // limit = 0 → 0.0 (defensive)
        assert!((percent_remaining(0, 0) - 0.0).abs() < f32::EPSILON);
        assert!((percent_remaining(100, 0) - 0.0).abs() < f32::EPSILON);
        // used = 0 → 100.0 (fresh window)
        assert!((percent_remaining(0, 44_000) - 100.0).abs() < f32::EPSILON);
        // used = limit → 0.0
        assert!((percent_remaining(44_000, 44_000) - 0.0).abs() < f32::EPSILON);
        // used > limit (saturating) → 0.0
        assert!((percent_remaining(100_000, 44_000) - 0.0).abs() < f32::EPSILON);
        // mid-window
        let p = percent_remaining(22_000, 44_000);
        assert!((p - 50.0).abs() < 0.01, "expected ~50.0, got {p}");
    }

    #[test]
    fn const_is_44000() {
        // D-44 binding: the published constant.
        assert_eq!(CLAUDE_5H_TOKEN_LIMIT, 44_000);
    }
}
