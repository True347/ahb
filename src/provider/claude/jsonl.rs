//! Streaming JSONL parser for `~/.claude/projects/**/*.jsonl`.
//!
//! Contract (D-35):
//! - `BufReader::new(File::open(path)?).lines()` streaming — never `read_to_string`.
//! - Mid-file `serde_json::from_str` failure → `tracing::warn!` + skip.
//! - Last-line `serde_json::from_str` failure → silent skip (truncated trailing line —
//!   Claude is mid-append; this is normal).
//! - File-open failure → `tracing::warn!` + return `Vec::new()`. Adapter wraps the
//!   higher-level error; IO layer never `?`-propagates.
//!
//! Schema (L1 binding): `Usage` fields are `u64` with `#[serde(default)]`. Plan 02's
//! drift detector uses a separate `serde_json::Value` re-parse path to distinguish
//! missing-vs-zero — Plan 01 schema does NOT change.
//!
//! Glob discovery: follows symlinks by default (L8); acceptable for user-owned
//! `~/.claude/projects/`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Envelope: discriminator on `type` field. Only `assistant` carries the token usage we need.
/// Other variants (user, file-history-snapshot, permission-mode, …) are silently skipped.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum JsonlEntry {
    Assistant(AssistantEntry),
    #[serde(other)]
    Other,
}

/// One assistant message envelope. Outer-level `timestamp` + `message.usage` is all we read.
#[derive(Debug, Deserialize, Clone)]
pub struct AssistantEntry {
    pub timestamp: jiff::Timestamp,
    pub message: ClaudeMessage,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ClaudeMessage {
    #[serde(default)]
    pub usage: Option<Usage>,
}

/// Token-usage block. L1 binding: only `cache_creation_input_tokens` is summed by the
/// adapter; the other three are deserialized for forward-compatibility and to keep
/// the schema explicit, but never participate in the budget calculation in Phase 1.
#[derive(Debug, Deserialize, Clone)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

/// Read all assistant entries from one JSONL file. Tolerates mid-file parse errors
/// (`tracing::warn!` + skip) and silently skips a truncated trailing line (D-35).
///
/// Never propagates a parse error out — IO-layer failures collapse to `Vec::new()`
/// with a `tracing::warn!`. The adapter wraps higher-level missing-directory errors.
#[must_use]
pub fn read_assistant_entries(path: &Path) -> Vec<AssistantEntry> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("could not open {}: {e}", path.display());
            return Vec::new();
        }
    };
    let reader = BufReader::new(file);
    let mut lines = reader.lines().peekable();
    let mut out: Vec<AssistantEntry> = Vec::new();
    while let Some(line_res) = lines.next() {
        let is_last = lines.peek().is_none();
        let line = match line_res {
            Ok(l) => l,
            Err(e) => {
                if is_last {
                    // Trailing-line IO failure: treat as in-flight append (silent).
                } else {
                    tracing::warn!("io error mid-file in {}: {e}", path.display());
                }
                continue;
            }
        };
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<JsonlEntry>(&line) {
            Ok(JsonlEntry::Assistant(a)) => out.push(a),
            Ok(JsonlEntry::Other) => {} // user / snapshot / permission-mode / etc.
            Err(e) => {
                if is_last {
                    // D-35: silent skip on truncated trailing line.
                } else {
                    tracing::warn!("malformed jsonl line in {}: {e}", path.display());
                }
            }
        }
    }
    out
}

/// Discover all `*.jsonl` files under `base/**`. Returns an empty Vec if `base` does
/// not exist (adapter handles the missing-directory error one layer up).
#[must_use]
pub fn discover_session_files(base: &Path) -> Vec<PathBuf> {
    if !base.exists() {
        return Vec::new();
    }
    let pattern = base.join("**").join("*.jsonl");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const FIXTURE_ASSISTANT_LINE: &str = r#"{"parentUuid":"abc","isSidechain":false,"message":{"model":"claude-opus-4-7","id":"msg_x","type":"message","role":"assistant","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"cache_creation_input_tokens":41630,"cache_read_input_tokens":1000,"output_tokens":186}},"type":"assistant","uuid":"u1","timestamp":"2026-05-23T05:10:50.300Z"}"#;
    const FIXTURE_USER_LINE: &str = r#"{"parentUuid":null,"isSidechain":false,"type":"user","message":{"role":"user","content":"hi"},"uuid":"u2","timestamp":"2026-05-23T05:10:40Z"}"#;
    const FIXTURE_TRUNCATED_LAST: &str = r#"{"type":"assist"#;

    #[test]
    fn parses_assistant_line_with_cache_creation_input_tokens() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp, "{FIXTURE_ASSISTANT_LINE}").unwrap();
        let entries = read_assistant_entries(tmp.path());
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.message.usage.as_ref().unwrap().cache_creation_input_tokens, 41_630);
        let ts: jiff::Timestamp = "2026-05-23T05:10:50.300Z".parse().unwrap();
        assert_eq!(entry.timestamp, ts);
    }

    #[test]
    fn skips_non_assistant_lines() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp, "{FIXTURE_USER_LINE}").unwrap();
        writeln!(&tmp, "{FIXTURE_ASSISTANT_LINE}").unwrap();
        let entries = read_assistant_entries(tmp.path());
        assert_eq!(entries.len(), 1, "user line must be skipped");
    }

    #[test]
    fn tolerates_truncated_trailing_line() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp, "{FIXTURE_ASSISTANT_LINE}").unwrap();
        // No newline after the truncated line — simulates mid-append.
        write!(&tmp, "{FIXTURE_TRUNCATED_LAST}").unwrap();
        let entries = read_assistant_entries(tmp.path());
        // The truncated final line must NOT contribute (D-35: silent skip).
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message.usage.as_ref().unwrap().cache_creation_input_tokens, 41_630);
    }

    #[test]
    fn nonexistent_file_returns_empty_vec_no_panic() {
        let entries = read_assistant_entries(Path::new("/this/does/not/exist/file.jsonl"));
        assert!(entries.is_empty());
    }

    #[test]
    fn discover_returns_empty_for_missing_base() {
        let entries = discover_session_files(Path::new("/tmp/nonexistent-ahb-base-xyz"));
        assert!(entries.is_empty());
    }

    #[test]
    fn discover_finds_jsonl_files_under_nested_base() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("proj-a");
        std::fs::create_dir_all(&nested).unwrap();
        let file = nested.join("session.jsonl");
        std::fs::write(&file, FIXTURE_ASSISTANT_LINE).unwrap();
        let files = discover_session_files(tmp.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], file);
    }

    #[test]
    fn empty_lines_are_skipped_silently() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(&tmp).unwrap();
        writeln!(&tmp, "{FIXTURE_ASSISTANT_LINE}").unwrap();
        writeln!(&tmp).unwrap();
        let entries = read_assistant_entries(tmp.path());
        assert_eq!(entries.len(), 1);
    }
}
