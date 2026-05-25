//! Claude Code provider adapter (REQ ADP-02).
//!
//! Phase 1 flow:
//! 1. Glob `~/.claude/projects/**/*.jsonl` (via `home_dir.join(".claude").join("projects")`).
//! 2. Stream each JSONL file via `jsonl::read_assistant_entries` (D-35 tolerance).
//! 3. Merge + sort by `timestamp`.
//! 4. Cluster anchor via `window::find_active_cluster` (D-33 amended: 5h gap walk).
//! 5. `percent_remaining = window::percent_remaining(used, limit)`.
//! 6. Build one `HpWindow` and return `ProviderState` (CORE-01 single row).
//!
//! Error rows (UI-SPEC LOCKED literals):
//! - missing `~/.claude/projects` → `Err(Unavailable { reason: "~/.claude/projects not found — is Claude Code installed?" })`
//! - empty cluster → `Err(Unavailable { reason: "no claude sessions found in ~/.claude/projects — run Claude Code at least once" })`

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::model::{HpWindow, ProviderError, ProviderId, ProviderState, ResetInfo};
use crate::provider::{FetchCtx, Provider};

pub mod jsonl;
pub mod window;

pub use window::CLAUDE_5H_TOKEN_LIMIT;

/// Claude Code adapter. `base_path` is `home_dir.join(".claude").join("projects")`
/// (configurable for test injection via `new(home_dir, token_limit)`); `token_limit`
/// defaults to `CLAUDE_5H_TOKEN_LIMIT` in production wiring.
pub struct ClaudeProvider {
    base_path: PathBuf,
    token_limit: u64,
}

impl ClaudeProvider {
    /// Construct from a `home_dir` (typically `dirs::home_dir()`) and a token-limit budget.
    /// Tests use a `tempfile::tempdir()` for `home_dir` to avoid touching the real home.
    #[must_use]
    pub fn new(home_dir: &Path, token_limit: u64) -> Self {
        Self {
            base_path: home_dir.join(".claude").join("projects"),
            token_limit,
        }
    }
}

