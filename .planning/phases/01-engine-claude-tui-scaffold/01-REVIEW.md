---
phase: 01-engine-claude-tui-scaffold
reviewed: 2026-05-23T00:00:00Z
depth: standard
files_reviewed: 28
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
  critical: 0
  blocker: 3
  warning: 8
  info: 5
  total: 16
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-05-23
**Depth:** standard
**Files Reviewed:** 28 (20 source + 10 tests; total 30 minus dupes)
**Status:** issues_found

## Summary

Phase 01 wires the engine spine, the Claude JSONL adapter, the TUI fixed-frame surface, and the secrets/keyring boundary end-to-end. The implementation is generally careful — clock injection is enforced via a grep test for `src/provider/`, panic isolation is verified via a real pty integration test, and secret redaction is double-asserted (unit + subprocess). However the adversarial sweep surfaced multiple correctness defects that need to ship-block:

1. **The TUI HP-row widget bypasses clock injection** (`hp_row.rs:73` calls `jiff::Timestamp::now()` mid-render). The grep guard in `tests/no_walltime_in_adapter.rs` only scans `src/provider/`, so this slipped past the test net — but it breaks the same testability seam the provider rule was designed to preserve.
2. **Compact-line output is in nondeterministic order**. `fanout::refresh_all_inner` returns rows in `join_next` arrival order; `cli::run_compact` iterates them as-is. Running `AHB` repeatedly will shuffle "claude / codex / gemini" rows on every invocation, contradicting the UI-SPEC fixed-row mock and degrading the multi-provider aligned-bar promise.
3. **The 5h cluster gap check uses hour-component comparison** (`window.rs:50-51`). Comparing `Span::get_hours() >= 5` treats a 5h 0m gap as a split (correct) but the code comment promises "first gap **> 5h**" — an off-by-one against the doc + a measurement granularity that ignores minutes/seconds.

Several quality findings about silent provider-disabled behavior, hardcoded UI strings, fallback path corruption when `HOME` is unset, and inconsistent error-handling between `read_recent_raw` and `read_assistant_entries` round out the warning tier.

---

## Blocker Issues

### BL-01: TUI row widget reads wall clock directly, breaking clock-injection contract

**File:** `src/tui/widgets/hp_row.rs:73`
**Issue:**
```rust
let countdown = format_countdown(&jiff::Timestamp::now(), &w.reset.resets_at);
```
`build_ok_line` is invoked from `ui::draw` on every 1s render tick. The clock-injection rule (`provider/mod.rs:24` docstring, `tests/no_walltime_in_adapter.rs`) was specifically designed so that adapter + UI layers can be tested with a frozen clock. The render-side `Timestamp::now()` call:

1. Defeats the only structural seam `ProviderState` provides for testing render output (`fetched_at` is set per-fetch but the countdown is recomputed from wall clock).
2. Reads wall clock per render tick (1Hz) rather than per fetch tick (15s) — causes the countdown to update without re-fetching, which is the intended behavior for D-31 BUT couples the renderer to a non-injectable singleton.
3. Is not covered by the `no_walltime_in_adapter.rs` grep because that walker is scoped to `src/provider/`. The TUI layer escapes the guard.

The `src/tui/mod.rs` module doc explicitly says "TUI is structurally main-adjacent (the entry-point to a long-running surface), so `tui_loop` is the second authorized callsite" — `tui_loop` is fine, but `hp_row::build_ok_line` is a leaf render fn that should receive `now` from its caller (carry it down through `ui::draw(f, &app, now)` or store it on `AppState`).

**Fix:**
```rust
// src/tui/app.rs
pub struct AppState {
    pub rows: Vec<RowState>,
    pub now: jiff::Timestamp,           // <-- add
}

// src/tui/mod.rs tui_loop — set on every fetch tick AND every render tick:
_ = render_tick.tick() => {
    app.now = jiff::Timestamp::now();   // single authorized wall-clock site
    terminal.draw(|f| ui::draw(f, &app))?;
}

// src/tui/widgets/hp_row.rs build_ok_line — take now from caller:
fn build_ok_line(state: &ProviderState, now: &jiff::Timestamp) -> Line<'static> {
    ...
    let countdown = format_countdown(now, &w.reset.resets_at);
    ...
}
```

Then extend `tests/no_walltime_in_adapter.rs` (or add a sibling) to scan `src/tui/widgets/` so this regression cannot reappear.

---

