//! Phase 3 Plan 03-05 integration tests: cache + stale-on-error end-to-end at the
//! engine layer.
//!
//! These tests drive `Engine::refresh_all` via the `#[cfg(test)]` `pub`
//! constructor `Engine::new_with_providers(providers, cfg, secrets)` (added by
//! this plan in src/engine/mod.rs) so they can plug in a stateful
//! `ScriptedProvider` that returns a programmed sequence of Ok/Err results.
//!
//! Coverage:
//! 1. D-71 three-state timeline: Fresh → Stale (transient + cache hit) → Fresh
//!    (cache updated).
//! 2. D-71 + Q2: non-transient error (Unavailable) NEVER promotes to Stale even
//!    with cache hit.
//! 3. No-cache + transient error → RowOutcome::Failed (Stale requires prior
//!    success).
//! 4. Cache hit within TTL → second call returns Fresh from cache WITHOUT
//!    calling provider.fetch (TTL gating works).
//! 5. SC-3 multi-provider polling cadence: Mock (TTL=5s) re-fetched at t+6s;
//!    Codex (TTL=600s) served from cache.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::default_constructed_unit_structs)] // Secrets::default() per test convention

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;

use ai_hp_bar::config::{Config, ProviderConfig, Providers};
use ai_hp_bar::engine::Engine;
use ai_hp_bar::engine::cache::RowOutcome;
use ai_hp_bar::model::{HpWindow, NetworkErr, ProviderError, ProviderId, ProviderState, ResetInfo};
use ai_hp_bar::provider::{FetchCtx, Provider};
use ai_hp_bar::secrets::Secrets;

/// Build a synthetic ProviderState whose `fetched_at` matches the injected `now`
/// (BL-01 — adapter clocks come from ctx.now, never wall-clock).
fn synthetic_state(id: ProviderId, now: jiff::Timestamp) -> ProviderState {
    ProviderState {
        id,
        windows: vec![HpWindow {
            label: Cow::Borrowed("scripted"),
            percent_remaining: 50.0,
            reset: ResetInfo {
                resets_at: now + jiff::Span::new().hours(1),
            },
            bar_color: None,
            detailed_label: None,
        }],
        fetched_at: now,
        source: Cow::Borrowed("scripted"),
    }
}

/// Test provider that returns each scripted Result in order, recording the
/// call count via `AtomicU64`.
///
/// Each `fetch` consults `ctx.now` to construct a fresh `ProviderState` (so the
/// cache's `fetched_at` matches the test's controlled `now`). Errors come from
/// the script verbatim; if the script entry is `Ok(_)`, we discard the
/// timestamp inside it and rebuild against `ctx.now` for clean controllability.
struct ScriptedProvider {
    id: ProviderId,
    calls: AtomicU64,
    script: Mutex<std::collections::VecDeque<Result<(), ProviderError>>>,
}

impl ScriptedProvider {
    /// Construct from a `Vec<Result<(), ProviderError>>` — `Ok(())` means
    /// "succeed: build state from ctx.now"; `Err(e)` means "return this error".
    fn new(id: ProviderId, script: Vec<Result<(), ProviderError>>) -> Self {
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
    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let next = {
            let mut script = self.script.lock().map_err(|_| ProviderError::Internal {
                source: anyhow::anyhow!("script mutex poisoned"),
            })?;
            script.pop_front()
        };
        match next {
            Some(Ok(())) => Ok(synthetic_state(self.id, ctx.now)),
            Some(Err(e)) => Err(e),
            None => Err(ProviderError::Internal {
                source: anyhow::anyhow!("script exhausted"),
            }),
        }
    }
}

// ----------------------------------------------------------------------------
// Test 1: D-71 three-state timeline — Fresh → Stale → Fresh.
// ----------------------------------------------------------------------------

