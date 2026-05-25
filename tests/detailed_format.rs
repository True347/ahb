//! Integration test for Phase 2 Plan 02-02 CORE-03 (`AHB --detailed`).
//!
//! Drives the `ahb` binary as a subprocess against a fake `$HOME` + Claude
//! session fixture (same shape `cli_walking_skeleton.rs` uses) and asserts:
//! D7 — `--detailed --color=never` emits header + indented 5h + indented weekly
//!      rows for Claude, with the weekly NaN sentinel + `(limit unknown)`
//!      footer, and zero ANSI escape bytes.
//! D8 — with both claude AND mock enabled, the two provider blocks are
//!      separated by exactly ONE blank line and there is NO trailing blank
//!      line after the last block.

#![allow(clippy::unwrap_used)]

use assert_cmd::Command;
use std::io::Write;

/// Build a synthetic Claude assistant JSONL line. Mirrors the helper in
/// `tests/cli_walking_skeleton.rs:17-21` verbatim (intentional duplication —
/// these integration tests stay self-contained per the plan's specifics).
fn make_fixture_jsonl(ts: &str, cache_creation: u64) -> String {
    format!(
        r#"{{"parentUuid":"abc","isSidechain":false,"message":{{"model":"claude-opus-4-7","id":"msg_x","type":"message","role":"assistant","content":[{{"type":"text","text":"hi"}}],"stop_reason":"end_turn","usage":{{"input_tokens":5,"cache_creation_input_tokens":{cache_creation},"cache_read_input_tokens":1000,"output_tokens":186}}}},"type":"assistant","uuid":"u1","timestamp":"{ts}"}}"#
    )
}

/// Set up a fake `$HOME` with a Claude session + an AHB config that enables
/// claude (and optionally `mock`). Returns the tempdir guard + xdg path.
fn setup_fake_home(enable_mock: bool) -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
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
    let cfg_path = ahb_cfg.join("config.toml");
    let mock_section = if enable_mock {
        "\n\n[providers.mock]\nenabled = true\n"
    } else {
        ""
    };
    let cfg_body = format!(
        "[providers.claude]\nenabled = true\n\n[providers.codex]\nenabled = false\n\n[providers.gemini]\nenabled = false\n{mock_section}"
    );
    std::fs::write(&cfg_path, cfg_body).unwrap();

    (tmp, xdg)
}

/// D7: `AHB --detailed --color=never` against a Claude-only fake home prints
/// the documented header + 5h + weekly block (with weekly NaN sentinel
/// rendering) and contains zero ANSI escape bytes.
#[test]
fn detailed_format_emits_header_plus_indented_window_lines() {
    let (_tmp, xdg) = setup_fake_home(false);
    let home = _tmp.path();

    let assert = Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env_remove("APPDATA")
        .env_remove("NO_COLOR")
        // Plan 02: bypass D-41 keyring init on backend-less hosts.
        .env("AHB_SECRETS_MOCK", "1")
        .arg("--detailed")
        .arg("--color=never")
        .assert()
        .success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // No ANSI escape bytes — `--color=never` is the explicit override.
    assert!(
        !stdout.contains("\x1b["),
        "piped --detailed --color=never stdout must contain zero ANSI bytes, got: {stdout:?}"
    );

    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 3,
        "expected ≥ 3 lines (header + 5h + weekly), got: {stdout:?}"
    );

    // Line 0: header literal `claude` (no trailing whitespace).
    assert_eq!(
        lines[0], "claude",
        "first line must be the `claude` header: {:?}",
        lines[0]
    );

    // Line 1: 5h row — matches the documented shape. `5h` is 2 chars, padded to
    // 6 (max("5h"=2, "weekly"=6)) so the row starts with `  5h      ` then 10
    // unicode glyphs + " " + pct% + " " + sep + " resets in Xh00m".
    let re_5h = regex::Regex::new(
        r"^  5h\s+\S{10}\s+\d{1,3}%\s+\S\s+resets in \d+h\d{2}m$",
    )
    .unwrap();
    assert!(
        re_5h.is_match(lines[1]),
        "5h row shape mismatch: {:?} — expected `  5h <pad> <bar> <pct>% • resets in Xh00m`",
        lines[1]
    );

    // Line 2: weekly row — NaN sentinel path. `??%` + `(limit unknown)` suffix.
    assert!(
        lines[2].starts_with("  weekly  "),
        "weekly row must start with `  weekly  `: {:?}",
        lines[2]
    );
    assert!(
        lines[2].contains("??%"),
        "weekly row must contain `??%`: {:?}",
        lines[2]
    );
    assert!(
        lines[2].contains("(limit unknown)"),
        "weekly row must contain `(limit unknown)` footer: {:?}",
        lines[2]
    );
}

/// D8: with both claude AND mock enabled, the output contains exactly ONE
/// blank line BETWEEN the two blocks, and NO trailing blank line after the
/// last block.
#[test]
fn detailed_format_provider_separator_empty_line_between_blocks() {
    let (_tmp, xdg) = setup_fake_home(true);
    let home = _tmp.path();

    let assert = Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env_remove("APPDATA")
        .env_remove("NO_COLOR")
        .env("AHB_SECRETS_MOCK", "1")
        .arg("--detailed")
        .arg("--color=never")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);

    // The two provider blocks (claude first, mock second per BL-02 order
    // Claude=0 < Mock=3) must be separated by a `\n\n` (blank line). The
    // raw stdout ends with a single `\n` after the last block's last
    // `println!` — explicitly assert there's no `\n\n` at the tail.
    assert!(
        stdout.contains("\n\n"),
        "expected one blank line between provider blocks: {stdout:?}"
    );
    assert!(
        !stdout.ends_with("\n\n"),
        "must NOT have a trailing blank line after the last block: {stdout:?}"
    );

    // Claude block is first; mock block follows. Find both headers.
    let lines: Vec<&str> = stdout.split('\n').collect();
    let claude_idx = lines
        .iter()
        .position(|l| *l == "claude")
        .expect("claude header line must exist");
    let mock_idx = lines
        .iter()
        .position(|l| *l == "mock")
        .expect("mock header line must exist");
    assert!(
        claude_idx < mock_idx,
        "claude block must precede mock block (BL-02): claude at {claude_idx}, mock at {mock_idx}"
    );
    // Between them: at least one empty line.
    let between: Vec<&str> = lines[claude_idx + 1..mock_idx].to_vec();
    assert!(
        between.iter().any(|l| l.is_empty()),
        "expected ≥ 1 blank line between claude and mock blocks, got: {between:?}"
    );
}
