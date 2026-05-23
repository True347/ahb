//! ADP-01 integration test — `AHB_DEBUG_PANIC=adapter:mock` triggers a panic inside
//! `MockProvider::fetch`. Asserts:
//!  (a) exit code is 0 (Phase 1; Phase 2 CORE-06 wires proper codes),
//!  (b) stdout contains a healthy `claude` row (the panic in mock did NOT blank claude),
//!  (c) stdout contains a `mock  ERROR:` row,
//!  (d) stderr contains the Phase 0 panic-hook prefix `ahb panicked:`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

#![allow(clippy::unwrap_used)] // tests: clippy.toml allow-unwrap-in-tests = true

use std::io::Write;

#[test]
fn mock_panic_yields_error_row_and_claude_stays_healthy() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();

    // Build a real (healthy) Claude session so the claude row is OK.
    let projects_dir = home.join(".claude").join("projects").join("proj-a");
    std::fs::create_dir_all(&projects_dir).unwrap();
    let mut f = std::fs::File::create(projects_dir.join("session.jsonl")).unwrap();
    writeln!(
        f,
        r#"{{"type":"assistant","timestamp":"2026-05-23T11:00:00Z","message":{{"role":"assistant","model":"claude-opus-4-7","usage":{{"cache_creation_input_tokens":4400}}}},"uuid":"u1"}}"#
    )
    .unwrap();
    drop(f);

    // Config: both claude AND mock enabled. Mock will panic via AHB_DEBUG_PANIC.
    let xdg = tmp.path().join("xdg");
    let config_dir = xdg.join("ahb");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "[providers.claude]\nenabled = true\n\n[providers.mock]\nenabled = true\n",
    )
    .unwrap();

    let output = assert_cmd::Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env("AHB_DEBUG_PANIC", "adapter:mock")
        .env("NO_COLOR", "1")
        .output()
        .expect("subprocess should run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // (a) Exit code is 0 (Phase 1).
    assert!(
        output.status.success(),
        "expected exit 0, got {:?}; stderr: {stderr}",
        output.status.code()
    );

    // (b) claude row is present (a panic in mock must NOT scorch claude).
    let claude_row_present = stdout.lines().any(|l| l.starts_with("claude"));
    assert!(
        claude_row_present,
        "expected a 'claude' row in stdout — claude must stay healthy when mock panics.\nstdout: {stdout}"
    );

    // (c) mock ERROR row is present.
    let mock_error_present = stdout.lines().any(|l| l.starts_with("mock  ERROR:"));
    assert!(
        mock_error_present,
        "expected a 'mock  ERROR:' row in stdout when AHB_DEBUG_PANIC=adapter:mock.\nstdout: {stdout}"
    );

    // (d) Phase 0 panic-hook prefix in stderr.
    assert!(
        stderr.contains("ahb panicked:"),
        "expected Phase 0 panic-hook prefix 'ahb panicked:' in stderr.\nstderr: {stderr}"
    );
}
