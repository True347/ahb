//! Phase 2 Plan 02-03 — CORE-06 integration coverage for the D-59 exit-code
//! grid:
//!
//! | situation | code |
//! |---|---|
//! | ≥1 provider Ok | 0 |
//! | all providers Err | 1 |
//! | zero providers enabled (CFG-04) | 0 |
//! | clap usage error (flag conflict) | 2 |
//!
//! Config / secrets unloadable (also exit 2) is covered by Phase 1 wiring; not
//! re-asserted here.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::unwrap_used)] // tests: clippy.toml allow-unwrap-in-tests = true

use assert_cmd::Command;
use std::io::Write;

fn make_fixture_jsonl(ts: &str, cache_creation: u64) -> String {
    format!(
        r#"{{"parentUuid":"abc","isSidechain":false,"message":{{"model":"claude-opus-4-7","id":"msg_x","type":"message","role":"assistant","content":[{{"type":"text","text":"hi"}}],"stop_reason":"end_turn","usage":{{"input_tokens":5,"cache_creation_input_tokens":{cache_creation},"cache_read_input_tokens":1000,"output_tokens":186}}}},"type":"assistant","uuid":"u1","timestamp":"{ts}"}}"#
    )
}

/// Claude enabled with a real fixture (status="ok") — used for the "≥1 Ok"
/// exit-0 branch.
fn setup_fake_home_with_claude_ok() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().to_path_buf();
    let xdg = home.join("config_home");

    let session_dir = home.join(".claude").join("projects").join("proj-a");
    std::fs::create_dir_all(&session_dir).unwrap();
    let session_file = session_dir.join("session.jsonl");
    let mut f = std::fs::File::create(&session_file).unwrap();
    let now = jiff::Timestamp::now();
    let an_hour_ago = (now - jiff::Span::new().hours(1)).to_string();
    writeln!(f, "{}", make_fixture_jsonl(&an_hour_ago, 4400)).unwrap();

    let ahb_cfg = xdg.join("ahb");
    std::fs::create_dir_all(&ahb_cfg).unwrap();
    std::fs::write(
        ahb_cfg.join("config.toml"),
        "[providers.claude]\nenabled = true\n[providers.codex]\nenabled = false\n[providers.gemini]\nenabled = false\n",
    )
    .unwrap();
    (tmp, xdg)
}

/// Claude enabled, NO `.claude/projects` dir → adapter returns
/// `Unavailable("…not found — is Claude Code installed?")`. The only enabled
/// provider, so the dispatch returns `AllFailed` (exit 1).
fn setup_fake_home_with_claude_only_err() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().to_path_buf();
    let xdg = home.join("config_home");
    let ahb_cfg = xdg.join("ahb");
    std::fs::create_dir_all(&ahb_cfg).unwrap();
    std::fs::write(
        ahb_cfg.join("config.toml"),
        "[providers.claude]\nenabled = true\n[providers.codex]\nenabled = false\n[providers.gemini]\nenabled = false\n",
    )
    .unwrap();
    // Deliberately NO ~/.claude/projects dir.
    (tmp, xdg)
}

fn setup_fake_home_with_no_providers() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().to_path_buf();
    let xdg = home.join("config_home");
    let ahb_cfg = xdg.join("ahb");
    std::fs::create_dir_all(&ahb_cfg).unwrap();
    std::fs::write(
        ahb_cfg.join("config.toml"),
        "[providers.claude]\nenabled = false\n[providers.codex]\nenabled = false\n[providers.gemini]\nenabled = false\n",
    )
    .unwrap();
    (tmp, xdg)
}

fn run_ahb(home: &std::path::Path, xdg: &std::path::Path, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::cargo_bin("ahb").unwrap();
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", xdg)
        .env_remove("APPDATA")
        .env_remove("NO_COLOR")
        .env("AHB_SECRETS_MOCK", "1");
    for a in args {
        cmd.arg(a);
    }
    cmd.output().expect("subprocess should run")
}

/// C1: ≥1 provider Ok → exit 0.
#[test]
fn exit_code_0_when_any_provider_ok() {
    let (_tmp, xdg) = setup_fake_home_with_claude_ok();
    let out = run_ahb(_tmp.path(), &xdg, &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 when ≥1 provider Ok; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// C2: all providers Err → exit 1.
#[test]
fn exit_code_1_when_all_providers_fail() {
    let (_tmp, xdg) = setup_fake_home_with_claude_only_err();
    let out = run_ahb(_tmp.path(), &xdg, &[]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 when all providers Err; stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
}

/// C3: zero providers enabled → exit 0 (CFG-04 special case).
#[test]
fn exit_code_0_when_zero_providers_enabled() {
    let (_tmp, xdg) = setup_fake_home_with_no_providers();
    let out = run_ahb(_tmp.path(), &xdg, &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0 when no providers enabled (CFG-04); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// C4: `AHB --compact --json` → clap conflict → exit 2.
#[test]
fn exit_code_2_on_compact_and_json_conflict() {
    let out = Command::cargo_bin("ahb")
        .unwrap()
        .arg("--compact")
        .arg("--json")
        .output()
        .expect("subprocess should run");
    assert_eq!(out.status.code(), Some(2), "expected clap exit 2");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cannot be used with"),
        "expected clap conflict message; stderr: {stderr}"
    );
}

/// C5: `AHB --compact --detailed` → exit 2.
#[test]
fn exit_code_2_on_compact_and_detailed_conflict() {
    let out = Command::cargo_bin("ahb")
        .unwrap()
        .arg("--compact")
        .arg("--detailed")
        .output()
        .expect("subprocess should run");
    assert_eq!(out.status.code(), Some(2), "expected clap exit 2");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cannot be used with"),
        "expected clap conflict message; stderr: {stderr}"
    );
}

/// C6: `AHB --detailed --json` → exit 2.
#[test]
fn exit_code_2_on_detailed_and_json_conflict() {
    let out = Command::cargo_bin("ahb")
        .unwrap()
        .arg("--detailed")
        .arg("--json")
        .output()
        .expect("subprocess should run");
    assert_eq!(out.status.code(), Some(2), "expected clap exit 2");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cannot be used with"),
        "expected clap conflict message; stderr: {stderr}"
    );
}
