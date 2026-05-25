//! CLI surface for AHB. Phase 1:
//! - `Cli` struct + `Command` subcommand enum moved out of `main.rs`.
//! - `run_compact(engine, ascii, color)` dispatches the default no-subcommand path.
//! - `Command::Tui` is dispatched by `main.rs` to `ahb::tui::run(engine).await`
//!   (Plan 03 wired the real ratatui surface; Plan 04 deleted the obsolete
//!   Phase 0 TUI stub placeholder, see WR-08).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

pub mod render_json;
pub mod render_text;
pub mod tty;

pub use tty::ColorMode;

use crate::engine::Engine;
use crate::model::ProviderId;

/// Phase 2 Plan 02-03 — D-59 exit-code discriminant. Each `run_*` dispatch fn
/// returns this from its `Ok` arm; `main.rs` calls `.exit_code()` and passes
/// the result to `std::process::exit`.
///
/// - `AnySuccess` (exit 0): ≥1 provider returned `Ok`, OR the engine had zero
///   providers enabled (CFG-04 special case — "not yet configured" is not an
///   error). `--help` documents the rule in the `after_help` block.
/// - `AllFailed` (exit 1): every provider returned `Err` (including
///   SchemaDrift — per D-60 SchemaDrift counts as fail, NOT degraded success;
///   `result.is_ok()` is the single discriminant).
///
/// Exit code 2 is owned by `main.rs` (config/secrets unloadable) and by clap
/// (`--compact --json` flag conflict via `ArgGroup`). Neither path reaches
/// `DispatchOutcome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    AnySuccess,
    AllFailed,
}

impl DispatchOutcome {
    /// Compute the outcome from the engine's `refresh_all` output. Empty
    /// results (zero providers enabled) collapse to `AnySuccess` per CFG-04 —
    /// running AHB with nothing configured is not a failure.
    #[must_use]
    pub fn from_results<T, E>(results: &[(ProviderId, Result<T, E>)]) -> Self {
        if results.is_empty() || results.iter().any(|(_, r)| r.is_ok()) {
            Self::AnySuccess
        } else {
            Self::AllFailed
        }
    }

    /// Map the outcome to a Unix exit code (0 = AnySuccess, 1 = AllFailed).
    #[must_use]
    pub fn exit_code(self) -> i32 {
        match self {
            Self::AnySuccess => 0,
            Self::AllFailed => 1,
        }
    }
}

/// AHB command-line surface.
///
/// Phase 2 Plan 02-03: the three output-format flags (`--compact`, `--detailed`,
/// `--json`) are bound into a single clap `ArgGroup` named `"format"` with
/// `multiple = false`. clap enforces the mutual-exclusion at parse time and
/// exits `2` with `"the argument '--X' cannot be used with '--Y'"` on stderr
/// (D-57 binding). Omitting all three falls through to the compact default
/// (byte-identical to Phase 1). The `after_help` block documents the D-59
/// exit-code grid (D-61 binding).
#[derive(clap::Parser, Debug)]
#[command(
    version,
    about = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance",
    after_help = "Exit codes:\n  0  at least one provider returned data (or no providers configured)\n  1  all configured providers failed\n  2  config / secrets unloadable, or invalid command-line usage",
    group(
        clap::ArgGroup::new("format")
            .required(false)
            .multiple(false)
            .args(["compact", "detailed", "json"]),
    ),
)]
pub struct Cli {
    /// Phase 2 CORE-02: force the compact one-line-per-provider view. Equivalent
    /// to no flag (Phase 1 default). Explicit form is grep-friendly for tmux /
    /// Starship integrations that want to assert "single-line output". Mutually
    /// exclusive with `--detailed` and `--json` (clap `ArgGroup`).
    #[arg(long)]
    pub compact: bool,

    /// Phase 2 D-53 / CORE-03: print a multi-line block per provider with header
    /// + indented per-window rows (Claude shows both 5h and weekly bars).
    /// Mutually exclusive with `--compact` and `--json` (clap `ArgGroup`).
    #[arg(long)]
    pub detailed: bool,

