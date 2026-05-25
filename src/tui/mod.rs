//! TUI surface for AHB (REQ TUI-01/02/04/05).
//!
//! Phase 1 Plan 03 — `AHB tui` opens a fixed full-screen ratatui frame showing one row
//! per enabled provider, with the same HP bar + countdown layout as the CLI compact line.
//!
//! ## ratatui 0.30 signature (Context7-verified — WARNING #4 resolved)
//!
//! `ratatui::run` is SYNC: `pub fn run<F, R>(f: F) -> R where F: FnOnce(&mut DefaultTerminal) -> R`.
//! `init()` (called inside `run()`) installs a panic hook via `set_panic_hook()` that
//! restores the terminal BEFORE the previously-installed hook runs (chain: ratatui restore
//! → Phase 0 stderr-print → default). The Phase 0 hook was installed FIRST in `main.rs`,
//! so `ratatui::run()` wraps cleanly on top.
//!
//! `set_panic_hook` here is ratatui's internal function — same call path as
//! `try_init`.
//!
//! ## sync-closure / async-loop bridge
//!
//! Because `ratatui::run` is sync but our event loop is async (tokio EventStream, time
//! intervals, engine.refresh_all), we use the canonical bridge:
//!
//! 1. `spawn_blocking` moves the sync work to the blocking-thread pool so `block_on`
//!    cannot deadlock the runtime's task threads.
//! 2. Inside the sync closure, `Handle::current().block_on(async_loop)` drives the
//!    select! loop.
//!
//! Forbidden alternative: the `ratatui::init` + `restore` manual-pair pattern
//! (Pitfall L2 + grep gate). `ratatui::run` is the only entry path used here.
//!
//! ## TUI-05 non-TTY refusal
//!
//! `std::io::IsTerminal::is_terminal` gates the spawn_blocking call. When stdout is not
//! a TTY (e.g. `AHB tui | cat`), we emit the verbatim UI-SPEC literal to stderr and exit
//! with code 2. This avoids the situation where ratatui's alt-screen escape sequences
//! would land in a non-terminal pipe.
//!
//! ## Wall-clock authorization
//!
//! Per Plan 01 the rule is "only main.rs calls `Timestamp::now()`". TUI is structurally
//! main-adjacent (the entry-point to a long-running surface), so `tui_loop` is the
//! second authorized callsite. The `src/provider/` tree continues to be grep-free of
//! `Timestamp::now`.
//!
//! Plan 04 extension (BL-01 fix): the render-tick arm in `tui_loop` is the SINGLE
//! authorized wall-clock site in the TUI render path. The leaf renderer
//! (`widgets::hp_row::build_ok_line`) MUST receive `now: &jiff::Timestamp` as a
//! parameter rather than calling `jiff::Timestamp::now()` itself. `src/tui/widgets/`
//! is now grep-forbidden (mirror of `src/provider/` rule), enforced by
//! `tests/no_walltime_in_adapter.rs`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)] // module-level prose mentions Timestamp / IsTerminal / etc. in narrative form

use std::io::IsTerminal;
use std::time::Duration;

use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::EventStream;

use crate::engine::Engine;

pub mod app;
pub mod ui;
pub mod widgets;