#[tokio::test]
async fn engine_fresh_stale_fresh_three_tick_sequence() {
    // ScriptedProvider drives a 3-tick sequence:
    //   Tick 1 (t0):       Ok      → RowOutcome::Fresh
    //   Tick 2 (t0 + 6s):  Network → RowOutcome::Stale (cache hit from tick 1)
    //   Tick 3 (t0 + 12s): Ok      → RowOutcome::Fresh (cache updated)
    //
    // TTL = 5s so each tick crosses the boundary cleanly.
    let t0: jiff::Timestamp = "2026-05-25T12:00:00Z"
        .parse()
        .map_err(|e: jiff::Error| ProviderError::Internal {
            source: anyhow::anyhow!("parse t0: {e}"),
        })
        .unwrap_or_else(|_| jiff::Timestamp::UNIX_EPOCH);
    let t1 = t0 + jiff::Span::new().seconds(6);
    let t2 = t0 + jiff::Span::new().seconds(12);

    let provider = Arc::new(ScriptedProvider::new(
        ProviderId::Mock,
        vec![
            Ok(()),
            Err(ProviderError::Network {
                source: NetworkErr("simulated transient".into()),
            }),
            Ok(()),
        ],
    ));
    let providers: Vec<Arc<dyn Provider>> = vec![provider.clone()];

    let cfg = Config {
        providers: Providers {
            mock: ProviderConfig {
                enabled: true,
                refresh_interval: Some(5),
            },
            ..Default::default()
        },
    };
    let engine = Engine::new_with_providers(providers, cfg, Secrets::default());

    // Tick 1: Fresh.
    let r1 = engine.refresh_all(t0).await;
    assert_eq!(r1.len(), 1, "tick 1 emits one row");
    assert_eq!(provider.call_count(), 1, "tick 1 must fetch");
    match &r1[0].1 {
        RowOutcome::Fresh(state) => assert_eq!(state.id, ProviderId::Mock),
        other => unreachable!("tick 1 expected Fresh, got {other:?}"),
    }

    // Tick 2: TTL elapsed (6s > 5s), fetch returns Network, cache hit from
    // tick 1 → Stale with stale_age_secs in [5, 8] (allowing for any sub-second
    // math).
    let r2 = engine.refresh_all(t1).await;
    assert_eq!(provider.call_count(), 2, "tick 2 must fetch (TTL elapsed)");
    match &r2[0].1 {
        RowOutcome::Stale {
            state,
            stale_age_secs,
        } => {
            assert_eq!(state.id, ProviderId::Mock);
            assert!(
                (5..=8).contains(stale_age_secs),
                "stale_age_secs = (t1 - t0) = 6s, allowed range [5,8]; got {stale_age_secs}"
            );
        }
        other => unreachable!("tick 2 expected Stale, got {other:?}"),
    }

    // Tick 3: TTL elapsed again, fetch succeeds → Fresh; cache updated to t2.
    let r3 = engine.refresh_all(t2).await;
    assert_eq!(provider.call_count(), 3, "tick 3 must fetch");
    match &r3[0].1 {
        RowOutcome::Fresh(state) => {
            assert_eq!(state.id, ProviderId::Mock);
            assert_eq!(
                state.fetched_at, t2,
                "tick 3 state.fetched_at must come from ctx.now (BL-01)"
            );
        }
        other => unreachable!("tick 3 expected Fresh (cache updated), got {other:?}"),
    }
}

// ----------------------------------------------------------------------------
// Test 2: Non-transient error never promotes to Stale (D-71 + Q2 binding).
// ----------------------------------------------------------------------------

#[tokio::test]
async fn engine_non_transient_error_does_not_stale_despite_cache() {
    // Tick 1: Ok → cache populated.
    // Tick 2 (TTL elapsed): Err(SchemaDrift) — non-transient. Even with the
    // cache populated from tick 1, the engine MUST emit Failed (NOT Stale),
    // because non-transient errors invalidate the stale fallback.
    let t0: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    let t1 = t0 + jiff::Span::new().seconds(30);

    let provider = Arc::new(ScriptedProvider::new(
        ProviderId::Mock,
        vec![
            Ok(()),
            Err(ProviderError::SchemaDrift {
                missing: vec!["foo".into()],
            }),
        ],
    ));
    let providers: Vec<Arc<dyn Provider>> = vec![provider.clone()];

    let cfg = Config {
        providers: Providers {
            mock: ProviderConfig {
                enabled: true,
                refresh_interval: Some(5),
            },
            ..Default::default()
        },
    };
    let engine = Engine::new_with_providers(providers, cfg, Secrets::default());

    // Prime cache.
    let _ = engine.refresh_all(t0).await;
    assert_eq!(provider.call_count(), 1);

    // SchemaDrift after TTL: must be Failed, NOT Stale.
    let r2 = engine.refresh_all(t1).await;
    match &r2[0].1 {
        RowOutcome::Failed(ProviderError::SchemaDrift { .. }) => {}
        other => unreachable!(
            "non-transient SchemaDrift must NOT promote to Stale (Q2 binding); got {other:?}"
        ),
    }
}

