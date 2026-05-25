//! Config loader. Phase 1: TOML schema per CONTEXT D-36, `ProjectDirs` path per D-39,
//! first-run auto-create per D-37, unknown-key warn-and-ignore per D-38. Phase 3
//! adds CFG-03 (per-provider `refresh_interval` / `auth_source`); the struct here
//! holds the surface those phases extend.
//!
//! Contract:
//! - `default_path()` resolves the cross-OS config path via `directories::ProjectDirs::from("", "", "ahb")`.
//! - `load_or_init(path)` returns a `LoadOutcome`: either `Initialized(path)` (first-run,
//!   default template written, caller decides whether to exit) or `Loaded(cfg)`.
//! - Unknown keys under `[providers.*]` emit `tracing::warn!` and are otherwise ignored
//!   (forward-compat per D-38; we deliberately do NOT enable serde's strict-fields mode).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Embedded default config written on first-run. D-37 binding.
const DEFAULT_CONFIG: &str = include_str!("templates/default-config.toml");

/// Known provider table keys (D-36). Anything else under `[providers]` triggers
/// the unknown-key warn path. Mock is included so Plan 02's panic-injection test
/// can opt in via config; the default template does NOT mention mock (power-user knob).
const KNOWN_PROVIDER_KEYS: &[&str] = &["claude", "codex", "gemini", "mock"];

/// Known per-provider keys. Phase 1 only `enabled`. Phase 3 may add
/// `refresh_interval` / `auth_source` (CFG-03).
const KNOWN_PROVIDER_FIELD_KEYS: &[&str] = &["enabled"];

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Providers {
    #[serde(default)]
    pub claude: ProviderConfig,
    #[serde(default)]
    pub codex: ProviderConfig,
    #[serde(default)]
    pub gemini: ProviderConfig,
    /// Power-user knob — NOT in the default template. Plan 02 uses this for the
    /// panic-injection integration test. Out-of-tree users may also set
    /// `[providers.mock] enabled = true` to see a synthetic HP bar.
    #[serde(default)]
    pub mock: ProviderConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: Providers,
}

/// Outcome of `load_or_init`: either we initialized a default config (the caller
/// decides whether to exit) or we loaded an existing one.
#[derive(Debug)]
pub enum LoadOutcome {
    /// First-run path: default config written at `PathBuf` (D-37). `main` typically `exit(0)`.
    Initialized(PathBuf),
    /// Existing config parsed successfully.
    Loaded(Config),
}

/// Cross-OS config path resolution per D-39: `~/.config/ahb/config.toml` on Linux,
/// `~/Library/Application Support/ahb/config.toml` on macOS, `%APPDATA%\ahb\config.toml`
/// on Windows.
///
/// # Errors
///
/// Returns an error if `directories` cannot resolve `ProjectDirs::from("", "", "ahb")`
/// (e.g., `HOME` unset on Linux — extremely rare).
pub fn default_path() -> anyhow::Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "ahb")
        .ok_or_else(|| anyhow::anyhow!("could not resolve config dir for ahb"))?;
    Ok(dirs.config_dir().join("config.toml"))
}

/// Load the config at `path`, or initialize it from the embedded default template
/// if missing. D-37 first-run path: writes the file, prints the D-37 literal, and
/// returns `LoadOutcome::Initialized(path)`. Caller decides whether to exit.
///
/// # Errors
///
/// Returns an error if the parent directory cannot be created, the file cannot be
/// written or read, or the TOML cannot be parsed.
pub fn load_or_init(path: &Path) -> anyhow::Result<LoadOutcome> {
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, DEFAULT_CONFIG)?;
        // D-37 literal: stdout, not stderr. Caller decides exit(0).
        println!(
            "initialized {} — enable providers and rerun",
            path.display()
        );
        return Ok(LoadOutcome::Initialized(path.to_path_buf()));
    }
    let text = std::fs::read_to_string(path)?;
    // D-38: pre-pass to detect unknown keys, then deserialize the typed struct (which
    // does NOT enable serde's strict-fields mode, so unknown keys silently drop). The
    // pre-pass is purely advisory — emit `tracing::warn!` per unknown key, see README.
    warn_unknown_keys(&text);
    let cfg: Config = toml::from_str(&text)
        .map_err(|e| anyhow::anyhow!("parse config {}: {e}", path.display()))?;
    Ok(LoadOutcome::Loaded(cfg))
}

