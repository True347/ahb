//! Cross-adapter contract types (the "spine") shared by every Provider implementation.
//!
//! Locks decisions D-08..D-14 from `.planning/phases/00-spike-spine/00-CONTEXT.md`.
//! Every later phase's adapter (Claude / Codex / Gemini / Mock) returns a `ProviderState`
//! built from these types. Adding or renaming a variant here is a deliberate API break.
//!
//! W-2: `ProviderState.source` is `Cow<'static, str>` (NOT `&'static str`) so the field
//! serde-round-trips. See plan 00-02 <interfaces> note.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// Closed enum of v1 providers (RESEARCH Q4 resolution of D-08).
/// `Mock` is required for Plan 03's `MockProvider`; do not omit.
/// EXT-01 (v2) will add `Other(Cow<'static, str>)` when a 4th provider concretely lands.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Claude,
    Codex,
    Gemini,
    Mock,
}

/// Percent remaining, 0.0..=100.0. Per CONTEXT D-10: percent only, NO raw token counts.
pub type HpUnit = f32;

/// Bar accent hint per D-09. Adapter-populated, optional.
/// Rendering rules (red < 10%, yellow < 30%, etc.) are deferred to Phase 1/2 per CONTEXT `<deferred>`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarColor {
    Red,
    Yellow,
    Green,
}

/// Absolute reset moment (D-11). UI layer computes countdown via `now() - resets_at`.
/// No `since` / `duration_until` helpers -- single source of truth, no derived state on this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetInfo {
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub resets_at: jiff::Timestamp,
}

/// One reset window. Providers may emit N windows (e.g. Claude's 5h session + weekly).
/// D-09 locked field set; `bar_color` is a render hint, not a rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpWindow {
    pub label: Cow<'static, str>,
    pub percent_remaining: HpUnit,
    pub reset: ResetInfo,
    pub bar_color: Option<BarColor>,
}

/// What a single `Provider::fetch` returns (D-08).
///
/// Multiple windows because one provider can have concurrent reset cadences (Claude session + weekly).
/// `source` is `Cow<'static, str>` (W-2) so the struct round-trips losslessly through `serde_json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderState {
    pub id: ProviderId,
    pub windows: Vec<HpWindow>,
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub fetched_at: jiff::Timestamp,
    /// W-2: Cow, NOT &'static str -- Deserialize-compatible without lifetime gymnastics.
    /// Adapters set `Cow::Borrowed("mock")` etc. for zero-cost; deserialization yields `Cow::Owned`.
    pub source: Cow<'static, str>,
}

/// Phase 0 stub -- Phase 3 widens to wrap `reqwest::Error`. CONTEXT D-12 mentioned a reqwest
/// wrapper in shorthand; this newtype keeps Phase 0 reqwest-free.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkErr(pub String);

impl std::fmt::Display for NetworkErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NetworkErr {}

/// Closed error enum exposed to the engine + UI (D-12).
///
/// Serialize-only (D-14): JSON form is `{"kind": "...", ...}` via `#[serde(tag = "kind")]`.
/// `Internal(anyhow::Error)` and `Network(NetworkErr)` use `serialize_with = "serialize_display"`
/// on a named field (NOT on a newtype payload) so the resulting JSON is a map -- serde's
/// internally-tagged form does not accept newtype variants whose inner type serializes to
/// a scalar (string). Constructor ergonomics preserved via `From` impls below.
/// Display-only serialization (W-7 / Pitfall 2): backtrace/Debug never leak.
#[derive(thiserror::Error, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderError {
    #[error("provider is not configured")]
    Unconfigured,

    #[error("provider unavailable: {reason}")]
    Unavailable { reason: String },

    #[error("schema drift: missing {missing:?}")]
    SchemaDrift { missing: Vec<String> },

    #[error("network: {source}")]
    Network {
        #[serde(serialize_with = "serialize_display")]
        source: NetworkErr,
    },

    /// `jiff::Span` is intentionally `#[serde(skip)]` in Phase 0 -- no adapter emits this variant
    /// yet, and Span's Serialize impl varies by feature flag. Phase 3 will add a custom serializer
    /// when the first adapter (Gemini) actually returns it.
    #[error("rate limited, retry after {retry_after:?}")]
    RateLimited {
        #[serde(skip)]
        retry_after: Option<jiff::Span>,
    },

    #[error("internal: {source}")]
    Internal {
        #[serde(serialize_with = "serialize_display")]
        source: anyhow::Error,
    },
}

