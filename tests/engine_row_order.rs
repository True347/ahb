//! BL-02 acceptance: `Engine::refresh_all` returns rows in canonical
//! `ProviderId` order (Claude=0, Codex=1, Gemini=2, Mock=3) REGARDLESS of which
//! adapter completed first.
//!
//! Without the engine-layer sort, `MockProvider` (synchronous-fast — zero `.await`
//! points in its `fetch`) would land in `join_next` BEFORE `ClaudeProvider`
//! (filesystem-bound, multiple `.await`). This test enables BOTH `claude` + `mock`,
//! seeds a synthetic `~/.claude/projects/proj-a/session.jsonl` fixture, and asserts
//! Claude appears first anyway — proving the sort holds end-to-end.
//!
//! Plan 04 BL-02. Mirrors the fixture pattern from `tests/cli_walking_skeleton.rs`
//! but consumes `Engine` directly (not via the `ahb` subprocess) so the assertion
//! is on the in-process row order rather than the rendered stdout.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::io::Write;

use ahb::config::{Config, ProviderConfig, Providers};
use ahb::engine::Engine;
use ahb::engine::cache::RowOutcome;
use ahb::model::ProviderId;
use ahb::secrets::Secrets;

/// Hand-built synthetic JSONL envelope — identical shape to the
/// `tests/cli_walking_skeleton.rs::make_fixture_jsonl` helper. Reusing the format
/// verbatim guarantees the Claude adapter finds a parseable session and returns
/// `Ok(_)` rather than `Err(Unavailable)`.
fn make_fixture_jsonl(ts: &str, cache_creation: u64) -> String {
    format!(
        r#"{{"parentUuid":"abc","isSidechain":false,"message":{{"model":"claude-opus-4-7","id":"msg_x","type":"message","role":"assistant","content":[{{"type":"text","text":"hi"}}],"stop_reason":"end_turn","usage":{{"input_tokens":5,"cache_creation_input_tokens":{cache_creation},"cache_read_input_tokens":1000,"output_tokens":186}}}},"type":"assistant","uuid":"u1","timestamp":"{ts}"}}"#
    )
}

#[tokio::test]
#[allow(clippy::unwrap_used)] // fixture setup — failures here mean the test is broken, not the code
#[allow(clippy::default_constructed_unit_structs)] // Secrets::default() matches other test files
async fn engine_refresh_all_returns_canonical_order_with_claude_and_mock_enabled() {
    // 1. tempdir + fake HOME (mirrors cli_walking_skeleton.rs setup_fake_home).
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().to_path_buf();

    // 2. Write ~/.claude/projects/proj-a/<uuid>.jsonl with one assistant entry whose
    //    timestamp is "1h ago" so the cluster is forward-looking and reset_at is in
    //    the future. Same envelope as cli_walking_skeleton.rs.
    let session_dir = home.join(".claude").join("projects").join("proj-a");
    std::fs::create_dir_all(&session_dir).unwrap();
    let session_file = session_dir.join("session.jsonl");
    let mut f = std::fs::File::create(&session_file).unwrap();
    let now_real = jiff::Timestamp::now();
    let an_hour_ago = (now_real - jiff::Span::new().hours(1)).to_string();
    writeln!(f, "{}", make_fixture_jsonl(&an_hour_ago, 4400)).unwrap();
    drop(f);

    // 3. Override HOME so directories::BaseDirs::new() (used by Engine::new for
    //    the Claude adapter's base_path) resolves to our tempdir.
    //
    //    SAFETY: integration test runs in its own process (cargo wraps each
    //    integration test file in a separate binary), and this is the only test
    //    in this file, so the mutation cannot race other tests.
    //    Rust 2024 made std::env::set_var unsafe; this is the standard test pattern.
    unsafe {
        std::env::set_var("HOME", &home);
    }

    // 4. Build the engine with BOTH claude + mock enabled. The default config has
    //    them all disabled; we override directly.
    let cfg = Config {
        providers: Providers {
            claude: ProviderConfig { enabled: true, ..Default::default() },
            mock: ProviderConfig { enabled: true, ..Default::default() },
            ..Default::default()
        },
    };
    let engine = Engine::new(cfg, Secrets::default());
    assert_eq!(engine.provider_count(), 2, "claude + mock both enabled");

    // 5. Drive a single refresh. `now` is the wall-clock snapshot the engine
    //    forwards to all adapters (clock-injection).
    let results = engine.refresh_all(now_real).await;

    // 6. Assertions — the canonical sort MUST land Claude (discriminant 0) before
    //    Mock (discriminant 3) regardless of adapter completion order.
    assert_eq!(results.len(), 2, "two rows returned");
    assert_eq!(
        results[0].0,
        ProviderId::Claude,
        "claude (discriminant 0) MUST be first per canonical order — BL-02 fix; without the sort, mock would arrive first because its fetch has zero await points"
    );
    assert_eq!(
        results[1].0,
        ProviderId::Mock,
        "mock (discriminant 3) MUST be last"
    );

    // Both must be Fresh results. Phase 3 Plan 02 change: Engine::refresh_all
    // now returns Vec<(ProviderId, RowOutcome)> instead of Vec<(_, Result<_,_>)>.
    // A successful first fetch lands as RowOutcome::Fresh(state).
    assert!(
        matches!(results[0].1, RowOutcome::Fresh(_)),
        "claude row should be Fresh with the synthetic JSONL fixture; got: {:?}",
        results[0].1
    );
    assert!(
        matches!(results[1].1, RowOutcome::Fresh(_)),
        "mock row should be Fresh; got: {:?}",
        results[1].1
    );
}
