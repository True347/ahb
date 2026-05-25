//! TUI application state cache (REQ TUI-01/02).
//!
//! `AppState` owns the cached `Vec<RowState>` rendered by `ui::draw` each render tick.
//! `apply_results` translates the engine's `Vec<(ProviderId, Result<ProviderState,
//! ProviderError>)>` into per-row state; `handle_event` decides whether the input
//! breaks the event loop (q / Ctrl-C → true).
//!
//! `RowState` carries `SchemaDrift` as a distinct variant (not flattened into `Err`)
//! so `ui::draw` can paint the verbatim UI-SPEC sentinel without re-deriving from
//! `ProviderError` — mirrors Plan 02's `format_error_row_colored` SchemaDrift special
//! case.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)]

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::model::{ProviderError, ProviderId, ProviderState};

/// One UI row, derived from one provider's `Result<ProviderState, ProviderError>`.
#[derive(Debug)]
pub enum RowState {
    /// Healthy row — render the HP bar.
    Ok(ProviderState),
    /// Schema-drift detected — render the verbatim UI-SPEC sentinel using `id_label(id)`.
    /// Distinguished from `Err` so the renderer can match cheaply (WARNING #5 path).
    SchemaDrift { id: ProviderId },
    /// Other adapter error — render `{label}  ERROR: {message}` (UI-SPEC LOCKED).
    Err { id: ProviderId, message: String },
}

/// Cached TUI state. Updated by `apply_results` once per fetch tick; consumed by
/// `ui::draw` once per render tick.
///
/// `now` is the snapshot of the wall clock at the most-recent render tick. It is
/// updated by `tui_loop` ONLY; the renderer must NEVER call `jiff::Timestamp::now()`
/// — clock-injection rule (BL-01 fix). `tests/no_walltime_in_adapter.rs` grep-rejects
/// any `Timestamp::now` call under `src/tui/widgets/` to enforce this.
#[derive(Debug)]
pub struct AppState {
    pub rows: Vec<RowState>,
    /// Snapshot of wall clock at the most-recent render tick. Updated by tui_loop
    /// ONLY; the renderer must NEVER call jiff::Timestamp::now() — clock-injection
    /// rule (BL-01 fix).
    pub now: jiff::Timestamp,
}

impl AppState {
    /// Build a fresh empty state seeded with the provided `now` snapshot. The TUI
    /// `tui_loop` then keeps `now` up-to-date before each render tick (the single
    /// authorized wall-clock site in the render path).
    #[must_use]
    pub fn new(now: jiff::Timestamp) -> Self {
        Self {
            rows: Vec::new(),
            now,
        }
    }

    /// Replace the cached rows from one engine fetch. Phase 1 has at most 3 providers,
    /// so the full replacement is cheaper than diffing.
    pub fn apply_results(
        &mut self,
        results: Vec<(ProviderId, Result<ProviderState, ProviderError>)>,
    ) {
        self.rows = results
            .into_iter()
            .map(|(id, result)| match result {
                Ok(state) => RowState::Ok(state),
                Err(ProviderError::SchemaDrift { .. }) => RowState::SchemaDrift { id },
                Err(other) => RowState::Err {
                    id,
                    message: other.to_string(),
                },
            })
            .collect();
    }

    /// Handle one terminal event. Returns `true` if the event signals quit
    /// (`q` with any modifier, or `Ctrl-C`); `false` otherwise. Takes `&Event` —
    /// we never need to consume it (no fields are moved out), and crossterm's
    /// `EventStream` yields owned values that the caller borrows in for matching.
    #[must_use]
    pub fn handle_event(&mut self, ev: &Event) -> bool {
        match ev {
            Event::Key(KeyEvent { code: KeyCode::Char('q'), .. }) => true,
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers,
                ..
            }) if modifiers.contains(KeyModifiers::CONTROL) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::cache::RowOutcome;
    use crate::model::{HpWindow, NetworkErr, ResetInfo};
    use ratatui::crossterm::event::KeyEventKind;
    use std::borrow::Cow;

    fn make_state(pct: f32) -> ProviderState {
        let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
        ProviderState {
            id: ProviderId::Mock,
            windows: vec![HpWindow {
                label: Cow::Borrowed("mock"),
                percent_remaining: pct,
                reset: ResetInfo { resets_at: now + jiff::Span::new().hours(2) },
                bar_color: None,
                detailed_label: None,
            }],
            fetched_at: now,
            source: Cow::Borrowed("mock"),
        }
    }

    fn fixture_now() -> jiff::Timestamp {
        "2026-05-23T12:00:00Z".parse().unwrap()
    }

