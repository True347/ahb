//! AHB — AI HP Bar — Phase 0 binary entry point.
//!
//! Wires the runtime spine end-to-end:
//!   panic-hook -> CLI parse -> `MockProvider` -> `render_text::compact_line` -> stdout.
//!
//! Phase 1 (TUI) will compose `ratatui::init()`'s panic hook over the Phase 0
//! hook installed below. The order documented in `install_phase0_panic_hook`
//! is contractual — do not move the call site (CONTEXT D-27 + RESEARCH Pitfall 5).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use clap::Parser;

use ahb::cli::render_text;
use ahb::provider::mock::MockProvider;
use ahb::provider::{FetchCtx, Provider};
use ahb::secrets::Secrets;

/// AHB command-line surface. Phase 0 honors D-17 (`--color`) and D-18 (`--ascii`);
/// `--color` is parsed but not yet applied (Phase 1 wires render coloring).
#[derive(Parser)]
#[command(
    version,
    about = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance"
)]
struct Cli {
    /// Force ASCII charset (uses '#' / '-' instead of the U+2588 / U+2591 blocks)
    #[arg(long)]
    ascii: bool,

    /// Color mode (Phase 0 accepts but does not yet apply — Phase 1 wires render coloring).
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    color: ColorMode,
}

#[derive(Copy, Clone, clap::ValueEnum)]
enum ColorMode {
    Auto,
    Always,
    Never,
}

/// Phase 0 panic hook. Composes via `take_hook()` + `set_hook()` so Phase 1's
/// `ratatui::init()` can wrap it (ratatui takes the hook AFTER we install ours
/// and chains: terminal-restore -> our stderr-print -> default). Order matters
/// — see docs.rs/ratatui/latest/ratatui/fn.init.html (RESEARCH Pitfall 5).
fn install_phase0_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("ahb panicked: {info}");
        original(info);
    }));
}

#[tokio::main(flavor = "current_thread")]
#[allow(clippy::default_constructed_unit_structs)]
async fn main() -> anyhow::Result<()> {
    // MUST be first: installs before any provider code runs so Phase 1 can wrap.
    install_phase0_panic_hook();

    let cli = Cli::parse();
    // `cli.color` is intentionally accepted but unused in Phase 0; silence the
    // unused-binding without a project-wide allow.
    let _ = cli.color;

    let secrets = Secrets::default();
    let now = jiff::Timestamp::now();
    let ctx = FetchCtx {
        now,
        secrets: &secrets,
    };

    // The bar value MUST flow through the Provider trait, never a hardcoded println.
    // This proves the Phase 0 spine end-to-end (CONTEXT specifics third bullet).
    let mock = MockProvider;
    let state = mock.fetch(&ctx).await
        .map_err(|e| anyhow::anyhow!("mock provider failed: {e}"))?;

    let line = render_text::compact_line(&state, &ctx.now, cli.ascii);
    println!("{line}");
    Ok(())
}