impl From<NetworkErr> for ProviderError {
    fn from(source: NetworkErr) -> Self {
        Self::Network { source }
    }
}

impl From<anyhow::Error> for ProviderError {
    fn from(source: anyhow::Error) -> Self {
        Self::Internal { source }
    }
}

/// Serialize any `Display` value as its Display string, NOT its Debug form.
/// This is the W-7 / Pitfall 2 binding: stack traces and backtraces never leak to JSON.
fn serialize_display<T: std::fmt::Display, S: serde::Serializer>(
    val: &T,
    ser: S,
) -> Result<S::Ok, S::Error> {
    ser.collect_str(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;

    // Compile-time: ProviderState must be Send + Sync (transitively, every adapter's return).
    assert_impl_all!(ProviderState: Send, Sync);

    #[test]
    fn provider_state_serde_roundtrip() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let resets_at: jiff::Timestamp = "2026-05-22T14:00:00Z".parse().unwrap();

        let state = ProviderState {
            id: ProviderId::Mock,
            windows: vec![HpWindow {
                label: Cow::Borrowed("mock-session"),
                percent_remaining: 60.0,
                reset: ResetInfo { resets_at },
                bar_color: None,
            }],
            fetched_at: now,
            source: Cow::Borrowed("mock"),
        };

        let json = serde_json::to_string(&state).unwrap();
        let back: ProviderState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.id, back.id);
        assert_eq!(state.windows.len(), back.windows.len());
        assert_eq!(state.fetched_at, back.fetched_at);

        // W-2 binding: source round-trips. Cow's PartialEq compares contents regardless of
        // whether one side is Borrowed and the other Owned, so this equality holds AND
        // proves that the field type is Cow<'static, str>, not &'static str (the latter
        // wouldn't deserialize cleanly from a String buffer).
        assert_eq!(state.source, back.source);
        assert_eq!(back.source, Cow::<'static, str>::Owned("mock".to_string()));

        // Sanity: window contents survive
        assert_eq!(state.windows[0].label, back.windows[0].label);
        assert!((state.windows[0].percent_remaining - back.windows[0].percent_remaining).abs()
            < f32::EPSILON);
        assert_eq!(
            state.windows[0].reset.resets_at,
            back.windows[0].reset.resets_at
        );
    }

    #[test]
    fn provider_error_internal_serializes_display() {
        let err = ProviderError::Internal {
            source: anyhow::anyhow!("boom"),
        };
        let json = serde_json::to_string(&err).unwrap();

        // Display string + tag present
        assert!(json.contains("boom"), "missing 'boom' in JSON: {json}");
        assert!(json.contains("internal"), "missing 'internal' tag: {json}");
        assert!(json.contains("kind"), "missing 'kind' field: {json}");

        // W-7 binding: backtrace metadata MUST NOT leak.
        assert!(
            !json.contains("Backtrace"),
            "JSON leaks Backtrace metadata: {json}"
        );
        assert!(
            !json.contains("stack backtrace"),
            "JSON leaks 'stack backtrace': {json}"
        );
        // file:line markers from anyhow's Debug impl would contain ":\n   0:" or " at ./src/" -- guard both.
        assert!(
            !json.contains("at /"),
            "JSON leaks 'at /' path marker: {json}"
        );
        assert!(
            !json.contains("at ./"),
            "JSON leaks 'at ./' path marker: {json}"
        );
    }

    #[test]
    fn provider_error_schema_drift_serializes() {
        let err = ProviderError::SchemaDrift {
            missing: vec!["foo".into(), "bar".into()],
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(
            json.contains("schema_drift"),
            "missing 'schema_drift' tag: {json}"
        );
        assert!(json.contains("foo"), "missing 'foo' entry: {json}");
        assert!(json.contains("bar"), "missing 'bar' entry: {json}");
    }

    #[test]
    fn reset_info_serde_roundtrip() {
        let ts: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let r = ResetInfo { resets_at: ts };
        let json = serde_json::to_string(&r).unwrap();
        let back: ResetInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(r.resets_at, back.resets_at);
    }
}
