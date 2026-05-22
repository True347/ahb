//! Phase 0 stub. Phase 1 (per CONTEXT `canonical_refs`) wires `keyring-core` 1.0 +
//! `Secret<T>` newtype + `#[serde(skip)]` redaction. Keep the type empty so
//! `FetchCtx<'_>`'s `&Secrets` reference can be constructed without ABI breakage
//! when Phase 1 widens it.

/// No-op secrets handle. Phase 1 replaces this file with the real keyring-backed type.
#[derive(Debug, Default, Clone)]
pub struct Secrets;
