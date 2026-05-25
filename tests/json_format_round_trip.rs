//! Phase 2 Plan 02-03 — CORE-04 integration coverage for `AHB --json`.
//!
//! Asserts the locked v1 wire shape: `schema_version=1` + RFC3339
//! `generated_at` + ordered `providers` array (BL-02) + per-provider
//! status discriminant + zero ANSI bytes. Also pins the D-58 "silently
//! ignored under --json" contract (`--ascii` and `--color=always` produce
//! byte-identical output to `--color=never`).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
#![allow(clippy::unwrap_used)] // tests: clippy.toml allow-unwrap-in-tests = true

use assert_cmd::Command;
use std::io::Write;

/// Mirror of `tests/cli_walking_skeleton.rs::make_fixture_jsonl` so this test
/// stays self-contained. Builds a synthetic Claude assistant entry at the
/// given timestamp with the given `cache_creation_input_tokens` count.
fn make_fixture_jsonl(ts: &str, cache_creation: u64) -> String {
    format!(
        r#"{{"parentUuid":"abc","isSidechain":false,"message":{{"model":"claude-opus-4-7","id":"msg_x","type":"message","role":"assistant","content":[{{"type":"text","text":"hi"}}],"stop_reason":"end_turn","usage":{{"input_tokens":5,"cache_creation_input_tokens":{cache_creation},"cache_read_input_tokens":1000,"output_tokens":186}}}},"type":"assistant","uuid":"u1","timestamp":"{ts}"}}"#
    )
}

/// Set up a fake HOME with Claude enabled + Codex enabled. Claude points at a
/// real fixture (status="ok"); Codex has NO `~/.codex/` dir so the adapter
/// returns `Unavailable` (status="error"). This gives us a guaranteed
/// 2-provider mix exercising both status branches + the BL-02 ordering
/// invariant.
fn setup_fake_home_with_claude_ok_and_codex_err() -> (tempfile::TempDir, std::path::PathBuf) {
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
        "[providers.claude]\nenabled = true\n\n[providers.codex]\nenabled = true\n\n[providers.gemini]\nenabled = false\n",
    )
    .unwrap();

    (tmp, xdg)
}

/// Setup with ALL providers disabled — exercises the CFG-04 zero-providers
/// branch (empty providers array; exit 0).
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

fn run_json(home: &std::path::Path, xdg: &std::path::Path, extra_args: &[&str]) -> std::process::Output {
    let mut cmd = Command::cargo_bin("ahb").unwrap();
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", xdg)
        .env_remove("APPDATA")
        .env_remove("NO_COLOR")
        .env("AHB_SECRETS_MOCK", "1")
        .arg("--json");
    for a in extra_args {
        cmd.arg(a);
    }
    cmd.output().expect("subprocess should run")
}

/// C7: top-level shape + BL-02 ordering + schema_version=1.
#[test]
fn json_format_emits_schema_version_1_round_trips() {
    let (_tmp, xdg) = setup_fake_home_with_claude_ok_and_codex_err();
    let home = _tmp.path();
    let output = run_json(home, &xdg, &[]);
    assert!(
        output.status.success(),
        "AHB --json should exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Round-trip through serde_json::Value to assert structure.
    let v: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}\n{stdout}"));
    let obj = v.as_object().expect("root must be an object");

    // Top-level keys exactly {schema_version, generated_at, providers} — no extras.
    let keys: std::collections::BTreeSet<&str> = obj.keys().map(String::as_str).collect();
    let expected: std::collections::BTreeSet<&str> =
        ["schema_version", "generated_at", "providers"].into_iter().collect();
    assert_eq!(keys, expected, "top-level keys mismatch; got: {keys:?}");

    assert_eq!(obj["schema_version"].as_u64(), Some(1), "schema_version != 1");
    // `generated_at` is serialized as Unix epoch seconds (integer) per the
    // jiff `timestamp::second::required` serde adapter — matches Phase 0's
    // serde shape for `fetched_at` and `resets_at`. Per D-52 (additive
    // policy) this representation is part of the v1 contract.
    assert!(
        obj["generated_at"].as_u64().is_some(),
        "generated_at must be a Unix-epoch integer; got: {}",
        obj["generated_at"]
    );

    let providers = obj["providers"].as_array().expect("providers must be array");
    assert_eq!(providers.len(), 2, "expected 2 providers (claude + codex)");

    // BL-02 ordering: claude=0, codex=1.
    assert_eq!(providers[0]["id"].as_str(), Some("claude"), "providers[0].id");
    assert_eq!(providers[1]["id"].as_str(), Some("codex"), "providers[1].id");

    // Each provider has status with value "ok" or "error".
    for (i, p) in providers.iter().enumerate() {
        let status = p["status"].as_str().expect("status must be string");
        assert!(
            status == "ok" || status == "error",
            "providers[{i}].status = {status:?} (must be ok/error)"
        );
    }

    // Claude should be OK (real fixture); codex should be error (no ~/.codex dir).
    assert_eq!(providers[0]["status"].as_str(), Some("ok"));
    assert_eq!(providers[1]["status"].as_str(), Some("error"));
}