    #[test]
    fn apply_results_translates_ok_schema_drift_and_err() {
        // Phase 3 Plan 03-03: input shape is now Vec<(ProviderId, RowOutcome)>.
        // SchemaDrift / other Failed errors still route to SchemaDrift / Err
        // RowState variants (Phase 1 behavior preserved).
        let mut app = AppState::new(fixture_now());
        let results = vec![
            (ProviderId::Claude, RowOutcome::Fresh(make_state(60.0))),
            (
                ProviderId::Codex,
                RowOutcome::Failed(ProviderError::SchemaDrift { missing: vec!["x".into()] }),
            ),
            (
                ProviderId::Gemini,
                RowOutcome::Failed(ProviderError::Unavailable { reason: "boom?".into() }),
            ),
        ];
        app.apply_results(results);
        assert_eq!(app.rows.len(), 3);
        assert!(matches!(app.rows[0], RowState::Ok(_)));
        assert!(matches!(
            app.rows[1],
            RowState::SchemaDrift { id: ProviderId::Codex }
        ));
        match &app.rows[2] {
            RowState::Err { id, message } => {
                assert_eq!(*id, ProviderId::Gemini);
                assert!(message.contains("boom?"), "msg: {message}");
            }
            other => panic!("expected Err row, got {other:?}"),
        }
    }

    #[test]
    fn apply_results_translates_fresh_to_ok() {
        let mut app = AppState::new(fixture_now());
        let results = vec![(ProviderId::Claude, RowOutcome::Fresh(make_state(75.0)))];
        app.apply_results(results);
        assert_eq!(app.rows.len(), 1);
        match &app.rows[0] {
            RowState::Ok(state) => assert_eq!(state.id, ProviderId::Mock),
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn apply_results_translates_stale_to_stale_ok() {
        let mut app = AppState::new(fixture_now());
        let results = vec![(
            ProviderId::Claude,
            RowOutcome::Stale {
                state: make_state(60.0),
                stale_age_secs: 47,
            },
        )];
        app.apply_results(results);
        assert_eq!(app.rows.len(), 1);
        match &app.rows[0] {
            RowState::StaleOk { state, stale_age_secs } => {
                assert_eq!(state.id, ProviderId::Mock);
                assert_eq!(*stale_age_secs, 47);
            }
            other => panic!("expected StaleOk, got {other:?}"),
        }
    }

    #[test]
    fn apply_results_passes_stale_age_secs_unchanged() {
        // stale_age_secs = 0, 15, 3600 — all pass through as-is (the engine
        // pre-computed it; AppState is a dumb translator).
        for secs in [0u64, 15, 3600] {
            let mut app = AppState::new(fixture_now());
            let results = vec![(
                ProviderId::Codex,
                RowOutcome::Stale {
                    state: make_state(50.0),
                    stale_age_secs: secs,
                },
            )];
            app.apply_results(results);
            match &app.rows[0] {
                RowState::StaleOk { stale_age_secs, .. } => {
                    assert_eq!(*stale_age_secs, secs);
                }
                other => panic!("expected StaleOk(secs={secs}), got {other:?}"),
            }
        }
    }

    #[test]
    fn apply_results_translates_schema_drift_to_schema_drift() {
        let mut app = AppState::new(fixture_now());
        let results = vec![(
            ProviderId::Codex,
            RowOutcome::Failed(ProviderError::SchemaDrift { missing: vec!["x".into()] }),
        )];
        app.apply_results(results);
        assert!(matches!(
            app.rows[0],
            RowState::SchemaDrift { id: ProviderId::Codex }
        ));
    }

    #[test]
    fn apply_results_translates_other_failed_to_err() {
        // Network is a transient error but the engine only emits Failed(Network)
        // when there's no cache to fall back to — TUI then renders ERROR row.
        let mut app = AppState::new(fixture_now());
        let results = vec![(
            ProviderId::Gemini,
            RowOutcome::Failed(ProviderError::Network {
                source: NetworkErr("offline".into()),
            }),
        )];
        app.apply_results(results);
        match &app.rows[0] {
            RowState::Err { id, message } => {
                assert_eq!(*id, ProviderId::Gemini);
                assert!(message.contains("offline"), "msg: {message}");
            }
            other => panic!("expected Err, got {other:?}"),
        }
    }

    fn key(code: KeyCode, mods: KeyModifiers) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: mods,
            kind: KeyEventKind::Press,
            state: ratatui::crossterm::event::KeyEventState::empty(),
        })
    }

    #[test]
    fn handle_event_quits_on_q() {
        let mut app = AppState::new(fixture_now());
        assert!(app.handle_event(&key(KeyCode::Char('q'), KeyModifiers::NONE)));
        assert!(app.handle_event(&key(KeyCode::Char('q'), KeyModifiers::SHIFT)));
    }

    #[test]
    fn handle_event_quits_on_ctrl_c() {
        let mut app = AppState::new(fixture_now());
        assert!(app.handle_event(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn handle_event_does_not_quit_on_other_keys() {
        let mut app = AppState::new(fixture_now());
        assert!(!app.handle_event(&key(KeyCode::Char('c'), KeyModifiers::NONE)));
        assert!(!app.handle_event(&key(KeyCode::Char('x'), KeyModifiers::NONE)));
        assert!(!app.handle_event(&key(KeyCode::Esc, KeyModifiers::NONE)));
    }
}
