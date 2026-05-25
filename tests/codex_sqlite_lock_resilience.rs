//! Pitfall 3 guard — Codex SQLite RESERVED-lock resilience integration test.
//!
//! Scenario: a second writer thread holds a RESERVED lock on `state_5.sqlite`
//! (Codex CLI is mid-write). AHB must:
//! 1. Not crash, panic, or hang > 1.5s when its codex adapter opens the same
//!    file read-only with `busy_timeout=250ms`.
//! 2. Exit successfully (status 0 in Phase 2 — exit codes land in Plan 03).
//! 3. Emit either a `codex` Ok row OR the `codex` SchemaDrift sentinel — what
//!    we FORBID is any Network / Internal row containing "database is locked".
//!
//! Mirrors `tests/cli_walking_skeleton.rs::setup_fake_home` env grid
//! (HOME + XDG_CONFIG_HOME + AHB_SECRETS_MOCK) so the AHB subprocess runs
//! against a tempdir-isolated fake home.

#![allow(clippy::unwrap_used)] // tests: clippy.toml allow-unwrap-in-tests = true

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::Command;

fn make_token_count_line(ts: &str, used_percent: f64, resets_in_seconds: u64) -> String {
    format!(
        r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":null,"rate_limits":{{"primary":{{"used_percent":{used_percent},"window_minutes":299,"resets_in_seconds":{resets_in_seconds}}}}}}}}}"#
    )
}

#[test]
fn codex_sqlite_busy_does_not_crash_adapter() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let xdg = home.join("xdg");

    // 1. Create the codex state DB with a minimal threads table + one row.
    let codex_dir = home.join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    let db_path = codex_dir.join("state_5.sqlite");
    {
        let setup_conn = rusqlite::Connection::open(&db_path).unwrap();
        setup_conn
            .execute(
                "CREATE TABLE threads (id INTEGER PRIMARY KEY, updated_at_ms INTEGER)",
                [],
            )
            .unwrap();
        setup_conn
            .execute("INSERT INTO threads VALUES (1, 1000)", [])
            .unwrap();
        // setup_conn drops at end of scope.
    }

    // 2. Create a valid rollout file with rate_limits.primary present.
    let sessions_dir = codex_dir.join("sessions").join("2026").join("05").join("25");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let rollout_path = sessions_dir.join("rollout-test.jsonl");
    // Timestamp ~1h in the past via jiff arithmetic (kept deterministic — string
    // doesn't matter for this test, we just need parser-valid).
    let one_h_ago = jiff::Timestamp::now() - jiff::Span::new().hours(1);
    let mut f = std::fs::File::create(&rollout_path).unwrap();
    writeln!(
        f,
        "{}",
        make_token_count_line(&one_h_ago.to_string(), 25.0, 3600)
    )
    .unwrap();
    drop(f);

    // 3. Spawn writer thread that holds a RESERVED lock on state_5.sqlite.
    let stopper = Arc::new(AtomicBool::new(false));
    let writer_db_path = db_path.clone();
    let writer_stopper = Arc::clone(&stopper);
    let writer_handle = thread::spawn(move || {
        let conn = rusqlite::Connection::open(&writer_db_path).unwrap();
        // `BEGIN IMMEDIATE` acquires a RESERVED lock (writer intent) without
        // blocking concurrent readers — but the read-only open from AHB still
        // exercises the busy_timeout path because rusqlite's open path probes
        // schema state and that probe can momentarily contend.
        conn.execute("BEGIN IMMEDIATE", []).unwrap();
        conn.execute("INSERT INTO threads VALUES (2, 2000)", [])
            .unwrap();
        // Hold the lock until the test signals us to release.
        while !writer_stopper.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(10));
        }
        // Release: roll back to avoid leaving stale data; we don't actually
        // care about the write — we only want the lock held during AHB's read.
        let _ = conn.execute("ROLLBACK", []);
        drop(conn);
    });

    // Give the writer ~50ms head start to ensure the lock is held before AHB starts.
    thread::sleep(Duration::from_millis(50));

    // 4. Configure AHB with codex enabled + run as subprocess.
    let ahb_cfg = xdg.join("ahb");
    std::fs::create_dir_all(&ahb_cfg).unwrap();
    std::fs::write(
        ahb_cfg.join("config.toml"),
        "[providers.codex]\nenabled = true\n",
    )
    .unwrap();

    let start = Instant::now();
    let output = Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", &xdg)
        .env_remove("APPDATA")
        .env("NO_COLOR", "1")
        .env("AHB_SECRETS_MOCK", "1")
        .output()
        .expect("AHB subprocess should run");
    let elapsed = start.elapsed();

    // Stop + join the writer thread BEFORE assertions so a panic still cleans up.
    stopper.store(true, Ordering::SeqCst);
    writer_handle.join().expect("writer thread should not panic");

    // 5. Assertions.
    assert!(
        elapsed < Duration::from_millis(1500),
        "AHB took {elapsed:?} (> 1.5s) — Pitfall 3 RESERVED-lock guard regression"
    );
    assert!(
        output.status.success(),
        "AHB should exit 0, got status {:?}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next().unwrap_or("");
    assert!(
        first_line.starts_with("codex  "),
        "first row should start with `codex  ` (Ok row OR SchemaDrift sentinel), got: {first_line:?}\nfull stdout: {stdout}"
    );
    // What we forbid: a Network / Internal error containing "database is locked".
    assert!(
        !stdout.contains("database is locked"),
        "codex row should NOT contain `database is locked` (Pitfall 3 regression); stdout: {stdout}"
    );
}
