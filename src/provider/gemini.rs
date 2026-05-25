//! Phase 2 CR-01 placeholder for Gemini (ADP-05, Phase 3).
//!
//! The real Gemini adapter lands in Phase 3. Phase 2 still has to handle the
//! case where a user sets `[providers.gemini] enabled = true` in their config:
//! before this module existed, `Engine::new` would emit `tracing::debug!` and
//! push **nothing**, which broke the D-59 exit-code grid in two ways:
//!
//! 1. Gemini-only configs returned an empty `Vec` from `refresh_all`, which
//!    `DispatchOutcome::from_results` collapses to `AnySuccess` (CFG-04 "zero
//!    providers enabled = exit 0"). The user enabled a provider, AHB exited 0
//!    + printed the empty-state hint — indistinguishable from a fresh install.
//! 2. The `--help` exit-code contract ("1 = all configured providers failed")
//!    silently did not apply, because the configured provider never got a
//!    chance to fail.
//!
//! This module mirrors the Claude / Codex pattern by always returning
//! `Err(ProviderError::Unavailable { … })`. The render layer then paints a
//! visible `gemini  ERROR: …` row (with the next-step hint) and the dispatch
//! layer correctly counts it toward `AllFailed` → exit 1.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use async_trait::async_trait;

use crate::model::{ProviderError, ProviderId, ProviderState};
use crate::provider::{FetchCtx, Provider};

/// Phase-3 placeholder: returns `Err(Unavailable)` so the user gets a visible
/// row + the exit-code grid behaves correctly. Swap this for the real adapter
/// in Phase 3 (ADP-05); no other wiring changes.
pub struct GeminiUnimplementedProvider;

#[async_trait]
impl Provider for GeminiUnimplementedProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Gemini
    }

    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        // Same SEC contract as the other adapters: acknowledge the secrets
        // handle to keep the trait shape honest even though we never query it.
        let _ = ctx.secrets;
        Err(ProviderError::Unavailable {
            reason: "Gemini provider is not yet implemented (Phase 3) — set \
                     [providers.gemini].enabled = false to suppress this row"
                .into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::Secrets;
    use static_assertions::assert_impl_all;

    assert_impl_all!(GeminiUnimplementedProvider: Send, Sync);

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn gemini_placeholder_returns_unavailable() {
        let secrets = Secrets::default();
        let now: jiff::Timestamp = "2026-05-25T12:00:00Z".parse().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };
        let provider = GeminiUnimplementedProvider;
        let err = provider.fetch(&ctx).await.unwrap_err();
        match err {
            ProviderError::Unavailable { reason } => {
                assert!(
                    reason.contains("not yet implemented"),
                    "reason should explain Phase 3 status: {reason}"
                );
                assert!(
                    reason.contains("enabled = false"),
                    "reason should include next-step hint per UI-SPEC: {reason}"
                );
            }
            other => panic!("expected Unavailable, got: {other:?}"),
        }
    }

    #[test]
    fn gemini_placeholder_id_is_gemini() {
        assert_eq!(GeminiUnimplementedProvider.id(), ProviderId::Gemini);
    }
}
