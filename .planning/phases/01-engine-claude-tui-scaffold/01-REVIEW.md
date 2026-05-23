---
phase: 01-engine-claude-tui-scaffold
reviewed: 2026-05-23T00:00:00Z
depth: standard
files_reviewed: 30
files_reviewed_list:
  - src/cli/mod.rs
  - src/cli/render_text.rs
  - src/cli/tty.rs
  - src/config.rs
  - src/engine/events.rs
  - src/engine/fanout.rs
  - src/engine/mod.rs
  - src/lib.rs
  - src/main.rs
  - src/provider/claude/jsonl.rs
  - src/provider/claude/mod.rs
  - src/provider/claude/window.rs
  - src/provider/mod.rs
  - src/secrets.rs
  - src/templates/default-config.toml
  - src/tui/app.rs
  - src/tui/mod.rs
  - src/tui/ui.rs
  - src/tui/widgets/hp_row.rs
  - src/tui/widgets/mod.rs
  - tests/cli_walking_skeleton.rs
  - tests/engine_row_order.rs
  - tests/first_run_init.rs
  - tests/keyring_init_sanity.rs
  - tests/no_walltime_in_adapter.rs
  - tests/panic_isolation.rs
  - tests/schema_drift_sentinel.rs
  - tests/secret_leak.rs
  - tests/secret_leak_subprocess.rs
  - tests/tui_non_tty_refusal.rs
  - tests/tui_panic_safe_restore.rs
findings:
  critical: 1
  warning: 7
  info: 6
  total: 14
status: issues_found
---

# Phase 01: Code Review Report (Re-review post gap-closure 01-04)

**Reviewed:** 2026-05-23
**Depth:** standard
**Files Reviewed:** 30 (20 source + 11 tests; one source file `src/provider/mock.rs` re-read despite being out of file-scope because the fault-injection knob is reachable in release builds — flagged below)
**Status:** issues_found

## Summary

Gap-closure plans 01-04 successfully retired all five prior findings:

| Prior finding | Status | Evidence |
|---|---|---|
| BL-01 (TUI widget reads wall clock) | **Closed** | `tui::ui::draw` plumbs `&app.now` → `hp_row::render` → `build_ok_line`. `app.now` is refreshed inside the render-tick arm at `tui/mod.rs:146`. Grep guard `tests/no_walltime_in_adapter.rs` now scans both `src/provider/` AND `src/tui/widgets/`. Verified: `grep Timestamp::now src/tui/widgets/` is empty. |
| BL-02 (nondeterministic row order) | **Closed** | `Engine::refresh_all` (`engine/mod.rs:89-105`) sorts via `sort_by_key(Self::sort_key)` with canonical order Claude=0, Codex=1, Gemini=2, Mock=3. `tests/engine_row_order.rs` enables claude + mock together and asserts claude (filesystem-bound, slower) lands at index 0 anyway. |
| BL-03 (5h gap hour-component comparison) | **Closed** | `window.rs:61-67` now uses `since(prev.timestamp)` + `gap.total(Unit::Second) > FIVE_HOURS_SECS` (strict-greater). Boundary tests cover 4h59m30s (no split), exactly 5h (no split per strict-greater doc), 5h0m30s (split). |
| WR-06 (Linux-only path in secret-store error) | **Closed** | `main.rs:78-84` resolves `config::default_path().ok()` and interpolates the cross-OS path into the error message; graceful fallback to literal "your AHB config file" if resolution fails. |
| WR-08 (`run_tui_stub` dead code) | **Closed** | `grep run_tui_stub src/` returns no matches. Removed cleanly. |

No regressions in the five closed findings.

The re-review surfaces one **new** critical-tier finding (CR-01: production-reachable panic injection via `AHB_DEBUG_PANIC`) plus carry-overs and incremental issues introduced by the gap-closure changes themselves:

1. **CR-01 (NEW):** `AHB_DEBUG_PANIC=adapter:mock` triggers a real panic in `MockProvider::fetch` in release builds, not just debug. The flag is `#[allow(clippy::panic)]`-scoped but NOT `#[cfg(debug_assertions)]`-gated. Any user environment (CI, sandboxed runner, a parent process running `AHB`) can trigger an adapter-tier panic by setting an env var — fault-injection should not survive into shipped binaries.
2. **WR-01..WR-05 carry-overs:** Schema-drift hardcoded "Claude" phrase, silent-skip codex/gemini, empty-HOME silent fallback, asymmetric error logging in `read_recent_raw`, full-file Vec load. None were in the gap-closure scope; documented here for the next planning cycle.
3. **WR-08 (new shape):** The BL-01 fix introduced a subtle staleness — the first TUI frame uses `app.now` from `AppState::new(...)` at `tui/mod.rs:108`, which is sampled BEFORE the priming fetch. With a 2s default fetch timeout the very first frame's countdown can be up to 2s stale (the next render tick at +1s refreshes it). Minor.
4. **Info items** include a `directories` qualifier-string concern, observability around `format_countdown` sub-minute truncation, an unused `EVENT_BUFFER` scaffolding constant, and the `Cluster` struct's `Option<Cluster>` swallowing of arithmetic errors.

