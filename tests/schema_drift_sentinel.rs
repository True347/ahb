//! ADP-03 integration test — fake JSONL fixture missing
//! `message.usage.cache_creation_input_tokens` on ≥ 2 of the last 3 assistant entries
//! triggers the schema-drift sentinel through the full `AHB` binary.
//!
//! Asserts stdout contains the verbatim UI-SPEC sentinel literal AND that the bar cells
//! are the U+2592 medium-shade character (NOT U+2591 light-shade).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

#![allow(clippy::unwrap_used)] // tests: clippy.toml allow-unwrap-in-tests = true

use std::io::Write;

const SENTINEL: &str = "claude  \u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592}\u{2592} ??% \u{2022} Claude adapter may be out-of-date";

#[test]
fn drift_in_recent_assistants_triggers_sentinel_literal() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let projects_dir = home.join(".claude").join("projects").join("proj-a");
    std::fs::create_dir_all(&projects_dir).unwrap();
    let session = projects_dir.join("session.jsonl");
    let mut f = std::fs::File::create(&session).unwrap();

    // Oldest entry: full usage block with cache_creation_input_tokens.
    writeln!(
        f,
        r#"{{"type":"assistant","timestamp":"2026-05-23T08:00:00Z","message":{{"role":"assistant","model":"claude-opus-4-7","usage":{{"cache_creation_input_tokens":41630}}}},"uuid":"u1"}}"#
    )
    .unwrap();
    // Two newer entries missing the usage block entirely → drift.
    writeln!(
        f,
        r#"{{"type":"assistant","timestamp":"2026-05-23T09:00:00Z","message":{{"role":"assistant","model":"claude-opus-4-7"}},"uuid":"u2"}}"#
    )
    .unwrap();
    writeln!(
        f,
        r#"{{"type":"assistant","timestamp":"2026-05-23T10:00:00Z","message":{{"role":"assistant","model":"claude-opus-4-7"}},"uuid":"u3"}}"#
    )
    .unwrap();
    drop(f);

    // Construct a config dir with claude enabled.
    let xdg = tmp.path().join("xdg");
    let config_dir = xdg.join("ahb");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "[providers.claude]\nenabled = true\n",
    )
    .unwrap();

    let output = assert_cmd::Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env("NO_COLOR", "1") // strip ANSI to make the byte assertions deterministic
        .output()
        .expect("subprocess should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "ahb should exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains(SENTINEL),
        "stdout missing UI-SPEC sentinel literal.\nGot: {stdout}\nWanted: {SENTINEL}"
    );

    // Byte-level: U+2592 (e2 96 92) must appear ≥ 10 times (the bar cells).
    let bytes = stdout.as_bytes();
    let count = bytes.windows(3).filter(|w| *w == [0xe2, 0x96, 0x92]).count();
    assert!(
        count >= 10,
        "expected ≥ 10 U+2592 bytes in stdout, found {count}; stdout: {stdout}"
    );
}
