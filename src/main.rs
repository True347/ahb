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
use ahb::secrets::{self, InitOutcome};

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

    // Plan 02 BLOCKER #1 (D-43 path-b): hidden debug-build-only fake-secret emitter.
    // Dispatched BEFORE secrets::init() / config loading so the subprocess test can run
    // on backend-less CI runners without needing a keyring.
    #[cfg(debug_assertions)]
    if cli.debug_emit_fake_secret {
        ahb::cli::debug_emit_fake_secret_and_exit();
    }

    let config_path = config::default_path()?;
    let cfg = match config::load_or_init(&config_path)? {
        LoadOutcome::Initialized(_) => {
            // D-37: load_or_init already printed `initialized {} — enable providers and rerun`.
            // Exit cleanly so the user can edit the freshly-written config.
            return Ok(());
        }
        LoadOutcome::Loaded(c) => c,
    };

    // D-41 hard-error path: keyring backend unavailable → exit 2 with the verbatim
    // UI-SPEC literal. NEVER silently fall back to a file backend (STACK.md binding).
    let secrets = match secrets::init()? {
        InitOutcome::Ready(s) => s,
        InitOutcome::Unavailable => {
            // TODO(future-phase): the [secrets].storage = "file" escape-hatch is
            // documented intent but not yet wired in Config. The message below
            // preserves the documented contract; a future plan must extend Config
            // with a [secrets] table + secrets::init to honor it. See 01-REVIEW.md
            // WR-06 disposition (a).
            //
            // WR-06 fix: cross-OS path resolution — directories::ProjectDirs picks
            // ~/.config on Linux, ~/Library/Application Support on macOS,
            // %APPDATA% on Windows. NEVER unwrap — degrade gracefully to the
            // literal fallback if resolution fails on exotic platforms.
            let cfg_path_display = config::default_path().ok().map_or_else(
                || "your AHB config file".to_string(),
                |p| p.display().to_string(),
            );
            eprintln!(
                "no secret store available on this system; set [secrets].storage = \"file\" in {cfg_path_display} to opt into 0600 file storage"
            );
            std::process::exit(2);
        }
    };

    let engine = Engine::new(cfg, secrets);

    match cli.command {
        Some(Command::Tui) => ahb::tui::run(engine).await,
        None => {
            // Phase 2 D-53 / CORE-03: `--detailed` selects the multi-line
            // per-provider block view. Plan 02-03 will introduce the full
            // `--compact / --detailed / --json` clap ArgGroup; in Plan 02-02
            // the flag stands alone and the no-flag default stays compact
            // (byte-identical to Phase 1).
            if cli.detailed {
                ahb::cli::run_detailed(&engine, cli.ascii, cli.color).await
            } else {
                ahb::cli::run_compact(&engine, cli.ascii, cli.color).await
            }
        }
    }
}