---

## Structural Findings (fallow)

No `<structural_findings>` block was provided to this re-review. The grep-style structural checks were performed in-band as part of the standard-depth narrative sweep (see CR-01 reachability check and WR-04 grep below).

---

## Narrative Findings (AI reviewer)

## Blocker / Critical Issues

### CR-01: `AHB_DEBUG_PANIC=adapter:mock` triggers a real panic in production release builds

**File:** `src/provider/mock.rs:27-35`
**Severity:** BLOCKER (security / robustness)
**Issue:**
```rust
if std::env::var_os("AHB_DEBUG_PANIC").as_deref()
    == Some(std::ffi::OsStr::new("adapter:mock"))
{
    #[allow(clippy::panic)]
    {
        panic!("AHB_DEBUG_PANIC injected");
    }
}
```

The block is `#[allow(clippy::panic)]`-scoped but NOT `#[cfg(debug_assertions)]`-gated. Compare to the sibling fault-injection knob `cli::debug_emit_fake_secret_and_exit` (`cli/mod.rs:113`) and the secrets `AHB_SECRETS_MOCK=1` affordance (`secrets.rs:115`), both of which ARE `#[cfg(debug_assertions)]`-gated and therefore literally cannot compile into a `cargo-dist` release artifact.

Concrete consequences in a release-built `ahb` binary:

1. **User-reachable adapter panic.** A user enables `[providers.mock] enabled = true` (the "power-user knob" per `config.rs:46-50`), inherits `AHB_DEBUG_PANIC=adapter:mock` from a parent shell or systemd unit, and runs `AHB` — the mock adapter panics every fetch. Pitfall L4's `JoinSet::join_next_with_id()` recovery (`fanout.rs:67-99`) catches this and renders `mock  ERROR: adapter panicked: Mock` instead of crashing the binary; however,
2. **Repeated panics** spam the `tracing::error!` log at `fanout.rs:77` once per fetch tick (every 15s in TUI mode), and the Phase 0 panic hook (`main.rs:25-28`) writes `ahb panicked: ...` to stderr on every adapter panic — `tail -F` on the user's log file fills with red noise.
3. **Test infrastructure leak.** `tests/panic_isolation.rs:45` and `tests/tui_panic_safe_restore.rs:62` are the only intentional callers. The flag has no documented production purpose. Shipping it is a "test-only knob that escaped into the release binary" anti-pattern — exactly the failure the `--debug-emit-fake-secret` and `AHB_SECRETS_MOCK` `#[cfg(debug_assertions)]` gating was designed to prevent.

The `PATTERNS.md provider/mock.rs (modified)` comment cites integration-test usage, but does NOT say production users must be able to trigger this — and the comment is not a substitute for compile-time gating.

**Fix:**
```rust
// src/provider/mock.rs
async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
    #[cfg(debug_assertions)]
    if std::env::var_os("AHB_DEBUG_PANIC").as_deref()
        == Some(std::ffi::OsStr::new("adapter:mock"))
    {
        #[allow(clippy::panic)]
        {
            panic!("AHB_DEBUG_PANIC injected");
        }
    }
    // ... rest unchanged
}
```