    /// Phase 2 CORE-04 / D-49..D-52: emit a single-line JSON envelope with
    /// `schema_version: 1`. ANSI styling and `--ascii` are silently ignored
    /// (D-58). Mutually exclusive with `--compact` and `--detailed` (clap
    /// `ArgGroup`).
    #[arg(long)]
    pub json: bool,

    /// Force ASCII charset (uses '#' / '-' instead of the U+2588 / U+2591 blocks).
    #[arg(long)]
    pub ascii: bool,

    /// Color mode. Auto-detects TTY + `NO_COLOR` by default; pass `never` / `always` to override.
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,

    /// D-43 integration tier: debug-build-only fake-secret emitter for
    /// `tests/secret_leak_subprocess.rs`. NOT compiled into release builds —
    /// SEC-03 covers `--json` emission paths in Phase 2 CORE-04. `hide = true`
    /// so `--help` does NOT advertise this flag even in debug builds.
    #[cfg(debug_assertions)]
    #[arg(long, hide = true)]
    pub debug_emit_fake_secret: bool,

    /// Optional subcommand. Default (no subcommand) prints the compact one-line view.
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Launch the fixed-frame TUI (Plan 03).
    Tui,
}

/// Render the compact default view. One line per enabled provider; empty-state pair
/// when no providers are configured (UI-SPEC CFG-04 + empty-state copy).
///
/// Color application uses `tty::should_colorize_env(cli_color, json_mode=false)`.
///
/// Returns `DispatchOutcome` for D-59 exit-code wiring: empty results map to
/// `AnySuccess` (CFG-04); otherwise the result depends on whether ≥1 provider
/// returned `Ok`.
///
/// # Errors
///
/// Currently infallible on the render path — typed as `anyhow::Result` for
/// future-compat with Phase 3+ adapters that may surface fatal failures.
pub async fn run_compact(
    engine: &Engine,
    ascii: bool,
    color_flag: ColorMode,
) -> anyhow::Result<DispatchOutcome> {
    let now = jiff::Timestamp::now();
    let results = engine.refresh_all(now).await;

    if results.is_empty() {
        println!("{}", render_text::EMPTY_STATE_HEADING);
        println!("{}", render_text::EMPTY_STATE_BODY);
        // CFG-04: zero providers enabled = exit 0 (not a failure).
        return Ok(DispatchOutcome::AnySuccess);
    }

    let color_on = tty::should_colorize_env(color_flag, false);
    for (id, result) in &results {
        match result {
            Ok(state) => {
                let line = render_text::compact_line_colored(state, &now, ascii, color_on);
                println!("{line}");
            }
            Err(err) => {
                // Plan 02: SchemaDrift renders the verbatim UI-SPEC sentinel (with
                // U+2592 cells + label via id_label(id)); other errors render the
                // Phase 0/Plan 01 `{label}  ERROR: {reason}` row.
                let line = render_text::format_error_row_colored(*id, err, ascii, color_on);
                println!("{line}");
            }
        }
    }
    Ok(DispatchOutcome::from_results(&results))
}