### BL-02: Compact-line provider rows are emitted in nondeterministic order

**File:** `src/engine/fanout.rs:67-99`, `src/cli/mod.rs:74-87`
**Issue:** `fanout::refresh_all_inner` collects results via `set.join_next_with_id().await` in arrival order — fast adapters land first, slow adapters last. The module doc admits this:
> "returns a `Vec<(ProviderId, Result<...>)>` whose order matches `join_next` arrival (NOT input order — Phase 0's contract is unordered, callers must look up by id)."

But `cli::run_compact` iterates the returned Vec without resorting:
```rust
for (id, result) in results {
    match result { ... println!(...) }
}
```

Concrete consequence: a user with all three providers enabled will see rows in random order between invocations. With Claude reading local JSONL (fast) and Codex/Gemini still stubs (no fetch), Phase 1 hides this, but the moment Phase 2/3 lands the rows will jitter. This contradicts the UI-SPEC LOCKED ordering shown in the mockup ("claude / codex / gemini") and breaks visual alignment between adjacent runs (a regression the user will notice immediately — "why did the order change?").

The TUI surface inherits the same bug via `AppState::apply_results` (`tui/app.rs:53-63`) which preserves arrival order on every fetch tick — so on each 15s tick the row order can shuffle.

**Fix:** Sort results by `ProviderId` discriminant order (or by config-declared order) at the engine boundary, OR sort at the CLI/TUI consumer. Simplest is at the engine — one canonical order, both front-ends benefit:
```rust
// src/engine/mod.rs refresh_all
pub async fn refresh_all(&self, now: jiff::Timestamp)
    -> Vec<(ProviderId, Result<ProviderState, ProviderError>)>
{
    let mut results = fanout::refresh_all_inner(...).await;
    results.sort_by_key(|(id, _)| Self::sort_key(*id));  // canonical order
    results
}

fn sort_key(id: ProviderId) -> u8 {
    match id {
        ProviderId::Claude => 0,
        ProviderId::Codex => 1,
        ProviderId::Gemini => 2,
        ProviderId::Mock => 3,
    }
}
```

Add a test asserting that `refresh_all` returns rows in this fixed order regardless of which adapter finished first (use `SlowProvider` + `OkProvider` mix as in `fanout::tests`).

---

### BL-03: Cluster gap detection compares only the hour component; doc/code disagreement

**File:** `src/provider/claude/window.rs:46-55`
**Issue:**
```rust
let Ok(gap) = curr.timestamp.since((jiff::Unit::Hour, prev.timestamp)) else {
    continue;
};
let gap_hours = gap.get_hours();
if gap_hours >= 5 {
    start_idx = i;
    break;
}
```

Two problems:

1. **Code says `>= 5`, comment + module doc say `> 5h`**. Module doc (`window.rs:6`): *"find the first gap > 5 h"*. A 5h 0m 0s gap currently triggers a split; per the doc, only gaps *strictly greater than* 5h should. Tighter: a 5h 0m 1s gap is what the doc author probably meant. The 5-vs->5 distinction matters because Claude's session window is exactly 5h — a session that ends exactly 5h after start is a boundary case, not a clean separator.

