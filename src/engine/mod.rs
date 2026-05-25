//! Engine — the orchestration layer between config + adapters + front-ends.
//!
//! Phase 1 layout: `Engine::new(config, secrets)` builds the provider list from
//! `config.providers.*.enabled` flags; `refresh_all(now)` fans the fetch out via
//! `fanout::refresh_all_inner` (`JoinSet` + per-adapter timeout + Pitfall L4 panic recovery).
//!
//! Phase 3 Plan 02 adds the stale-on-error cache + per-provider TTL gating
//! (D-66..D-72):
//! - Engine owns a `moka::sync::Cache<ProviderId, CacheEntry>` (Q4 internal-own;
//!   no injection point).
//! - Engine owns `refresh_intervals: HashMap<ProviderId, Duration>` populated
//!   from `cfg.providers.<id>.refresh_interval` (Plan 01) with clamp ≥5s (D-72)
//!   and per-provider `DEFAULT_REFRESH_INTERVAL_SECS` fallback.
//! - `Engine::refresh_all` now returns `Vec<(ProviderId, RowOutcome)>` (Q5):
//!   - Pre-filters providers within TTL → emits `Fresh` straight from cache
//!     (Option A — skip fan-out per Q3).
//!   - Calls `fanout::refresh_all_inner` only on the elapsed-TTL subset.
//!   - Per fanout result: `Ok` → update cache + `Fresh`; transient `Err` with
//!     cache hit → `Stale { state, stale_age_secs }`; non-transient or
//!     no-cache-hit → `Failed(err)`.
//!
//! Invariants:
//! - `fanout::refresh_all_inner` stays a pure fan-out (Q3 architectural fit).
//! - No `Timestamp::now()` callsite added — `now` flows in via the parameter
//!   (BL-01 / Q8).
//! - moka has no `time_to_live` / `time_to_idle` — eviction is purely manual
//!   (D-66 / Pitfall 1).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use moka::sync::Cache;

pub mod cache;
pub mod events;
pub mod fanout;

pub use cache::{CacheEntry, RowOutcome};
use cache::is_transient;
pub use events::{EngineEvent, EVENT_BUFFER};
pub use fanout::DEFAULT_PER_PROVIDER_TIMEOUT;

use crate::config::{Config, ProviderConfig};
use crate::model::ProviderId;
use crate::provider::{claude, codex, gemini, mock};
use crate::provider::Provider;
use crate::secrets::Secrets;

/// Safety floor for `refresh_interval` (D-72). Anything below this clamps up
/// to 5s with `tracing::warn!`. Rationale: avoid local-FS hammering and avoid
/// tripping any future-Gemini ToS heuristic on tight loops.
const REFRESH_INTERVAL_MIN_SECS: u64 = 5;

/// AHB engine. Holds the list of enabled providers + shared secrets handle. Front-ends
/// (CLI compact line, TUI) consume it via `refresh_all(now)`.
pub struct Engine {
    providers: Vec<Arc<dyn Provider>>,
    secrets: Arc<Secrets>,
    per_provider_timeout: Duration,
    /// Per-provider stale-on-error cache (D-66 / D-67). Pure in-memory; no TTL /
    /// TTI configured — stale semantics are manual (D-71 / Pitfall 1). Cloning
    /// is cheap (Arc-internal); ownership stays here.
    cache: Cache<ProviderId, CacheEntry>,
    /// Per-provider refresh interval. Built in `Engine::new` from
    /// `cfg.providers.<id>.refresh_interval` with the per-provider
    /// `DEFAULT_REFRESH_INTERVAL_SECS` fallback and the ≥5s clamp (D-72).
    refresh_intervals: HashMap<ProviderId, Duration>,
}

