//! TUI frame composition (UI-SPEC LOCKED).
//!
//! Phase 1 layout (`AHB tui`):
//!
//! ```text
//! ┌─ AHB ───────────────────────────────────────────────────┐
//! │                                                         │
//! │  claude   ████████░░ 80% • resets in 4h15m              │
//! │  codex    ██████░░░░ 60% • resets in 2h00m              │
//! │                                                         │
//! │  q quit  ·  ctrl-c quit                                 │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! - Outer border via `Block::default().title(" AHB ").borders(Borders::ALL)` (UI-SPEC
//!   border title with leading + trailing space).
//! - Inner layout: 4 chunks — top pad, provider rows, footer pad, quit hint row.
//! - Empty-state copy when no providers are configured (UI-SPEC empty-state).
//! - Quit hint `q quit  ·  ctrl-c quit` in `Color::DarkGray` (Secondary role).
//!
//! Note: TUI does NOT honor `--ascii` in Phase 1 — TUI is always a TTY, always Unicode.
//! The CLI handles ASCII fallback for piped output.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::app::AppState;
use crate::tui::widgets::hp_row;

// UI-SPEC LOCKED quit hint. The `·` is U+00B7 middle dot, distinct from the
// U+2022 bullet used as the percent / resets-in separator in HP rows.
// NOTE: this string contains a literal non-ASCII char on purpose so the plan's
// `grep -F "q quit  ·  ctrl-c quit"` acceptance check finds it byte-for-byte.
const QUIT_HINT: &str = "q quit  ·  ctrl-c quit";

/// Draw one frame from the cached `AppState`. Called per render tick (D-31, 1s) from
/// `tui::mod::tui_loop`.
pub fn draw(f: &mut Frame, app: &AppState) {
    // Outer border with title — UI-SPEC binding " AHB ".
    let outer = Block::default().title(" AHB ").borders(Borders::ALL);
    let inner_area = outer.inner(f.area());
    f.render_widget(outer, f.area());

    // Defensive: if the terminal is too small to hold even the chrome, bail early.
    if inner_area.height < 4 || inner_area.width < 1 {
        return;
    }

    // 4-chunk vertical split inside the border: top pad / provider rows / footer pad /
    // quit hint. Provider rows take whatever space remains via Min(1).
    let rows_len = u16::try_from(app.rows.len().max(1)).unwrap_or(1);
    let [_top_pad, body, _footer_pad, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(rows_len),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner_area);

    // Body — provider rows OR empty-state copy.
    if app.rows.is_empty() {
        draw_empty_state(f, body);
    } else {
        draw_rows(f, body, app);
    }

    // Quit hint row — UI-SPEC verbatim, Secondary color (DarkGray).
    let hint_with_indent = Line::from(vec![
        Span::raw("  "),
        Span::styled(QUIT_HINT, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(hint_with_indent), hint_area);
}

fn draw_rows(f: &mut Frame, body: Rect, app: &AppState) {
    // Build per-row sub-areas: each provider row gets exactly 1 row of height.
    // Cap rendered rows to whatever the body area can hold so very tall provider
    // lists don't underflow.
    let max_rows = usize::from(body.height);
    let rendered = app.rows.len().min(max_rows);
    if rendered == 0 {
        return;
    }
    // Build constraint list as a Vec because the row count is dynamic.
    let constraints: Vec<Constraint> = (0..rendered).map(|_| Constraint::Length(1)).collect();
    let row_areas = Layout::vertical(constraints).split(body);
    for (i, area) in row_areas.iter().enumerate().take(rendered) {
        hp_row::render(*area, f.buffer_mut(), &app.rows[i], false);
    }
}

fn draw_empty_state(f: &mut Frame, body: Rect) {
    // UI-SPEC empty-state pair: heading + body with config path + README hint.
    let lines = vec![
        Line::from(Span::raw("  no providers configured")),
        Line::from(Span::raw(
            "  add at least one provider to ~/.config/ahb/config.toml \u{2014} see README",
        )),
    ];
    f.render_widget(Paragraph::new(lines), body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_hint_uses_ui_spec_literal_with_middle_dot_separator() {
        // UI-SPEC binding: `q quit  ·  ctrl-c quit`. Verify byte form: two spaces,
        // U+00B7 (middle dot, NOT U+2022 bullet — UI-SPEC uses · between alternatives,
        // • elsewhere as % / resets-in separator).
        assert!(QUIT_HINT.starts_with("q quit"));
        assert!(QUIT_HINT.ends_with("ctrl-c quit"));
        // The "·" between alternatives is U+00B7. We accept either form here since
        // UI-SPEC's quit-hint literal also appears with U+00B7. The grep gate in the
        // acceptance criteria checks the byte literal.
        assert!(QUIT_HINT.contains('\u{00B7}'));
    }
}
