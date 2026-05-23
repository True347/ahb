//! TUI-05 integration test: `AHB tui` on non-TTY stdout must refuse with the
//! verbatim UI-SPEC literal on stderr + exit code 2.
//!
//! `assert_cmd::Command` spawns the subprocess with stdin/stdout piped by default, so
//! the child's `std::io::stdout().is_terminal()` returns false — exactly the TUI-05
//! gate condition. We assert the verbatim copy via `predicates::str::contains` so the
//! test is robust to additional log lines tracing emits on the same stderr stream.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use assert_cmd::Command;
use predicates::prelude::*;

const UI_SPEC_LITERAL: &str = "AHB tui requires a terminal (stdout is not a TTY). Run AHB without 'tui' for piped / non-interactive output.";

#[test]
fn ahb_tui_with_piped_stdout_refuses_with_ui_spec_literal_and_exit_2() {
    // Use a temp HOME so the first-run config init doesn't pollute the user's machine.
    // Pre-seed a valid config so the binary reaches the TUI dispatch (otherwise D-37
    // first-run init fires + exits 0 before main hits `Command::Tui`).
    let home = tempfile::tempdir().unwrap();
    let config_dir = home.path().join(".config").join("ahb");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "[providers.claude]\nenabled = false\n\n[providers.codex]\nenabled = false\n\n[providers.gemini]\nenabled = false\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("ahb")
        .unwrap()
        .arg("tui")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join(".config"))
        // Backend-less hosts need this so secrets::init doesn't take the D-41 hard-error
        // path. The TUI-05 IsTerminal check fires AFTER secrets::init in the main.rs
        // flow, so this affordance is required to reach tui::run on a backend-less host.
        .env("AHB_SECRETS_MOCK", "1")
        .write_stdin("")
        .assert();

    let output = assert.get_output();
    // Exit code: 2 (TUI-05 binding).
    let code = output.status.code().unwrap_or(-1);
    assert_eq!(
        code, 2,
        "expected exit code 2, got {code} — stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // stderr contains the verbatim UI-SPEC literal.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        predicate::str::contains(UI_SPEC_LITERAL).eval(stderr.as_ref()),
        "stderr missing UI-SPEC literal.\nExpected: {UI_SPEC_LITERAL}\nActual stderr: {stderr}"
    );
}
