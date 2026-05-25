//! `--json schema_version: 1` output format (CORE-04 / D-49..D-52).
//!
//! Phase 2 Plan 02-03 deliverable. Defines the **stable v1 wire shape** that
//! every downstream tmux / Starship / shell-pipeline consumer couples to:
//!
//! - `JsonRoot` — top-level envelope (`schema_version`, `generated_at`,
//!   `providers`).
//! - `JsonProvider` — per-provider entry with `status: "ok" | "error"`,
//!   `windows`, optional `source` / `fetched_at` / `error`.
//! - `JsonWindow` — per-window record (`label`, `percent_remaining`,
//!   `reset_at`, optional `detailed_label`).
//! - `JsonError` — error envelope keyed by `kind` (snake_case ProviderError
//!   variant).
//!
//! **D-49 binding (stable DTO ↔ internal model decoupled):** these DTOs are
//! independent of `ProviderState` / `ProviderError` so a future refactor of the
//! internal model cannot accidentally break the wire shape. The converter
//! `to_json_root` is the single producer of these DTOs; every callsite that
//! emits JSON funnels through it.
//!
//! **D-50 (top-level shape):** array of objects with `id` field; providers
//! ordered by BL-02 (Claude=0 / Codex=1 / Gemini=2 / Mock=3 — engine boundary).
//! `generated_at` and per-provider `fetched_at` are RFC3339 UTC via jiff's
//! `timestamp::second::required` adapter.
//!
//! **D-51 (error envelope):** `status` is the binary discriminant. On `"ok"`
//! we emit `source` + `fetched_at` + `windows`; on `"error"` we emit `error`
//! + empty `windows: []`. The `error.kind` taxonomy is closed (snake_case of
//! ProviderError variants).
//!
//! **D-52 (semver policy):** the field set is *additive* — adding a new
//! provider id, a new `JsonWindow` field, or a new `error.kind` does NOT
//! bump `schema_version`. Consumers MUST tolerate unknown fields. Removing
//! / renaming / changing semantics is a v2 bump (out of scope for Phase 2).
//!
//! **SEC binding (D-49 + CONTEXT Claude's Discretion #7):** `JsonError.message`
//! emits `Display` only via `e.to_string()`; anyhow's cause chain is never
//! expanded (would risk leaking internal paths or secret-shaped strings).
//! `format_one_line` collapses any whitespace to single spaces (Phase 1
//! sanitizer rule, shared with `format_error_row_colored`).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::borrow::Cow;
use std::io::Write;

use serde::Serialize;

use crate::cli::render_text::{format_one_line, id_label};
// WR-06: `crate::cli::tty` import dropped — see `run_json` for the rationale
// (D-58 is now satisfied structurally, not by a runtime `should_colorize_env`
// call).
use crate::cli::{ColorMode, DispatchOutcome};
use crate::engine::Engine;
use crate::model::{HpWindow, ProviderError, ProviderId, ProviderState};

/// v1 wire-shape version. **Additive changes do not bump this constant** — only
/// removal / rename / semantic shift would (D-52). Phase 2 ships at `1`.
pub const SCHEMA_VERSION: u8 = 1;

/// Top-level JSON envelope. One per AHB invocation.
#[derive(Serialize)]
pub struct JsonRoot<'a> {
    pub schema_version: u8,
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub generated_at: jiff::Timestamp,
    pub providers: Vec<JsonProvider<'a>>,
}

/// Per-provider record. `'a` flows through from the `&[(ProviderId, Result<...>)]`
/// slice supplied to `to_json_root` so `source` can borrow without an extra
/// allocation when it is already `Cow::Borrowed`.
#[derive(Serialize)]
pub struct JsonProvider<'a> {
    pub id: &'static str,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Cow<'a, str>>,
    #[serde(
        with = "jiff::fmt::serde::timestamp::second::optional",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub fetched_at: Option<jiff::Timestamp>,
    pub windows: Vec<JsonWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonError>,
}

