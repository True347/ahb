//! HP-bar row widget for the TUI fixed view (REQ TUI-01 + UI-SPEC).
//!
//! Builds one `Line` of `Span`s per provider row using the shared `cli::render_text`
//! helpers Plan 01 Task 3 promoted to `pub(crate)` (WARNING #3 — NO duplication,
//! NO scoped clippy allow):
//!
//! - `filled_cells(pct)` — same f32→usize math as compact_line
//! - `format_countdown(now, target)` — same `Xh{MM}m` formatter
//! - `id_label(id)` — same closed-enum → snake_case label
//!
//! UI-SPEC color thresholds (`Style::default().fg(...)`):
//! - bar filled: Green (pct ≥ 30), Yellow (10 ≤ pct < 30), Red (pct < 10)
//! - bar empty: DarkGray (Secondary role per 60/30/10)
//! - SchemaDrift sentinel cells: DarkGray (Secondary — "unknown, not critical")
//! - SchemaDrift trailing phrase: Red + Bold (Destructive role)
//! - ERROR keyword: Red + Bold (Destructive role)

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::doc_markdown)]

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::cli::render_text::{filled_cells, format_countdown, id_label};
use crate::tui::app::RowState;

/// Phase 0 D-16 — fixed bar width for horizontal alignment.
const BAR_WIDTH: usize = 10;

/// Render one provider row into the buffer area. TUI always uses Unicode (the CLI's
/// `--ascii` flag does NOT apply — TUI is always a TTY, always Unicode; module note).
///
/// `_ascii` is reserved for symmetry with `compact_line` should a future phase reintroduce
/// ASCII fallback in the TUI surface.
///
/// `now` is the wall-clock snapshot supplied by the caller (`ui::draw` plumbs `&app.now`).
/// BL-01 fix: this widget MUST NOT read wall clock itself — the render-tick arm in
/// `tui_loop` is the single authorized site (enforced by
/// `tests/no_walltime_in_adapter.rs`).
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    row: &RowState,
    _ascii: bool,
    now: &jiff::Timestamp,
) {
    let line = build_line(row, now);
    Paragraph::new(line).render(area, buf);
}

/// Build the `Line` for one row (factored out so unit tests can inspect the spans).
///
/// `now` is forwarded to `build_ok_line` so the countdown reflects the caller-supplied
/// wall clock (BL-01 fix). SchemaDrift / Err branches do not consume `now` but the
/// uniform signature lets the caller pass it without per-variant branching.
#[must_use]
pub fn build_line(row: &RowState, now: &jiff::Timestamp) -> Line<'static> {
    match row {
        RowState::Ok(state) => build_ok_line(state, now),
        RowState::SchemaDrift { id } => build_schema_drift_line(*id),
        RowState::Err { id, message } => build_err_line(*id, message),
    }
}

fn build_ok_line(state: &crate::model::ProviderState, now: &jiff::Timestamp) -> Line<'static> {
    // Phase 1 mirrors compact_line: render only windows[0]. Empty windows is a contract
    // violation but we degrade gracefully to an empty error row rather than panic.
    if state.windows.is_empty() {
        return build_err_line(state.id, "(no windows in ProviderState)");
    }
    let w = &state.windows[0];
    let pct = w.percent_remaining.clamp(0.0, 100.0);
    let filled = filled_cells(pct);
    let empty = BAR_WIDTH - filled;
    let accent = if pct >= 30.0 {
        Color::Green
    } else if pct >= 10.0 {
        Color::Yellow
    } else {
        Color::Red
    };

    let label = id_label(state.id);
    let countdown = format_countdown(now, &w.reset.resets_at);
    let pct_int = pct_int(pct);

    Line::from(vec![
        Span::raw(format!("{label}  ")),
        Span::styled("\u{2588}".repeat(filled), Style::default().fg(accent)),
        Span::styled("\u{2591}".repeat(empty), Style::default().fg(Color::DarkGray)),
        Span::raw(format!(" {pct_int}% ")),
        Span::styled("\u{2022}", Style::default().fg(Color::DarkGray)),
        Span::raw(format!(" resets in {countdown}")),
    ])
}