/// Entry point for `AHB tui`. Refuses non-TTY stdout (TUI-05), then drives the ratatui
/// fixed-view loop with two ticks: fetch every 15s (D-30 / TUI-02) and render every 1s
/// (D-31 countdown re-render cadence). Quits cleanly on `q` or `Ctrl-C` (UI-SPEC
/// interaction table). Panic-safe terminal restore is provided by ratatui's
/// auto-installed panic hook composing over Phase 0's hook (verified by
/// `tests/tui_panic_safe_restore.rs`, WARNING #6).
///
/// # Errors
///
/// Returns the underlying tokio JoinError or anyhow error from the sync closure if the
/// blocking task itself fails to complete. The TUI loop's own errors (drawing, event
/// stream) are propagated through `?`.
pub async fn run(engine: Engine) -> anyhow::Result<()> {
    // TUI-05: refuse non-TTY stdout with the verbatim UI-SPEC literal + exit 2.
    if !std::io::stdout().is_terminal() {
        eprintln!(
            "AHB tui requires a terminal (stdout is not a TTY). Run AHB without 'tui' for piped / non-interactive output."
        );
        std::process::exit(2);
    }

    // spawn_blocking + Handle::current().block_on bridge — see module doc. The sync
    // closure is `ratatui::run(|terminal| handle.block_on(async_loop))`. spawn_blocking
    // moves us off the tokio task-thread pool so the inner block_on cannot starve.
    let handle = tokio::runtime::Handle::current();
    let join: tokio::task::JoinHandle<anyhow::Result<()>> = tokio::task::spawn_blocking(move || {
        // `ratatui::run` returns whatever the closure returns; we propagate the inner
        // `anyhow::Result<()>` out of both layers.
        ratatui::run(|terminal: &mut DefaultTerminal| -> anyhow::Result<()> {
            handle.block_on(async move { tui_loop(terminal, engine).await })
        })
    });
    join.await??;
    Ok(())
}

/// The async event-loop body. Drives three concurrent sources via `tokio::select!`:
/// terminal events (q/Ctrl-C → quit), a 15s fetch tick (refreshes engine), and a 1s
/// render tick (re-draws cached state so the countdown updates).
async fn tui_loop(terminal: &mut DefaultTerminal, engine: Engine) -> anyhow::Result<()> {
    let mut app = app::AppState::new(jiff::Timestamp::now());

    // Prime the cache so the first frame is not empty. The 1s render tick fires shortly
    // after — the user sees real data immediately. Plan 03-03 wired
    // AppState::apply_results to consume RowOutcome directly (RowOutcome::Stale →
    // RowState::StaleOk yellow row + (stale Ns ago) suffix per D-69).
    let outcomes = engine.refresh_all(jiff::Timestamp::now()).await;
    app.apply_results(outcomes);
    // IN-01 fix: refresh app.now immediately before the priming draw so the first
    // rendered frame is not seeded with the pre-prime timestamp (the prime fetch can
    // take up to DEFAULT_PER_PROVIDER_TIMEOUT). The render-tick arm below already
    // refreshes per cycle — this closes the priming-frame gap only and keeps the
    // BL-01 contract intact (the render path is still the SINGLE authorized
    // wall-clock site in the TUI).
    app.now = jiff::Timestamp::now();
    terminal.draw(|f| ui::draw(f, &app))?;

    let mut events = EventStream::new();
    let mut fetch_tick = tokio::time::interval(Duration::from_secs(15));
    // First tick fires immediately for any `interval` — we already primed above, so skip.
    fetch_tick.tick().await;
    let mut render_tick = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            maybe_ev = events.next() => {
                match maybe_ev {
                    Some(Ok(ev)) => {
                        if app.handle_event(&ev) {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::warn!("event stream error: {e}");
                    }
                    None => break,
                }
            }
            _ = fetch_tick.tick() => {
                // Plan 03-03: RowOutcome passes straight through to apply_results;
                // the translator inside AppState routes Fresh → Ok, Stale → StaleOk
                // (yellow row + "(stale Ns ago)" suffix per D-69), Failed → Err /
                // SchemaDrift. D-74 invariant: this `tokio::time::interval(15s)`
                // is the global tick — per-provider rate limiting is handled inside
                // Engine::refresh_all via the refresh_intervals map (Plan 03-01).
                let outcomes = engine.refresh_all(jiff::Timestamp::now()).await;
                app.apply_results(outcomes);
            }
            _ = render_tick.tick() => {
                // BL-01: render-tick arm is the SINGLE authorized wall-clock site in
                // the TUI render path. Update `app.now` immediately before the draw
                // so the leaf widget (`hp_row::build_ok_line`) sees a fresh snapshot
                // via `&app.now` rather than calling `Timestamp::now()` itself.
                app.now = jiff::Timestamp::now();
                terminal.draw(|f| ui::draw(f, &app))?;
            }
        }
    }
    Ok(())
}
