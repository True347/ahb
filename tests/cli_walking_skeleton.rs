//! Walking-skeleton integration test for Plan 01-01.
//!
//! Sets up a fake HOME + XDG_CONFIG_HOME with:
//! - `<home>/.claude/projects/proj-a/<uuid>.jsonl` containing one assistant entry
//!   (cache_creation_input_tokens=4400, timestamp ~1h ago for deterministic countdown)
//! - `<config>/ahb/config.toml` with `[providers.claude] enabled = true`
//!
//! Then runs `ahb` and asserts:
//! - stdout has one `claude ...` line shaped per UI-SPEC
//! - piped stdout (no TTY) contains zero ANSI escape bytes (CORE-05)

use assert_cmd::Command;
use std::io::Write;

/// Hard-coded fixture using a deliberately recent timestamp so the cluster is "now-ish".
/// We hand-build a synthetic JSONL whose `timestamp` is RFC3339 + cache_creation_input_tokens=4400.
fn make_fixture_jsonl(ts: &str, cache_creation: u64) -> String {
    format!(
        r#"{{"parentUuid":"abc","isSidechain":false,"message":{{"model":"claude-opus-4-7","id":"msg_x","type":"message","role":"assistant","content":[{{"type":"text","text":"hi"}}],"stop_reason":"end_turn","usage":{{"input_tokens":5,"cache_creation_input_tokens":{cache_creation},"cache_read_input_tokens":1000,"output_tokens":186}}}},"type":"assistant","uuid":"u1","timestamp":"{ts}"}}"#
    )
}

fn setup_fake_home() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().to_path_buf();
    let xdg = home.join("config_home");

    // Fake claude session — use a fresh ISO timestamp slightly in the past so reset is forward.
    let session_dir = home.join(".claude").join("projects").join("proj-a");
    std::fs::create_dir_all(&session_dir).unwrap();
    let session_file = session_dir.join("session.jsonl");
    let mut f = std::fs::File::create(&session_file).unwrap();
    // Use a fixed timestamp 1h before "now" (will be a slightly past clock). Real `now`
    // when the test runs is later than this — the gap < 5h so the cluster includes it
    // and reset_at = ts + 5h.
    // Compute "now - 1h" inline; using the current system time keeps the bar countdown
    // future-positive (between 4h and 4h05m typically).
    let now = jiff::Timestamp::now();
    let an_hour_ago = (now - jiff::Span::new().hours(1)).to_string();
    writeln!(f, "{}", make_fixture_jsonl(&an_hour_ago, 4400)).unwrap();

    // Fake AHB config with claude enabled.
    let ahb_cfg = xdg.join("ahb");
    std::fs::create_dir_all(&ahb_cfg).unwrap();
    let cfg_path = ahb_cfg.join("config.toml");
    std::fs::write(
        &cfg_path,
        "[providers.claude]\nenabled = true\n\n[providers.codex]\nenabled = false\n\n[providers.gemini]\nenabled = false\n",
    )
    .unwrap();

    (tmp, xdg)
}

#[test]
fn ahb_default_run_emits_one_claude_row_with_real_numbers() {
    let (_tmp, xdg) = setup_fake_home();
    let home = _tmp.path();

    let assert = Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env_remove("APPDATA")
        .env_remove("NO_COLOR")
        // Plan 02: bypass D-41 keyring init on backend-less hosts so Plan 01 happy
        // path stays exercisable on CI / dev machines without dbus / Keychain.
        .env("AHB_SECRETS_MOCK", "1")
        // assert_cmd captures stdout so by definition stdout is NOT a TTY:
        // the binary must therefore emit zero ANSI bytes (CORE-05).
        .assert()
        .success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Must contain exactly one line starting with "claude".
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "expected exactly one output row, got:\n{stdout}"
    );
    let line = lines[0];
    assert!(
        line.starts_with("claude  "),
        "row must start with 'claude  ', got: {line:?}"
    );
    // Look for the % and "resets in Xh00m" tail.
    let re = regex::Regex::new(r"^claude  \S{10}\s+\d{1,3}%\s+.\s+resets in \d+h\d{2}m$").unwrap();
    assert!(
        re.is_match(line),
        "row shape mismatch: {line:?} — expected: claude  <10 glyphs> <pct>% • resets in Xh00m"
    );

    // CORE-05 + UI-SPEC: piped stdout must contain zero ANSI escape bytes.
    assert!(
        !stdout.contains("\x1b["),
        "piped stdout must not contain ANSI escapes, got: {stdout:?}"
    );
}

#[test]
fn ahb_ascii_mode_uses_pipe_and_hash_dash_glyphs() {
    let (_tmp, xdg) = setup_fake_home();
    let home = _tmp.path();

    let assert = Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env_remove("APPDATA")
        .env_remove("NO_COLOR")
        // Plan 02: bypass D-41 keyring init on backend-less hosts so Plan 01 happy
        // path stays exercisable on CI / dev machines without dbus / Keychain.
        .env("AHB_SECRETS_MOCK", "1")
        .arg("--ascii")
        .assert()
        .success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // ASCII row uses `#` / `-` and `|` separator.
    let line = stdout.lines().next().expect("at least one line");
    assert!(line.starts_with("claude  "), "ascii row: {line}");
    let re = regex::Regex::new(r"^claude  [#-]{10}\s+\d{1,3}%\s+\|\s+resets in \d+h\d{2}m$").unwrap();
    assert!(re.is_match(line), "ascii shape mismatch: {line:?}");
}

#[test]
fn ahb_with_all_providers_disabled_prints_empty_state() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let xdg = home.join("config_home");
    let ahb_cfg = xdg.join("ahb");
    std::fs::create_dir_all(&ahb_cfg).unwrap();
    std::fs::write(
        ahb_cfg.join("config.toml"),
        "[providers.claude]\nenabled = false\n[providers.codex]\nenabled = false\n[providers.gemini]\nenabled = false\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env_remove("APPDATA")
        .env_remove("NO_COLOR")
        // Plan 02: bypass D-41 keyring init on backend-less hosts so Plan 01 happy
        // path stays exercisable on CI / dev machines without dbus / Keychain.
        .env("AHB_SECRETS_MOCK", "1")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("no providers configured"),
        "expected empty-state heading, got: {stdout}"
    );
    assert!(
        stdout.contains("config.toml"),
        "expected empty-state body referencing config.toml, got: {stdout}"
    );
}

#[test]
fn ahb_with_broken_claude_config_prints_error_row_not_crash() {
    // ClaudeProvider's base_path is HOME/.claude/projects. If we don't create that
    // directory, the adapter returns Err(Unavailable { reason: "~/.claude/projects not found — is Claude Code installed?" }).
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let xdg = home.join("config_home");
    let ahb_cfg = xdg.join("ahb");
    std::fs::create_dir_all(&ahb_cfg).unwrap();
    std::fs::write(
        ahb_cfg.join("config.toml"),
        "[providers.claude]\nenabled = true\n",
    )
    .unwrap();
    // Deliberately do NOT create .claude/projects under HOME.

    let assert = Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env_remove("APPDATA")
        .env_remove("NO_COLOR")
        // Plan 02: bypass D-41 keyring init on backend-less hosts so Plan 01 happy
        // path stays exercisable on CI / dev machines without dbus / Keychain.
        .env("AHB_SECRETS_MOCK", "1")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let line = stdout.lines().next().expect("at least one line");
    assert!(line.starts_with("claude  ERROR:"), "expected ERROR row, got: {line:?}");
    assert!(
        line.ends_with("is Claude Code installed?"),
        "ERROR row must end with next-step hint, got: {line:?}"
    );
}
