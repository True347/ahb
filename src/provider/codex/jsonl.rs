//! Streaming JSONL parser for `~/.codex/sessions/**/rollout-*.jsonl` (REQ ADP-04).
//!
//! Contract (D-35-amended for Codex per RESEARCH §Codex JSONL Schema):
//! - `BufReader::new(File::open(path)?).lines()` streaming — never `read_to_string`.
//! - Each line is a `RolloutLine` envelope; only `line_type == "event_msg"` with a
//!   `payload.type == "token_count"` payload carries rate-limit data we need.
//! - Mid-file `serde_json::from_str` failure → `tracing::warn!` + skip.
//! - Trailing-line failure → silent skip (Codex CLI is mid-append; D-35 tolerance).
//! - File-open failure → `tracing::warn!` + return `Vec::new()` / propagate as
//!   `SchemaDrift` from the public `parse_codex_rollout_windows` (depends on context).
//!
//! Schema (verified against `openai/codex` issue #14728):
//! ```text
//! { "timestamp": "<RFC3339>",
//!   "type": "event_msg",
//!   "payload": { "type": "token_count",
//!                "rate_limits": { "primary":   {"used_percent":0.0, "window_minutes":299,   "resets_in_seconds":17940},
//!                                 "secondary": {"used_percent":6.0, "window_minutes":10079, "resets_in_seconds":275281} } } }
//! ```
//! `rate_limits: null` is widespread (issue #14880) — D-47 maps this to `SchemaDrift`.
//!
//! Walking strategy: walk the file forward and keep the LATEST line whose
//! `rate_limits` is `Some` AND at least one of `primary` / `secondary` is `Some`.
//! Anchor `resets_at` math on the rollout line's OWN `timestamp` (RESEARCH §Codex
//! JSONL Schema bullet 3) — NOT `ctx.now`. The no_walltime grep test forbids
//! `Timestamp::now()` calls but explicitly allows arithmetic on a parsed timestamp.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::model::{HpWindow, ProviderError};

