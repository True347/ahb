//! Cross-OS sanity that `secrets::init()` is callable without segfault and returns
//! either `InitOutcome::Ready(_)` (backend available on this host) OR
//! `InitOutcome::Unavailable` (CI runner with no dbus / no keychain). Either
//! branch passes; the test fails only on panic / segfault / impossible third state.
//!
//! Regression guard for Pitfall L3: catches breakage in `set_default_store`
//! registration before Phase 2/3 plug real credentials.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

#![allow(clippy::unwrap_used, clippy::panic)] // tests: clippy.toml allow-* = true

use ahb::secrets::{self, InitOutcome};

#[test]
fn init_returns_ready_or_unavailable() {
    match secrets::init() {
        // Either branch is acceptable: dev machines with a working backend take
        // `Ready`; CI / headless runners take `Unavailable` (D-41 path at main.rs).
        Ok(InitOutcome::Ready(_) | InitOutcome::Unavailable) => {}
        Err(e) => {
            // The wrapper is supposed to surface backend errors as Ok(Unavailable), never
            // as a hard anyhow::Error. If we see one, it's a regression.
            panic!("secrets::init() returned an unexpected anyhow::Error: {e:?}");
        }
    }
}
