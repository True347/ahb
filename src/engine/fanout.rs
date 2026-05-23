//! Engine fan-out: spawn each adapter into a `JoinSet` with a per-task timeout,
//! drain via `join_next`, and convert task panics into `ProviderError::Internal`
//! (Pitfall L4: `HashMap<task::Id, ProviderId>` recovers the `ProviderId` when
//! `JoinError::is_panic()` fires).
//!
//! D-28 + D-29 + ADP-01 binding. Phase 1 default timeout is 2 s (pure local IO);
//! Phase 3 CFG-03 will allow per-provider override.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::model::{ProviderError, ProviderId, ProviderState};
use crate::provider::{FetchCtx, Provider};
use crate::secrets::Secrets;

/// Phase 1 default per-provider timeout. Claude reads local JSONL, 2 s is generous.
/// Phase 3 CFG-03 will allow per-provider override (HTTP adapters need more).
pub const DEFAULT_PER_PROVIDER_TIMEOUT: Duration = Duration::from_secs(2);

/// Fan out a fetch across `providers`, wrap each in `tokio::time::timeout`, and
/// return a `Vec<(ProviderId, Result<...>)>` whose order matches `join_next` arrival
/// (NOT input order — Phase 0's contract is unordered, callers must look up by id).
///
/// Pitfall L4 fix: maintains `HashMap<tokio::task::Id, ProviderId>` so a
/// `JoinError::is_panic()` becomes `(pid, Err(ProviderError::Internal))` instead of
/// a lost task. Adapter panics no longer disappear silently.
pub async fn refresh_all_inner(
    providers: &[Arc<dyn Provider>],
    now: jiff::Timestamp,
    secrets: Arc<Secrets>,
    per_provider_timeout: Duration,
) -> Vec<(ProviderId, Result<ProviderState, ProviderError>)> {
    let mut set: JoinSet<(ProviderId, Result<ProviderState, ProviderError>)> = JoinSet::new();
    let mut id_map: HashMap<tokio::task::Id, ProviderId> = HashMap::new();

    for p in providers {
        let provider = Arc::clone(p);
        let pid = provider.id();
        let secrets_arc = Arc::clone(&secrets);
        let handle = set.spawn(async move {
            // Build a fresh FetchCtx inside the task. `secrets_arc` lives for the
            // duration of the closure; `&*secrets_arc` borrows it locally.
            let ctx = FetchCtx {
                now,
                secrets: &secrets_arc,
            };
            let result = match timeout(per_provider_timeout, provider.fetch(&ctx)).await {
                Ok(Ok(state)) => Ok(state),
                Ok(Err(e)) => Err(e),
                Err(_elapsed) => Err(ProviderError::Unavailable {
                    reason: format!("timed out after {per_provider_timeout:?}"),
                }),
            };
            (pid, result)
        });
        id_map.insert(handle.id(), pid);
    }

    let mut out = Vec::with_capacity(providers.len());
    while let Some(join_result) = set.join_next_with_id().await {
        match join_result {
            Ok((_task_id, pair)) => out.push(pair),
            Err(je) => {
                let task_id = je.id();
                let recovered = id_map.get(&task_id).copied();
                if je.is_panic() {
                    // Pitfall L4 binding: recover ProviderId from the map; never lose
                    // the task even if the adapter panicked mid-fetch.
                    let pid = recovered.unwrap_or(ProviderId::Mock);
                    tracing::error!("adapter task panicked: pid={pid:?} task_id={task_id:?}");
                    out.push((
                        pid,
                        Err(ProviderError::Internal {
                            source: anyhow::anyhow!("adapter panicked: {pid:?}"),
                        }),
                    ));
                } else {
                    // Cancelled or other non-panic JoinError. Phase 1 doesn't cancel,
                    // so this is an unexpected path — surface as Internal too.
                    let pid = recovered.unwrap_or(ProviderId::Mock);
                    tracing::error!("adapter task non-panic JoinError: pid={pid:?} task_id={task_id:?} err={je:?}");
                    out.push((
                        pid,
                        Err(ProviderError::Internal {
                            source: anyhow::anyhow!("adapter task failed: {je}"),
                        }),
                    ));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::HpWindow;
    use crate::model::ResetInfo;
    use async_trait::async_trait;
    use std::borrow::Cow;

    /// Adapter that panics during fetch — tests Pitfall L4 recovery.
    struct PanicProvider;

    #[async_trait]
    impl Provider for PanicProvider {
        fn id(&self) -> ProviderId {
            ProviderId::Mock
        }
        #[allow(clippy::panic)] // intentional fault injection for ADP-01 test
        async fn fetch(&self, _ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
            panic!("intentional panic for fanout test");
        }
    }

    /// Adapter that sleeps longer than the test timeout — tests the timeout branch.
    struct SlowProvider;

    #[async_trait]
    impl Provider for SlowProvider {
        fn id(&self) -> ProviderId {
            ProviderId::Claude
        }
        async fn fetch(&self, _ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
            tokio::time::sleep(Duration::from_secs(10)).await;
            // Never reached.
            Err(ProviderError::Unconfigured)
        }
    }

    /// Adapter that returns Ok immediately — tests happy path.
    struct OkProvider;

    #[async_trait]
    impl Provider for OkProvider {
        fn id(&self) -> ProviderId {
            ProviderId::Gemini
        }
        async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
            Ok(ProviderState {
                id: ProviderId::Gemini,
                windows: vec![HpWindow {
                    label: Cow::Borrowed("ok"),
                    percent_remaining: 50.0,
                    reset: ResetInfo {
                        resets_at: ctx.now + jiff::Span::new().hours(1),
                    },
                    bar_color: None,
                }],
                fetched_at: ctx.now,
                source: Cow::Borrowed("test-ok"),
            })
        }
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn panic_in_adapter_becomes_internal_error_not_lost_task() {
        let providers: Vec<Arc<dyn Provider>> = vec![Arc::new(PanicProvider)];
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let secrets = Arc::new(Secrets::default());
        let results = refresh_all_inner(&providers, now, secrets, Duration::from_millis(500)).await;
        assert_eq!(results.len(), 1, "panic must not lose the task (Pitfall L4)");
        let (pid, result) = &results[0];
        assert_eq!(*pid, ProviderId::Mock);
        match result {
            Err(ProviderError::Internal { source }) => {
                assert!(source.to_string().contains("panicked"));
            }
            other => panic!("expected Internal error, got {other:?}"),
        }
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn slow_adapter_returns_unavailable_after_timeout() {
        let providers: Vec<Arc<dyn Provider>> = vec![Arc::new(SlowProvider)];
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let secrets = Arc::new(Secrets::default());
        let start = std::time::Instant::now();
        let results = refresh_all_inner(&providers, now, secrets, Duration::from_millis(100)).await;
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "timeout must short-circuit (elapsed: {elapsed:?})"
        );
        assert_eq!(results.len(), 1);
        let (pid, result) = &results[0];
        assert_eq!(*pid, ProviderId::Claude);
        match result {
            Err(ProviderError::Unavailable { reason }) => {
                assert!(reason.contains("timed out"));
            }
            other => panic!("expected Unavailable error, got {other:?}"),
        }
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn happy_path_returns_ok() {
        let providers: Vec<Arc<dyn Provider>> = vec![Arc::new(OkProvider)];
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let secrets = Arc::new(Secrets::default());
        let results = refresh_all_inner(&providers, now, secrets, Duration::from_secs(1)).await;
        assert_eq!(results.len(), 1);
        let (pid, result) = &results[0];
        assert_eq!(*pid, ProviderId::Gemini);
        let state = result.as_ref().unwrap();
        assert_eq!(state.id, ProviderId::Gemini);
        assert_eq!(state.fetched_at, now);
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn empty_provider_list_returns_empty_vec() {
        let providers: Vec<Arc<dyn Provider>> = vec![];
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        let secrets = Arc::new(Secrets::default());
        let results =
            refresh_all_inner(&providers, now, secrets, DEFAULT_PER_PROVIDER_TIMEOUT).await;
        assert!(results.is_empty());
    }
}
