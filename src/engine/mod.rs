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

pub mod cache;
pub mod events;
pub mod fanout;

pub use cache::{CacheEntry, RowOutcome};
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
    use crate::model::{HpWindow, NetworkErr, ResetInfo};
    use async_trait::async_trait;
    use std::borrow::Cow;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

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
        let (pid, outcome) = &results[0];
        assert_eq!(*pid, ProviderId::Mock);
        match outcome {
            RowOutcome::Fresh(state) => {
                assert_eq!(state.id, ProviderId::Mock);
                assert_eq!(state.fetched_at, now);
            }
            other => panic!("expected Fresh, got {other:?}"),
        }
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

    // ============================================================================
    // Phase 3 Plan 02 — Cache + TTL + RowOutcome behavioral tests (D-66..D-72).
    //
    // These tests drive the engine via `new_for_test(...)` (a `pub(crate)` helper
    // that bypasses Config-based construction so we can plug in
    // stateful test providers). The cache itself is owned by `Engine` (Q4
    // resolution — internal own, no injection).
    // ============================================================================

    /// Synthetic state for a test provider — deterministic shape so tests can
    /// assert on Fresh/Stale state contents.
    fn synthetic_state(id: ProviderId, now: jiff::Timestamp) -> ProviderState {
        ProviderState {
            id,
            windows: vec![HpWindow {
                label: Cow::Borrowed("test"),
                percent_remaining: 50.0,
                reset: ResetInfo {
                    resets_at: now + jiff::Span::new().hours(1),
                },
                bar_color: None,
                detailed_label: None,
            }],
            fetched_at: now,
            source: Cow::Borrowed("test"),
        }
    }

    /// A test provider that records `fetch` call count and whose return value
    /// can be programmatically scripted from an external `Mutex<VecDeque<...>>`.
    struct ScriptedProvider {
        id: ProviderId,
        calls: AtomicU64,
        script: Mutex<std::collections::VecDeque<Result<ProviderState, ProviderError>>>,
    }

    impl ScriptedProvider {
        fn new(id: ProviderId, script: Vec<Result<ProviderState, ProviderError>>) -> Self {
            Self {
                id,
                calls: AtomicU64::new(0),
                script: Mutex::new(script.into_iter().collect()),
            }
        }
        fn call_count(&self) -> u64 {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Provider for ScriptedProvider {
        fn id(&self) -> ProviderId {
            self.id
        }
        async fn fetch(&self, _ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let next = {
                let mut script = self
                    .script
                    .lock()
                    .map_err(|_| ProviderError::Internal {
                        source: anyhow::anyhow!("script poisoned"),
                    })?;
                script.pop_front()
            };
            match next {
                Some(r) => r,
                None => Err(ProviderError::Internal {
                    source: anyhow::anyhow!("script exhausted"),
                }),
            }
        }
    }

    #[tokio::test]
    async fn engine_caches_successful_fetch() {
        // D-71 row 1: t < last + refresh_interval → reuse cache → Fresh (no
        // tag). The second refresh_all within the TTL window must NOT call
        // the provider's fetch.
        let t0: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let provider = Arc::new(ScriptedProvider::new(
            ProviderId::Mock,
            vec![Ok(synthetic_state(ProviderId::Mock, t0))],
        ));
        let providers: Vec<Arc<dyn Provider>> = vec![provider.clone()];
        let mut refresh_intervals = std::collections::HashMap::new();
        refresh_intervals.insert(ProviderId::Mock, Duration::from_secs(15));
        let engine = Engine::new_for_test(providers, Secrets::default(), refresh_intervals);

        // First call: fetches.
        let results1 = engine.refresh_all(t0).await;
        assert_eq!(results1.len(), 1);
        assert_eq!(provider.call_count(), 1, "first call must fetch");
        match &results1[0].1 {
            RowOutcome::Fresh(_) => {}
            other => panic!("first call should be Fresh, got {other:?}"),
        }

        // Second call within TTL: NO fetch, returns cache as Fresh.
        let t1 = t0 + jiff::Span::new().seconds(5);
        let results2 = engine.refresh_all(t1).await;
        assert_eq!(results2.len(), 1);
        assert_eq!(
            provider.call_count(),
            1,
            "second call within TTL must NOT fetch"
        );
        match &results2[0].1 {
            RowOutcome::Fresh(state) => assert_eq!(state.id, ProviderId::Mock),
            other => panic!("second call should be Fresh from cache, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn engine_returns_stale_on_transient_error_with_cache_hit() {
        // D-71 row 3: t >= last + TTL & fetch transiently fails + cache hit
        // → RowOutcome::Stale with correct stale_age_secs.
        let t0: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let provider = Arc::new(ScriptedProvider::new(
            ProviderId::Mock,
            vec![
                Ok(synthetic_state(ProviderId::Mock, t0)),
                Err(ProviderError::Network {
                    source: NetworkErr("simulated transient".into()),
                }),
            ],
        ));
        let providers: Vec<Arc<dyn Provider>> = vec![provider.clone()];
        let mut refresh_intervals = std::collections::HashMap::new();
        refresh_intervals.insert(ProviderId::Mock, Duration::from_secs(5));
        let engine = Engine::new_for_test(providers, Secrets::default(), refresh_intervals);

        // Prime: successful fetch populates cache at t0.
        let _ = engine.refresh_all(t0).await;
        assert_eq!(provider.call_count(), 1);

        // 30 seconds later: TTL elapsed, fetch returns Network error, cache
        // hit → Stale with stale_age_secs = 30.
        let t1 = t0 + jiff::Span::new().seconds(30);
        let results = engine.refresh_all(t1).await;
        assert_eq!(provider.call_count(), 2, "TTL elapsed must trigger fetch");
        match &results[0].1 {
            RowOutcome::Stale { state, stale_age_secs } => {
                assert_eq!(state.id, ProviderId::Mock);
                assert_eq!(
                    *stale_age_secs, 30,
                    "stale age = (now - cache.fetched_at).total(Second)"
                );
            }
            other => panic!("expected Stale, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn engine_returns_failed_on_non_transient_error_regardless_of_cache() {
        // D-71 + Q2 mapping: non-transient (Unavailable) error must NEVER
        // promote to Stale even when cache has a hit.
        let t0: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let provider = Arc::new(ScriptedProvider::new(
            ProviderId::Mock,
            vec![
                Ok(synthetic_state(ProviderId::Mock, t0)),
                Err(ProviderError::Unavailable {
                    reason: "structurally broken".into(),
                }),
            ],
        ));
        let providers: Vec<Arc<dyn Provider>> = vec![provider.clone()];
        let mut refresh_intervals = std::collections::HashMap::new();
        refresh_intervals.insert(ProviderId::Mock, Duration::from_secs(5));
        let engine = Engine::new_for_test(providers, Secrets::default(), refresh_intervals);

        // Prime: successful fetch populates cache.
        let _ = engine.refresh_all(t0).await;

        // After TTL elapses: Unavailable is non-transient → Failed (NOT Stale).
        let t1 = t0 + jiff::Span::new().seconds(30);
        let results = engine.refresh_all(t1).await;
        match &results[0].1 {
            RowOutcome::Failed(ProviderError::Unavailable { .. }) => {}
            other => panic!(
                "expected Failed(Unavailable), got {other:?} (must NOT promote to Stale per Q2)"
            ),
        }
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn refresh_interval_clamps_to_five_seconds_minimum() {
        // D-72: refresh_interval < 5s must clamp to 5s.
        let cfg = Config {
            providers: Providers {
                mock: ProviderConfig {
                    enabled: true,
                    refresh_interval: Some(2), // below clamp
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        let engine = Engine::new(cfg, Secrets::default());
        let stored = engine
            .refresh_interval_for(ProviderId::Mock)
            .expect("mock interval present");
        assert_eq!(
            stored,
            Duration::from_secs(5),
            "raw value 2 < 5 must clamp to 5s (D-72)"
        );
    }

    #[tokio::test]
    async fn engine_row_order_preserved_with_row_outcome() {
        // BL-02 / Pitfall 16: sort_by_key still applies to the new Vec<(_, RowOutcome)>
        // shape; also covers Pitfall 16 (must emit Fresh per provider even when
        // all are cached — empty Vec would mistake "all cached" for "no providers").
        let t0: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let claude_provider = Arc::new(ScriptedProvider::new(
            ProviderId::Claude,
            vec![
                Ok(synthetic_state(ProviderId::Claude, t0)),
                Ok(synthetic_state(ProviderId::Claude, t0)),
            ],
        ));
        let mock_provider = Arc::new(ScriptedProvider::new(
            ProviderId::Mock,
            vec![
                Ok(synthetic_state(ProviderId::Mock, t0)),
                Ok(synthetic_state(ProviderId::Mock, t0)),
            ],
        ));
        // Intentionally reversed input order — engine must sort to Claude=0, Mock=3.
        let providers: Vec<Arc<dyn Provider>> =
            vec![mock_provider.clone(), claude_provider.clone()];
        let mut refresh_intervals = std::collections::HashMap::new();
        refresh_intervals.insert(ProviderId::Claude, Duration::from_secs(15));
        refresh_intervals.insert(ProviderId::Mock, Duration::from_secs(15));
        let engine = Engine::new_for_test(providers, Secrets::default(), refresh_intervals);

        // First pass: both fetched.
        let r1 = engine.refresh_all(t0).await;
        assert_eq!(r1.len(), 2);
        assert_eq!(r1[0].0, ProviderId::Claude, "Claude first per BL-02");
        assert_eq!(r1[1].0, ProviderId::Mock, "Mock last per BL-02");

        // Second pass within TTL: both cache-hit. Order MUST still be canonical
        // AND we must still get 2 rows (Pitfall 16 — not empty Vec).
        let r2 = engine.refresh_all(t0 + jiff::Span::new().seconds(5)).await;
        assert_eq!(r2.len(), 2, "all-cached pass must emit one row per provider");
        assert_eq!(r2[0].0, ProviderId::Claude);
        assert_eq!(r2[1].0, ProviderId::Mock);
        for (_, outcome) in &r2 {
            assert!(matches!(outcome, RowOutcome::Fresh(_)));
        }
    }
}
