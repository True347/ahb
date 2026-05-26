//! D-43 unit-tier secret-leak grep test.
//!
//! Asserts that a `Secret<T>` carrying a high-entropy fixture string emits NEITHER
//! the literal fixture NOR any 20-char alphanumeric run through either the `Debug`
//! impl OR the `Serialize` impl. Catches both "direct leak" and "encoded leak"
//! (CONTEXT D-43 binding).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

#![allow(clippy::unwrap_used)] // tests: clippy.toml allow-unwrap-in-tests = true

use ai_hp_bar::secrets::Secret;

const FIXTURE: &str = "deadbeefcafe1234567890abcdef";

#[test]
fn debug_does_not_leak() {
    let s = Secret::new(FIXTURE.to_string());
    let d = format!("{s:?}");
    assert!(!d.contains(FIXTURE), "Debug leaked literal fixture: {d}");
    let re = regex::Regex::new("[A-Za-z0-9]{20,}").unwrap();
    assert!(
        !re.is_match(&d),
        "Debug emitted a 20+-char alphanumeric run: {d}"
    );
    assert_eq!(d, "***", "Debug must produce exactly '***' (D-42 binding)");
}

#[test]
fn serde_does_not_leak() {
    let s = Secret::new(FIXTURE.to_string());
    let j = serde_json::to_string(&s).unwrap();
    assert!(!j.contains(FIXTURE), "Serialize leaked literal fixture: {j}");
    let re = regex::Regex::new("[A-Za-z0-9]{20,}").unwrap();
    assert!(
        !re.is_match(&j),
        "Serialize emitted a 20+-char alphanumeric run: {j}"
    );
    assert_eq!(
        j, "\"[REDACTED]\"",
        "Serialize must produce exactly '\"[REDACTED]\"' (D-42 binding)"
    );
}