/// Render the Phase 2 `--detailed` view (D-53 / CORE-03). Per provider: one
/// header line (`id_label`) followed by one indented row per `HpWindow`
/// (2-space indent, shared compact-mode bar styling per D-56). Provider blocks
/// are separated by a single blank line; the last block does NOT trail an
/// empty line (CONTEXT D-53 specifics line 341 — "末行不額外空行").
///
/// `--ascii` (from `cli.ascii`) switches the glyphs per D-58 (silently honored).
/// `--color=never` short-circuits `should_colorize_env` to `false`, producing
/// zero ANSI bytes in stdout.
///
/// # Errors
///
/// Currently infallible on the render path — typed as `anyhow::Result` for
/// future-compat with Phase 3+ adapters that may surface fatal failures.
///
/// Returns `DispatchOutcome` for D-59 exit-code wiring (same rules as
/// `run_compact`).
pub async fn run_detailed(
    engine: &Engine,
    ascii: bool,
    color_flag: ColorMode,
) -> anyhow::Result<DispatchOutcome> {
    let now = jiff::Timestamp::now();
    let results = engine.refresh_all(now).await;

    if results.is_empty() {
        // Empty-state mirrors compact (Phase 1 LOCKED literal — shared with
        // `run_compact`). Detailed and compact agree here so a tmux user
        // switching modes doesn't see different empty-state copy.
        println!("{}", render_text::EMPTY_STATE_HEADING);
        println!("{}", render_text::EMPTY_STATE_BODY);
        // CFG-04: zero providers enabled = exit 0.
        return Ok(DispatchOutcome::AnySuccess);
    }

    let color_on = tty::should_colorize_env(color_flag, false);
    let last_idx = results.len() - 1;
    for (i, (id, result)) in results.iter().enumerate() {
        match result {
            Ok(state) => {
                println!(
                    "{}",
                    render_text::detailed_block(state, &now, ascii, color_on)
                );
            }
            Err(err) => {
                // Detailed error rendering per D-53: header line + 1 indented
                // row reusing the existing format_error_row_colored sentinel
                // (covers SchemaDrift `▒▒▒▒▒▒▒▒▒▒ ??% • {Label} adapter…`
                // AND Unavailable / Network / etc. `{label}  ERROR: …`).
                let row = render_text::format_error_row_colored(*id, err, ascii, color_on);
                println!("{}", render_text::id_label(*id));
                println!("  {row}");
            }
        }
        // Provider separator: blank line between blocks, NONE after the last.
        if i < last_idx {
            println!();
        }
    }
    Ok(DispatchOutcome::from_results(&results))
}