2. **`get_hours()` is hour-component only, not total hours**. With `since((Unit::Hour, prev))`, the returned `Span` decomposes as `H hours + M minutes + S seconds` (jiff's largest-unit semantics). So a gap of 4h 59m 59s has `get_hours() == 4` (correctly under threshold) but a gap of 5h 0m 1s also has `get_hours() == 5` (correctly above threshold). The bug surfaces near the exact 5h boundary: a 5h 0m 0s gap reports `get_hours() == 5` and triggers the split, but a 4h 59m 30s gap reports `get_hours() == 4` and does NOT split — a 30-second swing across the integer-hour boundary changes cluster membership by a full session. The test fixtures use hour-aligned timestamps so this isn't caught.

**Fix:** Convert the gap to a total-seconds (or total-minutes) scalar for comparison, and align the operator with the doc:
```rust
// jiff::Span.total(Unit::Second) is the rigorous comparison.
let Ok(gap) = curr.timestamp.since(prev.timestamp) else { continue };
let Ok(gap_secs) = gap.total(jiff::Unit::Second) else { continue };
const FIVE_HOURS_SECS: f64 = 5.0 * 3600.0;
if gap_secs > FIVE_HOURS_SECS {           // strict-greater matches the doc
    start_idx = i;
    break;
}
```

Add a unit test with a 4h 59m 30s gap (should NOT split) and a 5h 0m 30s gap (should split) to lock the boundary.

---

## Warnings

### WR-01: SchemaDrift sentinel hard-codes "Claude adapter may be out-of-date" for all providers

**File:** `src/cli/render_text.rs:127`, `src/tui/widgets/hp_row.rs:99`
**Issue:** Both renderers paint the literal phrase `Claude adapter may be out-of-date` regardless of which provider's adapter drifted. The `id_label(id)` resolution (WARNING #5 in plan notes) was applied to the row PREFIX but not to the trailing phrase. If Phase 2's Codex adapter ever emits `ProviderError::SchemaDrift`, the row will read:
```
codex  ▒▒▒▒▒▒▒▒▒▒ ??% • Claude adapter may be out-of-date
```
which is misleading. The current code path only constructs `SchemaDrift` from Claude (`provider/claude/mod.rs:93`), so it's latent, but the contract isn't enforced by the type system.

**Fix:** Either (a) parametrize the phrase by `id_label(id)`:
```rust
let phrase = format!("{} adapter may be out-of-date", id_label(id));
```
or (b) document that `SchemaDrift` is only emittable by the Claude adapter and add a compile-time assertion (e.g., a sealed trait or a `#[doc(hidden)]` constructor).

---

### WR-02: Enabled-but-unimplemented providers (codex, gemini) silently disappear

**File:** `src/engine/mod.rs:59-66`
**Issue:**
```rust
if cfg.providers.codex.enabled {
    tracing::debug!("providers.codex.enabled is true but Codex adapter is Phase 2 — skipping");
}
if cfg.providers.gemini.enabled {
    tracing::debug!("providers.gemini.enabled is true but Gemini adapter is Phase 3 — skipping");
}
```

`tracing::debug!` is suppressed at default `EnvFilter` levels. A user who follows the README, enables `[providers.codex] enabled = true`, and runs `AHB` sees ZERO codex output — neither row, nor error, nor warning. They can't tell whether their config is malformed, the adapter crashed, or codex isn't implemented yet.

**Fix:** Render an explicit "not yet implemented" error row so the user sees that their config WAS read and the gap is in AHB, not their setup:
```rust
if cfg.providers.codex.enabled {
    // Push a stub provider that always returns Unavailable with the canonical reason.
    providers.push(Arc::new(NotImplementedProvider::new(
        ProviderId::Codex,
        "codex adapter not yet implemented (Phase 2)",
    )));
}
```

Alternatively, escalate the log to `tracing::warn!` so it surfaces at default filter level AND adjust the docs to say "set `RUST_LOG=ahb=warn` to see skipped providers."

---

### WR-03: Empty-or-unset HOME on Linux silently falls back to CWD

**File:** `src/engine/mod.rs:49-58`
**Issue:**
```rust
let home = directories::BaseDirs::new()
    .map(|d| d.home_dir().to_path_buf())
    .unwrap_or_default();   // <-- PathBuf::new() = ""
providers.push(... ClaudeProvider::new(&home, ...));
```

When `BaseDirs::new()` returns `None` (HOME unset on Linux, or rare locked-down environments), `unwrap_or_default()` yields an empty `PathBuf`. `ClaudeProvider::new` then joins it: `"".join(".claude").join("projects") == ".claude/projects"` — a relative path resolved against CWD at fetch time. If the user's CWD happens to be their real home, the adapter would read REAL session data even though config resolution failed. More likely it returns `Unavailable`, but the failure mode is wrong (silent vs. explicit).

**Fix:** Treat the missing-home case as an explicit error row rather than silently using `PathBuf::new()`:
```rust
match directories::BaseDirs::new() {
    Some(d) => providers.push(Arc::new(ClaudeProvider::new(d.home_dir(), CLAUDE_5H_TOKEN_LIMIT))),
    None => {
        // Push a stub adapter that always returns Unavailable with a specific reason.
        // Or refuse engine construction with an anyhow::Error.
        tracing::warn!("could not resolve home dir; claude adapter disabled");
    }
}
```

---

### WR-04: `read_recent_raw` and `read_assistant_entries` have inconsistent error handling

**File:** `src/provider/claude/jsonl.rs:124-148` vs `:71-111`
**Issue:** `read_assistant_entries` warns on mid-file IO errors and parse failures (`tracing::warn!`). `read_recent_raw` silently discards them all:
```rust
let Ok(line) = line_res else { continue };
...
let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
    continue;
};
```

These two functions read the SAME file (the most-recent session), so a malformed line that triggers a warning via the typed reader will produce zero observability through the raw-Value reader. That makes the drift detector silently disagree with the typed parser. Worse, `read_recent_raw` also doesn't `tracing::warn!` on file-open failure, while `read_assistant_entries` does — so a permission-denied error on the newest file would log via one path and not the other.

**Fix:** Mirror the logging in `read_recent_raw`:
```rust
let file = match File::open(path) {
    Ok(f) => f,
    Err(e) => {
        tracing::warn!("read_recent_raw: could not open {}: {e}", path.display());
        return Vec::new();
    }
};
// ... and inside the loop, warn on Err(line_res) like read_assistant_entries does.
```

---

### WR-05: `read_recent_raw` loads entire file before truncating to last N

**File:** `src/provider/claude/jsonl.rs:124-148`
**Issue:**
```rust
let mut all: Vec<serde_json::Value> = Vec::new();
for line_res in reader.lines() {
    ...
    all.push(v);
}
if all.len() > n {
    let start = all.len() - n;
    all.drain(..start);
}
```

Reads the entire file into memory, parses every line as `serde_json::Value`, then discards all but the last `n`. For a long-running Claude session JSONL file (some users have thousands of assistant messages per project), this allocates and parses ~N× more than needed every fetch tick (every 15s in TUI mode).

This is in the v1-out-of-scope "performance" tier per the review charter, BUT the contract issue is real: if the schema-drift detector runs every tick, the memory pressure compounds. At minimum, document this; ideally use a ring buffer or backward-read.

**Fix:** Use a fixed-size ring buffer to hold the last N entries while streaming forward:
```rust
let mut ring: std::collections::VecDeque<serde_json::Value> = VecDeque::with_capacity(n);
for line_res in reader.lines() {
    ...
    if v.get("type").and_then(|t| t.as_str()) == Some("assistant") {
        if ring.len() == n { ring.pop_front(); }
        ring.push_back(v);
    }
}
ring.into_iter().collect()
```

---

### WR-06: Configuration path in error message is hardcoded to Linux convention

**File:** `src/main.rs:69`
**Issue:**
```rust
eprintln!(
    "no secret store available on this system; set [secrets].storage = \"file\" in ~/.config/ahb/config.toml to opt into 0600 file storage"
);
```

On macOS the path is `~/Library/Application Support/ahb/config.toml`; on Windows it's `%APPDATA%\ahb\config.toml`. The error message tells the user to edit a path that doesn't exist on their OS — they'll create a Linux-style file that the binary will never read.

Note also: `[secrets].storage = "file"` is documented here but the `Config` struct in `src/config.rs:53-57` has NO `[secrets]` section. The error suggests a fix that the code doesn't support.

**Fix:** Use the resolved config path:
```rust
let cfg_path = config::default_path().ok();
eprintln!(
    "no secret store available on this system; set [secrets].storage = \"file\" in {} to opt into 0600 file storage",
    cfg_path.as_ref().map_or_else(|| "your AHB config file".to_string(), |p| p.display().to_string()),
);
```
And EITHER add the `[secrets]` section to `Config`, OR rewrite the message to point at the documented (but not-yet-implemented) escape hatch.

---

### WR-07: `tui::ui::draw` uses `Constraint::Min(rows_len)` which can starve other chunks

**File:** `src/tui/ui.rs:57-64`
**Issue:**
```rust
let rows_len = u16::try_from(app.rows.len().max(1)).unwrap_or(1);
let [_top_pad, body, _footer_pad, hint_area] = Layout::vertical([
    Constraint::Length(1),
    Constraint::Min(rows_len),
    Constraint::Length(1),
    Constraint::Length(1),
]).areas(inner_area);
```

`Constraint::Min(rows_len)` says "the body must be AT LEAST rows_len rows tall". If `inner_area.height = 5` and `app.rows.len() = 4`, `rows_len = 4`. The required total is `1 + 4 + 1 + 1 = 7`, but only 5 is available. ratatui's solver will distort (either skip the hint or compress the pads); the early-return at `inner_area.height < 4` only catches the truly-tiny terminal.

This is benign for ≤3 providers but the user can enable mock + claude + codex + gemini (4 rows) on a small terminal and the quit hint disappears.

**Fix:** Use `Constraint::Min(1)` for the body (let it absorb whatever's left) OR clamp `rows_len` against `inner_area.height.saturating_sub(3)`:
```rust
let max_body = inner_area.height.saturating_sub(3);  // reserve 1+1+1 for pads + hint
let rows_len = u16::try_from(app.rows.len()).unwrap_or(0).min(max_body).max(1);
```

---

### WR-08: `run_tui_stub` is dead code

**File:** `src/cli/mod.rs:97-99`
**Issue:** `pub fn run_tui_stub() -> anyhow::Result<()>` is defined and exported but main.rs dispatches `Some(Command::Tui) => ahb::tui::run(engine).await` — the stub is never called. Dead public API surface that confuses readers.

**Fix:** Delete `run_tui_stub` (it was a Phase 0 scaffold; Plan 03 replaced it with `tui::run`).

---

## Info

### IN-01: `pick_newest_file` clones path on every iteration

**File:** `src/provider/claude/mod.rs:54-64`
**Issue:** `filter_map(|p| { ... Some((p.clone(), mtime)) })` clones every PathBuf, then `.max_by_key` consumes them. For a session directory with hundreds of files this is wasteful. Use indices instead:
```rust
files.iter()
    .filter_map(|p| std::fs::metadata(p).ok()?.modified().ok().map(|m| (p, m)))
    .max_by_key(|(_, m)| *m)
    .map(|(p, _)| p.clone())
```

---

### IN-02: `format_one_line` whitespace collapse swallows valid intra-word whitespace

**File:** `src/cli/render_text.rs:144-161`
**Issue:** `format_one_line` collapses ALL runs of spaces into one. For an error message like `"path  with  intentional  doubled-spaces"` (e.g., a Windows file path or a quoted error), the user sees `"path with intentional doubled-spaces"`. Functionally fine; just noting that the collapse is irreversible.

**Fix:** Limit the substitution to literal `\n` / `\r` / `\t` — leaving multi-space runs alone:
```rust
let cleaned = s.replace(['\n', '\r', '\t'], " ");
cleaned
```

---

### IN-03: `EVENT_BUFFER = 64` is declared but unused in Phase 1

**File:** `src/engine/events.rs:14`
**Issue:** No mpsc channel is constructed in Phase 1 — `tui_loop` calls `engine.refresh_all(...)` directly inside the select! arm instead of subscribing to a channel. The `EVENT_BUFFER` constant + `EngineEvent` enum are scaffolding for a future phase. Leave them in but the module docstring promises "Plan 03's TUI subscribes via `mpsc::Receiver<EngineEvent>`" — Plan 03 actually doesn't subscribe; the doc lies about the current architecture.

**Fix:** Update `engine/events.rs` module doc to clarify: "Plan 03's TUI calls `engine.refresh_all` directly; the channel surface lands in a future phase when the engine spawns its own driver loop."

---

### IN-04: `format_countdown` uses `unwrap_or_else` on a `since` that takes `Unit::Hour`

**File:** `src/cli/render_text.rs:199-209`
**Issue:** `target.since((jiff::Unit::Hour, *now))` — `Unit::Hour` is the largest unit, so the returned Span has the form `Xh Ym Zs`. For a target that's 1h 30m away, returns `1h 30m 0s` — `get_minutes()` returns 30, which the format `{m:02}` prints as `30`. Good. For a target 2h 0m 30s away, prints `2h00m` (the 30s is silently dropped). That's the intended behavior (the doc says `Xh00m` format), but a future maintainer might mis-read.

**Fix:** Document the truncation explicitly:
```rust
/// Format the gap as `Xh{MM}m`. Sub-minute seconds are TRUNCATED (not rounded) so a
/// 1h 0m 59s gap prints `1h00m`. Use `format_countdown_with_seconds` if you need
/// sub-minute precision.
```

---

### IN-05: `directories::ProjectDirs::from("", "", "ahb")` empty qualifier is fragile

**File:** `src/config.rs:78`
**Issue:** Calling `ProjectDirs::from("", "", "ahb")` (empty qualifier + organization) works on current `directories` 6.x but is documented as "implementation-defined" for empty args. A future major bump could reject this. Use a deliberate qualifier:
```rust
directories::ProjectDirs::from("dev", "ahb", "ahb")
```
This changes the macOS path to `~/Library/Application Support/dev.ahb.ahb/` — a one-time migration for current users, but stable for the future. Or pin the `directories` crate to `=6.x` and document the constraint.

---

_Reviewed: 2026-05-23_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