This matches the gating style of `secrets::init()` at `secrets.rs:115` and `Cli::debug_emit_fake_secret` at `cli/mod.rs:37`. Both `tests/panic_isolation.rs` and `tests/tui_panic_safe_restore.rs` compile against the debug binary (cargo's test target uses dev profile by default), so the gate keeps the tests working while removing the release-binary attack surface.

---

## Warnings

### WR-01: SchemaDrift sentinel hard-codes "Claude adapter may be out-of-date" for all providers (carry-over)

**File:** `src/cli/render_text.rs:127`, `src/tui/widgets/hp_row.rs:114`
**Issue:** Carry-over from prior 01-REVIEW (not in gap-closure scope). The `id_label(id)` resolution applied to the row prefix but NOT to the trailing phrase. When Phase 2's Codex adapter (or anyone else) emits `ProviderError::SchemaDrift`, the row reads:

```
codex  ▒▒▒▒▒▒▒▒▒▒ ??% • Claude adapter may be out-of-date
```

— misleading because the literal "Claude" is now structurally wrong. `ProviderError::SchemaDrift` is constructed today only in `provider/claude/mod.rs:93`, so the bug is latent; but the contract isn't enforced by the type system.

**Fix:** Parametrize the phrase by `id_label(id)`:
```rust
let phrase = format!("{} adapter may be out-of-date", id_label(id));
```

Apply the same change in `tui/widgets/hp_row.rs:114`. Update the byte-exact `SENTINEL` in `tests/schema_drift_sentinel.rs:15` to match (the test already uses `claude  ` so the constant just becomes `format!`-derived).

---

### WR-02: Enabled-but-unimplemented codex/gemini providers silently disappear (carry-over)

**File:** `src/engine/mod.rs:59-66`
**Issue:** Carry-over.
```rust
if cfg.providers.codex.enabled {
    tracing::debug!("providers.codex.enabled is true but Codex adapter is Phase 2 — skipping");
}
if cfg.providers.gemini.enabled {
    tracing::debug!(...);
}
```

`tracing::debug!` is suppressed at default `EnvFilter` levels. A user who enables `[providers.codex] enabled = true` and runs `AHB` sees NO codex output, NO error, NO warning. They cannot distinguish "my config is wrong" from "the adapter crashed" from "AHB hasn't implemented codex yet."

**Fix:** Either escalate to `tracing::warn!` (visible at default level) AND surface a one-time stderr notice, or push a stub `NotImplementedProvider` that always returns `Unavailable { reason: "codex adapter not yet implemented (Phase 2)" }` so the user sees an `codex  ERROR:` row.

---

### WR-03: Empty-or-unset HOME on Linux silently falls back to CWD for ClaudeProvider (carry-over)

**File:** `src/engine/mod.rs:49-58`
**Issue:** Carry-over.
```rust
let home = directories::BaseDirs::new()
    .map(|d| d.home_dir().to_path_buf())
    .unwrap_or_default();   // PathBuf::new() = empty
providers.push(... ClaudeProvider::new(&home, ...));
```

When `BaseDirs::new()` returns `None` (HOME unset, locked-down env), `unwrap_or_default()` yields an empty `PathBuf`. `ClaudeProvider::new` then constructs `base_path = "".join(".claude").join("projects")` — a RELATIVE path resolved against CWD at fetch time. If CWD happens to be the user's real home, the adapter would read real session data even though config resolution silently failed.

Note the symmetric path in `main.rs:53` for config resolution DOES propagate the error via `?`. Only the Claude adapter's base path swallows the failure.

**Fix:** Either fail closed (return `Err` from `Engine::new` when HOME is unavailable) or push a stub provider that returns `Unavailable { reason: "could not resolve home dir" }`. Do not silently degrade to a relative path.

---

### WR-04: `read_recent_raw` and `read_assistant_entries` have asymmetric error handling (carry-over)

**File:** `src/provider/claude/jsonl.rs:124-148` vs `:71-111`
**Issue:** Carry-over. The two functions read the SAME files. `read_assistant_entries` emits `tracing::warn!` on file-open errors and mid-file parse errors; `read_recent_raw` silently swallows ALL errors via `let Ok(...) = ... else { return / continue }`. The drift-detector path therefore has zero observability — a permission error on the newest file produces drift = `None` indistinguishable from "no drift."

Verified persistence with grep: `grep -n "warn" src/provider/claude/jsonl.rs` shows three `tracing::warn!` calls — all in `read_assistant_entries` + `discover_session_files`, none in `read_recent_raw`.

**Fix:** Mirror the logging in `read_recent_raw`:
```rust
let file = match File::open(path) {
    Ok(f) => f,
    Err(e) => {
        tracing::warn!("read_recent_raw: could not open {}: {e}", path.display());
        return Vec::new();
    }
};
// inside the loop, warn on Err(line_res) and parse failure too.
```

---

### WR-05: `read_recent_raw` loads entire file before truncating to last N (carry-over)

**File:** `src/provider/claude/jsonl.rs:124-148`
**Issue:** Carry-over (performance — flagged because it compounds with WR-04 schema-drift unobservability on long sessions).

```rust
let mut all: Vec<serde_json::Value> = Vec::new();
for line_res in reader.lines() { ... all.push(v); }
if all.len() > n { all.drain(..(all.len() - n)); }
```

Parses every line as `serde_json::Value`, then discards all but the last `n`. For a long-running Claude session (thousands of assistant messages), every fetch tick (every 15s in TUI mode) re-parses the entire file. The schema-drift detector only needs the last 3.

**Fix:** Use a fixed-size `VecDeque<Value>` ring buffer or read backward. Performance is technically out-of-v1-scope per the review charter, but the rolling 15s tick lifts this past "irrelevant micro-allocation" into "noticeable IO + parse load."

---

### WR-06: `[secrets].storage = "file"` referenced in error message but not implemented in Config

**File:** `src/main.rs:73-84`, `src/config.rs:53-57`
**Issue:** The error message at `main.rs:83` instructs the user to set `[secrets].storage = "file"` in their config to opt into 0600 file storage:
```
no secret store available on this system; set [secrets].storage = "file" in {cfg_path_display} to opt into 0600 file storage
```

But the `Config` struct (`config.rs:53-57`) has NO `[secrets]` field, and `KNOWN_PROVIDER_KEYS` at `config.rs:26` does not include `secrets`. A user who follows this instruction sees their `[secrets]` key emit `tracing::warn!("unrecognized config key 'secrets'")` (visible if `RUST_LOG` is set) and the binary exits 2 anyway because `secrets::init()` still cannot find a backend.

`main.rs:69-73` acknowledges this:
```
// TODO(future-phase): the [secrets].storage = "file" escape-hatch is
// documented intent but not yet wired in Config.
```

Acknowledged ≠ fixed. The user-facing copy promises a fix that doesn't exist.

**Fix:** Either (a) implement the escape hatch (add `[secrets]` section to `Config`, wire `secrets::init()` to honor `storage = "file"` with a TOML-backed 0600 file fallback), OR (b) rewrite the message to remove the false promise:
```
no secret store available on this system; ahb requires an OS keyring (libsecret/Keychain/CredMan). See README for headless-Linux setup.
```

Option (b) is the lower-risk patch for this phase.

---

### WR-07: `tui::ui::draw` Constraint::Min(rows_len) can starve the quit-hint area (carry-over)

**File:** `src/tui/ui.rs:57-64`
**Issue:** Carry-over (unchanged since prior review).
```rust
let rows_len = u16::try_from(app.rows.len().max(1)).unwrap_or(1);
let [_top_pad, body, _footer_pad, hint_area] = Layout::vertical([
    Constraint::Length(1),
    Constraint::Min(rows_len),
    Constraint::Length(1),
    Constraint::Length(1),
]).areas(inner_area);
```

`Min(rows_len)` says "body must be AT LEAST rows_len rows tall." With 4 enabled providers and a 7-row inner area, required is `1+4+1+1=7` — fits. With 6 enabled providers (Claude + Codex + Gemini + Mock + 2 future) and a 7-row inner area, required is `1+6+1+1=9` — body wins the contention and the quit hint silently disappears. The early-return at `ui.rs:51` only catches `inner_area.height < 4`.

This was acknowledged in the prior review and not addressed in gap-closure. Worth tracking because Phase 2/3 will plausibly add a 4th provider.

**Fix:**
```rust
let max_body = inner_area.height.saturating_sub(3);  // reserve top_pad + footer_pad + hint
let rows_len = u16::try_from(app.rows.len()).unwrap_or(0).min(max_body).max(1);
```

---

## Info

### IN-01: `AppState::new(Timestamp::now())` seed is stale by the duration of the prime fetch (BL-01 fix side effect)

**File:** `src/tui/mod.rs:108-114`
**Issue:** The BL-01 fix introduced an `app.now` field and authorizes ONLY the render-tick arm to update it (`tui/mod.rs:146`). However, `tui_loop`'s initial sequence is:
```rust
let mut app = app::AppState::new(jiff::Timestamp::now());     // T0
let results = engine.refresh_all(jiff::Timestamp::now()).await; // T1 (T0 + up to 2s timeout)
app.apply_results(results);
terminal.draw(|f| ui::draw(f, &app))?;                          // uses app.now == T0
```

The first frame draws with `app.now = T0`, but the wall clock at draw time is `T1 ≥ T0`. The countdown is therefore stale by up to the per-provider timeout (default 2s, per `DEFAULT_PER_PROVIDER_TIMEOUT`). The next render tick at +1s refreshes `app.now`, so the user only sees this for ~1s.

Minor — but the BL-01 fix's docstring promises "the render-tick arm is the SINGLE authorized wall-clock site in the TUI render path," and this first-frame draw violates that letter (the wall clock used IS the render path, just snapshotted earlier).

**Fix:** Refresh `app.now` immediately before the prime draw:
```rust
let mut app = app::AppState::new(jiff::Timestamp::now());
let results = engine.refresh_all(jiff::Timestamp::now()).await;
app.apply_results(results);
app.now = jiff::Timestamp::now();   // <-- add: keep the first frame fresh
terminal.draw(|f| ui::draw(f, &app))?;
```

Or fold the prime into a manual "fire one fetch_tick early" so the existing render-tick arm handles it.

---

### IN-02: `cli::run_compact` reuses one `now` snapshot for both fetch and render (countdown drift)

**File:** `src/cli/mod.rs:66-79`
**Issue:** `run_compact` snapshots `now` once at line 66, passes it to `engine.refresh_all(now)`, then re-uses the SAME `now` for each `compact_line_colored(&state, &now, ...)` countdown. Per-provider fetch can take up to 2s (default timeout), so the printed countdown is stale by 0-2s relative to the actual wall clock at print time.

For a CLI one-shot invocation this is invisible to the user (the value is "good enough"). The contract is that `refresh_all` and the rendered countdown share the same `now` — that's CORRECT behavior (the fetch's `fetched_at` and the rendered countdown should be consistent). This Info item is descriptive, not prescriptive. No fix recommended; just noting that "share one `now` snapshot" is the design choice.

