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
#[derive(Debug, Default)]
pub struct AppState {
    pub rows: Vec<RowState>,
}

impl AppState {
    /// Build a fresh empty state. Equivalent to `AppState::default()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
    use crate::model::{HpWindow, ResetInfo};
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
            }],
            fetched_at: now,
            source: Cow::Borrowed("mock"),
        }
    }

    #[test]
    fn apply_results_translates_ok_schema_drift_and_err() {
        let mut app = AppState::new();
        let results = vec![
            (ProviderId::Claude, Ok(make_state(60.0))),
            (
                ProviderId::Codex,
                Err(ProviderError::SchemaDrift { missing: vec!["x".into()] }),
            ),
            (
                ProviderId::Gemini,
                Err(ProviderError::Unavailable { reason: "boom?".into() }),
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
        let mut app = AppState::new();
        assert!(app.handle_event(&key(KeyCode::Char('q'), KeyModifiers::NONE)));
        assert!(app.handle_event(&key(KeyCode::Char('q'), KeyModifiers::SHIFT)));
    }

    #[test]
    fn handle_event_quits_on_ctrl_c() {
        let mut app = AppState::new();
        assert!(app.handle_event(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn handle_event_does_not_quit_on_other_keys() {
        let mut app = AppState::new();
        assert!(!app.handle_event(&key(KeyCode::Char('c'), KeyModifiers::NONE)));
        assert!(!app.handle_event(&key(KeyCode::Char('x'), KeyModifiers::NONE)));
        assert!(!app.handle_event(&key(KeyCode::Esc, KeyModifiers::NONE)));
    }
}
