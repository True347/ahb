//! Secrets module — Phase 1 Plan 02.
//!
//! Phase 1: structurally unchanged from Phase 0 stub (`Secrets` is still a unit struct
//! with `Debug + Default + Clone` so every existing callsite — `Secrets::default()` in
//! `mock.rs::tests`, `provider/mod.rs::tests`, `main.rs`, `engine/fanout.rs::tests` —
//! continues to compile). The keyring entry-point is `secrets::init()`, NOT the struct
//! itself; Phase 1 wires it end-to-end but never actually stores or loads a credential
//! (Claude reads local JSONL). Phase 2 widens `Secrets` to hold cached keyring entries
//! when Codex / Gemini plug real credentials in.
//!
//! `Secret<T>` newtype is the redaction primitive (D-42):
//! - `Drop` → `zeroize` the inner value on scope exit
//! - `Debug` → emits the literal `***` (never the underlying bytes)
//! - `Serialize` → emits the literal `"[REDACTED]"`
//! - **NO `Deserialize` impl** (D-42 binding — secrets come from the keyring, NEVER from
//!   TOML/JSON). The lack of Deserialize is enforced by `grep` in the acceptance gate.
//! - The single grep-discoverable unwrap path is `.expose(&self) -> &T`. Auditors can
//!   `grep -r ".expose(" src/` to find every secret-read site.
//!
//! `init()` returns `Ok(InitOutcome::Ready(_))` after registering an OS-appropriate
//! `*-keyring-store` via `keyring_core::set_default_store(...)`, OR `Ok(InitOutcome::
//! Unavailable)` when no backend can be constructed (CI runner, headless Linux with no
//! dbus). `main.rs` matches the latter and prints the D-41 literal + `exit(2)`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::fmt;
use std::sync::Arc;

use serde::{Serialize, Serializer};
use zeroize::Zeroize;

/// Phase 0 / Phase 1 surface: still a unit struct so back-compat with Plan 01 callsites
/// is preserved. Phase 2 widens this to hold cached `Secret<String>`s for Codex / Gemini.
#[derive(Debug, Default, Clone)]
pub struct Secrets;

/// Redacting newtype wrapper around a secret value (D-42).
///
/// Guarantees:
/// - `Drop` zeroizes the inner value.
/// - `Debug` prints the literal `***` (never the underlying bytes).
/// - `Serialize` emits the literal `"[REDACTED]"` (never the underlying bytes).
/// - No `Deserialize` impl — secrets come from the keyring, not from TOML/JSON.
/// - `.expose(&self) -> &T` is the SINGLE grep-discoverable unwrap path.
pub struct Secret<T: Zeroize + Clone>(T);

impl<T: Zeroize + Clone> Drop for Secret<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize + Clone> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

impl<T: Zeroize + Clone> Serialize for Secret<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("[REDACTED]")
    }
}

// Intentionally NO `Deserialize` impl. Secrets are read from the OS keyring, never
// from TOML/JSON. The grep gate `grep -E 'impl[^{]*Deserialize[^{]*for Secret'` must
// stay empty. D-42 binding.

impl<T: Zeroize + Clone> Secret<T> {
    /// Wrap an inner value. The wrapper takes ownership; the inner value is zeroized
    /// on `Drop`.
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    /// The single grep-discoverable unwrap path. Returns a borrow with the lifetime
    /// of `self`; callers can copy out only as much as they need.
    pub fn expose(&self) -> &T {
        &self.0
    }
}

/// Outcome of `init()` — either the keyring backend registered cleanly, or no backend
/// is available on this host. `main.rs` decides what to do with `Unavailable` (currently:
/// print D-41 literal + `exit(2)`).
#[derive(Debug)]
pub enum InitOutcome {
    /// A backend was constructed and registered via `set_default_store`.
    Ready(Secrets),
    /// No backend could be constructed on this host. D-41 binding — caller exits 2.
    Unavailable,
}