/// Per-window record. `label` is owned `String` to avoid Cow-lifetime gymnastics
/// across the converter boundary. `detailed_label` is the Plan 02-02 additive
/// field (D-52 policy): present iff the adapter set a distinct detailed-mode
/// label (Claude `"5h"` / `"weekly"`); omitted from JSON when `None`.
#[derive(Serialize)]
pub struct JsonWindow {
    pub label: String,
    pub percent_remaining: Option<f32>,
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub reset_at: jiff::Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detailed_label: Option<String>,
}

/// Per-provider error record (only present when `status == "error"`).
///
/// `kind` is the snake_case `ProviderError` variant name (closed taxonomy).
/// `message` is the sanitized one-line Display string. `missing` is only
/// populated for `SchemaDrift`; `retry_after_seconds` only for `RateLimited`.
#[derive(Serialize)]
pub struct JsonError {
    pub kind: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

/// Build the v1 JSON envelope from the engine's `refresh_all` output.
///
/// Iterates `results` in the engine's BL-02 canonical order (the caller is
/// expected to pass `Engine::refresh_all`'s output unchanged — `to_json_root`
/// performs NO re-sorting, per D-55).
///
/// Window passthrough order matches the adapter (`5h` then `weekly` for Claude,
/// `primary` then `secondary` for Codex) — `window_to_json` is a 1:1 transform.
#[must_use]
pub fn to_json_root<'a>(
    results: &'a [(ProviderId, Result<ProviderState, ProviderError>)],
    generated_at: jiff::Timestamp,
) -> JsonRoot<'a> {
    let providers = results
        .iter()
        .map(|(id, result)| match result {
            Ok(state) => JsonProvider {
                id: id_label(*id),
                status: "ok",
                source: Some(Cow::Borrowed(state.source.as_ref())),
                fetched_at: Some(state.fetched_at),
                windows: state.windows.iter().map(window_to_json).collect(),
                error: None,
            },
            Err(err) => JsonProvider {
                id: id_label(*id),
                status: "error",
                source: None,
                fetched_at: None,
                windows: Vec::new(),
                error: Some(error_to_json(err)),
            },
        })
        .collect();
    JsonRoot {
        schema_version: SCHEMA_VERSION,
        generated_at,
        providers,
    }
}

/// 1:1 transform from internal `HpWindow` to `JsonWindow`. NaN
/// `percent_remaining` (Claude weekly limit-unknown sentinel) converts to
/// `None`; serde emits `"percent_remaining":null` to make the
/// "we don't know" state explicit and machine-distinguishable from `0.0`.
fn window_to_json(w: &HpWindow) -> JsonWindow {
    JsonWindow {
        label: w.label.to_string(),
        percent_remaining: if w.percent_remaining.is_nan() {
            None
        } else {
            Some(w.percent_remaining)
        },
        reset_at: w.reset.resets_at,
        detailed_label: w.detailed_label.as_deref().map(str::to_string),
    }
}

/// SEC binding (D-49 + Claude's Discretion #7): emits Display only, never
/// Debug; anyhow cause chain stays hidden. `format_one_line` collapses any
/// embedded whitespace to single spaces per the Phase 1 sanitizer rule.
///
/// `kind` taxonomy is closed (every `ProviderError` variant maps to exactly
/// one snake_case string). Future `ProviderError` variants would extend this
/// match additively (D-52: new kinds are additive, consumers must tolerate
/// unknown kinds).
fn error_to_json(e: &ProviderError) -> JsonError {
    match e {
        ProviderError::Unconfigured => JsonError {
            kind: "unconfigured",
            message: format_one_line(&e.to_string()),
            missing: None,
            retry_after_seconds: None,
        },
        ProviderError::Unavailable { reason } => JsonError {
            kind: "unavailable",
            message: format_one_line(reason),
            missing: None,
            retry_after_seconds: None,
        },
        ProviderError::SchemaDrift { missing } => JsonError {
            kind: "schema_drift",
            message: format_one_line(&e.to_string()),
            missing: Some(missing.clone()),
            retry_after_seconds: None,
        },
        ProviderError::Network { source } => JsonError {
            kind: "network",
            message: format_one_line(&source.to_string()),
            missing: None,
            retry_after_seconds: None,
        },
        ProviderError::RateLimited { retry_after } => JsonError {
            kind: "rate_limited",
            message: format_one_line(&e.to_string()),
            missing: None,
            retry_after_seconds: retry_after.and_then(|s| {
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                s.total(jiff::Unit::Second)
                    .ok()
                    .map(|secs| secs.max(0.0) as u64)
            }),
        },
        ProviderError::Internal { source } => JsonError {
            kind: "internal",
            // anyhow's Display emits the top-level message ONLY; cause chain
            // (which could contain internal paths or secret-shaped strings)
            // is NOT expanded. D-49 binding + CONTEXT Claude's Discretion #7.
            message: format_one_line(&source.to_string()),
            missing: None,
            retry_after_seconds: None,
        },
    }
}

