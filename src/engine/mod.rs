//! Engine — the orchestration layer between config + adapters + front-ends.
//!
//! Phase 1 layout: `Engine::new(config, secrets)` builds the provider list from
//! `config.providers.*.enabled` flags; `refresh_all(now)` fans the fetch out via
//! `fanout::refresh_all_inner` (`JoinSet` + per-adapter timeout + Pitfall L4 panic recovery).
//!
//! Task 1a wires the engine against `MockProvider` only. Task 1b extends `Engine::new`
//! to push `ClaudeProvider` when `cfg.providers.claude.enabled`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::sync::Arc;
use std::time::Duration;

pub mod events;
pub mod fanout;

pub use events::{EngineEvent, EVENT_BUFFER};
pub use fanout::DEFAULT_PER_PROVIDER_TIMEOUT;

use crate::config::Config;
use crate::model::{ProviderError, ProviderId, ProviderState};
use crate::provider::Provider;
use crate::secrets::Secrets;

/// AHB engine. Holds the list of enabled providers + shared secrets handle. Front-ends
/// (CLI compact line, TUI) consume it via `refresh_all(now)`.
pub struct Engine {
    providers: Vec<Arc<dyn Provider>>,
    secrets: Arc<Secrets>,
    per_provider_timeout: Duration,
}

impl Engine {
    /// Build an engine from the parsed config. Phase 1 Task 1a: pushes `MockProvider`
    /// when `cfg.providers.mock.enabled`. Codex / Gemini flags emit `tracing::debug!`
    /// only (not implemented yet). Task 1b adds the `ClaudeProvider` branch.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // builder-style API: takes ownership of config + secrets
    pub fn new(cfg: Config, secrets: Secrets) -> Self {
        let mut providers: Vec<Arc<dyn Provider>> = Vec::new();

        if cfg.providers.claude.enabled {
            // Resolve the user's HOME for ClaudeProvider's base_path. Unavailable HOME
            // collapses to an empty PathBuf — the adapter will then surface the
            // UI-SPEC literal `~/.claude/projects not found — is Claude Code installed?`
            // via the Provider::fetch error path.
            let home = directories::BaseDirs::new()
                .map(|d| d.home_dir().to_path_buf())
                .unwrap_or_default();
            providers.push(std::sync::Arc::new(
                crate::provider::claude::ClaudeProvider::new(
                    &home,
                    crate::provider::claude::CLAUDE_5H_TOKEN_LIMIT,
                ),
            ));
        }
        if cfg.providers.codex.enabled {
            // Mirror the Claude branch's HOME resolution. Unavailable HOME collapses
            // to an empty PathBuf — the adapter will then surface
            // `no ~/.codex/state_*.sqlite found — is Codex CLI installed?` via the
            // Provider::fetch error path.
            let home = directories::BaseDirs::new()
                .map(|d| d.home_dir().to_path_buf())
                .unwrap_or_default();
            providers.push(Arc::new(crate::provider::codex::CodexProvider::new(&home)));
        }
        if cfg.providers.gemini.enabled {
            // CR-01 fix: mirror the Claude / Codex branch and push a real
            // provider that returns `Err(Unavailable)`. Phase 3 swaps in the
            // real adapter; the Engine wiring does not change. Without this
            // branch, a Gemini-only config silently collapsed to the
            // empty-state path (exit 0 "no providers configured") instead of
            // the documented "all configured providers failed" exit 1 — see
            // D-59 / D-61.
            providers.push(Arc::new(crate::provider::gemini::GeminiUnimplementedProvider));
        }
        if cfg.providers.mock.enabled {
            providers.push(Arc::new(crate::provider::mock::MockProvider));
        }

        Self {
            providers,
            secrets: Arc::new(secrets),
            per_provider_timeout: DEFAULT_PER_PROVIDER_TIMEOUT,
        }
    }

    /// Number of enabled providers (useful for front-end empty-state detection).
    #[must_use]
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Fan out one fetch across all enabled providers. Returns
    /// `Vec<(ProviderId, Result<ProviderState, ProviderError>)>` in canonical
    /// `ProviderId` order (Claude=0, Codex=1, Gemini=2, Mock=3) regardless of which
    /// adapter completed first — fanout produces arrival order, the engine sorts
    /// to satisfy the UI-SPEC fixed-row contract (BL-02 fix).
    pub async fn refresh_all(
        &self,
        now: jiff::Timestamp,
    ) -> Vec<(ProviderId, Result<ProviderState, ProviderError>)> {
        let mut results = fanout::refresh_all_inner(
            &self.providers,
            now,
            Arc::clone(&self.secrets),
            self.per_provider_timeout,
        )
        .await;
        // BL-02: canonical row order lives at the engine boundary — single source of
        // truth. Fanout still advertises arrival order; CLI / TUI consumers do not
        // re-sort.
        results.sort_by_key(|(id, _)| Self::sort_key(*id));
        results
    }

    /// Canonical row order for `refresh_all` output. Mock is last because it is
    /// debug / fault-injection only, not a user-facing provider.
    #[must_use]
    fn sort_key(id: ProviderId) -> u8 {
        match id {
            ProviderId::Claude => 0,
            ProviderId::Codex => 1,
            ProviderId::Gemini => 2,
            ProviderId::Mock => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ProviderConfig, Providers};

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn engine_with_no_providers_has_count_zero() {
        let cfg = Config::default();
        let engine = Engine::new(cfg, Secrets::default());
        assert_eq!(engine.provider_count(), 0);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn engine_with_mock_enabled_has_count_one() {
        let cfg = Config {
            providers: Providers {
                mock: ProviderConfig { enabled: true, ..Default::default() },
                ..Default::default()
            },
        };
        let engine = Engine::new(cfg, Secrets::default());
        assert_eq!(engine.provider_count(), 1);
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn engine_refresh_all_against_mock_returns_one_ok_row() {
        let cfg = Config {
            providers: Providers {
                mock: ProviderConfig { enabled: true, ..Default::default() },
                ..Default::default()
            },
        };
        let engine = Engine::new(cfg, Secrets::default());
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let results = engine.refresh_all(now).await;
        assert_eq!(results.len(), 1);
        let (pid, result) = &results[0];
        assert_eq!(*pid, ProviderId::Mock);
        let state = result.as_ref().unwrap();
        assert_eq!(state.id, ProviderId::Mock);
        assert_eq!(state.fetched_at, now);
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn refresh_all_returns_canonical_order_with_mock_only() {
        // BL-02: confirm the engine-layer sort runs even with a single provider.
        // The multi-provider permutation case (where order actually matters) is
        // covered by tests/engine_row_order.rs.
        let cfg = Config {
            providers: Providers {
                mock: ProviderConfig { enabled: true, ..Default::default() },
                ..Default::default()
            },
        };
        let engine = Engine::new(cfg, Secrets::default());
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let results = engine.refresh_all(now).await;
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].0,
            ProviderId::Mock,
            "single mock row appears with canonical position"
        );
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn engine_with_disabled_providers_returns_empty() {
        // All-disabled config: refresh_all returns empty Vec (no error — CFG-04).
        let cfg = Config::default();
        let engine = Engine::new(cfg, Secrets::default());
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let results = engine.refresh_all(now).await;
        assert!(results.is_empty());
    }
}
