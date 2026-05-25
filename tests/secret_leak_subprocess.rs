//! D-43 integration-tier (BLOCKER #1 path-b) — invokes the debug-built
//! `AHB --debug-emit-fake-secret` subprocess (flag gated by `#[cfg(debug_assertions)]`)
//! and asserts stdout against the same double-assert as the unit tier, plus a positive
//! assertion that the `[REDACTED]` marker IS present (proving the Serialize path actually
//! ran rather than the output being empty).
//!
//! Proves the `Secret<T>` redaction holds across a real subprocess boundary — the same
//! machinery that `--json` will use in Phase 2 CORE-04. The hidden flag is debug-build-
//! only; release builds (cargo-dist) literally cannot compile the flag.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

#![allow(clippy::unwrap_used)] // tests: clippy.toml allow-unwrap-in-tests = true

const FIXTURE: &str = "deadbeefcafe1234567890abcdef";

#[test]
#[cfg(debug_assertions)]
fn subprocess_secret_does_not_leak() {
    let output = assert_cmd::Command::cargo_bin("ahb")
        .unwrap()
        .arg("--debug-emit-fake-secret")
        .output()
        .expect("subprocess should run");
    assert!(
        output.status.success(),
        "subprocess should exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(FIXTURE),
        "stdout leaked the literal secret fixture: {stdout}"
    );
    let re = regex::Regex::new("[A-Za-z0-9]{20,}").unwrap();
    assert!(
        !re.is_match(&stdout),
        "stdout contained a high-entropy 20+-char alphanumeric run: {stdout}"
    );
    // Positive assertion: redaction marker IS present (proves the path ran).
    assert!(
        stdout.contains("[REDACTED]"),
        "expected Serialize redaction marker on stdout; got: {stdout}"
    );
}

/// Phase 2 Plan 02-03 / D-62 SEC-03 extension. Drives the same fake-secret
/// fixture through the `--json` route (`debug_emit_fake_secret_and_exit(as_json=true)`)
/// to prove that `Secret<T>::Serialize → "[REDACTED]"` holds when the secret
/// flows through the production `run_json` serialization path (top-level
/// JsonRoot-shaped envelope, not the Plan 02 sibling envelope).
///
/// Three assertions mirror the original test:
/// 1. Literal fixture absent from stdout.
/// 2. No 20+-char alphanumeric run on stdout (defense against `Display` bypass
///    that might emit the secret without going through `Serialize`).
/// 3. `[REDACTED]` marker present (positive proof the `Serialize` path ran).
#[test]
#[cfg(debug_assertions)]
fn subprocess_json_path_redacts_secret() {
    let output = assert_cmd::Command::cargo_bin("ahb")
        .unwrap()
        .arg("--json")
        .arg("--debug-emit-fake-secret")
        .output()
        .expect("subprocess should run");
    assert!(
        output.status.success(),
        "subprocess should exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(FIXTURE),
        "--json route leaked the literal secret fixture: {stdout}"
    );
    let re = regex::Regex::new("[A-Za-z0-9]{20,}").unwrap();
    assert!(
        !re.is_match(&stdout),
        "--json route emitted a high-entropy 20+-char alphanumeric run: {stdout}"
    );
    assert!(
        stdout.contains("[REDACTED]"),
        "expected `[REDACTED]` marker on --json route stdout; got: {stdout}"
    );
}

#[test]
#[cfg(not(debug_assertions))]
fn subprocess_secret_skipped_in_release() {
    eprintln!("skipped: --debug-emit-fake-secret is #[cfg(debug_assertions)]-gated");
}