/// Pick the JSONL file with the most-recent `modified()` timestamp. Returns `None` if
/// `files` is empty or every stat call fails. Used by the schema-drift probe to focus
/// on the currently-active session.
fn pick_newest_file(files: &[PathBuf]) -> Option<PathBuf> {
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

#[async_trait]
impl Provider for ClaudeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Claude
    }

    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        // SEC-04: Claude needs no secrets; the contract still hands them through.
        let _ = ctx.secrets;

        if !self.base_path.exists() {
            return Err(ProviderError::Unavailable {
                reason: "~/.claude/projects not found — is Claude Code installed?".into(),
            });
        }
        let files = jsonl::discover_session_files(&self.base_path);

        // ADP-03 schema-drift probe (Plan 02): re-parse the last 3 assistant entries
        // from the most-recently-modified session file as raw `serde_json::Value`s.
        // If ≥ 2 of them lack `/message/usage/cache_creation_input_tokens`, surface
        // `ProviderError::SchemaDrift` so the renderer paints the UI-SPEC sentinel.
        // Uses raw `Value` (NOT the typed `Usage` schema) to distinguish
        // "field absent" from "field present with 0" — Plan 01's typed `u64` schema
        // stays UNCHANGED (WARNING #2 path-a).
        if let Some(newest) = pick_newest_file(&files) {
            let recent = jsonl::read_recent_raw(&newest, 3);
            if let Some(missing) = jsonl::detect_drift(&recent) {
                return Err(ProviderError::SchemaDrift { missing });
            }
        }

        let mut merged: Vec<jsonl::AssistantEntry> = Vec::new();
        for f in &files {
            let entries = jsonl::read_assistant_entries(f);
            merged.extend(entries);
        }
        // Sort ascending by timestamp.
        merged.sort_by_key(|e| e.timestamp);
        let Some(cluster) = window::find_active_cluster(&merged) else {
            return Err(ProviderError::Unavailable {
                reason: "no claude sessions found in ~/.claude/projects — run Claude Code at least once".into(),
            });
        };
        let pct = window::percent_remaining(cluster.used_tokens, self.token_limit);
        // UI-SPEC LOCKED: `label` stays "claude" because the compact-mode
        // renderer's row prefix comes from `id_label(state.id)` (Phase 2 Plan
        // 02-01 [Rule 2] binding) — keeping `label="claude"` preserves Phase 1
        // model-layer invariants and any consumer that historically read
        // `windows[0].label` (the JSON shape was additive only). The Phase 2
        // `--detailed` view reads `detailed_label.as_deref().unwrap_or(&label)`
        // and the override `Some("5h")` here is what surfaces in the indented
        // per-window row.
        let win_5h = HpWindow {
            label: Cow::Borrowed("claude"),
            percent_remaining: pct,
            reset: ResetInfo {
                resets_at: cluster.reset_at,
            },
            bar_color: None,
            detailed_label: Some(Cow::Borrowed("5h")),
        };
        // Phase 2 D-54: same JSONL scan pass also computes the weekly window
        // (`label="weekly"` + `detailed_label=Some("weekly")` + NaN sentinel
        // when `CLAUDE_WEEKLY_TOKEN_LIMIT` is None). Passthrough order is
        // `[5h, weekly]` per D-55 (Phase 2 adapter passthrough rule).
        let weekly = window::compute_weekly_window(&merged, ctx.now);
        let mut windows = vec![win_5h];
        if let Some(w) = weekly {
            windows.push(w);
        }
        Ok(ProviderState {
            id: ProviderId::Claude,
            windows,
            fetched_at: ctx.now,
            source: Cow::Borrowed("claude-jsonl"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::Secrets;
    use static_assertions::assert_impl_all;
    use std::io::Write;

    assert_impl_all!(ClaudeProvider: Send, Sync);
    assert_impl_all!(Box<dyn Provider>: Send, Sync);

    const FIXTURE_ASSISTANT_LINE: &str = r#"{"parentUuid":"abc","isSidechain":false,"message":{"model":"claude-opus-4-7","id":"msg_x","type":"message","role":"assistant","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":5,"cache_creation_input_tokens":4400,"cache_read_input_tokens":1000,"output_tokens":186}},"type":"assistant","uuid":"u1","timestamp":"2026-05-23T11:00:00Z"}"#;

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn fetch_against_tempdir_with_one_assistant_entry() {
        let home = tempfile::tempdir().unwrap();
        let projects_dir = home.path().join(".claude").join("projects").join("proj-a");
        std::fs::create_dir_all(&projects_dir).unwrap();
        let mut file =
            std::fs::File::create(projects_dir.join("session.jsonl")).unwrap();
        writeln!(file, "{FIXTURE_ASSISTANT_LINE}").unwrap();

        let provider = ClaudeProvider::new(home.path(), CLAUDE_5H_TOKEN_LIMIT);
        let secrets = Secrets::default();
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };

        let state = provider.fetch(&ctx).await.unwrap();
        assert_eq!(state.id, ProviderId::Claude);
        // Phase 2 D-54: ClaudeProvider now emits TWO windows in passthrough order
        // `[5h, weekly]`. Phase 1's `windows.len() == 1` invariant is replaced
        // with `== 2`; `windows[0]` continues to carry the 5h signal byte-
        // identical to Phase 1 (compact mode unaffected — see the dedicated
        // `compact_prefix_preserves_phase1_literal` test below).
        assert_eq!(state.windows.len(), 2);
        assert_eq!(state.source, "claude-jsonl");
        assert_eq!(state.fetched_at, now);
        // Single cluster, 4400 used out of 44000 → 90% remaining (windows[0] = 5h).
        assert!(
            (state.windows[0].percent_remaining - 90.0).abs() < 0.01,
            "expected ~90% remaining, got {}",
            state.windows[0].percent_remaining
        );
        // session_start = 11:00 → reset_at = 16:00
        let expected_reset: jiff::Timestamp = "2026-05-23T16:00:00Z".parse().unwrap();
        assert_eq!(state.windows[0].reset.resets_at, expected_reset);
        // Phase 1 invariant PRESERVED: windows[0].label is still "claude".
        assert_eq!(state.windows[0].label, "claude");
        // Phase 2 D-54 additive: windows[0] gets a per-row detailed override.
        assert_eq!(state.windows[0].detailed_label.as_deref(), Some("5h"));
        // Phase 2 D-54 additive: windows[1] is the weekly window with NaN sentinel
        // (because CLAUDE_WEEKLY_TOKEN_LIMIT is None in Phase 2).
        assert_eq!(state.windows[1].label, "weekly");
        assert_eq!(state.windows[1].detailed_label.as_deref(), Some("weekly"));
        assert!(
            state.windows[1].percent_remaining.is_nan(),
            "weekly window must be NaN when limit is None, got {}",
            state.windows[1].percent_remaining
        );
    }

    /// Phase 2 C2: build a `ProviderState` matching what `ClaudeProvider::fetch`
    /// emits today and confirm the compact renderer still produces a string
    /// starting with the byte-identical Phase 1 LOCKED prefix `claude  ` — this
    /// pins the invariant that `compact_line` reads `id_label(state.id)` (NOT
    /// `windows[0].label` or `windows[0].detailed_label`), so Plan 02-02 Task 2's
    /// addition of `detailed_label` cannot silently regress the compact view.
    #[tokio::test]
    async fn compact_prefix_preserves_phase1_literal() {
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let resets_at = now + jiff::Span::new().hours(2);
        let state = ProviderState {
            id: ProviderId::Claude,
            windows: vec![HpWindow {
                label: Cow::Borrowed("claude"),
                percent_remaining: 60.0,
                reset: ResetInfo { resets_at },
                bar_color: None,
                detailed_label: Some(Cow::Borrowed("5h")),
            }],
            fetched_at: now,
            source: Cow::Borrowed("claude-jsonl"),
        };
        let line = crate::cli::render_text::compact_line(&state, &now, true);
        assert!(
            line.starts_with("claude  "),
            "compact line must start with byte-identical Phase 1 LOCKED prefix `claude  `, got: {line:?}"
        );
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn missing_projects_directory_returns_ui_spec_literal() {
        let home = tempfile::tempdir().unwrap();
        // Deliberately do NOT create .claude/projects.
        let provider = ClaudeProvider::new(home.path(), CLAUDE_5H_TOKEN_LIMIT);
        let secrets = Secrets::default();
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };

        let err = provider.fetch(&ctx).await.unwrap_err();
        match err {
            ProviderError::Unavailable { reason } => {
                assert_eq!(
                    reason,
                    "~/.claude/projects not found — is Claude Code installed?"
                );
                assert!(reason.ends_with('?'), "must end with a next-step hint");
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn empty_projects_directory_returns_no_sessions_literal() {
        let home = tempfile::tempdir().unwrap();
        let projects_dir = home.path().join(".claude").join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        // Empty directory: no .jsonl files.

        let provider = ClaudeProvider::new(home.path(), CLAUDE_5H_TOKEN_LIMIT);
        let secrets = Secrets::default();
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };

        let err = provider.fetch(&ctx).await.unwrap_err();
        match err {
            ProviderError::Unavailable { reason } => {
                assert!(
                    reason.contains("no claude sessions found"),
                    "expected next-step hint, got: {reason}"
                );
                assert!(
                    reason.contains("run Claude Code at least once"),
                    "must end with next-step hint, got: {reason}"
                );
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn id_returns_claude() {
        let home = tempfile::tempdir().unwrap();
        let provider = ClaudeProvider::new(home.path(), CLAUDE_5H_TOKEN_LIMIT);
        assert_eq!(provider.id(), ProviderId::Claude);
    }
}
