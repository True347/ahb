//! Codex CLI provider adapter (REQ ADP-04).
//!
//! Phase 2 flow:
//! 1. Discover `<home>/.codex/state_*.sqlite` (D-46 version-glob, pick-highest-N).
//!    Missing → `Err(Unavailable { reason: "no ~/.codex/state_*.sqlite found — is Codex CLI installed?" })`.
//! 2. Glob `<home>/.codex/sessions/**/rollout-*.jsonl` + pick newest by mtime.
//!    Missing → `Err(Unavailable { reason: "no ~/.codex/sessions/**/rollout-*.jsonl found — run Codex CLI at least once to generate one?" })`.
//! 3. In `tokio::task::spawn_blocking` (narrow-scope per RESEARCH §spawn_blocking Pattern):
//!    open SQLite read-only with `busy_timeout=250ms` (Phase 2 contract — D-45 — runs
//!    ZERO SELECT queries; opening + drop is enough to prove the contract), then
//!    parse the newest rollout for the LATEST non-null `rate_limits` event.
//! 4. `rate_limits: null` or no usable snapshot → `Err(SchemaDrift { missing: ["rate_limits"] })`
//!    (D-47 — render layer emits `Codex adapter may be out-of-date` sentinel).
//! 5. `HpWindow.label = "primary" | "secondary"` passthrough (D-48); no reordering,
//!    no synthesis. `resets_at` anchored on rollout line timestamp (NOT `ctx.now`).
//!
//! Decisions binding (CONTEXT D-45..D-48):
//! - SQLite is supplemental metadata; JSONL is primary signal.
//! - `OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX`; never write.
//! - `tracing::warn!` on >1 `state_*.sqlite` coexistence; never block.
//! - `Cow::Borrowed("codex-jsonl")` source (SQLite contributes no row data in Phase 2).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::model::{ProviderError, ProviderId, ProviderState};
use crate::provider::{FetchCtx, Provider};

pub mod jsonl;
pub mod sqlite;
pub mod window;

/// Codex CLI adapter. `codex_dir` is `home_dir.join(".codex")` (test-injectable
/// via `new(home_dir)`); secrets are not consumed by Codex in Phase 2 but the
/// SEC-04 contract is honored via `let _ = ctx.secrets;` in `fetch`.
pub struct CodexProvider {
    codex_dir: PathBuf,
}

impl CodexProvider {
    /// Construct from a `home_dir`. Tests use `tempfile::tempdir()` to avoid touching
    /// the real `~/.codex`.
    #[must_use]
    pub fn new(home_dir: &Path) -> Self {
        Self {
            codex_dir: home_dir.join(".codex"),
        }
    }
}