impl Engine {
    /// Build an engine from the parsed config. Pushes one `Arc<dyn Provider>`
    /// per `cfg.providers.<id>.enabled` flag and stores the resolved
    /// per-provider refresh interval in `refresh_intervals` (with clamp +
    /// `tracing::warn!` when the config value is below the 5s floor).
    #[must_use]
    #[allow(clippy::needless_pass_by_value)] // builder-style API: takes ownership of config + secrets
    pub fn new(cfg: Config, secrets: Secrets) -> Self {
        let mut providers: Vec<Arc<dyn Provider>> = Vec::new();
        let mut refresh_intervals: HashMap<ProviderId, Duration> = HashMap::new();

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
            refresh_intervals.insert(
                ProviderId::Claude,
                Self::resolve_interval(
                    "claude",
                    &cfg.providers.claude,
                    claude::DEFAULT_REFRESH_INTERVAL_SECS,
                ),
            );
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
            refresh_intervals.insert(
                ProviderId::Codex,
                Self::resolve_interval(
                    "codex",
                    &cfg.providers.codex,
                    codex::DEFAULT_REFRESH_INTERVAL_SECS,
                ),
            );
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
            refresh_intervals.insert(
                ProviderId::Gemini,
                Self::resolve_interval(
                    "gemini",
                    &cfg.providers.gemini,
                    gemini::DEFAULT_REFRESH_INTERVAL_SECS,
                ),
            );
        }
        if cfg.providers.mock.enabled {
            providers.push(Arc::new(crate::provider::mock::MockProvider));
            refresh_intervals.insert(
                ProviderId::Mock,
                Self::resolve_interval(
                    "mock",
                    &cfg.providers.mock,
                    mock::DEFAULT_REFRESH_INTERVAL_SECS,
                ),
            );
        }

