//! TTY-aware color decision (CORE-05, UI-SPEC color-off paths).
//!
//! Phase 1: implements the priority chain `--json` then `--color=never` then
//! `--color=always` then `NO_COLOR` then `IsTerminal` then default-true
//! (RESEARCH Pattern 4).
//!
//! The pure decision `should_colorize(cli_flag, json_mode, is_tty, no_color)` lives
//! here; an env-aware wrapper `should_colorize_env(cli, json)` reads stdout TTY +
//! `NO_COLOR` env at the call site. Pure fn is testable without env manipulation.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::io::IsTerminal;

/// CLI `--color` flag. Wired into `Cli` via clap `ValueEnum` (see `cli::mod`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorMode {
    /// Auto-detect: colored only when stdout is a TTY and `NO_COLOR` is unset.
    Auto,
    /// Always emit ANSI escapes regardless of TTY (e.g., piping into `less -R`).
    Always,
    /// Never emit ANSI escapes.
    Never,
}

/// Pure color-decision function. No env / stdout introspection — caller supplies
/// the booleans. Easy to unit-test all six paths from UI-SPEC + RESEARCH Pattern 4.
#[must_use]
pub fn should_colorize(cli_flag: ColorMode, json_mode: bool, is_tty: bool, no_color: bool) -> bool {
    // Path 1 (highest priority): JSON output is always uncolored.
    if json_mode {
        return false;
    }
    match cli_flag {
        ColorMode::Never => false,
        ColorMode::Always => true,
        ColorMode::Auto => !no_color && is_tty,
    }
}

/// Env-aware wrapper: reads stdout TTY state + `NO_COLOR` env var, then delegates
/// to the pure `should_colorize`. Use this from `main`-adjacent CLI dispatch code.
#[must_use]
pub fn should_colorize_env(cli_flag: ColorMode, json_mode: bool) -> bool {
    let is_tty = std::io::stdout().is_terminal();
    let no_color = std::env::var_os("NO_COLOR").is_some();
    should_colorize(cli_flag, json_mode, is_tty, no_color)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The six paths from UI-SPEC Color section + RESEARCH Pattern 4 truth table.

    #[test]
    fn path_1_json_mode_always_false() {
        // json overrides everything else (including Always).
        assert!(!should_colorize(ColorMode::Always, true, true, false));
        assert!(!should_colorize(ColorMode::Auto, true, true, false));
        assert!(!should_colorize(ColorMode::Never, true, true, false));
    }

    #[test]
    fn path_2_never_is_false() {
        assert!(!should_colorize(ColorMode::Never, false, true, false));
    }

    #[test]
    fn path_3_always_is_true_regardless_of_tty() {
        // Always emits color even when piped (UI-SPEC: "Use case: piping into less -R").
        assert!(should_colorize(ColorMode::Always, false, false, false));
        assert!(should_colorize(ColorMode::Always, false, true, false));
        // NO_COLOR does NOT override --color=always (explicit user intent wins).
        assert!(should_colorize(ColorMode::Always, false, true, true));
    }

    #[test]
    fn path_4_auto_no_color_set_is_false() {
        assert!(!should_colorize(ColorMode::Auto, false, true, true));
    }

    #[test]
    fn path_5_auto_non_tty_is_false() {
        assert!(!should_colorize(ColorMode::Auto, false, false, false));
    }

    #[test]
    fn path_6_auto_tty_and_no_color_unset_is_true() {
        assert!(should_colorize(ColorMode::Auto, false, true, false));
    }
}