// ----------------------------------------------------------------------------
// Test 3: No prior cache + transient error → Failed.
// ----------------------------------------------------------------------------

#[tokio::test]
async fn engine_no_cache_with_transient_error_produces_failed() {
    // First-ever call returns Network. Cache is empty → no stale fallback
    // available → engine must emit Failed (not Stale).
    let t0: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap_or(jiff::Timestamp::UNIX_EPOCH);

    let provider = Arc::new(ScriptedProvider::new(
        ProviderId::Mock,
        vec![Err(ProviderError::Network {
            source: NetworkErr("first call fails".into()),
        })],
    ));
    let providers: Vec<Arc<dyn Provider>> = vec![provider.clone()];

    let cfg = Config {
        providers: Providers {
            mock: ProviderConfig {
                enabled: true,
                refresh_interval: Some(5),
            },
            ..Default::default()
        },
    };
    let engine = Engine::new_with_providers(providers, cfg, Secrets::default());

    let r1 = engine.refresh_all(t0).await;
    assert_eq!(provider.call_count(), 1);
    match &r1[0].1 {
        RowOutcome::Failed(ProviderError::Network { .. }) => {}
        other => unreachable!(
            "first-call Network with no cache must be Failed (not Stale); got {other:?}"
        ),
    }
}

// ----------------------------------------------------------------------------
// Test 4: Cache hit within TTL → second call returns Fresh from cache (no fetch).
// ----------------------------------------------------------------------------

#[tokio::test]
async fn engine_cache_hit_within_ttl_returns_fresh_without_fetch() {
    // script = [Ok(()), Err(Network)]. Both calls at t0 (TTL not elapsed): the
    // second call MUST be served from cache; the second script entry MUST NOT
    // be consumed. Final call_count == 1 proves the TTL gate skipped fanout.
    let t0: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap_or(jiff::Timestamp::UNIX_EPOCH);

    let provider = Arc::new(ScriptedProvider::new(
        ProviderId::Mock,
        vec![
            Ok(()),
            Err(ProviderError::Network {
                source: NetworkErr("must NOT be reached".into()),
            }),
        ],
    ));
    let providers: Vec<Arc<dyn Provider>> = vec![provider.clone()];

    let cfg = Config {
        providers: Providers {
            mock: ProviderConfig {
                enabled: true,
                refresh_interval: Some(5),
            },
            ..Default::default()
        },
    };
    let engine = Engine::new_with_providers(providers, cfg, Secrets::default());

    let r1 = engine.refresh_all(t0).await;
    assert_eq!(provider.call_count(), 1);
    match &r1[0].1 {
        RowOutcome::Fresh(_) => {}
        other => unreachable!("first call expected Fresh, got {other:?}"),
    }

    // Same `now` — TTL not elapsed; cache hit must serve Fresh.
    let r2 = engine.refresh_all(t0).await;
    assert_eq!(
        provider.call_count(),
        1,
        "second call within TTL must NOT fetch (cache hit)"
    );
    match &r2[0].1 {
        RowOutcome::Fresh(state) => assert_eq!(state.id, ProviderId::Mock),
        other => unreachable!("second call within TTL expected Fresh-from-cache, got {other:?}"),
    }
}

// ----------------------------------------------------------------------------
// Test 5: SC-3 multi-provider polling cadence — different TTLs produce
// different fetch counts.
// ----------------------------------------------------------------------------