/// Walk the parsed TOML `Value` for unknown keys under `[providers.*]` and emit
/// `tracing::warn!` for each. Pure advisory — does not mutate or reject.
fn warn_unknown_keys(text: &str) {
    let Ok(value) = toml::from_str::<toml::Value>(text) else {
        // If raw parse fails, the typed parse will also fail with a real error — no
        // point emitting a noisy warning here. The caller's typed-parse error wins.
        return;
    };
    let Some(top) = value.as_table() else { return };
    // Top-level: only `providers` is known.
    for top_key in top.keys() {
        if top_key != "providers" {
            tracing::warn!("unrecognized config key '{top_key}' — see README");
        }
    }
    let Some(providers) = top.get("providers").and_then(toml::Value::as_table) else {
        return;
    };
    for (prov_name, prov_value) in providers {
        if !KNOWN_PROVIDER_KEYS.contains(&prov_name.as_str()) {
            tracing::warn!("unrecognized config key 'providers.{prov_name}' — see README");
            continue;
        }
        let Some(prov_table) = prov_value.as_table() else {
            continue;
        };
        for field in prov_table.keys() {
            if !KNOWN_PROVIDER_FIELD_KEYS.contains(&field.as_str()) {
                tracing::warn!(
                    "unrecognized config key 'providers.{prov_name}.{field}' — see README"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_returns_a_path() {
        // Cannot assert exact path (varies per OS) but it should resolve.
        let p = default_path().unwrap();
        assert!(p.to_string_lossy().contains("ahb"));
        assert!(p.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    fn load_or_init_initializes_missing_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("config.toml");
        assert!(!path.exists());

        let outcome = load_or_init(&path).unwrap();
        match outcome {
            LoadOutcome::Initialized(p) => assert_eq!(p, path),
            LoadOutcome::Loaded(_) => panic!("expected Initialized, got Loaded"),
        }
        assert!(path.exists(), "config file must exist on disk after init");
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("[providers.claude]"));
        assert!(written.contains("[providers.codex]"));
        assert!(written.contains("[providers.gemini]"));
        // Mock is NOT in the default template (power-user-only knob).
        assert!(!written.contains("[providers.mock]"));
    }

    #[test]
    fn load_or_init_loads_existing_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            "[providers.claude]\nenabled = true\n\n[providers.codex]\nenabled = false\n",
        )
        .unwrap();

        let outcome = load_or_init(&path).unwrap();
        let cfg = match outcome {
            LoadOutcome::Loaded(c) => c,
            LoadOutcome::Initialized(_) => panic!("expected Loaded"),
        };
        assert!(cfg.providers.claude.enabled);
        assert!(!cfg.providers.codex.enabled);
        assert!(!cfg.providers.gemini.enabled);
        assert!(!cfg.providers.mock.enabled);
    }

    #[test]
    fn load_or_init_tolerates_unknown_keys() {
        // D-38: unknown keys should NOT cause a parse failure (forward-compat).
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            "[providers.claude]\nenabled = true\nbogus = 1\n\n\
             [providers.bogus_provider]\nenabled = true\n",
        )
        .unwrap();

        let outcome = load_or_init(&path).unwrap();
        let cfg = match outcome {
            LoadOutcome::Loaded(c) => c,
            LoadOutcome::Initialized(_) => panic!("expected Loaded"),
        };
        // Known field still parsed.
        assert!(cfg.providers.claude.enabled);
    }

    #[test]
    fn warn_unknown_keys_detects_top_level_unknown() {
        // This test exercises the warn-key walker (no #[tracing-subscriber] capture —
        // we just call the fn and confirm it doesn't panic; the warn output is best-effort
        // observability per UI-SPEC). The contract here is "must not crash on weird TOML".
        warn_unknown_keys("foo = 1\n[providers.claude]\nenabled = true\nbogus = 1\n");
        warn_unknown_keys("[providers.unknown_provider]\nenabled = true\n");
        warn_unknown_keys(""); // empty
        warn_unknown_keys("not = valid = toml = at = all"); // garbage — should not panic
    }

    #[test]
    fn default_template_parses_cleanly() {
        // Round-trip the embedded template through the parser to confirm it's valid.
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert!(!cfg.providers.claude.enabled);
        assert!(!cfg.providers.codex.enabled);
        assert!(!cfg.providers.gemini.enabled);
    }

    // ---- Phase 3 Plan 01 Task 1: refresh_interval (CFG-03, D-72) ----

    #[test]
    fn provider_config_refresh_interval_parses_value() {
        // CFG-03 / D-72: TOML `refresh_interval = 30` must deserialize to Some(30).
        let cfg: Config = toml::from_str(
            "[providers.claude]\nenabled = true\nrefresh_interval = 30\n",
        )
        .unwrap();
        assert!(cfg.providers.claude.enabled);
        assert_eq!(cfg.providers.claude.refresh_interval, Some(30));
    }

    #[test]
    fn provider_config_refresh_interval_absent_is_none() {
        // D-72: absent `refresh_interval` deserializes to None — engine then uses
        // the per-provider DEFAULT_REFRESH_INTERVAL_SECS.
        let cfg: Config =
            toml::from_str("[providers.claude]\nenabled = true\n").unwrap();
        assert!(cfg.providers.claude.enabled);
        assert_eq!(cfg.providers.claude.refresh_interval, None);
    }

    #[test]
    fn provider_config_refresh_interval_zero_is_accepted_by_parser() {
        // D-72: clamp ≥ 5s is Engine's job (Plan 02). The parse layer accepts
        // any valid u64 including 0 — no sentinel meaning at this layer.
        let cfg: Config = toml::from_str(
            "[providers.claude]\nenabled = true\nrefresh_interval = 0\n",
        )
        .unwrap();
        assert_eq!(cfg.providers.claude.refresh_interval, Some(0));
    }

    #[test]
    fn provider_config_known_key_refresh_interval_does_not_warn() {
        // KNOWN_PROVIDER_FIELD_KEYS must contain "refresh_interval" so the
        // D-38 forward-compat warn-walker does NOT emit a noisy warning for
        // this newly-added field. The live warn path is exercised in the
        // integration test (tests/refresh_interval_config_parse.rs); here we
        // assert the array contents directly.
        assert!(
            KNOWN_PROVIDER_FIELD_KEYS.contains(&"refresh_interval"),
            "KNOWN_PROVIDER_FIELD_KEYS must include \"refresh_interval\" — got {KNOWN_PROVIDER_FIELD_KEYS:?}"
        );
        // And the typo variant must NOT be in the allow-list (sanity-check):
        assert!(!KNOWN_PROVIDER_FIELD_KEYS.contains(&"refresh_intervall"));
    }
}
