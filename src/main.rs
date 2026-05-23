//! AHB — AI HP Bar — Phase 1 binary entry point.
//!
//! Wires the spine end-to-end:
//!   panic-hook -> tracing init -> CLI parse -> config `load_or_init` -> Engine -> dispatch.
//!
//! The first-line `install_phase0_panic_hook()` is contractual (D-27 + RESEARCH
//! Pitfall 5 / L7). Plan 03's TUI will wrap (not replace) it via `ratatui::run`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use clap::Parser;

use ahb::cli::{Cli, Command};
use ahb::config::{self, LoadOutcome};
use ahb::engine::Engine;
use ahb::secrets::Secrets;

/// Phase 0 panic hook. Composes via `take_hook()` + `set_hook()` so Phase 1's
/// `ratatui::run` (Plan 03) can wrap it (ratatui takes the hook AFTER we install ours
/// and chains: terminal-restore -> our stderr-print -> default). Order matters
/// — see docs.rs/ratatui/latest/ratatui/fn.init.html (RESEARCH Pitfall 5).
fn install_phase0_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("ahb panicked: {info}");
        original(info);
    }));
}

#[tokio::main]
#[allow(clippy::default_constructed_unit_structs)]
async fn main() -> anyhow::Result<()> {
    // MUST be first: installs before any provider code runs so Plan 03 can wrap.
    install_phase0_panic_hook();

    // Initialize tracing (RESEARCH Pitfall L7: panic hook uses eprintln! so no actual
    // race, but the canonical order keeps it future-proof).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let cli = Cli::parse();

    let config_path = config::default_path()?;
    let cfg = match config::load_or_init(&config_path)? {
        LoadOutcome::Initialized(_) => {
            // D-37: load_or_init already printed `initialized {} — enable providers and rerun`.
            // Exit cleanly so the user can edit the freshly-written config.
            return Ok(());
        }
        LoadOutcome::Loaded(c) => c,
    };

    // Phase 1 Secrets stub — Plan 02 will replace with `ahb::secrets::init()?`.
    let secrets = Secrets::default();

    let engine = Engine::new(cfg, secrets);

    match cli.command {
        None => ahb::cli::run_compact(&engine, cli.ascii, cli.color).await,
        Some(Command::Tui) => ahb::cli::run_tui_stub(),
    }
}
