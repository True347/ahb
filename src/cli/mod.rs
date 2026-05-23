//! CLI surface for AHB. Phase 1 (Task 3):
//! - `Cli` struct + `Command` subcommand enum moved out of `main.rs`.
//! - `run_compact(engine, ascii, color)` dispatches the default no-subcommand path.
//! - TUI subcommand is reserved for Plan 03; current stub returns an error.

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
                let line = render_text::format_error_row(id, &err, ascii);
                println!("{line}");
            }
        }
    }
    Ok(())
}

/// Stub for `AHB tui` until Plan 03 wires the real ratatui surface.
///
/// # Errors
///
/// Always returns an error in Phase 1 — Plan 03 replaces with the real TUI loop.
pub fn run_tui_stub() -> anyhow::Result<()> {
    Err(anyhow::anyhow!("AHB tui will be wired in Plan 03"))
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