fn build_schema_drift_line(id: crate::model::ProviderId) -> Line<'static> {
    let label = id_label(id);
    // UI-SPEC LOCKED: 10× U+2592 medium-shade (NOT U+2591), DarkGray.
    let bar = "\u{2592}".repeat(BAR_WIDTH);
    Line::from(vec![
        Span::raw(format!("{label}  ")),
        Span::styled(bar, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::styled("??%", Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::styled("\u{2022}", Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::styled(
            "Claude adapter may be out-of-date",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn build_err_line(id: crate::model::ProviderId, message: &str) -> Line<'static> {
    let label = id_label(id);
    Line::from(vec![
        Span::raw(format!("{label}  ")),
        Span::styled(
            "ERROR: ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(message.to_string()),
    ])
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pct_int(pct: f32) -> u32 {
    pct.round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HpWindow, ProviderError, ProviderId, ProviderState, ResetInfo};
    use std::borrow::Cow;

    fn fixture_now() -> jiff::Timestamp {
        "2026-05-23T12:00:00Z".parse().unwrap()
    }

    fn ok_row(pct: f32) -> RowState {
        let now = fixture_now();
        RowState::Ok(ProviderState {
            id: ProviderId::Claude,
            windows: vec![HpWindow {
                label: Cow::Borrowed("claude"),
                percent_remaining: pct,
                reset: ResetInfo { resets_at: now + jiff::Span::new().hours(4) },
                bar_color: None,
            }],
            fetched_at: now,
            source: Cow::Borrowed("claude-jsonl"),
        })
    }

    #[test]
    fn schema_drift_row_uses_id_label_not_hardcoded_claude() {
        let drift = RowState::SchemaDrift { id: ProviderId::Codex };
        let line = build_line(&drift, &fixture_now());
        let plain: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            plain.starts_with("codex  "),
            "schema-drift label should come from id_label, got: {plain}"
        );
        assert!(
            plain.contains("Claude adapter may be out-of-date"),
            "UI-SPEC sentinel phrase must appear: {plain}"
        );
        // U+2592 medium-shade present
        assert!(plain.contains('\u{2592}'), "medium-shade glyph missing: {plain:?}");
    }

    #[test]
    fn err_row_starts_with_label_and_error_keyword() {
        let row = RowState::Err {
            id: ProviderId::Gemini,
            message: "gemini.google.com unreachable — check network".into(),
        };
        let line = build_line(&row, &fixture_now());
        let plain: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(plain.starts_with("gemini  "), "label prefix wrong: {plain}");
        assert!(plain.contains("ERROR: "), "ERROR keyword missing: {plain}");
        assert!(
            plain.ends_with("check network"),
            "next-step hint missing: {plain}"
        );
    }

    #[test]
    fn ok_row_color_thresholds_per_ui_spec() {
        let now = fixture_now();
        // pct ≥ 30 → Green
        let line = build_line(&ok_row(50.0), &now);
        let bar_span = line
            .spans
            .iter()
            .find(|s| s.content.contains('\u{2588}'))
            .unwrap();
        assert_eq!(bar_span.style.fg, Some(Color::Green));

        // 10 ≤ pct < 30 → Yellow
        let line = build_line(&ok_row(15.0), &now);
        let bar_span = line
            .spans
            .iter()
            .find(|s| s.content.contains('\u{2588}'))
            .unwrap();
        assert_eq!(bar_span.style.fg, Some(Color::Yellow));

        // pct < 10 → Red. 5% rounds to 1 filled cell — find that cell.
        let line = build_line(&ok_row(5.0), &now);
        let bar_span = line
            .spans
            .iter()
            .find(|s| s.content.contains('\u{2588}'))
            .unwrap();
        assert_eq!(bar_span.style.fg, Some(Color::Red));
    }

    #[test]
    fn ok_row_label_uses_id_label() {
        let line = build_line(&ok_row(60.0), &fixture_now());
        let plain: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(plain.starts_with("claude  "), "label: {plain}");
    }

    #[test]
    fn build_line_for_empty_windows_degrades_gracefully() {
        let now = fixture_now();
        let row = RowState::Ok(ProviderState {
            id: ProviderId::Mock,
            windows: vec![],
            fetched_at: now,
            source: Cow::Borrowed("mock"),
        });
        let line = build_line(&row, &now);
        let plain: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(plain.contains("ERROR:"), "empty-windows row should degrade to ERROR: {plain}");
    }

    // Touch ProviderError::SchemaDrift constructor so the import isn't dead-code-flagged.
    #[test]
    fn schema_drift_variant_constructs() {
        let _ = ProviderError::SchemaDrift { missing: vec!["x".into()] };
    }
}