/// Outer rollout-line envelope. We only consume `timestamp` + `line_type` + `payload`.
/// All other rollout-line variants (`response_item`, `session_meta`, …) deserialize
/// fine because `payload` is `Option<serde_json::Value>` — typed parsing happens
/// at the next layer (`RolloutPayload`).
#[derive(Debug, Deserialize)]
pub(super) struct RolloutLine {
    pub timestamp: jiff::Timestamp,
    #[serde(rename = "type")]
    pub line_type: String,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

/// Inner payload — discriminator on `type`. We only act on `token_count`; all other
/// shapes (response items, session meta, …) collapse to `Other` and are skipped.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum RolloutPayload {
    TokenCount(TokenCountPayload),
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub(super) struct TokenCountPayload {
    // `info` is intentionally NOT parsed (D-47 — no estimation from token counts).
    #[serde(default)]
    pub rate_limits: Option<RateLimits>,
}

// TODO Phase 3: consider BTreeMap<String, RateLimitTier> if upstream adds 'weekly'
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct RateLimits {
    #[serde(default)]
    pub primary: Option<RateLimitTier>,
    #[serde(default)]
    pub secondary: Option<RateLimitTier>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct RateLimitTier {
    pub used_percent: f64,
    #[allow(dead_code)] // Phase 2: informational only; Phase 3 may surface in --detailed.
    pub window_minutes: u64,
    pub resets_in_seconds: u64,
}

/// Discover all rollout JSONL files under `codex_dir/sessions/**/rollout-*.jsonl`.
/// Returns an empty Vec if `codex_dir` does not exist (adapter handles the
/// missing-directory error one layer up). Mirrors
/// `claude::jsonl::discover_session_files`.
#[must_use]
pub fn discover_rollouts(codex_dir: &Path) -> Vec<PathBuf> {
    let sessions = codex_dir.join("sessions");
    if !sessions.exists() {
        return Vec::new();
    }
    let pattern = sessions.join("**").join("rollout-*.jsonl");
    let pattern_str = pattern.to_string_lossy();
    let opts = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };
    match glob::glob_with(&pattern_str, opts) {
        Ok(paths) => paths.filter_map(Result::ok).collect(),
        Err(e) => {
            tracing::warn!("glob error for {}: {e}", pattern_str);
            Vec::new()
        }
    }
}

/// Pick the JSONL file with the most-recent `modified()` timestamp. Returns `None`
/// if `files` is empty or every stat call fails. Duplicated (intentionally) from
/// `provider::claude::mod::pick_newest_file` per PATTERNS Pattern 1 §225 — keep
/// the duplication until a third caller appears.
#[must_use]
pub fn pick_newest_file(files: &[PathBuf]) -> Option<PathBuf> {
    files
        .iter()
        .filter_map(|p| {
            let meta = std::fs::metadata(p).ok()?;
            let mtime = meta.modified().ok()?;
            Some((p.clone(), mtime))
        })
        .max_by_key(|(_, mtime)| *mtime)
        .map(|(p, _)| p)
}

/// Walk the rollout file and return the HpWindow vector derived from the
/// LATEST `token_count` event whose `rate_limits` is non-null AND has at least
/// one of `primary` / `secondary` present.
///
/// Returns `Err(ProviderError::SchemaDrift { missing: vec!["rate_limits"] })`
/// if no usable snapshot is found in the file (D-47).
///
/// IO errors (open/read) are tolerated per D-35 — mid-file failures warn and
/// continue; trailing-line failures are silent. A wholesale open failure on
/// `path` yields `SchemaDrift` because the caller already validated the
/// rollout file existed via `discover_rollouts` + `pick_newest_file`.
pub fn parse_codex_rollout_windows(path: &Path) -> Result<Vec<HpWindow>, ProviderError> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("could not open codex rollout {}: {e}", path.display());
            return Err(ProviderError::SchemaDrift {
                missing: vec!["rate_limits".to_string()],
            });
        }
    };
    let reader = BufReader::new(file);
    let mut lines = reader.lines().peekable();
    let mut latest: Option<(jiff::Timestamp, RateLimits)> = None;

    while let Some(line_res) = lines.next() {
        let is_last = lines.peek().is_none();
        let line = match line_res {
            Ok(l) => l,
            Err(e) => {
                if !is_last {
                    tracing::warn!("io error mid-file in {}: {e}", path.display());
                }
                continue;
            }
        };
        if line.is_empty() {
            continue;
        }
        // Step 1: parse the outer envelope. Cheap reject of unrelated rollout lines.
        let envelope = match serde_json::from_str::<RolloutLine>(&line) {
            Ok(e) => e,
            Err(e) => {
                if !is_last {
                    tracing::warn!(
                        "malformed codex rollout line in {}: {e}",
                        path.display()
                    );
                }
                continue;
            }
        };
        if envelope.line_type != "event_msg" {
            continue;
        }
        let Some(payload_value) = envelope.payload else {
            continue;
        };
        // Step 2: parse the payload variant.
        let payload = match serde_json::from_value::<RolloutPayload>(payload_value) {
            Ok(p) => p,
            Err(e) => {
                if !is_last {
                    tracing::warn!(
                        "malformed codex payload in {}: {e}",
                        path.display()
                    );
                }
                continue;
            }
        };
        let RolloutPayload::TokenCount(tc) = payload else {
            continue;
        };
        let Some(rl) = tc.rate_limits else {
            continue;
        };
        if rl.primary.is_none() && rl.secondary.is_none() {
            continue;
        }
        // D-47: keep the LATEST non-null snapshot (last-found wins).
        latest = Some((envelope.timestamp, rl));
    }

    let Some((line_ts, rate_limits)) = latest else {
        return Err(ProviderError::SchemaDrift {
            missing: vec!["rate_limits".to_string()],
        });
    };
    Ok(super::window::to_hp_windows(&rate_limits, line_ts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const RL_PRIMARY_25_RESET_7200: &str = r#"{"timestamp":"2026-05-25T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"primary":{"used_percent":25.0,"window_minutes":299,"resets_in_seconds":7200}}}}"#;
    const RL_BOTH_TIERS: &str = r#"{"timestamp":"2026-05-25T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"primary":{"used_percent":10.0,"window_minutes":299,"resets_in_seconds":3600},"secondary":{"used_percent":20.0,"window_minutes":10079,"resets_in_seconds":7200}}}}"#;
    const RL_NULL: &str = r#"{"timestamp":"2026-05-25T12:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":14676,"output_tokens":412,"total_tokens":15088}},"rate_limits":null}}"#;
    const RL_OLDER: &str = r#"{"timestamp":"2026-05-25T11:00:00Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"primary":{"used_percent":80.0,"window_minutes":299,"resets_in_seconds":1800}}}}"#;
    const NON_EVENT_MSG: &str = r#"{"timestamp":"2026-05-25T12:00:00Z","type":"response_item","payload":{"foo":"bar"}}"#;

    #[test]
    fn test_1_parses_primary_only_and_computes_75_percent_with_correct_reset_anchor() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp, "{RL_PRIMARY_25_RESET_7200}").unwrap();
        let windows = parse_codex_rollout_windows(tmp.path()).unwrap();
        assert_eq!(windows.len(), 1);
        let w = &windows[0];
        assert_eq!(w.label, "primary");
        assert!(
            (w.percent_remaining - 75.0).abs() < 0.01,
            "expected ~75% remaining, got {}",
            w.percent_remaining
        );
        // line_ts = 2026-05-25T12:00:00Z + 7200s = 2026-05-25T14:00:00Z
        let expected_reset: jiff::Timestamp = "2026-05-25T14:00:00Z".parse().unwrap();
        assert_eq!(w.reset.resets_at, expected_reset);
    }

    #[test]
    fn test_2_rate_limits_null_yields_schema_drift() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp, "{RL_NULL}").unwrap();
        let err = parse_codex_rollout_windows(tmp.path()).unwrap_err();
        match err {
            ProviderError::SchemaDrift { missing } => {
                assert_eq!(missing, vec!["rate_limits".to_string()]);
            }
            other => panic!("expected SchemaDrift, got {other:?}"),
        }
    }

    #[test]
    fn test_3_both_tiers_emit_passthrough_order_primary_first() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp, "{RL_BOTH_TIERS}").unwrap();
        let windows = parse_codex_rollout_windows(tmp.path()).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].label, "primary");
        assert_eq!(windows[1].label, "secondary");
        // 100 - 10 = 90; 100 - 20 = 80
        assert!((windows[0].percent_remaining - 90.0).abs() < 0.01);
        assert!((windows[1].percent_remaining - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_4_multiple_token_count_events_latest_non_null_wins() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp, "{RL_OLDER}").unwrap();
        writeln!(&tmp, "{RL_PRIMARY_25_RESET_7200}").unwrap();
        let windows = parse_codex_rollout_windows(tmp.path()).unwrap();
        // Latest wins → 25% used → 75% remaining
        assert_eq!(windows.len(), 1);
        assert!((windows[0].percent_remaining - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_5_malformed_lines_are_silently_skipped_with_trailing_silent_tolerance() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // Mid-file: bogus JSON (will tracing::warn but parse continues).
        writeln!(&tmp, "this is not valid json").unwrap();
        writeln!(&tmp, "{RL_PRIMARY_25_RESET_7200}").unwrap();
        // Trailing: truncated line (silent skip).
        write!(&tmp, r#"{{"timestamp":"2026"#).unwrap();
        let windows = parse_codex_rollout_windows(tmp.path()).unwrap();
        assert_eq!(windows.len(), 1);
        assert!((windows[0].percent_remaining - 75.0).abs() < 0.01);
    }

    #[test]
    fn non_event_msg_lines_are_skipped() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp, "{NON_EVENT_MSG}").unwrap();
        writeln!(&tmp, "{RL_PRIMARY_25_RESET_7200}").unwrap();
        let windows = parse_codex_rollout_windows(tmp.path()).unwrap();
        assert_eq!(windows.len(), 1);
    }

    #[test]
    fn file_with_only_null_rate_limits_yields_schema_drift() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp, "{RL_NULL}").unwrap();
        writeln!(&tmp, "{RL_NULL}").unwrap();
        let err = parse_codex_rollout_windows(tmp.path()).unwrap_err();
        match err {
            ProviderError::SchemaDrift { missing } => {
                assert_eq!(missing, vec!["rate_limits".to_string()]);
            }
            other => panic!("expected SchemaDrift, got {other:?}"),
        }
    }

    #[test]
    fn empty_file_yields_schema_drift() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let err = parse_codex_rollout_windows(tmp.path()).unwrap_err();
        matches!(err, ProviderError::SchemaDrift { .. });
    }

    #[test]
    fn discover_rollouts_returns_empty_for_missing_sessions_dir() {
        let tmp = tempfile::tempdir().unwrap();
        // `.codex` exists, but no `sessions/` subdir.
        std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();
        let files = discover_rollouts(&tmp.path().join(".codex"));
        assert!(files.is_empty());
    }

    #[test]
    fn discover_rollouts_finds_nested_rollout_files() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("sessions").join("2026").join("05").join("25");
        std::fs::create_dir_all(&nested).unwrap();
        let file = nested.join("rollout-foo.jsonl");
        std::fs::write(&file, RL_PRIMARY_25_RESET_7200).unwrap();
        let files = discover_rollouts(tmp.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], file);
    }

    #[test]
    fn pick_newest_file_returns_none_for_empty() {
        let files: Vec<PathBuf> = Vec::new();
        assert!(pick_newest_file(&files).is_none());
    }
}
