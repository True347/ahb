//! Integration tests for Phase 3 Plan 01 / CFG-03 / D-72: the
//! `[providers.<id>].refresh_interval` config knob.
//!
//! These tests exercise the **public** `ai_hp_bar::config::load_or_init` path with
//! tempfile-backed configs. The clamp-and-warn behavior described in D-72 is
//! enforced inside `Engine::new` (Plan 02 of this phase); the parser layer is
//! a pure DTO that accepts any valid `u64`. These tests pin that DTO contract
//! so Plan 02 can layer Engine-side clamping on top without disturbing the
//! schema.
//!
//! Test names follow `03-RESEARCH.md § Test Surface` so `cargo test
//! refresh_interval_config_parse` filter-matches all five.

use ai_hp_bar::config::{load_or_init, LoadOutcome};

/// Helper: write `body` to a fresh tempfile and call `load_or_init`.
/// Returns the parsed `Config` (panics if the loader chose `Initialized` — the
/// path is `tempfile::NamedTempFile::path()` which always exists, so we should
/// always hit the `Loaded` arm).
fn parse_config(body: &str) -> ai_hp_bar::config::Config {
    let tmp = tempfile::NamedTempFile::new().expect("tempfile");
    std::fs::write(tmp.path(), body).expect("write");
    let outcome = load_or_init(tmp.path()).expect("load");
    match outcome {
        LoadOutcome::Loaded(c) => c,
        LoadOutcome::Initialized(_) => panic!(
            "expected Loaded — tempfile path exists so load_or_init should not init"
        ),
    }
}

#[test]
fn refresh_interval_parses_from_toml() {
    // CFG-03 / D-72: `refresh_interval = 30` → Some(30) on the matching provider.
    let cfg = parse_config(
        "[providers.claude]\nenabled = true\nrefresh_interval = 30\n",
    );
    assert!(cfg.providers.claude.enabled);
    assert_eq!(cfg.providers.claude.refresh_interval, Some(30));
}

#[test]
fn refresh_interval_absent_deserializes_to_none() {
    // D-72: absent field → None; Engine then uses DEFAULT_REFRESH_INTERVAL_SECS.
    let cfg = parse_config("[providers.claude]\nenabled = true\n");
    assert!(cfg.providers.claude.enabled);
    assert_eq!(cfg.providers.claude.refresh_interval, None);
}

#[test]
fn refresh_interval_zero_accepted_by_parser() {
    // D-72: clamp ≥ 5s is Engine's job (Plan 02). The parse layer accepts
    // any valid u64 including 0 — no sentinel meaning at this layer.
    let cfg = parse_config(
        "[providers.codex]\nenabled = true\nrefresh_interval = 0\n",
    );
    assert_eq!(cfg.providers.codex.refresh_interval, Some(0));
}

#[test]
fn refresh_interval_large_value_accepted() {
    // D-72: no upper bound (\"上限不設\"). 86400 = 24h in seconds; a slow
    // network adapter could realistically want this.
    let cfg = parse_config(
        "[providers.gemini]\nenabled = true\nrefresh_interval = 86400\n",
    );
    assert_eq!(cfg.providers.gemini.refresh_interval, Some(86400));
}

#[test]
fn refresh_interval_typo_key_does_not_panic() {
    // D-38 forward-compat: a typo like `refresh_intervall` (double-l) must
    // NOT cause `load_or_init` to fail. The warn-walker emits a
    // `tracing::warn!` advisory; the typed parse simply drops the unknown
    // field. Asserting that `load_or_init` returns Ok(LoadOutcome::Loaded) is
    // sufficient to prove the forward-compat path holds — the warning text
    // itself is best-effort observability.
    let cfg = parse_config(
        "[providers.claude]\nenabled = true\nrefresh_intervall = 10\n",
    );
    assert!(cfg.providers.claude.enabled);
    // The typo did NOT populate `refresh_interval`; the field stays None.
    assert_eq!(cfg.providers.claude.refresh_interval, None);
}