---

### IN-03: `EVENT_BUFFER = 64` and `EngineEvent` enum are unused scaffolding in Phase 1

**File:** `src/engine/events.rs:14,21-31`
**Issue:** `tui_loop` calls `engine.refresh_all(...)` directly inside the `select!` arm; no `mpsc::channel(EVENT_BUFFER)` is constructed anywhere in Phase 1. The module doc claims "Plan 03's TUI subscribes via `mpsc::Receiver<EngineEvent>`," but Plan 03 actually drives `refresh_all` from inside the TUI loop directly.

This is dead scaffolding for a future phase. Harmless, but the module doc and the code disagree about whether the channel is wired today.

**Fix:** Update the module doc:
```rust
//! Engine event channel types. Reserved for a future engine-driver-loop phase
//! where the TUI subscribes via `mpsc::Receiver<EngineEvent>`. **Phase 1 is NOT
//! a consumer** — `tui::tui_loop` calls `engine.refresh_all` directly inside the
//! fetch-tick arm. The enum + buffer constant stay in tree so future plans land
//! without touching `engine/`.
```

---

### IN-04: `find_active_cluster` silently swallows `Span::since` and `Span::total` errors

**File:** `src/provider/claude/window.rs:61-66`
**Issue:** The BL-03 fix uses:
```rust
let Ok(gap) = curr.timestamp.since(prev.timestamp) else { continue };
let Ok(gap_secs) = gap.total(jiff::Unit::Second) else { continue };
if gap_secs > FIVE_HOURS_SECS { ... }
```