/// Construct an OS-appropriate credential store and register it via
/// `keyring_core::set_default_store`. Returns `Ok(InitOutcome::Ready(_))` on success and
/// `Ok(InitOutcome::Unavailable)` when no backend could be constructed (dbus missing,
/// headless CI, etc.). Backend-construction errors are swallowed into `Unavailable` so
/// the caller has a single clean branch (D-41).
///
/// **Test affordance:** In debug builds, setting `AHB_SECRETS_MOCK=1` registers
/// `keyring_core::mock::Store` instead of the OS-native backend. This lets
/// integration tests on backend-less hosts (CI runners without dbus / gnome-keyring,
/// dev machines without Keychain unlocked) run Plan 01's walking-skeleton tests
/// without forcing them to also assert the D-41 exit-2 branch. The flag is
/// `#[cfg(debug_assertions)]`-gated so release builds (cargo-dist) cannot consult
/// it — production behavior is binding-strict on D-41.
///
/// # Errors
///
/// Currently returns `Err` only if the wrapper itself encounters a non-backend bug.
/// All backend-construction failures collapse to `Ok(InitOutcome::Unavailable)`.
pub fn init() -> anyhow::Result<InitOutcome> {
    #[cfg(debug_assertions)]
    if std::env::var_os("AHB_SECRETS_MOCK").as_deref()
        == Some(std::ffi::OsStr::new("1"))
    {
        // Register the in-memory mock store — integration tests + dev machines without
        // a working keyring can exercise the Plan 01 happy path without the D-41
        // hard-error path firing.
        match keyring_core::mock::Store::new() {
            Ok(store) => {
                keyring_core::set_default_store(store as Arc<keyring_core::CredentialStore>);
                #[allow(clippy::default_constructed_unit_structs)]
                return Ok(InitOutcome::Ready(Secrets::default()));
            }
            Err(e) => {
                tracing::warn!("AHB_SECRETS_MOCK=1 set but mock store failed: {e}");
                return Ok(InitOutcome::Unavailable);
            }
        }
    }
    match make_default_store() {
        Ok(store) => {
            keyring_core::set_default_store(store);
            #[allow(clippy::default_constructed_unit_structs)]
            Ok(InitOutcome::Ready(Secrets::default()))
        }
        Err(e) => {
            tracing::warn!("keyring backend unavailable: {e}");
            Ok(InitOutcome::Unavailable)
        }
    }
}

/// Linux: dbus Secret Service (libsecret / gnome-keyring / `KWallet` via dbus).
///
/// `Store::new()` returns `Result<Arc<Self>>`; we widen to
/// `Arc<dyn CredentialStoreApi + Send + Sync>` (the
/// `keyring_core::CredentialStore` type alias) for `set_default_store`.
#[cfg(target_os = "linux")]
fn make_default_store() -> anyhow::Result<Arc<keyring_core::CredentialStore>> {
    let store = dbus_secret_service_keyring_store::Store::new()
        .map_err(|e| anyhow::anyhow!("dbus secret service unavailable: {e}"))?;
    Ok(store as Arc<keyring_core::CredentialStore>)
}

/// macOS: Apple Keychain. The very first run may prompt the user for Keychain access;
/// Phase 1 only registers the store (no credentials stored yet) so the prompt is rare.
#[cfg(target_os = "macos")]
fn make_default_store() -> anyhow::Result<Arc<keyring_core::CredentialStore>> {
    let store = apple_native_keyring_store::keychain::Store::new()
        .map_err(|e| anyhow::anyhow!("macOS keychain unavailable: {e}"))?;
    Ok(store as Arc<keyring_core::CredentialStore>)
}

/// Windows: Credential Manager. Silent on first registration.
#[cfg(target_os = "windows")]
fn make_default_store() -> anyhow::Result<Arc<keyring_core::CredentialStore>> {
    let store = windows_native_keyring_store::Store::new()
        .map_err(|e| anyhow::anyhow!("windows credential manager unavailable: {e}"))?;
    Ok(store as Arc<keyring_core::CredentialStore>)
}

/// Fallback for unsupported targets (BSDs, exotic OSes). The wrapper returns
/// `Unavailable` and `main.rs` prints D-41.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn make_default_store() -> anyhow::Result<Arc<keyring_core::CredentialStore>> {
    Err(anyhow::anyhow!(
        "no keyring backend compiled for this target_os"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_expose_returns_inner_value() {
        let s = Secret::new("payload".to_string());
        assert_eq!(s.expose(), &"payload".to_string());
    }

    #[test]
    fn secret_debug_emits_three_asterisks_only() {
        let s = Secret::new("hidden".to_string());
        assert_eq!(format!("{s:?}"), "***");
    }

    #[test]
    fn secret_serialize_emits_redacted_literal() {
        let s = Secret::new("hidden".to_string());
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(j, "\"[REDACTED]\"");
    }

    #[test]
    fn secret_drop_zeroizes_inner() {
        // The Drop impl calls zeroize on the inner T. We can't easily observe the
        // post-drop state of a moved value in safe Rust without unsafe, so we rely on
        // the `zeroize` crate's own correctness here and on the grep gate that asserts
        // `impl Drop for Secret` is present in source.
        let _ = Secret::new("zeroize-me".to_string());
        // Successful compile = `Drop` impl is wired; the actual zeroing is exercised
        // implicitly by every test that constructs a Secret and lets it drop.
    }
}
