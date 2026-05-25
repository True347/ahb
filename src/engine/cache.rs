//! Engine-layer cache types for the Phase 3 stale-on-error path (D-66..D-71).
//!
//! - [`CacheEntry`] is what `Engine` stores per-provider in its
//!   `moka::sync::Cache<ProviderId, CacheEntry>`. `fetched_at` is sourced from
//!   the inner `ProviderState`'s `fetched_at` field (which itself comes from
//!   `FetchCtx::now` per BL-01) — the cache write site MUST NOT call
//!   `jiff::Timestamp::now()` itself.
//! - [`RowOutcome`] is the engine's per-provider verdict after consulting the
//!   cache: either fresh data, stale data with an age, or a hard failure.
//!   `Engine::refresh_all` returns `Vec<(ProviderId, RowOutcome)>` (Plan 03-02
//!   Q5 resolution) so the TUI translator (Plan 03-03) and the CLI dispatch
//!   can both consume a single shape.
//! - [`is_transient`] is the closed-set predicate that decides whether a
//!   `ProviderError` is eligible to fall back to a cached `ProviderState`. Only
//!   `Network` and `RateLimited` qualify (03-RESEARCH Q2). `Unavailable`
//!   covers both the Gemini stub (permanent) and adapter timeouts
//!   (structural) and is explicitly NOT transient — widening this predicate
//!   would silently mask real failures.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use crate::model::{ProviderError, ProviderState};

/// One cache entry per provider: the last successful `ProviderState` plus the
/// `fetched_at` instant it was observed at. `Clone` is required because
/// `moka::sync::Cache::get` returns a clone of the value (not a reference) —
/// `ProviderState` already derives `Clone` (`src/model.rs:76`) so the derive
/// here propagates cleanly.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub state: ProviderState,
    pub fetched_at: jiff::Timestamp,
}

/// What `Engine::refresh_all` emits per provider after consulting the cache.
///
/// - `Fresh(state)` — either a successful fetch, or a TTL-not-elapsed cache
///   hit. The TUI renders this as the normal `RowState::Ok` path.
/// - `Stale { state, stale_age_secs }` — the latest fetch returned a
///   transient error AND the cache had a previous good entry. The TUI
///   renders this as `RowState::StaleOk` (yellow row + `(stale Ns ago)`
///   suffix per D-69). CLI dispatch (D-66 / D-73) treats this as
///   unreachable because CLI never builds up a cache.
/// - `Failed(err)` — the fetch failed AND we have no usable cached state
///   (either non-transient error, or no cache hit). Renders the existing
///   Phase 1/2 ERROR row path.
#[derive(Debug)]
pub enum RowOutcome {
    /// Provider data is fresh (either from a successful fetch or within TTL).
    Fresh(ProviderState),
    /// Last fetch failed transiently; serving cached data with stale age in seconds.
    Stale {
        state: ProviderState,
        stale_age_secs: u64,
    },
    /// Adapter returned a non-transient error (or no cache available for transient).
    Failed(ProviderError),
}

/// Returns `true` only for `Network` and `RateLimited` errors — the two variants
/// that represent recoverable transient conditions. `Unavailable` covers both
/// the Gemini stub (permanent) and adapter timeouts (structural) and does NOT
/// trigger stale fallback (see 03-RESEARCH.md Q2 + Pitfall 6).
///
/// Restricted to `pub(crate)` because the predicate is meaningful only to the
/// engine layer — external consumers should not branch on transience.
pub(crate) fn is_transient(err: &ProviderError) -> bool {
    matches!(
        err,
        ProviderError::Network { .. } | ProviderError::RateLimited { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NetworkErr;

    #[test]
    fn is_transient_network_is_true() {
        assert!(is_transient(&ProviderError::Network {
            source: NetworkErr("x".into()),
        }));
    }

    #[test]
    fn is_transient_rate_limited_is_true() {
        assert!(is_transient(&ProviderError::RateLimited {
            retry_after: None
        }));
    }

    #[test]
    fn is_transient_unavailable_is_false() {
        assert!(!is_transient(&ProviderError::Unavailable {
            reason: "x".into()
        }));
    }

    #[test]
    fn is_transient_schema_drift_is_false() {
        assert!(!is_transient(&ProviderError::SchemaDrift {
            missing: vec!["x".into()]
        }));
    }

    #[test]
    fn is_transient_internal_is_false() {
        assert!(!is_transient(&ProviderError::Internal {
            source: anyhow::anyhow!("x"),
        }));
    }

    #[test]
    fn is_transient_unconfigured_is_false() {
        assert!(!is_transient(&ProviderError::Unconfigured));
    }
}