/// Drive the `--json` output format. Emits a single-line JSON envelope to
/// stdout (no pretty-printing — pipe-friendly default per CONTEXT deferred
/// ideas) and returns the `DispatchOutcome` for main.rs exit-code wiring.
///
/// `color_flag` is accepted for API symmetry with `run_compact` / `run_detailed`
/// but its value is silently ignored per D-58: this module emits zero ANSI
/// bytes (no `.green()` / `.red()` calls anywhere — that is the compile-time
/// guarantee that satisfies the D-58 binding, NOT a runtime call to
/// `should_colorize_env`). The parameter is accepted-and-discarded with a
/// leading underscore so the call sites in `cli/mod.rs` and `main.rs` can
/// pass `cli.color` uniformly.
///
/// # Errors
///
/// Returns `Err` if `serde_json::to_writer` fails (e.g. stdout pipe broken
/// mid-write) or the trailing newline `writeln!` fails. Both collapse to
/// `anyhow::Error` so `main.rs` can apply the exit-1 path uniformly.
pub async fn run_json(
    engine: &Engine,
    _color_flag: ColorMode,
) -> anyhow::Result<DispatchOutcome> {
    let now = jiff::Timestamp::now();
    let outcomes = engine.refresh_all(now).await;
    // D-66 + D-73: CLI cache is always empty → translate RowOutcome → Result
    // so to_json_root and DispatchOutcome stay byte-identical to Phase 2
    // (schema_version: 1 unchanged per D-68).
    let results: Vec<(ProviderId, Result<ProviderState, ProviderError>)> = outcomes
        .into_iter()
        .map(|(id, o)| (id, crate::cli::outcome_to_result(o)))
        .collect();

    // WR-06: the prior `let _color_ignored = tty::should_colorize_env(...)`
    // binding has been removed. `should_colorize_env(_, true)` is documented
    // to always return `false`, so calling it with `json_mode=true` was a
    // no-op whose return value was already discarded. The "documents the
    // contract at the call site" intent now lives in this comment (and the
    // fn's `# Errors` doc above): D-58 is satisfied by the structural fact
    // that NO `owo_colors` / ANSI emission exists in this module, not by a
    // runtime call.
    let root = to_json_root(&results, now);
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &root)
        .map_err(|e| anyhow::anyhow!("json emit: {e}"))?;
    writeln!(stdout).map_err(|e| anyhow::anyhow!("stdout newline: {e}"))?;

    Ok(DispatchOutcome::from_results(&results))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::model::{HpWindow, ResetInfo};
    use std::borrow::Cow;

    fn ts(s: &str) -> jiff::Timestamp {
        s.parse().unwrap()
    }

    // J1: empty results → schema_version=1, generated_at=now, providers=[].
    #[test]
    fn to_json_root_empty_results() {
        let now = ts("2026-05-25T13:45:22Z");
        let root = to_json_root(&[], now);
        assert_eq!(root.schema_version, 1);
        assert_eq!(root.generated_at, now);
        assert!(root.providers.is_empty());
    }

    // J2: Claude 2-window Ok → status="ok", source=Some, fetched_at=Some,
    // windows[0].label="claude" + detailed_label=Some("5h") +
    // percent_remaining=Some(60.0); windows[1].label="weekly" +
    // detailed_label=Some("weekly") + percent_remaining=None (NaN→None).
    #[test]
    fn to_json_root_claude_two_window_ok() {
        let now = ts("2026-05-25T13:45:22Z");
        let reset_5h = ts("2026-05-25T15:45:22Z");
        let reset_wk = ts("2026-05-29T20:00:00Z");
        let state = ProviderState {
            id: ProviderId::Claude,
            windows: vec![
                HpWindow {
                    label: Cow::Borrowed("claude"),
                    percent_remaining: 60.0,
                    reset: ResetInfo { resets_at: reset_5h },
                    bar_color: None,
                    detailed_label: Some(Cow::Borrowed("5h")),
                },
                HpWindow {
                    label: Cow::Borrowed("weekly"),
                    percent_remaining: f32::NAN,
                    reset: ResetInfo { resets_at: reset_wk },
                    bar_color: None,
                    detailed_label: Some(Cow::Borrowed("weekly")),
                },
            ],
            fetched_at: now,
            source: Cow::Borrowed("claude-jsonl"),
        };
        let results = vec![(ProviderId::Claude, Ok::<_, ProviderError>(state))];
        let root = to_json_root(&results, now);
        assert_eq!(root.providers.len(), 1);
        let p = &root.providers[0];
        assert_eq!(p.id, "claude");
        assert_eq!(p.status, "ok");
        assert_eq!(p.source.as_deref(), Some("claude-jsonl"));
        assert_eq!(p.fetched_at, Some(now));
        assert!(p.error.is_none());
        assert_eq!(p.windows.len(), 2);
        // windows[0]: 5h
        assert_eq!(p.windows[0].label, "claude");
        assert_eq!(p.windows[0].detailed_label.as_deref(), Some("5h"));
        assert_eq!(p.windows[0].percent_remaining, Some(60.0));
        assert_eq!(p.windows[0].reset_at, reset_5h);
        // windows[1]: weekly with NaN → None
        assert_eq!(p.windows[1].label, "weekly");
        assert_eq!(p.windows[1].detailed_label.as_deref(), Some("weekly"));
        assert!(p.windows[1].percent_remaining.is_none(), "NaN must map to None");
        assert_eq!(p.windows[1].reset_at, reset_wk);
    }

    // J3: Codex SchemaDrift Err → status="error", source/fetched_at=None,
    // windows=[], error.kind="schema_drift", error.missing=Some(...).
    #[test]
    fn to_json_root_codex_schema_drift_err() {
        let now = ts("2026-05-25T13:45:22Z");
        let err = ProviderError::SchemaDrift {
            missing: vec!["rate_limits".to_string()],
        };
        let results: Vec<(ProviderId, Result<ProviderState, ProviderError>)> =
            vec![(ProviderId::Codex, Err(err))];
        let root = to_json_root(&results, now);
        assert_eq!(root.providers.len(), 1);
        let p = &root.providers[0];
        assert_eq!(p.id, "codex");
        assert_eq!(p.status, "error");
        assert!(p.source.is_none());
        assert!(p.fetched_at.is_none());
        assert!(p.windows.is_empty());
        let e = p.error.as_ref().expect("error envelope present");
        assert_eq!(e.kind, "schema_drift");
        assert!(e.message.contains("schema drift"), "message: {}", e.message);
        assert!(e.message.contains("rate_limits"), "message: {}", e.message);
        assert_eq!(e.missing.as_deref(), Some(&["rate_limits".to_string()][..]));
        assert!(e.retry_after_seconds.is_none());
    }

    // J4: serde round-trip → top-level keys exactly {schema_version,
    // generated_at, providers}. No extras.
    #[test]
    fn to_json_root_serializes_with_exactly_top_level_keys() {
        let now = ts("2026-05-25T13:45:22Z");
        let root = to_json_root(&[], now);
        let json_str = serde_json::to_string(&root).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let obj = value.as_object().expect("root must be an object");
        let keys: std::collections::BTreeSet<&str> = obj.keys().map(String::as_str).collect();
        let expected: std::collections::BTreeSet<&str> =
            ["schema_version", "generated_at", "providers"].into_iter().collect();
        assert_eq!(
            keys, expected,
            "top-level keys must be exactly schema_version/generated_at/providers, got {keys:?}"
        );
    }

    // J5: Internal source with embedded newline → message is one-line via
    // format_one_line, Display only (no Debug bracket noise, no backtrace).
    #[test]
    fn error_to_json_internal_collapses_whitespace_and_uses_display() {
        let err = ProviderError::Internal {
            source: anyhow::anyhow!("boom\nlinetwo"),
        };
        let je = error_to_json(&err);
        assert_eq!(je.kind, "internal");
        // anyhow Display emits just the top-level message; format_one_line
        // collapses the embedded \n to a single space.
        assert_eq!(je.message, "boom linetwo", "message: {:?}", je.message);
        // SEC: no debug-bracket leak, no backtrace markers.
        assert!(!je.message.contains("Backtrace"), "leaked backtrace: {:?}", je.message);
        assert!(!je.message.contains("\n"), "newline not collapsed: {:?}", je.message);
    }

    // J6: RateLimited with Span::seconds(300) → retry_after_seconds = Some(300).
    #[test]
    fn error_to_json_rate_limited_emits_retry_after_seconds() {
        let span = jiff::Span::new().seconds(300);
        let err = ProviderError::RateLimited {
            retry_after: Some(span),
        };
        let je = error_to_json(&err);
        assert_eq!(je.kind, "rate_limited");
        assert_eq!(je.retry_after_seconds, Some(300));
        // None retry_after also handled cleanly:
        let err_none = ProviderError::RateLimited { retry_after: None };
        let je2 = error_to_json(&err_none);
        assert_eq!(je2.kind, "rate_limited");
        assert!(je2.retry_after_seconds.is_none());
    }

    // J7: ProviderId::Mock serializes as id="mock".
    #[test]
    fn mock_provider_serializes_as_id_mock() {
        let now = ts("2026-05-25T13:45:22Z");
        let state = ProviderState {
            id: ProviderId::Mock,
            windows: vec![HpWindow {
                label: Cow::Borrowed("mock-session"),
                percent_remaining: 60.0,
                reset: ResetInfo { resets_at: now },
                bar_color: None,
                detailed_label: None,
            }],
            fetched_at: now,
            source: Cow::Borrowed("mock"),
        };
        let results = vec![(ProviderId::Mock, Ok::<_, ProviderError>(state))];
        let root = to_json_root(&results, now);
        assert_eq!(root.providers.len(), 1);
        assert_eq!(root.providers[0].id, "mock");
        assert_eq!(root.providers[0].status, "ok");
    }

    // J8: detailed_label = None is omitted from serialized JSON; detailed_label
    // = Some(...) appears as `"detailed_label":"5h"`.
    #[test]
    fn detailed_label_omitted_when_none_and_present_when_some() {
        let now = ts("2026-05-25T13:45:22Z");

        // None branch: Codex primary
        let codex_state = ProviderState {
            id: ProviderId::Codex,
            windows: vec![HpWindow {
                label: Cow::Borrowed("primary"),
                percent_remaining: 90.0,
                reset: ResetInfo { resets_at: now },
                bar_color: None,
                detailed_label: None,
            }],
            fetched_at: now,
            source: Cow::Borrowed("codex-jsonl"),
        };
        let codex_results = vec![(ProviderId::Codex, Ok::<_, ProviderError>(codex_state))];
        let codex_root = to_json_root(&codex_results, now);
        let codex_json = serde_json::to_string(&codex_root).unwrap();
        assert!(
            !codex_json.contains("detailed_label"),
            "Codex primary window must NOT include detailed_label key, got: {codex_json}"
        );

        // Some branch: Claude 5h
        let claude_state = ProviderState {
            id: ProviderId::Claude,
            windows: vec![HpWindow {
                label: Cow::Borrowed("claude"),
                percent_remaining: 60.0,
                reset: ResetInfo { resets_at: now },
                bar_color: None,
                detailed_label: Some(Cow::Borrowed("5h")),
            }],
            fetched_at: now,
            source: Cow::Borrowed("claude-jsonl"),
        };
        let claude_results = vec![(ProviderId::Claude, Ok::<_, ProviderError>(claude_state))];
        let claude_root = to_json_root(&claude_results, now);
        let claude_json = serde_json::to_string(&claude_root).unwrap();
        assert!(
            claude_json.contains("\"detailed_label\":\"5h\""),
            "Claude 5h window must include detailed_label=5h, got: {claude_json}"
        );
    }
}