#[tokio::test]
async fn engine_multi_provider_different_intervals_only_stale_provider_fetched() {
    // provider_a = Mock with TTL=5s
    // provider_b = Codex with TTL=600s
    //
    // Call 1 at t0: no cache yet → both fetched (call_count_a=1, call_count_b=1).
    // Call 2 at t0 + 6s: Mock TTL (5s) elapsed → Mock re-fetched (call_count_a=2);
    //                    Codex TTL (600s) NOT elapsed → Codex served from cache
    //                    (call_count_b=1). Both outcomes Fresh.
    //
    // The Mock provider uses ProviderId::Mock as `provider_a`; the Codex
    // provider is built from a ScriptedProvider with id ProviderId::Codex.
    // Engine sorts results by canonical row order: Codex=1, Mock=3.
    let t0: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap_or(jiff::Timestamp::UNIX_EPOCH);
    let t1 = t0 + jiff::Span::new().seconds(6);

    let provider_a = Arc::new(ScriptedProvider::new(
        ProviderId::Mock,
        vec![Ok(()), Ok(()), Ok(())],
    ));
    let provider_b = Arc::new(ScriptedProvider::new(
        ProviderId::Codex,
        vec![Ok(()), Ok(()), Ok(())],
    ));
    let providers: Vec<Arc<dyn Provider>> = vec![provider_a.clone(), provider_b.clone()];

    let cfg = Config {
        providers: Providers {
            mock: ProviderConfig {
                enabled: true,
                refresh_interval: Some(5),
            },
            codex: ProviderConfig {
                enabled: true,
                refresh_interval: Some(600),
            },
            ..Default::default()
        },
    };

    // For new_with_providers we pass our scripted providers (not the
    // Config-driven Claude/Codex builders), but Config's per-provider
    // refresh_interval values are still consumed to populate
    // refresh_intervals correctly via the standard `resolve_interval`
    // path. This matches the documented test affordance.
    let engine = Engine::new_with_providers(providers, cfg, Secrets::default());

    // Call 1 at t0: both fetched (no prior cache).
    let r1 = engine.refresh_all(t0).await;
    assert_eq!(r1.len(), 2, "two providers enabled, two rows");
    assert_eq!(provider_a.call_count(), 1, "Mock fetched on first call");
    assert_eq!(provider_b.call_count(), 1, "Codex fetched on first call");
    let mut by_id: HashMap<ProviderId, &RowOutcome> = HashMap::new();
    for (pid, outcome) in &r1 {
        by_id.insert(*pid, outcome);
    }
    assert!(
        matches!(by_id.get(&ProviderId::Mock), Some(RowOutcome::Fresh(_))),
        "Mock first call Fresh"
    );
    assert!(
        matches!(by_id.get(&ProviderId::Codex), Some(RowOutcome::Fresh(_))),
        "Codex first call Fresh"
    );

    // Call 2 at t0 + 6s: Mock TTL (5s) elapsed → re-fetched.
    //                    Codex TTL (600s) NOT elapsed → served from cache.
    let r2 = engine.refresh_all(t1).await;
    assert_eq!(r2.len(), 2, "two rows on second call as well");
    assert_eq!(
        provider_a.call_count(),
        2,
        "SC-3: Mock with TTL=5s must be re-fetched at t+6s"
    );
    assert_eq!(
        provider_b.call_count(),
        1,
        "SC-3: Codex with TTL=600s must NOT be re-fetched at t+6s (served from cache)"
    );
    let mut by_id2: HashMap<ProviderId, &RowOutcome> = HashMap::new();
    for (pid, outcome) in &r2 {
        by_id2.insert(*pid, outcome);
    }
    assert!(
        matches!(by_id2.get(&ProviderId::Mock), Some(RowOutcome::Fresh(_))),
        "Mock second-call Fresh (re-fetched OK)"
    );
    assert!(
        matches!(by_id2.get(&ProviderId::Codex), Some(RowOutcome::Fresh(_))),
        "Codex second-call Fresh (from cache)"
    );

    // BL-02 / Pitfall 16: even on the second call (where Codex is served from
    // cache and Mock is re-fetched), the result Vec is still canonical-order:
    // Codex=1, Mock=3.
    assert_eq!(r2[0].0, ProviderId::Codex, "BL-02 canonical order: Codex before Mock");
    assert_eq!(r2[1].0, ProviderId::Mock);

    // Silence the unused-import warning when this constant is unused above.
    let _ = Duration::from_secs(0);
}
