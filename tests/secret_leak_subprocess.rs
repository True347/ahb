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

#[test]
#[cfg(not(debug_assertions))]
fn subprocess_secret_skipped_in_release() {
    eprintln!("skipped: --debug-emit-fake-secret is #[cfg(debug_assertions)]-gated");
}