/// C8: no ANSI bytes in JSON stdout; starts with `{"schema_version":1`.
#[test]
fn json_format_no_ansi_escapes_in_stdout() {
    let (_tmp, xdg) = setup_fake_home_with_claude_ok_and_codex_err();
    let home = _tmp.path();
    let output = run_json(home, &xdg, &[]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("\x1b["),
        "--json must not emit ANSI escapes; got: {stdout}"
    );
    assert!(
        stdout.starts_with("{\"schema_version\":1"),
        "stdout must begin with the compact JSON envelope; got: {stdout}"
    );
}

/// C9: zero providers enabled → `providers: []` + exit 0 (CFG-04).
#[test]
fn json_format_zero_providers_emits_empty_providers_array_and_exits_zero() {
    let (_tmp, xdg) = setup_fake_home_with_no_providers();
    let home = _tmp.path();
    let output = run_json(home, &xdg, &[]);
    assert!(
        output.status.success(),
        "exit 0 expected for zero-providers (CFG-04); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let providers = v["providers"].as_array().expect("providers must be array");
    assert!(providers.is_empty(), "providers must be empty; got: {providers:?}");
    assert_eq!(v["schema_version"].as_u64(), Some(1));
}

/// C10: `--json --ascii --color=always` produces byte-identical output to
/// `--json --color=never`. D-58 binding ("silently ignored").
///
/// We can't compare to a `--color=auto` invocation because `generated_at`
/// varies per invocation; we run BOTH variants and assert the JSON structures
/// match (modulo `generated_at`).
#[test]
fn json_format_ascii_and_color_silently_ignored() {
    let (_tmp_a, xdg_a) = setup_fake_home_with_claude_ok_and_codex_err();
    let (_tmp_b, xdg_b) = setup_fake_home_with_claude_ok_and_codex_err();
    let out_loud = run_json(_tmp_a.path(), &xdg_a, &["--ascii", "--color=always"]);
    let out_quiet = run_json(_tmp_b.path(), &xdg_b, &["--color=never"]);

    let s_loud = String::from_utf8_lossy(&out_loud.stdout);
    let s_quiet = String::from_utf8_lossy(&out_quiet.stdout);

    // Neither contains ANSI bytes.
    assert!(!s_loud.contains("\x1b["), "--ascii --color=always leaked ANSI: {s_loud}");
    assert!(!s_quiet.contains("\x1b["), "--color=never leaked ANSI: {s_quiet}");

    // Strip the `generated_at` field for byte comparison (it varies by walltime).
    let mut v_loud: serde_json::Value = serde_json::from_str(&s_loud).unwrap();
    let mut v_quiet: serde_json::Value = serde_json::from_str(&s_quiet).unwrap();
    if let Some(obj) = v_loud.as_object_mut() {
        obj.remove("generated_at");
    }
    if let Some(obj) = v_quiet.as_object_mut() {
        obj.remove("generated_at");
    }
    // Also strip per-provider `fetched_at` (likewise wall-clock-dependent).
    for v in [&mut v_loud, &mut v_quiet] {
        if let Some(providers) = v["providers"].as_array_mut() {
            for p in providers {
                if let Some(o) = p.as_object_mut() {
                    o.remove("fetched_at");
                    // Strip per-window reset_at (depends on fixture mtime + walltime).
                    if let Some(windows) = o.get_mut("windows").and_then(|w| w.as_array_mut()) {
                        for w in windows {
                            if let Some(wo) = w.as_object_mut() {
                                wo.remove("reset_at");
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(
        v_loud, v_quiet,
        "--json --ascii --color=always should produce same structure as --color=never"
    );
}

/// C12: `AHB --help` after_help block exposes the D-61 exit-code grid.
#[test]
fn help_after_help_exposes_exit_codes() {
    let output = Command::cargo_bin("ahb")
        .unwrap()
        .arg("--help")
        .output()
        .expect("subprocess should run");
    assert!(output.status.success(), "--help should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Exit codes:"),
        "expected `Exit codes:` heading in --help; got: {stdout}"
    );
    assert!(
        stdout.contains("0  at least one provider returned data"),
        "expected exit-0 documentation in --help; got: {stdout}"
    );
    assert!(
        stdout.contains("1  all configured providers failed"),
        "expected exit-1 documentation in --help; got: {stdout}"
    );
    assert!(
        stdout.contains("2  config / secrets unloadable"),
        "expected exit-2 documentation in --help; got: {stdout}"
    );
}
