use std::borrow::Cow;

use async_trait::async_trait;

use crate::model::{HpWindow, ProviderError, ProviderId, ProviderState, ResetInfo};
use crate::provider::{FetchCtx, Provider};

/// Phase 0 placeholder provider. Returns a hardcoded `HpWindow` per CONTEXT D-25
/// so the spine can be exercised before any real adapter exists. Phase 1 will
/// gate this behind a config-only flag; production builds will not invoke
/// `MockProvider` unless the user explicitly enables it. NOT `cfg(test)`-only --
/// production-reachable for Phase 0 `cargo run`.
pub struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Mock
    }

    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        // Plan 02 ADP-01 integration test injection. The `AHB_DEBUG_PANIC=adapter:mock`
        // env var is operator-controlled; tests/panic_isolation.rs uses it to prove that
        // a panic inside one adapter does NOT crash the process and does NOT blank
        // healthy adapters. The scoped `#[allow(clippy::panic)]` is the only deviation
        // from the lib.rs lint floor — see PATTERNS.md `provider/mock.rs (modified)`.
        if std::env::var_os("AHB_DEBUG_PANIC").as_deref()
            == Some(std::ffi::OsStr::new("adapter:mock"))
        {
            #[allow(clippy::panic)]
            // intentional fault injection for ADP-01 integration test — see PATTERNS provider/mock.rs (modified)
            {
                panic!("AHB_DEBUG_PANIC injected");
            }
        }

        // CRITICAL: use ctx.now (the injected clock), never a wall-clock read,
        // per RESEARCH Anti-Patterns. Clock-injection contract for testability.
        let resets_at = ctx.now + jiff::Span::new().hours(2);

        Ok(ProviderState {
            id: ProviderId::Mock,
            windows: vec![HpWindow {
                label: Cow::Borrowed("mock-session"),
                percent_remaining: 60.0,
                reset: ResetInfo { resets_at },
                bar_color: None,
            }],
            fetched_at: ctx.now,
            source: Cow::Borrowed("mock"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;

    // Compile-time assertions (Test 4): MockProvider + Box<dyn Provider> are both Send+Sync.
    // Re-asserts the dyn-safety from provider/mod.rs as defense in depth.
    assert_impl_all!(MockProvider: Send, Sync);
    assert_impl_all!(Box<dyn Provider>: Send, Sync);

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn mock_returns_expected_shape() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let secrets = crate::secrets::Secrets::default();
        let ctx = FetchCtx { now, secrets: &secrets };
        let provider = MockProvider;
        let state = provider.fetch(&ctx).await.unwrap();

        assert_eq!(state.id, ProviderId::Mock);
        assert_eq!(state.windows.len(), 1);
        assert_eq!(state.windows[0].label, "mock-session");
        assert!((state.windows[0].percent_remaining - 60.0).abs() < f32::EPSILON);
        assert_eq!(
            state.windows[0].reset.resets_at,
            now + jiff::Span::new().hours(2)
        );
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn mock_uses_injected_clock() {
        // Pick a fixed past timestamp so the test would obviously fail if
        // fetch used Timestamp::now() instead.
        let now: jiff::Timestamp = "2020-01-01T00:00:00Z".parse().unwrap();
        let secrets = crate::secrets::Secrets::default();
        let ctx = FetchCtx { now, secrets: &secrets };
        let provider = MockProvider;
        let state = provider.fetch(&ctx).await.unwrap();

        assert_eq!(state.fetched_at, now);
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn provider_state_serde_roundtrip_full() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let secrets = crate::secrets::Secrets::default();
        let ctx = FetchCtx { now, secrets: &secrets };
        let provider = MockProvider;
        let state = provider.fetch(&ctx).await.unwrap();

        let json = serde_json::to_string(&state).unwrap();
        let back: ProviderState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.id, back.id);
        assert_eq!(state.windows.len(), back.windows.len());
        assert_eq!(state.fetched_at, back.fetched_at);
        assert_eq!(state.source, back.source);
    }

    #[test]
    fn mock_id_returns_mock() {
        assert_eq!(MockProvider.id(), ProviderId::Mock);
    }
}
