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

use ai_hp_bar::cli::{Cli, Command};
use ai_hp_bar::config::{self, LoadOutcome};
use ai_hp_bar::engine::Engine;
use ai_hp_bar::secrets::{self, InitOutcome};

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

    // Plan 02 BLOCKER #1 (D-43 path-b) + Plan 02-03 D-62 SEC-03 extension:
    // hidden debug-build-only fake-secret emitter. Dispatched BEFORE secrets::init()
    // / config loading so the subprocess test can run on backend-less CI runners
    // without needing a keyring. `cli.json` selects which envelope shape is exercised
    // (Plan 02 shape when false; JsonRoot-shaped envelope when true).
    #[cfg(debug_assertions)]
    if cli.debug_emit_fake_secret {
        ai_hp_bar::cli::debug_emit_fake_secret_and_exit(cli.json);
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

    // Plan 02-03 D-59 exit-code wiring: each `run_*` dispatch fn returns a
    // `DispatchOutcome`; we map it to a Unix exit code and call
    // `std::process::exit`. TUI is unconditional exit-0 (it has its own
    // exit semantics). The clap `ArgGroup` on `Cli` rejects flag conflicts
    // with exit-2 BEFORE this code runs; config / secrets unloadable also
    // exits with 2 above (lines 53-87), so this dispatch sees only the
    // 0/1 paths.
    let outcome = match cli.command {
        Some(Command::Tui) => {
            ai_hp_bar::tui::run(engine).await?;
            // TUI doesn't gate exit code on the provider grid — explicit 0.
            return Ok(());
        }
        None => {
            // `--compact` falls through to the default `run_compact` branch
            // (semantically equivalent to no flag — D-57 + CORE-02). The clap
            // ArgGroup guarantees at most one of compact/detailed/json is set.
            if cli.json {
                ai_hp_bar::cli::render_json::run_json(&engine, cli.color).await?
            } else if cli.detailed {
                ai_hp_bar::cli::run_detailed(&engine, cli.ascii, cli.color).await?
            } else {
                ai_hp_bar::cli::run_compact(&engine, cli.ascii, cli.color).await?
            }
        }
    };
    std::process::exit(outcome.exit_code());
}