Both `continue` branches silently skip the comparison. In practice `jiff::Timestamp::since` and `Span::total` on contiguous timestamps will not fail, BUT a `tracing::warn!` on the `else` arms costs nothing and makes any future regression observable:
```rust
let gap = match curr.timestamp.since(prev.timestamp) {
    Ok(g) => g,
    Err(e) => {
        tracing::warn!("since failed on cluster boundary check: {e}");
        continue;
    }
};
```

Same for `total`. Defensive, not blocking.

---

### IN-05: `directories::ProjectDirs::from("", "", "ahb")` empty qualifier (carry-over)

**File:** `src/config.rs:78`
**Issue:** Carry-over. `ProjectDirs::from("", "", "ahb")` (empty qualifier + organization) works on current `directories` 6.x but is documented as "implementation-defined" for empty args. A future major bump could reject this. Use a deliberate qualifier `from("dev", "ahb", "ahb")` OR pin `directories` to `=6.x` and document the constraint.

---

### IN-06: `secrets::init()` calls `set_default_store` which is one-shot global state

**File:** `src/secrets.rs:124,136`
**Issue:** `keyring_core::set_default_store(...)` is a process-global one-time write. Calling `secrets::init()` twice in the same process (e.g., in a hypothetical future daemon mode with hot-reload, or in a unit test that doesn't fork) would either fail or silently replace. Phase 1 is safe because integration tests are one-process-per-test-file and `main.rs` calls `init()` exactly once. But if anyone refactors to call `init()` from a long-running path, this becomes a latent bug.

**Fix:** Document the one-shot constraint in the `secrets::init` doc:
```rust
/// **One-shot:** `keyring_core::set_default_store` is a process-global write that
/// can only succeed once per process. Tests that exercise both code paths must
/// run as separate integration-test binaries (cargo's default) rather than as
/// unit tests in the same binary.
```

---

_Reviewed: 2026-05-23_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
