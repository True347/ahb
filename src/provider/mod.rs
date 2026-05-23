//! `Provider` trait + `FetchCtx` -- the cross-adapter contract that every later phase
//! plugs into.
//!
//! The same trait is consumed by both CLI (Phase 0) and TUI (Phase 1) front-ends: that's
//! the spine.
//!
//! Per RESEARCH Q8 / `async_trait` README: native async-fn-in-trait stabilized in Rust
//! 1.75 but `dyn Trait` containing async fn is still not `dyn`-compatible at Rust 1.88.
//! `#[async_trait]` boxes the returned future so the vtable works -- the
//! `assert_impl_all!(Box<dyn Provider>: Send, Sync)` line in tests below is the
//! compile-time proof that this matters; drop `#[async_trait]` and the assertion fails
//! with a dyn-compatibility error.

use async_trait::async_trait;

use crate::model::{ProviderError, ProviderId, ProviderState};
use crate::secrets::Secrets;

pub mod claude;
pub mod mock;

/// Per-fetch context. Currently the minimal 2 fields recommended by RESEARCH Q5:
/// the wall clock and a shared-reference to the secrets handle. Adapters MUST use
/// `ctx.now` instead of `jiff::Timestamp::now()` so tests can inject a frozen clock
/// (ARCHITECTURE.md Testing Seam 5).
///
/// `Copy` derives cleanly because both fields are `Copy` (shared references always are,
/// and `jiff::Timestamp` is `Copy`).
#[derive(Debug, Clone, Copy)]
pub struct FetchCtx<'a> {
    pub now: jiff::Timestamp,
    pub secrets: &'a Secrets,
}

/// Per-provider adapter contract. Same trait is consumed by both CLI and TUI front-ends
/// (the spine). Adapter failures must be isolated by the caller -- never panic; return
/// `ProviderError`. Per ADP-01 (Phase 1), the engine wraps each fetch in a
/// `Vec<Result<...>>` so one adapter failure cannot blank the whole bar.
#[async_trait]
pub trait Provider: Send + Sync + 'static {
    fn id(&self) -> ProviderId;
    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;

    // Compile-time dyn-safety proof. If `#[async_trait]` is removed above, this fails to
    // compile with "the trait `Provider` cannot be made into an object" because native
    // async-fn-in-trait is not yet `dyn`-compatible at Rust 1.88 (RESEARCH Q8).
    assert_impl_all!(Box<dyn Provider>: Send, Sync);

    // The plan's acceptance criteria explicitly require `Secrets::default()` as a
    // behavior check; suppress the unit-struct lint at the test scope so the call survives.
    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn secrets_default_constructs() {
        let s = Secrets::default();
        // Read it so clippy doesn't flag "no side effect" on the binding.
        let _: &Secrets = &s;
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn fetch_ctx_constructs() {
        let s = Secrets::default();
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let ctx = FetchCtx { now, secrets: &s };
        // Touch every field so the binding has an observable use.
        assert_eq!(ctx.now, now);
        // Pointer equality is the cheapest way to confirm secrets ref is the same handle.
        let secrets_ptr: *const Secrets = &raw const s;
        assert!(std::ptr::eq(ctx.secrets, secrets_ptr));
    }
}