        Self {
            providers,
            secrets: Arc::new(secrets),
            per_provider_timeout: DEFAULT_PER_PROVIDER_TIMEOUT,
            // D-66 + Pitfall 1: no time_to_live / time_to_idle. Eviction is
            // manual (cache.insert overwrites on success); capacity 8 comfortably
            // exceeds the closed-set ProviderId enum (4 variants today).
            cache: Cache::builder().max_capacity(8).build(),
            refresh_intervals,
        }
    }

    /// Resolve a single provider's refresh interval: `None` → per-provider
    /// default; `Some(raw)` < 5s → clamp to 5s + `tracing::warn!`; otherwise
    /// pass through as `Duration::from_secs(raw)`.
    fn resolve_interval(id_str: &str, pc: &ProviderConfig, default_secs: u64) -> Duration {
        let Some(raw) = pc.refresh_interval else {
            return Duration::from_secs(default_secs);
        };
        if raw < REFRESH_INTERVAL_MIN_SECS {
            tracing::warn!(
                provider = id_str,
                raw = raw,
                "refresh_interval clamped to 5s"
            );
            Duration::from_secs(REFRESH_INTERVAL_MIN_SECS)
        } else {
            Duration::from_secs(raw)
        }
    }

    /// Number of enabled providers (useful for front-end empty-state detection).
    #[must_use]
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Fan out one fetch across all enabled providers. Returns
    /// `Vec<(ProviderId, RowOutcome)>` in canonical `ProviderId` order
    /// (Claude=0, Codex=1, Gemini=2, Mock=3). For each provider:
    /// - TTL not elapsed (`now < last_fetch_at + refresh_interval`) → emit
    ///   `RowOutcome::Fresh(cached_state)` straight from cache (Option A —
    ///   skip fan-out per Q3).
    /// - TTL elapsed → call `fanout::refresh_all_inner` on the elapsed subset.
    ///   For each fanout result:
    ///   - `Ok(state)` → update cache + emit `Fresh(state)`.
    ///   - `Err(e) if is_transient(&e)` + cache hit → emit
    ///     `Stale { state, stale_age_secs }`.
    ///   - `Err(e)` non-transient or no cache hit → emit `Failed(e)`.
    ///
    /// Order: results sorted by `Self::sort_key` so CLI/TUI consumers receive
    /// canonical row order regardless of fanout arrival order (BL-02).
    pub async fn refresh_all(&self, now: jiff::Timestamp) -> Vec<(ProviderId, RowOutcome)> {
        // Partition providers: TTL hit → emit Fresh from cache; TTL elapsed
        // (or no cache yet) → must fetch. Q3 Option A pre-filter.
        let mut from_cache: Vec<(ProviderId, RowOutcome)> = Vec::new();
        let mut needs_fetch: Vec<Arc<dyn Provider>> = Vec::new();

        for p in &self.providers {
            let id = p.id();
            let interval = self
                .refresh_intervals
                .get(&id)
                .copied()
                .unwrap_or(Duration::from_secs(15));
            // Compute "is TTL still in window?" via the cache entry's fetched_at
            // + interval vs now. If no entry, we must fetch.
            let mut hit_fresh = false;
            if let Some(entry) = self.cache.get(&id) {
                let elapsed = duration_since(now, entry.fetched_at);
                if elapsed < interval {
                    from_cache.push((id, RowOutcome::Fresh(entry.state.clone())));
                    hit_fresh = true;
                }
            }
            if !hit_fresh {
                needs_fetch.push(Arc::clone(p));
            }
        }

        // Call fanout only on providers whose TTL has elapsed (or have no cache
        // entry yet). When `needs_fetch` is empty, skip the await entirely —
        // emit only the cached `Fresh` rows. Pitfall 16: even when all
        // providers are within TTL, we must still emit one row per provider
        // (NOT an empty Vec). The `from_cache` accumulator does that.
        let fanout_results = if needs_fetch.is_empty() {
            Vec::new()
        } else {
            fanout::refresh_all_inner(
                &needs_fetch,
                now,
                Arc::clone(&self.secrets),
                self.per_provider_timeout,
            )
            .await
        };

        // Map fanout results into RowOutcome + write cache on success.
        let mut from_fetch: Vec<(ProviderId, RowOutcome)> = Vec::with_capacity(fanout_results.len());
        for (id, result) in fanout_results {
            let outcome = match result {
                Ok(state) => {
                    // Q8 / BL-01: cache fetched_at MUST come from state.fetched_at
                    // (which itself comes from FetchCtx::now), NOT a fresh
                    // jiff::Timestamp::now() call.
                    self.cache.insert(
                        id,
                        CacheEntry {
                            state: state.clone(),
                            fetched_at: state.fetched_at,
                        },
                    );
                    RowOutcome::Fresh(state)
                }
                Err(e) => {
                    if is_transient(&e) {
                        if let Some(entry) = self.cache.get(&id) {
                            let stale_age_secs = duration_since(now, entry.fetched_at).as_secs();
                            RowOutcome::Stale {
                                state: entry.state.clone(),
                                stale_age_secs,
                            }
                        } else {
                            // Transient but no cache to fall back on → Failed.
                            RowOutcome::Failed(e)
                        }
                    } else {
                        RowOutcome::Failed(e)
                    }
                }
            };
            from_fetch.push((id, outcome));
        }

        // Combine cache-hit rows + fetch-result rows, then sort by canonical
        // ProviderId order (BL-02).
        let mut combined: Vec<(ProviderId, RowOutcome)> =
            Vec::with_capacity(from_cache.len() + from_fetch.len());
        combined.extend(from_cache);
        combined.extend(from_fetch);
        combined.sort_by_key(|(id, _)| Self::sort_key(*id));
        combined
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

    /// Test affordance: returns the stored refresh interval for the given
    /// provider id, or `None` if the provider was not enabled in the config
    /// passed to `Engine::new`. Used by the D-72 clamp test to assert that the
    /// stored value reflects the safety floor after parse.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn refresh_interval_for(&self, id: ProviderId) -> Option<Duration> {
        self.refresh_intervals.get(&id).copied()
    }

    /// Test affordance: build an `Engine` directly from a provider list +
    /// per-provider refresh intervals, bypassing `Config`-based construction.
    /// Used by the in-file behavioral tests so they can plug in stateful
    /// providers (`ScriptedProvider`) without going through the config
    /// machinery.
    ///
    /// Production code MUST use `Engine::new(cfg, secrets)` — this constructor
    /// is `pub(crate)` and `#[cfg(test)]`-only by intent.
    #[cfg(test)]
    pub(crate) fn new_for_test(
        providers: Vec<Arc<dyn Provider>>,
        secrets: Secrets,
        refresh_intervals: HashMap<ProviderId, Duration>,
    ) -> Self {
        Self {
            providers,
            secrets: Arc::new(secrets),
            per_provider_timeout: DEFAULT_PER_PROVIDER_TIMEOUT,
            cache: Cache::builder().max_capacity(8).build(),
            refresh_intervals,
        }
    }

    /// Integration-test affordance: build an `Engine` from an explicit provider
    /// list + the user's `Config`, bypassing Config-based provider construction
    /// while still honoring the per-provider `refresh_interval` (with ≥5s clamp
    /// per D-72) from the config block matching each provider's id.
    ///
    /// This constructor is `pub` so integration tests under `tests/` can plug
    /// in stateful test providers (e.g., `ScriptedProvider`) without going
    /// through the full Config-driven provider construction path. `#[doc(hidden)]`
    /// hides it from the public API surface — production code MUST use
    /// `Engine::new(cfg, secrets)`. The original plan called for `#[cfg(test)]`
    /// here but that would gate this constructor out of integration test crates
    /// (Rust's `cfg(test)` only flips on for the crate currently being built as
    /// a `--test` target — when `cargo test` builds the lib for an integration
    /// test target it does NOT pass `--cfg test` to rustc). `#[doc(hidden)]` +
    /// `pub` is the canonical Rust idiom for "test helper that must cross the
    /// crate boundary but is not part of the public contract."
    ///
    /// For each provider in `providers`, the matching `cfg.providers.<id>`
    /// block is consulted: if `refresh_interval` is `Some(n)`, the standard
    /// `resolve_interval` helper applies the ≥5s clamp + `tracing::warn!`;
    /// otherwise the per-provider `DEFAULT_REFRESH_INTERVAL_SECS` is used.
    #[doc(hidden)]
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new_with_providers(
        providers: Vec<Arc<dyn Provider>>,
        cfg: Config,
        secrets: Secrets,
    ) -> Self {
        let mut refresh_intervals: HashMap<ProviderId, Duration> = HashMap::new();
        for p in &providers {
            let id = p.id();
            let (id_str, pc, default_secs) = match id {
                ProviderId::Claude => (
                    "claude",
                    &cfg.providers.claude,
                    claude::DEFAULT_REFRESH_INTERVAL_SECS,
                ),
                ProviderId::Codex => (
                    "codex",
                    &cfg.providers.codex,
                    codex::DEFAULT_REFRESH_INTERVAL_SECS,
                ),
                ProviderId::Gemini => (
                    "gemini",
                    &cfg.providers.gemini,
                    gemini::DEFAULT_REFRESH_INTERVAL_SECS,
                ),
                ProviderId::Mock => (
                    "mock",
                    &cfg.providers.mock,
                    mock::DEFAULT_REFRESH_INTERVAL_SECS,
                ),
            };
            refresh_intervals.insert(id, Self::resolve_interval(id_str, pc, default_secs));
        }
        Self {
            providers,
            secrets: Arc::new(secrets),
            per_provider_timeout: DEFAULT_PER_PROVIDER_TIMEOUT,
            cache: Cache::builder().max_capacity(8).build(),
            refresh_intervals,
        }
    }
}

/// Compute `now - earlier` as a non-negative `Duration`. Negative spans (clock
/// going backwards or earlier-after-now) clamp to zero. Pattern matches the
/// `format_countdown` style in `src/cli/render_text.rs`.
fn duration_since(now: jiff::Timestamp, earlier: jiff::Timestamp) -> Duration {
    let secs: u64 = now
        .since((jiff::Unit::Second, earlier))
        .ok()
        .map(|span| span.get_seconds())
        .and_then(|s| u64::try_from(s.max(0)).ok())
        .unwrap_or(0);
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ProviderConfig, Providers};
    use crate::model::{HpWindow, NetworkErr, ProviderError, ProviderState, ResetInfo};
    use crate::provider::FetchCtx;
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
