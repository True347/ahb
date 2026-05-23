//! Integration test for D-37 first-run init.
//!
//! Empty `XDG_CONFIG_HOME` → AHB runs, prints the D-37 literal, exits 0, and the
//! config file actually exists on disk afterwards.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn first_run_creates_config_and_exits_zero() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let xdg_config = tmp.path().join("config_home");
    // Note: directories::ProjectDirs::from("", "", "ahb") uses XDG_CONFIG_HOME on Linux;
    // on macOS it uses $HOME/Library/Application Support; on Windows %APPDATA%.
    // Override HOME + XDG_CONFIG_HOME so the test is portable.
    Command::cargo_bin("ahb")
        .expect("binary")
        .env("HOME", tmp.path())
        .env("XDG_CONFIG_HOME", &xdg_config)
        // Don't inherit any user env that would change behavior:
        .env_remove("APPDATA")
        .env_remove("NO_COLOR")
        .assert()
        .success()
        .stdout(predicate::str::contains("initialized "))
        .stdout(predicate::str::contains(" — enable providers and rerun"));

    let expected_path = xdg_config.join("ahb").join("config.toml");
    assert!(
        expected_path.exists(),
        "config file must exist at {} after first-run init",
        expected_path.display()
    );

    let written = std::fs::read_to_string(&expected_path).expect("read");
    assert!(written.contains("[providers.claude]"));
    assert!(written.contains("[providers.codex]"));
    assert!(written.contains("[providers.gemini]"));
}
