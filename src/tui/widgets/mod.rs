//! TUI widget submodules. Each widget is a thin render fn that builds ratatui `Line`s
//! from the per-row `RowState` cache; the outer `ui::draw` composes them into the
//! frame layout.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

pub mod hp_row;