#[async_trait]
impl Provider for CodexProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Codex
    }

    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        // SEC-04: Codex needs no secrets; the contract still hands them through.
        let _ = ctx.secrets;

        // Step 1 (async, cheap): SQLite discovery.
        let sqlite_path = match sqlite::discover_state_sqlite(&self.codex_dir) {
            Some(p) => p,
            None => {
                return Err(ProviderError::Unavailable {
                    reason: "no ~/.codex/state_*.sqlite found — is Codex CLI installed?".into(),
                });
            }
        };

        // Step 2 (async, cheap): JSONL discovery + newest pick.
        let rollout_paths = jsonl::discover_rollouts(&self.codex_dir);
        let Some(newest_rollout) = jsonl::pick_newest_file(&rollout_paths) else {
            return Err(ProviderError::Unavailable {
                reason:
                    "no ~/.codex/sessions/**/rollout-*.jsonl found — run Codex CLI at least once to generate one?"
                        .into(),
            });
        };

        // Step 3 (blocking): rusqlite open + busy_timeout + JSONL parse.
        // Captured by `move` — all owned (PathBuf) or Copy (none here).
        let sqlite_path_owned = sqlite_path.clone();
        let rollout_path_owned = newest_rollout.clone();
        let blocking_handle = tokio::task::spawn_blocking(move || -> Result<Vec<crate::model::HpWindow>, ProviderError> {
            // Phase 2 D-45 contract: open + busy_timeout, then immediately drop.
            // No SELECT queries — schema reads deferred to Phase 3.
            let conn = sqlite::open_readonly(&sqlite_path_owned)?;
            drop(conn);
            jsonl::parse_codex_rollout_windows(&rollout_path_owned)
        });

        // Step 4 (async): map JoinError per RESEARCH §spawn_blocking Pattern.
        let windows = match blocking_handle.await {
            Ok(Ok(ws)) => ws,
            Ok(Err(e)) => return Err(e),
            Err(je) if je.is_panic() => {
                return Err(ProviderError::Internal {
                    source: anyhow::anyhow!("codex adapter blocking thread panicked"),
                });
            }
            Err(je) => {
                return Err(ProviderError::Internal {
                    source: anyhow::anyhow!("codex adapter blocking task failed: {je}"),
                });
            }
        };

        Ok(ProviderState {
            id: ProviderId::Codex,
            windows,
            fetched_at: ctx.now,
            source: Cow::Borrowed("codex-jsonl"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::Secrets;
    use static_assertions::assert_impl_all;
    use std::io::Write;

    assert_impl_all!(CodexProvider: Send, Sync);
    assert_impl_all!(Box<dyn Provider>: Send, Sync);

    fn touch_sqlite(home: &Path) -> PathBuf {
        let p = home.join(".codex").join("state_5.sqlite");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let conn = rusqlite::Connection::open(&p).unwrap();
        drop(conn);
        p
    }

    fn write_rollout(home: &Path, line: &str) -> PathBuf {
        let dir = home.join(".codex").join("sessions").join("2026").join("05").join("25");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("rollout-test.jsonl");
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "{line}").unwrap();
        p
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn test_a_empty_codex_dir_returns_no_sqlite_literal() {
        let home = tempfile::tempdir().unwrap();
        // Do NOT create .codex/ — fully empty home.
        let provider = CodexProvider::new(home.path());
        let secrets = Secrets::default();
        let now: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };
        let err = provider.fetch(&ctx).await.unwrap_err();
        match err {
            ProviderError::Unavailable { reason } => {
                assert_eq!(
                    reason,
                    "no ~/.codex/state_*.sqlite found — is Codex CLI installed?"
                );
                assert!(reason.ends_with('?'), "must end with next-step hint");
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn test_b_sqlite_present_but_no_rollouts_returns_unavailable_literal() {
        let home = tempfile::tempdir().unwrap();
        let _ = touch_sqlite(home.path());
        // No rollout files.
        let provider = CodexProvider::new(home.path());
        let secrets = Secrets::default();
        let now: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };
        let err = provider.fetch(&ctx).await.unwrap_err();
        match err {
            ProviderError::Unavailable { reason } => {
                assert!(
                    reason.contains("no ~/.codex/sessions"),
                    "expected sessions hint, got: {reason}"
                );
                assert!(reason.ends_with('?'), "must end with next-step hint");
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn test_c_sqlite_and_valid_rollout_yields_ok_state_with_codex_jsonl_source() {
        let home = tempfile::tempdir().unwrap();
        let _ = touch_sqlite(home.path());
        let line = r#"{"timestamp":"2026-05-25T11:00:00Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":{"primary":{"used_percent":25.0,"window_minutes":299,"resets_in_seconds":3600}}}}"#;
        let _ = write_rollout(home.path(), line);

        let provider = CodexProvider::new(home.path());
        let secrets = Secrets::default();
        let now: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };
        let state = provider.fetch(&ctx).await.unwrap();
        assert_eq!(state.id, ProviderId::Codex);
        assert_eq!(state.source, "codex-jsonl");
        assert_eq!(state.fetched_at, now);
        assert!(!state.windows.is_empty());
        assert_eq!(state.windows[0].label, "primary");
        assert!((state.windows[0].percent_remaining - 75.0).abs() < 0.01);
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn test_d_rate_limits_null_returns_schema_drift() {
        let home = tempfile::tempdir().unwrap();
        let _ = touch_sqlite(home.path());
        let line = r#"{"timestamp":"2026-05-25T11:00:00Z","type":"event_msg","payload":{"type":"token_count","info":null,"rate_limits":null}}"#;
        let _ = write_rollout(home.path(), line);

        let provider = CodexProvider::new(home.path());
        let secrets = Secrets::default();
        let now: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };
        let err = provider.fetch(&ctx).await.unwrap_err();
        match err {
            ProviderError::SchemaDrift { missing } => {
                assert_eq!(missing, vec!["rate_limits".to_string()]);
            }
            other => panic!("expected SchemaDrift, got {other:?}"),
        }
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn id_returns_codex() {
        let home = tempfile::tempdir().unwrap();
        let provider = CodexProvider::new(home.path());
        assert_eq!(provider.id(), ProviderId::Codex);
    }
}
