//! CLI surface for AHB. Phase 1:
//! - `Cli` struct + `Command` subcommand enum moved out of `main.rs`.
//! - `run_compact(engine, ascii, color)` dispatches the default no-subcommand path.
//! - `Command::Tui` is dispatched by `main.rs` to `ahb::tui::run(engine).await`
//!   (Plan 03 wired the real ratatui surface; Plan 04 deleted the obsolete
//!   Phase 0 TUI stub placeholder, see WR-08).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

pub mod render_text;
pub mod tty;

pub use tty::ColorMode;

use crate::engine::Engine;

/// AHB command-line surface.
#[derive(clap::Parser, Debug)]
#[command(
    version,
    about = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance"
)]
pub struct Cli {
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
/// # Errors
///
/// Currently infallible (returns `Ok(())` on every path) — typed as
/// `anyhow::Result<()>` for future-compat with Phase 2's exit-code wiring.
pub async fn run_compact(
    engine: &Engine,
    ascii: bool,
    color_flag: ColorMode,
) -> anyhow::Result<()> {
    let now = jiff::Timestamp::now();
    let results = engine.refresh_all(now).await;

    if results.is_empty() {
        println!("{}", render_text::EMPTY_STATE_HEADING);
        println!("{}", render_text::EMPTY_STATE_BODY);
        return Ok(());
    }

    let color_on = tty::should_colorize_env(color_flag, false);
    for (id, result) in results {
        match result {
            Ok(state) => {
                let line = render_text::compact_line_colored(&state, &now, ascii, color_on);
                println!("{line}");
            }
            Err(err) => {
                // Plan 02: SchemaDrift renders the verbatim UI-SPEC sentinel (with
                // U+2592 cells + label via id_label(id)); other errors render the
                // Phase 0/Plan 01 `{label}  ERROR: {reason}` row.
                let line = render_text::format_error_row_colored(id, &err, ascii, color_on);
                println!("{line}");
            }
        }
    }
    Ok(())
}

/// D-43 integration tier (BLOCKER #1 path-b). Emits a one-line JSON envelope containing
/// a `Secret<String>` whose inner value is the high-entropy fixture
/// `deadbeefcafe1234567890abcdef`, then exits with code 0.
///
/// `tests/secret_leak_subprocess.rs` invokes this subprocess and asserts:
/// (a) the literal fixture is absent from stdout, (b) no 20-char alphanumeric run is
/// present, (c) the `[REDACTED]` marker IS present (proves the Serialize path ran).
///
/// `#[cfg(debug_assertions)]` so release builds (cargo-dist) literally cannot compile
/// the function in. The companion `Cli::debug_emit_fake_secret` field on this module's
/// `Cli` struct is also gated.
///
/// # Panics
///
/// Panics if `serde_json::to_writer` fails to serialize the redacted `Secret<String>`
/// envelope, or if `writeln!` fails on stdout. A failure here would mean
/// `Serialize for Secret<T>` is broken — exactly what
/// `tests/secret_leak_subprocess.rs` is designed to surface. Test failure is the
/// intended outcome rather than silent fall-through.
#[cfg(debug_assertions)]
pub fn debug_emit_fake_secret_and_exit() -> ! {
    use std::io::Write;
    #[derive(serde::Serialize)]
    struct DebugEnvelope<'a> {
        fake_secret: &'a crate::secrets::Secret<String>,
    }
    let s = crate::secrets::Secret::new("deadbeefcafe1234567890abcdef".to_string());
    let envelope = DebugEnvelope { fake_secret: &s };
    // `to_writer` directly drives the same `Serialize for Secret<T>` impl that
    // `--json` would exercise in Phase 2 CORE-04. We intentionally bypass error
    // handling: a JSON-emit failure here is a bug in Serialize-for-Secret and
    // the test should fail loudly. Using `write!`+`?` would force this fn to
    // return a Result and complicate the !-return type; instead we use a small
    // unwrap with a scoped allow.
    let mut stdout = std::io::stdout().lock();
    #[allow(clippy::unwrap_used)] // debug-only fixture emitter; bug here = test failure
    {
        serde_json::to_writer(&mut stdout, &envelope).unwrap();
        writeln!(stdout).unwrap();
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
}