/// D-43 + D-62 integration tier. Emits a JSON envelope containing a
/// `Secret<String>` whose inner value is the high-entropy fixture
/// `deadbeefcafe1234567890abcdef`, then exits with code 0.
///
/// `as_json` selects which envelope shape is exercised:
///
/// - `false` — the Plan 02 emission shape `{"fake_secret":"[REDACTED]"}`
///   (byte-identical to Phase 1 behavior; `subprocess_secret_does_not_leak`
///   continues to assert this path).
/// - `true` — a JsonRoot-shaped envelope `{"schema_version":1,"fake_secret_in_label":"[REDACTED]"}`
///   that exercises the same `Serialize` path the production `run_json` driver
///   uses, proving `Secret<T>::Serialize → "[REDACTED]"` holds when emitted
///   through the `--json` route (SEC-03 / D-62).
///
/// `tests/secret_leak_subprocess.rs` invokes the binary with
/// `--debug-emit-fake-secret` for the non-JSON path and
/// `--json --debug-emit-fake-secret` for the JSON path; both assert:
/// (a) the literal fixture is absent from stdout, (b) no 20-char alphanumeric
/// run is present, (c) the `[REDACTED]` marker IS present (proves the
/// `Serialize` path ran).
///
/// `#[cfg(debug_assertions)]` so release builds (cargo-dist) literally cannot
/// compile the function in. The companion `Cli::debug_emit_fake_secret` field
/// on this module's `Cli` struct is also gated.
///
/// # Panics
///
/// Panics if `serde_json::to_writer` fails to serialize the redacted
/// `Secret<String>` envelope, or if `writeln!` fails on stdout. A failure here
/// would mean `Serialize for Secret<T>` is broken — exactly what
/// `tests/secret_leak_subprocess.rs` is designed to surface. Test failure is
/// the intended outcome rather than silent fall-through.
#[cfg(debug_assertions)]
pub fn debug_emit_fake_secret_and_exit(as_json: bool) -> ! {
    use std::io::Write;
    let s = crate::secrets::Secret::new("deadbeefcafe1234567890abcdef".to_string());
    let mut stdout = std::io::stdout().lock();
    #[allow(clippy::unwrap_used)] // debug-only fixture emitter; bug here = test failure
    {
        if as_json {
            // D-62 SEC-03 extension: exercise the same Serialize path the
            // production --json driver uses. The envelope mimics JsonRoot's
            // top-level schema_version field so a future grep for
            // `"schema_version":1` on stdout would match either path.
            #[derive(serde::Serialize)]
            struct DebugJsonEnvelope<'a> {
                schema_version: u8,
                fake_secret_in_label: &'a crate::secrets::Secret<String>,
            }
            let env = DebugJsonEnvelope {
                schema_version: 1,
                fake_secret_in_label: &s,
            };
            serde_json::to_writer(&mut stdout, &env).unwrap();
            writeln!(stdout).unwrap();
        } else {
            // Plan 02 emission shape — preserved byte-identical so the
            // existing `subprocess_secret_does_not_leak` test stays green.
            #[derive(serde::Serialize)]
            struct DebugEnvelope<'a> {
                fake_secret: &'a crate::secrets::Secret<String>,
            }
            let envelope = DebugEnvelope { fake_secret: &s };
            serde_json::to_writer(&mut stdout, &envelope).unwrap();
            writeln!(stdout).unwrap();
        }
    }
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ProviderConfig, Providers};
    use crate::secrets::Secrets;

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn run_compact_with_empty_engine_prints_empty_state() {
        // No way to assert stdout here without capturing — just confirm it runs OK
        // (integration tests cover stdout shape).
        let engine = Engine::new(Config::default(), Secrets::default());
        let result = run_compact(&engine, false, ColorMode::Never).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn run_compact_with_mock_provider_succeeds() {
        let cfg = Config {
            providers: Providers {
                mock: ProviderConfig { enabled: true },
                ..Default::default()
            },
        };
        let engine = Engine::new(cfg, Secrets::default());
        let result = run_compact(&engine, false, ColorMode::Never).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn run_detailed_with_empty_engine_prints_empty_state() {
        // Same shape as `run_compact_with_empty_engine_prints_empty_state` — confirms
        // the detailed dispatch path is wired and infallible.
        let engine = Engine::new(Config::default(), Secrets::default());
        let result = run_detailed(&engine, false, ColorMode::Never).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn run_detailed_with_mock_provider_succeeds() {
        let cfg = Config {
            providers: Providers {
                mock: ProviderConfig { enabled: true },
                ..Default::default()
            },
        };
        let engine = Engine::new(cfg, Secrets::default());
        let result = run_detailed(&engine, false, ColorMode::Never).await;
        assert!(result.is_ok());
    }

    // Phase 2 Plan 02-03 — DispatchOutcome unit tests (D-59 mapping).

    #[test]
    fn dispatch_outcome_empty_is_any_success() {
        // CFG-04: zero providers enabled → AnySuccess (exit 0).
        let results: Vec<(ProviderId, Result<(), ()>)> = Vec::new();
        assert_eq!(
            DispatchOutcome::from_results(&results),
            DispatchOutcome::AnySuccess
        );
    }

    #[test]
    fn dispatch_outcome_all_err_is_all_failed() {
        let results: Vec<(ProviderId, Result<(), &str>)> = vec![
            (ProviderId::Claude, Err("boom")),
            (ProviderId::Codex, Err("nope")),
        ];
        assert_eq!(
            DispatchOutcome::from_results(&results),
            DispatchOutcome::AllFailed
        );
    }

    #[test]
    fn dispatch_outcome_any_ok_is_any_success() {
        let results: Vec<(ProviderId, Result<(), &str>)> = vec![
            (ProviderId::Claude, Ok(())),
            (ProviderId::Codex, Err("nope")),
        ];
        assert_eq!(
            DispatchOutcome::from_results(&results),
            DispatchOutcome::AnySuccess
        );
    }

    #[test]
    fn dispatch_outcome_exit_code_mapping() {
        assert_eq!(DispatchOutcome::AnySuccess.exit_code(), 0);
        assert_eq!(DispatchOutcome::AllFailed.exit_code(), 1);
    }
}
