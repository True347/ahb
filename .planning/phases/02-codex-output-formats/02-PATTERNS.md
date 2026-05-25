# Phase 2: Codex + Output Formats — Pattern Map

**Mapped:** 2026-05-25
**Files analyzed:** 17 (8 new, 9 modified)
**Analogs found:** 14 / 17 (3 require novel patterns — see "No Analog Found")

This document tells the planner: "for each Phase-2 file to be created or modified, what concrete existing code is the canonical analog, and which 5–15 line excerpt encodes the pattern to mirror." Excerpts are quoted by line number from `src/` files so the planner can paste them into PLAN.md `<actions>` and `<read_first>` blocks.

---

## File Classification

| File | NEW/MOD | Role | Data Flow | Closest Analog | Match Quality |
|------|---------|------|-----------|----------------|---------------|
| `src/provider/codex/mod.rs` | NEW | adapter (provider) | request-response (async fetch returns `Result<ProviderState>`) | `src/provider/claude/mod.rs` | exact (sibling adapter) |
| `src/provider/codex/jsonl.rs` | NEW | parser (streaming JSONL) | file-I/O streaming + transform | `src/provider/claude/jsonl.rs` | exact (sibling JSONL parser) |
| `src/provider/codex/sqlite.rs` | NEW | discovery + read-only DB open | file-I/O (sync, wrapped in `spawn_blocking`) | partial — `discover_session_files` in `claude/jsonl.rs:179` for glob discovery; **no SQLite analog exists** | role-match only (novel SQLite pattern) |
| `src/provider/codex/window.rs` | NEW | rate-limit → HpWindow transform | pure transform (`RateLimits` → `Vec<HpWindow>`) | `src/provider/claude/window.rs` | role-match (different math — % conversion + passthrough order) |
| `src/cli/render_json.rs` | NEW | DTO + serializer (`JsonRoot`/`JsonProvider`/`JsonWindow`/`JsonError`) | transform (Vec<Result> → serde-derive struct → `to_writer`) | partial — `src/model.rs:62` for `Serialize` struct shape; `src/cli/render_text.rs` for converter-from-engine-result pattern | role-match (new DTO file; no other file is a serde-only DTO module) |
| `src/cli/render_text.rs` | MOD (add `detailed_block`) | renderer | transform (`ProviderState` → multi-line `String`) | `src/cli/render_text.rs::compact_line_colored` (lines 53–94) | exact (extending the same module) |
| `src/cli/render_text.rs::format_error_row_colored` (line 116) | MOD (generalize `"Claude adapter"` literal) | renderer | transform | itself, lines 116–140 | exact |
| `src/cli/mod.rs` | MOD (add 3 flags + `run_detailed`/`run_json` + `after_help`) | CLI dispatch | request-response | `src/cli/mod.rs::run_compact` (lines 61–92) | exact (sibling dispatch fn) |
| `src/cli/tty.rs` | MOD (no code change expected; verify `json_mode=true` path) | pure decision fn | n/a | itself (lines 30–49) — already supports `json_mode=true` | exact (zero diff anticipated) |
| `src/provider/claude/window.rs` | MOD (add `WEEKLY_TOKEN_LIMIT` + `compute_weekly_window`) | window builder | transform (entries → `Cluster`/`HpWindow`) | `src/provider/claude/window.rs::find_active_cluster` (lines 46–91) | exact (extending the same module) |
| `src/provider/mod.rs` | MOD (register `codex` submodule) | module index | n/a | `src/provider/mod.rs:19` (`pub mod claude; pub mod mock;`) | exact |
| `src/lib.rs` | MOD (no code change — `provider` already re-exports the codex submodule via `pub mod provider`) | crate root | n/a | `src/lib.rs:11` (already exports `pub mod provider`) | exact — likely zero diff |
| `src/main.rs` | MOD (dispatch on 3 flags + exit-code mapping + SIGPIPE recommendation) | binary entry | request-response | `src/main.rs:91-95` (`match cli.command` block) | exact (extending the dispatch match) |
| `Cargo.toml` | MOD (add `rusqlite = { version = "0.39", features = ["bundled"] }`) | manifest | n/a | `Cargo.toml:40-41` (`# Phase 1 — …` comment + `glob = "0.3"` line) | exact (same `# Phase N — purpose.` comment idiom) |
| `src/templates/default-config.toml` | MOD (update `[providers.codex]` comment) | embedded template | n/a | `src/templates/default-config.toml:8-9` | exact |
| `tests/secret_leak_subprocess.rs` | MOD (add `--json` route variant per RESEARCH §SEC-03) | integration test | request-response | `tests/secret_leak_subprocess.rs:18-46` | exact (extending same file) |
| `tests/integration_codex.rs` | NEW | integration test (SQLite lock guard) | request-response | `tests/panic_isolation.rs` + `tests/cli_walking_skeleton.rs::setup_fake_home` | role-match (new SQLite-lock scenario, no exact analog) |
| `tests/integration_output_formats.rs` | NEW | integration test (stdout shape per format) | request-response | `tests/cli_walking_skeleton.rs` (entire file) + `tests/schema_drift_sentinel.rs` (env injection idiom) | exact (sibling integration test pattern) |

---

## Pattern Assignments

### 1. `src/provider/codex/mod.rs` (NEW — adapter, request-response)

**Analog:** `src/provider/claude/mod.rs`

**Cues for `<read_first>`:** `src/provider/claude/mod.rs:1-129`, `src/provider/mod.rs:29-43` (`FetchCtx` + `Provider` trait), `src/model.rs:49-70` (`HpWindow` + `ProviderState`), `src/engine/fanout.rs:46-62` (timeout wrap), RESEARCH §Codex JSONL Schema + §spawn_blocking Pattern.

**Module-doc + lint floor pattern** (lines 1-16 of analog):
```rust
//! Claude Code provider adapter (REQ ADP-02).
//!
//! Phase 1 flow:
//! 1. Glob `~/.claude/projects/**/*.jsonl` …
//! 2. Stream each JSONL file via `jsonl::read_assistant_entries` (D-35 tolerance).
//! …
//! Error rows (UI-SPEC LOCKED literals):
//! - missing `~/.claude/projects` → `Err(Unavailable { reason: "…installed?" })`

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
```

**Struct + `new()` ctor pattern** (lines 34-49 of analog):
```rust
pub struct ClaudeProvider {
    base_path: PathBuf,
    token_limit: u64,
}

impl ClaudeProvider {
    #[must_use]
    pub fn new(home_dir: &Path, token_limit: u64) -> Self {
        Self {
            base_path: home_dir.join(".claude").join("projects"),
            token_limit,
        }
    }
}
```
Codex equivalent: `CodexProvider { codex_dir: PathBuf }`; ctor takes `home_dir: &Path` and builds `home_dir.join(".codex")`. NO `token_limit` (Codex `used_percent` is upstream-emitted).

**`Provider` impl skeleton + Unavailable error literal pattern** (lines 66-129 of analog):
```rust
#[async_trait]
impl Provider for ClaudeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Claude
    }

    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        // SEC-04: Claude needs no secrets; the contract still hands them through.
        let _ = ctx.secrets;

        if !self.base_path.exists() {
            return Err(ProviderError::Unavailable {
                reason: "~/.claude/projects not found — is Claude Code installed?".into(),
            });
        }
        let files = jsonl::discover_session_files(&self.base_path);
        // … schema-drift probe …
        // … merge + sort + cluster …
        Ok(ProviderState {
            id: ProviderId::Claude,
            windows: vec![win],
            fetched_at: ctx.now,
            source: Cow::Borrowed("claude-jsonl"),
        })
    }
}
```

Codex deviations the planner must encode:
- Discovery returns `Option<PathBuf>` for SQLite + `Vec<PathBuf>` for JSONL. Two unavailable literals (per CONTEXT specifics): `"no ~/.codex/state_*.sqlite found — is Codex CLI installed?"` and (RESEARCH §Codex Adapter recommendation) `"no ~/.codex/sessions/**/rollout-*.jsonl found"`.
- All sync IO (sqlite open + jsonl read) wrapped in **one** `tokio::task::spawn_blocking` per RESEARCH §spawn_blocking Pattern (lines 442-516). Capture owned `PathBuf` + owned `Vec<PathBuf>` + `Copy` `jiff::Timestamp` so the closure is `move + 'static`.
- `JoinError::is_panic()` mapping pattern is shown in RESEARCH §spawn_blocking lines 492-501 — produces `ProviderError::Internal { source: anyhow::anyhow!("…") }`.
- `source: Cow::Borrowed("codex-jsonl")` (per RESEARCH recommendation — drop the `+sqlite` suffix when SQLite SELECT is skipped).
- `ctx.secrets` is touched once via `let _ = ctx.secrets;` (Codex needs no secrets — per CONTEXT D-Established Patterns "Phase 1 keyring wired but Codex needs no secret").

**`Send + Sync` static_assertions pattern** (lines 137-139 of analog):
```rust
assert_impl_all!(ClaudeProvider: Send, Sync);
assert_impl_all!(Box<dyn Provider>: Send, Sync);
```
Mirror with `assert_impl_all!(CodexProvider: Send, Sync);` in `#[cfg(test)] mod tests`.

---

### 2. `src/provider/codex/jsonl.rs` (NEW — streaming JSONL parser, file-I/O + transform)

**Analog:** `src/provider/claude/jsonl.rs`

**Cues for `<read_first>`:** `src/provider/claude/jsonl.rs:1-197`, RESEARCH §Codex JSONL Schema (lines 197-305) — especially the verified `serde(tag="type", rename_all="snake_case")` shape for `RolloutPayload`.

**File-open + tolerant streaming pattern** (lines 70-111 of analog):
```rust
#[must_use]
pub fn read_assistant_entries(path: &Path) -> Vec<AssistantEntry> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("could not open {}: {e}", path.display());
            return Vec::new();
        }
    };
    let reader = BufReader::new(file);
    let mut lines = reader.lines().peekable();
    let mut out: Vec<AssistantEntry> = Vec::new();
    while let Some(line_res) = lines.next() {
        let is_last = lines.peek().is_none();
        // … D-35 tolerance: warn-and-skip mid-file; silent skip on truncated last line
        match serde_json::from_str::<JsonlEntry>(&line) {
            Ok(JsonlEntry::Assistant(a)) => out.push(a),
            Ok(JsonlEntry::Other) => {} // user / snapshot / permission-mode / etc.
            Err(e) => { /* … same warn-or-silent logic … */ }
        }
    }
    out
}
```
Codex variant: `read_token_count_events(path) -> Vec<RolloutLine>` (or `parse_codex_rollout_windows(path, ctx_now) -> Result<Vec<HpWindow>, ProviderError>` per RESEARCH §spawn_blocking line 478). Same tolerance contract (D-35 amended for Codex: mid-file warn + skip; truncated trailing line silent skip).

**Serde envelope with `#[serde(tag = "type", rename_all = …)]`** (lines 29-35 of analog):
```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum JsonlEntry {
    Assistant(AssistantEntry),
    #[serde(other)]
    Other,
}
```
Codex variant per RESEARCH §Codex JSONL Schema lines 263-304: tag is `"type"`, rename is `snake_case`, payload-inner-tag is also `"type"`. Use the exact `RolloutPayload`/`TokenCountPayload`/`RateLimits`/`RateLimitTier`/`RolloutLine` shapes from RESEARCH lines 265-302 verbatim — these were verified against the Codex source via issue #14728.

**`#[serde(default)]` for forward-compat fields** (lines 53-63 of analog):
```rust
#[derive(Debug, Deserialize, Clone)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}
```
Codex: per RESEARCH lines 281-287 use `#[serde(default)] rate_limits: Option<RateLimits>` + `#[serde(default)] primary: Option<RateLimitTier>` / `secondary: Option<RateLimitTier>`. Missing fields default to `None`, which the D-47 logic treats as schema drift.

**Glob discovery pattern** (lines 178-197 of analog):
```rust
#[must_use]
pub fn discover_session_files(base: &Path) -> Vec<PathBuf> {
    if !base.exists() {
        return Vec::new();
    }
    let pattern = base.join("**").join("*.jsonl");
    let pattern_str = pattern.to_string_lossy();
    let opts = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };
    match glob::glob_with(&pattern_str, opts) {
        Ok(paths) => paths.filter_map(Result::ok).collect(),
        Err(e) => {
            tracing::warn!("glob error for {}: {e}", pattern_str);
            Vec::new()
        }
    }
}
```
Codex variant: `pattern = base.join("sessions").join("**").join("rollout-*.jsonl");` — identical opts. Mirror the `tracing::warn!` + `Vec::new()` fallback.

**`pick_newest_file` mtime-sort pattern** (lines 51-64 of analog):
```rust
fn pick_newest_file(files: &[PathBuf]) -> Option<PathBuf> {
    files
        .iter()
        .filter_map(|p| {
            let meta = std::fs::metadata(p).ok()?;
            let mtime = meta.modified().ok()?;
            Some((p.clone(), mtime))
        })
        .max_by_key(|(_, mtime)| *mtime)
        .map(|(p, _)| p)
}
```
Reuse verbatim in `src/provider/codex/jsonl.rs` (or factor to a shared utility module — planner's call; recommendation is to keep duplication for now, the function is tiny).

**Critical Codex deviation from Claude pattern (per RESEARCH §Codex JSONL Schema bullet 3):** `resets_in_seconds` is **relative to the RolloutLine's own timestamp**, NOT `ctx.now`. The parser must read `RolloutLine.timestamp` (deserialized as `jiff::Timestamp`) and compute `reset_at = line_ts + Span::new().seconds(resets_in_seconds)`. Document this in the fn doc-comment — the no-walltime-in-adapter test (`tests/no_walltime_in_adapter.rs`) does **not** forbid this (it forbids `Timestamp::now()`, not arithmetic from a parsed timestamp).

---

### 3. `src/provider/codex/sqlite.rs` (NEW — read-only DB open + version glob)

**Analog:** **No exact analog.** Closest partial: `src/provider/claude/jsonl.rs::discover_session_files` (lines 179-197) for the glob pattern.

**Cues for `<read_first>`:** RESEARCH §Codex SQLite Schema (lines 307-351), RESEARCH §glob 0.3 Version Sort (lines 520-576), `src/provider/claude/jsonl.rs:179-197` (glob_with idiom), `rusqlite::OpenFlags` + `Connection::busy_timeout` docs (cargo doc link in RESEARCH).

**Version-sorted glob pattern** (from RESEARCH §glob Version Sort lines 531-571, verbatim recommendation):
```rust
pub fn discover_state_sqlite(codex_dir: &Path) -> Option<PathBuf> {
    let pattern = codex_dir.join("state_*.sqlite");
    let pattern_str = pattern.to_string_lossy();
    let paths: Vec<PathBuf> = glob::glob(&pattern_str)
        .ok()?
        .filter_map(Result::ok)
        .collect();
    if paths.is_empty() { return None; }
    let mut paths_with_version: Vec<(PathBuf, u32)> = paths
        .into_iter()
        .map(|p| {
            let n = p.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("state_"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            (p, n)
        })
        .collect();
    paths_with_version.sort_by_key(|(_, n)| *n);
    let highest = paths_with_version.pop()?;
    if !paths_with_version.is_empty() {
        let names: Vec<String> = paths_with_version.iter()
            .map(|(p, _)| p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
            .collect();
        tracing::warn!(
            "multi-version Codex state files detected — picked {}, found {}",
            highest.0.display(), names.join(", ")
        );
    }
    Some(highest.0)
}
```
Note: `clippy::unwrap_used` is denied crate-wide — replace `.unwrap_or(0)` with explicit `match … None => 0` if needed, but `unwrap_or` is allowed (it's `unwrap` proper that's forbidden).

**rusqlite read-only open pattern** (from RESEARCH §Codex SQLite Schema lines 330-343, verbatim — to be wrapped inside `spawn_blocking` per §spawn_blocking lines 466-470):
```rust
let conn = rusqlite::Connection::open_with_flags(
    &sqlite_path,
    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
).map_err(|e| ProviderError::Internal { source: anyhow::anyhow!("sqlite open: {e}") })?;
conn.busy_timeout(std::time::Duration::from_millis(250))
    .map_err(|e| ProviderError::Internal { source: anyhow::anyhow!("busy_timeout: {e}") })?;
// Phase 2 RECOMMENDED: no SELECT queries — opening + busy_timeout is enough to honor D-45.
drop(conn);
```

**Lint-floor banner** (mirroring `src/provider/claude/window.rs:17-18`):
```rust
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
```

---

### 4. `src/provider/codex/window.rs` (NEW — `RateLimits` → `Vec<HpWindow>` transform)

**Analog:** `src/provider/claude/window.rs` (role-match — same module purpose, different math)

**Cues for `<read_first>`:** `src/provider/claude/window.rs:1-110`, `src/model.rs:49-55` (`HpWindow` shape), RESEARCH §Codex JSONL Schema bullets 1-3 (lines 252-258) — passthrough labels, `100.0 - used_percent` conversion, `resets_in_seconds` relative-to-line-timestamp rule.

**Pure transform fn signature pattern** (lines 95-110 of analog):
```rust
#[must_use]
pub fn percent_remaining(used: u64, limit: u64) -> f32 {
    if limit == 0 {
        return 0.0;
    }
    let remaining = limit.saturating_sub(used);
    pct_from_ratio(remaining, limit)
}

#[allow(clippy::cast_precision_loss)]
fn pct_from_ratio(remaining: u64, limit: u64) -> f32 {
    let raw = (remaining as f32) / (limit as f32) * 100.0;
    raw.clamp(0.0, 100.0)
}
```
Codex variant: `pub fn to_hp_windows(rate_limits: &RateLimits, line_ts: jiff::Timestamp) -> Vec<HpWindow>`. For each tier present (`primary` / `secondary`, in that passthrough order per D-48 + D-55):
```rust
HpWindow {
    label: Cow::Borrowed("primary"),  // or "secondary"
    percent_remaining: (100.0 - tier.used_percent as f32).clamp(0.0, 100.0),
    reset: ResetInfo {
        // per RESEARCH §Codex JSONL Schema bullet 3 — anchor on line_ts, NOT ctx.now
        resets_at: line_ts.checked_add(jiff::Span::new().seconds(tier.resets_in_seconds as i64))
            .unwrap_or(line_ts),
    },
    bar_color: None,
}
```
Critical: passthrough order — primary first if present, secondary second. Do NOT sort. Per D-48 the adapter does not synthesize windows.

**Cluster-style struct (optional helper) pattern** (lines 36-40 of analog):
```rust
#[derive(Debug, Clone)]
pub struct Cluster {
    pub session_start: jiff::Timestamp,
    pub reset_at: jiff::Timestamp,
    pub used_tokens: u64,
}
```
Codex equivalent if planner wants a similar typed intermediate: a thin newtype around `Vec<HpWindow>` is overkill here — return `Vec<HpWindow>` directly.

---

### 5. `src/cli/render_json.rs` (NEW — DTO + serializer for `--json schema_version: 1`)

**Analog:** Partial — no existing DTO-only module. Closest analogs:
- `src/model.rs:49-69` for `Serialize`-derived struct shape with `#[serde(with = "jiff::fmt::serde::timestamp::second::required")]` on `jiff::Timestamp` fields.
- `src/cli/render_text.rs::compact_line_colored` for the engine-Result → output pipeline.
- RESEARCH §--json schema_version: 1 Design (lines 578-674) — explicit DTO shapes verbatim.

**Cues for `<read_first>`:** `src/model.rs:1-146` (full file — to understand existing serde shapes AND how `ProviderError` already has a `#[serde(tag = "kind", rename_all = "snake_case")]` enum that the planner should leverage for `JsonError.kind`), RESEARCH §--json schema_version: 1 Design lines 583-674, RESEARCH §Claude Weekly Limit Handling lines 399-410.

**jiff Timestamp serde-with pattern** (lines 41-45 of analog `src/model.rs`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetInfo {
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub resets_at: jiff::Timestamp,
}
```
Reuse on `JsonRoot.generated_at`, `JsonProvider.fetched_at` (optional variant), `JsonWindow.reset_at`. For the optional `fetched_at` variant the serde attribute is the `required::option` sibling — see RESEARCH lines 607-610.

**`#[serde(tag = ...)]` enum pattern for the error variants** (lines 93-125 of analog `src/model.rs`):
```rust
#[derive(thiserror::Error, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderError {
    #[error("provider is not configured")]
    Unconfigured,
    #[error("provider unavailable: {reason}")]
    Unavailable { reason: String },
    #[error("schema drift: missing {missing:?}")]
    SchemaDrift { missing: Vec<String> },
    #[error("network: {source}")]
    Network { #[serde(serialize_with = "serialize_display")] source: NetworkErr },
    #[error("rate limited, retry after {retry_after:?}")]
    RateLimited { #[serde(skip)] retry_after: Option<jiff::Span> },
    #[error("internal: {source}")]
    Internal { #[serde(serialize_with = "serialize_display")] source: anyhow::Error },
}
```
**Important planner decision:** D-51's `JsonError` shape is NOT a `#[serde(tag)]` adjacent-enum because the *envelope* (`status: "ok" | "error"`) lives one level up on `JsonProvider`. Don't try to use `ProviderError`'s built-in serde directly — it would emit `{"kind": ...}` *at the top of the provider object*, which contradicts D-51. Build `JsonError` as a separate plain struct per RESEARCH lines 624-633 and write a `error_to_json(&ProviderError) -> JsonError` mapper (RESEARCH line 650 references it).

**DTO shape (verbatim from RESEARCH §--json schema_version: 1 lines 583-633):**
```rust
const SCHEMA_VERSION: u8 = 1;

#[derive(Serialize)]
pub struct JsonRoot<'a> {
    pub schema_version: u8,
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub generated_at: jiff::Timestamp,
    pub providers: Vec<JsonProvider<'a>>,
}

#[derive(Serialize)]
pub struct JsonProvider<'a> {
    pub id: &'static str,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Cow<'a, str>>,
    #[serde(with = "jiff::fmt::serde::timestamp::second::required::option",
            skip_serializing_if = "Option::is_none", default)]
    pub fetched_at: Option<jiff::Timestamp>,
    pub windows: Vec<JsonWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonError>,
}

#[derive(Serialize)]
pub struct JsonWindow {
    pub label: String,
    pub percent_remaining: Option<f32>,  // None for Claude-weekly-unknown fallback (D-54)
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub reset_at: jiff::Timestamp,
}

#[derive(Serialize)]
pub struct JsonError {
    pub kind: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}
```

**Converter fn pattern** (RESEARCH lines 638-661, verbatim):
```rust
pub fn to_json_root<'a>(
    results: &'a [(ProviderId, Result<ProviderState, ProviderError>)],
    generated_at: jiff::Timestamp,
) -> JsonRoot<'a> {
    let providers = results.iter().map(|(id, r)| match r {
        Ok(state) => JsonProvider {
            id: id_to_str(*id),
            status: "ok",
            source: Some(state.source.as_ref().into()),
            fetched_at: Some(state.fetched_at),
            windows: state.windows.iter().map(window_to_json).collect(),
            error: None,
        },
        Err(e) => JsonProvider {
            id: id_to_str(*id),
            status: "error",
            source: None,
            fetched_at: None,
            windows: Vec::new(),
            error: Some(error_to_json(e)),
        },
    }).collect();
    JsonRoot { schema_version: SCHEMA_VERSION, generated_at, providers }
}
```
Reuse the `id_label` fn from `src/cli/render_text.rs:166-173` — it already returns `&'static str` ("claude" / "codex" / "gemini" / "mock") and is `pub(crate)`. **Promote `id_label` to `pub fn id_label_str(id) -> &'static str`** (planner: confirm visibility upgrade is consistent with existing API; or import via `crate::cli::render_text::id_label` since both modules are in the same `cli` parent — `pub(crate)` should already permit this).

**`error_to_json` sanitize-for-Internal-variant rule** (from CONTEXT Claude's Discretion #7 + Deferred-Ideas line 367): only emit top-level `Display` for `Internal { source: anyhow::Error }`, never `Debug` (which would expand the cause chain and risk path/secret leaks). Pattern: `error.to_string()` calls `Display` (one-line per the `#[error(...)]` attributes); do NOT call `format!("{error:?}")` anywhere in this module.

**Emit pattern** (RESEARCH lines 666-669):
```rust
let stdout = std::io::stdout().lock();
serde_json::to_writer(stdout, &root)?;
println!();  // trailing newline
```
This is exactly what `src/cli/mod.rs::debug_emit_fake_secret_and_exit` (lines 113-135) already does — re-use that idiom verbatim for `run_json`. SEC-03 hits the same `Serialize` path because anything inside `JsonRoot` that wraps a `Secret<T>` would go through `Secret<T>::serialize → "[REDACTED]"` (`src/secrets.rs:61-65`).

---

### 6. `src/cli/render_text.rs::detailed_block` (MOD — add new fn)

**Analog:** `src/cli/render_text.rs::compact_line_colored` (lines 53-94, same file)

**Cues for `<read_first>`:** `src/cli/render_text.rs:1-388` (full file — small enough), CONTEXT D-53 (header + indented window lines layout), D-54 (Claude emits 5h + weekly), D-56 (100% shared compact styling — only diff is header + indent).

**Compact-line styling pattern to reuse verbatim** (lines 64-94 of analog):
```rust
let pct = w.percent_remaining.clamp(0.0, 100.0);
let filled = filled_cells(pct);

let (filled_glyph, empty_glyph): (&str, &str) = if ascii {
    ("#", "-")
} else {
    ("\u{2588}", "\u{2591}")
};
let filled_str = filled_glyph.repeat(filled);
let empty_str = empty_glyph.repeat(BAR_WIDTH - filled);

let bar = if color_on {
    match pct {
        p if p >= 30.0 => format!("{}{}", filled_str.green(), empty_str.bright_black()),
        p if p >= 10.0 => format!("{}{}", filled_str.yellow(), empty_str.bright_black()),
        _ => format!("{}{}", filled_str.red(), empty_str.bright_black()),
    }
} else {
    format!("{filled_str}{empty_str}")
};

let countdown = format_countdown(now, &w.reset.resets_at);
let sep = if ascii { '|' } else { '\u{2022}' };

format!(
    "{label}  {bar} {pct}% {sep} resets in {countdown}",
    label = w.label,
    pct = pct_int(pct)
)
```

**`detailed_block` shape — planner should mirror the structure, NOT copy-paste the body.** Recommended:
```rust
pub(crate) fn detailed_block(state: &ProviderState, now: &jiff::Timestamp, ascii: bool, color_on: bool) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(state.windows.len() + 1);
    lines.push(id_label(state.id).to_string());                 // header line per D-53
    for w in &state.windows {
        // build a synthetic single-window ProviderState OR factor out a shared
        // `render_one_window_line(w, now, ascii, color_on, label_width_pad) -> String`
        // helper and call it for both compact AND detailed paths.
        let line = render_one_window_line_indented(w, now, ascii, color_on);
        lines.push(format!("  {line}"));                        // 2-space indent per D-53
    }
    lines.join("\n")
}
```
**Important:** D-56 binds 100% sharing of the bar styling — extract `compact_line_colored` body into a `pub(crate) fn render_bar_segment(w, now, ascii, color_on) -> String` and have BOTH `compact_line_colored` AND `detailed_block` call it. Do NOT duplicate the threshold match.

**Error row in detailed mode pattern** (CONTEXT D-53 — `format_error_row_colored` with 2-space indent prepended):
```rust
// per-provider error case in detailed:
lines.push(id_label(state.id).to_string());                     // still emit header
let err_row = format_error_row_colored(id, &err, ascii, color_on);
lines.push(format!("  {err_row}"));                             // indent it
```
The `format_error_row_colored` fn already emits `{label}  ERROR: …` or the SchemaDrift sentinel — both render correctly under 2-space indent.

**Window label width-pad (CONTEXT Claude's Discretion #6 — 8 chars left-aligned recommended):**
```rust
let label_padded = format!("{:<8}", w.label);  // "5h      " / "weekly  " / "primary " / "secondary"
```
"secondary" is 9 chars — overflow falls back to no padding (planner: pick the width that fits the longest expected label OR truncate; recommendation = `{:<width$}` with width = max-label-len across `state.windows` computed up front).

---

### 7. `src/cli/render_text.rs::format_error_row_colored` (MOD — generalize SchemaDrift phrase)

**Analog:** itself, lines 116-140

**Cues for `<read_first>`:** RESEARCH §Codex JSONL Schema bullet about `rate_limits: null` policy, CONTEXT specifics line 287-289 ("若覺得 `Claude adapter may be out-of-date` 字面對 Codex 不對, 再 generalize")

**Current literal** (line 127 of analog):
```rust
let phrase = "Claude adapter may be out-of-date";
```

**Generalized pattern** (per CONTEXT specifics § lines 287-289 + Deferred-Ideas line 368):
```rust
let label = id_label(id);
let label_titlecased = match id {
    ProviderId::Claude => "Claude",
    ProviderId::Codex => "Codex",
    ProviderId::Gemini => "Gemini",
    ProviderId::Mock => "Mock",
};
let phrase_owned = format!("{label_titlecased} adapter may be out-of-date");
let phrase = phrase_owned.as_str();
```
**UI-SPEC impact:** the Phase 1 LOCKED literal `"Claude adapter may be out-of-date"` continues to render for `ProviderId::Claude` (byte-identical), so the existing snapshot tests at `tests/schema_drift_sentinel.rs:15` keep passing. The new `"Codex adapter may be out-of-date"` line is the Codex-specific sentinel.

---

### 8. `src/cli/mod.rs` (MOD — add 3 flags + `run_detailed` + `run_json` + `after_help`)

**Analog:** `src/cli/mod.rs::Cli` struct (lines 19-44) + `run_compact` (lines 61-92), same file.

**Cues for `<read_first>`:** `src/cli/mod.rs:1-167` (full file), RESEARCH §clap conflicts_with lines 678-734 (`ArgGroup` recommendation), CONTEXT D-57 (clap `conflicts_with_all` interlock), D-58 (`--ascii`/`--color` silently ignored under `--json`), D-61 (`after_help` exit-code docs).

**Cli struct + clap derive pattern** (lines 19-44 of analog):
```rust
#[derive(clap::Parser, Debug)]
#[command(
    version,
    about = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance"
)]
pub struct Cli {
    #[arg(long)]
    pub ascii: bool,

    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,

    #[cfg(debug_assertions)]
    #[arg(long, hide = true)]
    pub debug_emit_fake_secret: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}
```

**Pattern to mirror — add 3 mutually-exclusive flags via `ArgGroup`** (per RESEARCH §clap conflicts_with lines 698-714, **Option B is the recommendation**):
```rust
#[derive(clap::Parser, Debug)]
#[command(
    version,
    about = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance",
    after_help = "Exit codes:\n  0  at least one provider returned data (or no providers configured)\n  1  all configured providers failed\n  2  config / secrets unloadable, or invalid command-line usage",
    group(
        clap::ArgGroup::new("format")
            .required(false)
            .multiple(false)
            .args(["compact", "detailed", "json"]),
    ),
)]
pub struct Cli {
    #[arg(long)]
    pub compact: bool,
    #[arg(long)]
    pub detailed: bool,
    #[arg(long)]
    pub json: bool,
    // … existing ascii / color / debug_emit_fake_secret / command fields unchanged
}
```

**`run_compact` dispatch pattern to mirror for `run_detailed` and `run_json`** (lines 61-92 of analog):
```rust
pub async fn run_compact(
    engine: &Engine,
    ascii: bool,
    color_flag: ColorMode,
) -> anyhow::Result<()> {
    let now = jiff::Timestamp::now();
    let results = engine.refresh_all(now).await;

    if results.is_empty() {
        println!("{}", render_text::EMPTY_STATE_HEADING);
        println!("{}", render_text::EMPTY_STATE_BODY);
        return Ok(());
    }

    let color_on = tty::should_colorize_env(color_flag, false);
    for (id, result) in results {
        match result {
            Ok(state) => {
                let line = render_text::compact_line_colored(&state, &now, ascii, color_on);
                println!("{line}");
            }
            Err(err) => {
                let line = render_text::format_error_row_colored(id, &err, ascii, color_on);
                println!("{line}");
            }
        }
    }
    Ok(())
}
```

`run_detailed` mirrors this exactly — substitute `render_text::detailed_block(&state, &now, ascii, color_on)` for the Ok line and `format!("{header}\n  {err_row}", …)` for the Err line. **CRITICAL:** D-53 requires an empty newline between providers — `println!()` (empty line) AFTER each block.

`run_json` mirrors the same shape but:
1. Builds `JsonRoot` via `render_json::to_json_root(&results, now)`.
2. `color_on = tty::should_colorize_env(color_flag, true)` — `json_mode=true` short-circuits to `false` per `src/cli/tty.rs:32-34` (verified — D-58 silent ignore is automatic).
3. Empty-state still emits valid JSON: `{"schema_version": 1, "generated_at": "...", "providers": []}` (NOT the two-line text empty-state). Per CONTEXT D-50 + Code Context line 256 — planner verifies the tmux-friendly choice.
4. Use `serde_json::to_writer(stdout.lock(), &root)?; println!();` per the verbatim pattern from `debug_emit_fake_secret_and_exit` (lines 128-133 of analog) — proves the SEC-03 path.

**Dispatch fn return signature `anyhow::Result<()>` and exit-code mapping:** the three `run_*` fns return `Ok(())` always for now; exit code is computed in `main.rs` AFTER the fn returns, by re-running `engine.refresh_all` or by changing the signature to return the result-vec. **Planner decision:** the cleanest path is to refactor each `run_*` to return `Result<Vec<(ProviderId, Result<ProviderState, ProviderError>)>>` and let `main.rs` compute exit code from the Vec. RESEARCH §Architectural Responsibility Map line 132 confirms "Exit code computation lives in `src/main.rs`". The current `anyhow::Result<()>` signature in `run_compact` is the analog — bump it to return the Vec, OR have each `run_*` set a process-local flag.

---

### 9. `src/cli/tty.rs` (MOD — confirm no change needed)

**Analog:** itself (lines 30-49)

**Cues for `<read_first>`:** `src/cli/tty.rs:1-94` (full file)

**Pattern to verify** (lines 30-49 of analog):
```rust
#[must_use]
pub fn should_colorize(cli_flag: ColorMode, json_mode: bool, is_tty: bool, no_color: bool) -> bool {
    // Path 1 (highest priority): JSON output is always uncolored.
    if json_mode {
        return false;
    }
    match cli_flag { ColorMode::Never => false, ColorMode::Always => true, ColorMode::Auto => !no_color && is_tty }
}

#[must_use]
pub fn should_colorize_env(cli_flag: ColorMode, json_mode: bool) -> bool {
    let is_tty = std::io::stdout().is_terminal();
    let no_color = std::env::var_os("NO_COLOR").is_some();
    should_colorize(cli_flag, json_mode, is_tty, no_color)
}
```
**Verification:** `run_json` calls `should_colorize_env(cli.color, true)` and immediately gets `false` regardless of `--color=always` or `--ascii`. **Zero diff to this file is the expected outcome** — D-58's "silent ignore" is already wired by Phase 1.

---

### 10. `src/provider/claude/window.rs` (MOD — add weekly window)

**Analog:** itself — `CLAUDE_5H_TOKEN_LIMIT` const (line 26) + `find_active_cluster` (lines 46-91)

**Cues for `<read_first>`:** `src/provider/claude/window.rs:1-276` (full file), RESEARCH §Claude Weekly Limit Handling (lines 353-428), CONTEXT D-54 (Phase 2 補 Claude weekly bar).

**Const + doc-comment pattern** (lines 23-26 of analog):
```rust
/// Best-effort 5h budget estimate (Pro tier ~44k tokens / 5h window). Anthropic does not
/// publish exact numbers; revisit quarterly. Max5 / Max20 subscribers will see undercounted
/// bars — Phase 2 may add a `plan_tier` knob (CFG-03). Source: tokenmix.ai 2026 +
/// ccusage community measurements.
pub const CLAUDE_5H_TOKEN_LIMIT: u64 = 44_000;
```

**Pattern to mirror for the weekly const** (per RESEARCH §Claude Weekly Limit Handling lines 376-394):
```rust
/// Best-effort Claude weekly budget. Anthropic publishes only relative guidance
/// (~5x-7x of 5h limit; May-Jul 2026 +50% temporary increase). `None` means
/// "AHB has no reliable estimate — emit window with null percent". Revisit quarterly.
/// Source: community consensus (ccusage, tokenmix.ai, faros.ai) 2026-05;
/// Anthropic does not officially publish token counts since 2025.
pub const CLAUDE_WEEKLY_TOKEN_LIMIT: Option<u64> = None;  // planner picks None vs Some(220_000)

pub enum WeekAnchor { Iso, FirstPrompt }
pub const CLAUDE_WEEKLY_ANCHOR: WeekAnchor = WeekAnchor::Iso;
```
Per the RESEARCH recommendation, default to `None` (safest path) — emit weekly window with `percent_remaining: Option<f32>` = `None` → `JsonWindow.percent_remaining: None` (serializes to JSON `null`) AND render layer paints the SchemaDrift-style `▒▒▒▒▒▒▒▒▒▒ ??%` with footer comment. Planner may upgrade to `Some(220_000)` if it wants a populated bar — both paths are documented in RESEARCH.

**`find_active_cluster` walk pattern** (lines 46-91 of analog):
```rust
pub fn find_active_cluster(sorted_msgs: &[AssistantEntry]) -> Option<Cluster> {
    if sorted_msgs.is_empty() {
        return None;
    }
    let five_hours = jiff::Span::new().hours(5);
    let mut start_idx = 0_usize;
    for i in (1..sorted_msgs.len()).rev() {
        let prev = &sorted_msgs[i - 1];
        let curr = &sorted_msgs[i];
        let Ok(gap) = curr.timestamp.since(prev.timestamp) else { continue };
        let Ok(gap_secs) = gap.total(jiff::Unit::Second) else { continue };
        if gap_secs > FIVE_HOURS_SECS {
            start_idx = i;
            break;
        }
    }
    let cluster = &sorted_msgs[start_idx..];
    let first = cluster.first()?;
    let session_start = first.timestamp;
    let reset_at = session_start.checked_add(five_hours).ok()?;
    let used_tokens: u64 = cluster.iter().map(|m|
        m.message.usage.as_ref().map_or(0_u64, |u| u.cache_creation_input_tokens)
    ).sum();
    Some(Cluster { session_start, reset_at, used_tokens })
}
```

**New `compute_weekly_window` fn pattern** (RESEARCH §Claude Weekly Limit Handling lines 416-424, pseudocode — refine at impl time):
```rust
pub fn compute_weekly_window(sorted_msgs: &[AssistantEntry], now: jiff::Timestamp) -> Option<HpWindow> {
    let reset_at = next_iso_week_anchor(now)?;  // next Monday 00:00 local → UTC Timestamp
    let week_start = reset_at.checked_sub(jiff::Span::new().days(7)).ok()?;
    let used_tokens: u64 = sorted_msgs.iter()
        .filter(|m| m.timestamp >= week_start && m.timestamp <= now)
        .map(|m| m.message.usage.as_ref().map_or(0_u64, |u| u.cache_creation_input_tokens))
        .sum();
    let percent = CLAUDE_WEEKLY_TOKEN_LIMIT.map(|lim| percent_remaining(used_tokens, lim));
    Some(HpWindow {
        label: Cow::Borrowed("weekly"),
        percent_remaining: percent.unwrap_or(f32::NAN),  // sentinel — see note
        reset: ResetInfo { resets_at: reset_at },
        bar_color: None,
    })
}
```
**Note:** `HpWindow.percent_remaining` is `HpUnit = f32` (per `src/model.rs:27`), NOT `Option<f32>`. The `Option<f32>` lives ONLY in `JsonWindow.percent_remaining` (DTO layer). Internal model passes `f32::NAN` as the sentinel; `render_json::window_to_json` converts NaN → None; `render_text::compact_line_colored` / `detailed_block` paint the SchemaDrift sentinel when `percent_remaining.is_nan()`. Planner verifies this NaN-as-sentinel path is acceptable (RESEARCH §Alternatives Considered line 165 flags `Option<f32>` as cleaner but requires a model-layer break; planner picks).

**Pure transform unit test pattern** (lines 112-275 of analog):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::claude::jsonl::{AssistantEntry, ClaudeMessage, Usage};

    fn make_entry(ts: &str, cache_creation: u64) -> AssistantEntry { /* … */ }

    #[test]
    fn single_message_returns_single_cluster() { /* boundary + percent + reset_at assert */ }
}
```
Mirror with `mod weekly_tests`: anchor-rounding tests, single-week vs cross-week clusters, the `None` limit fallback path, the `Some(220_000)` path.

---

### 11. `src/provider/mod.rs` (MOD — register `codex` submodule)

**Analog:** `src/provider/mod.rs:19-20`

**Pattern to mirror** (lines 19-20 of analog):
```rust
pub mod claude;
pub mod mock;
```
Add:
```rust
pub mod codex;
```
Zero other changes to this file.

---

### 12. `src/lib.rs` (MOD — likely zero diff)

**Analog:** `src/lib.rs:1-17`

**Pattern** (lines 10-16 of analog):
```rust
pub mod model;
pub mod provider;
pub mod secrets;
pub mod cli;
pub mod config;
pub mod engine;
pub mod tui;
```
`pub mod provider` already re-exports everything underneath. **Expected diff: zero.** If planner decides to add a top-level `pub use ahb::provider::codex::CodexProvider` alias, that's optional convenience — Phase 1 does not do this for `ClaudeProvider`, so skip for consistency.

---

### 13. `src/main.rs` (MOD — dispatch + exit-code mapping + SIGPIPE)

**Analog:** `src/main.rs::main` lines 31-95

**Cues for `<read_first>`:** `src/main.rs:1-95` (full file), CONTEXT D-59 (exit code grid), D-60 (SchemaDrift = fail), RESEARCH §Architectural Responsibility Map row 6 ("Exit code computation: Entry (src/main.rs)"), CONTEXT Deferred-Ideas line 366 (SIGPIPE planner-discretion).

**Existing dispatch pattern** (lines 91-95 of analog):
```rust
match cli.command {
    None => ahb::cli::run_compact(&engine, cli.ascii, cli.color).await,
    Some(Command::Tui) => ahb::tui::run(engine).await,
}
```

**New dispatch pattern to mirror** (after adding 3 mutually-exclusive flags + the clap `ArgGroup`):
```rust
let exit_code = match cli.command {
    Some(Command::Tui) => {
        ahb::tui::run(engine).await?;
        0
    }
    None => {
        if cli.json {
            ahb::cli::run_json(&engine, cli.color).await?
        } else if cli.detailed {
            ahb::cli::run_detailed(&engine, cli.ascii, cli.color).await?
        } else {
            // compact = explicit OR default-when-no-flag
            ahb::cli::run_compact(&engine, cli.ascii, cli.color).await?
        }
    }
};
std::process::exit(exit_code);
```
Where each `run_*` returns `anyhow::Result<i32>` representing 0/1 per the D-59 grid. The config/secrets `exit(2)` path (lines 55-87 of analog) is already wired — Phase 2 keeps it byte-identical.

**Panic-hook installation pattern stays first** (lines 23-29 of analog):
```rust
fn install_phase0_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("ahb panicked: {info}");
        original(info);
    }));
}

// in main():
install_phase0_panic_hook();  // MUST be first
```
**Do not modify this** — it's D-27 LOCKED. Phase 2 adds dispatch logic AFTER the panic-hook install, NOT before.

**SIGPIPE handling (Deferred-Ideas line 366):** planner-discretion. Conservative recommendation: insert near top of `main()`, after panic-hook install:
```rust
#[cfg(unix)]
unsafe {
    libc::signal(libc::SIGPIPE, libc::SIG_DFL);
}
```
(requires adding `libc` dep, ~50KB). Alternative: catch `BrokenPipe` in stdout writes and exit 0 silently. Planner picks; RESEARCH §SIGPIPE Deferred says "Phase 2 不顯式管" so this can be punted to Phase 4 if the verification UAT doesn't surface it.

---

### 14. `Cargo.toml` (MOD — add `rusqlite`)

**Analog:** `Cargo.toml:40-41` (any `# Phase N — purpose.` comment + dep line)

**Pattern to mirror** (lines 40-41 of analog):
```toml
# Phase 1 — JSONL file discovery for Claude adapter (REQ ADP-02).
glob = "0.3"
```
Add (per CONTEXT canonical_refs § dependencies line 243):
```toml
# Phase 2 — ADP-04 Codex state DB read-only (bundled SQLite = no system dep).
rusqlite = { version = "0.39", features = ["bundled"] }
```
Place after the `glob = "0.3"` line (keep Phase ordering). Confirm `cargo tree` shows only one `libsqlite3-sys` post-build.

---

### 15. `src/templates/default-config.toml` (MOD — update Codex comment)

**Analog:** `src/templates/default-config.toml:8-9`

**Current** (lines 8-9 of analog):
```toml
[providers.codex]
enabled = false  # Codex CLI subscription — not yet implemented (Phase 2)
```

**Replace with** (per CONTEXT specifics line 346-349):
```toml
[providers.codex]
enabled = false  # Codex CLI subscription — reads ~/.codex/sessions/**/rollout-*.jsonl + state_*.sqlite (read-only)
```
**Test impact:** `src/config.rs::default_template_parses_cleanly` (lines 235-242) still passes — only the comment changed.

---

### 16. `tests/secret_leak_subprocess.rs` (MOD — add `--json` route variant)

**Analog:** itself, lines 18-46 (the existing `subprocess_secret_does_not_leak` test)

**Cues for `<read_first>`:** `tests/secret_leak_subprocess.rs:1-53` (full file), `src/cli/mod.rs::debug_emit_fake_secret_and_exit` (lines 113-135) — the Phase 1 emitter pattern that Phase 2 extends.

**Existing test pattern** (lines 18-46 of analog):
```rust
#[test]
#[cfg(debug_assertions)]
fn subprocess_secret_does_not_leak() {
    let output = assert_cmd::Command::cargo_bin("ahb")
        .unwrap()
        .arg("--debug-emit-fake-secret")
        .output()
        .expect("subprocess should run");
    assert!(output.status.success(), …);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains(FIXTURE), …);
    let re = regex::Regex::new("[A-Za-z0-9]{20,}").unwrap();
    assert!(!re.is_match(&stdout), …);
    assert!(stdout.contains("[REDACTED]"), …);
}
```

**Pattern to mirror — add a sibling test for the `--json` route** (per CONTEXT D-62 + RESEARCH §SEC-03):
```rust
#[test]
#[cfg(debug_assertions)]
fn subprocess_json_path_redacts_secret() {
    // Setup: tempdir + fake config with mock provider enabled + AHB_SECRETS_MOCK=1
    let tmp = tempfile::tempdir().unwrap();
    // … (mirror tests/cli_walking_skeleton.rs::setup_fake_home minus the .claude/projects bit)
    let output = assert_cmd::Command::cargo_bin("ahb")
        .unwrap()
        .env("HOME", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join("xdg"))
        .env("AHB_SECRETS_MOCK", "1")
        .env("NO_COLOR", "1")
        .arg("--json")
        .arg("--debug-emit-fake-secret")
        .output()
        .expect("subprocess should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains(FIXTURE), "json route leaked fixture: {stdout}");
    let re = regex::Regex::new("[A-Za-z0-9]{20,}").unwrap();
    assert!(!re.is_match(&stdout), "json route emitted 20+-char alphanumeric: {stdout}");
    assert!(stdout.contains("[REDACTED]"), "json route missing [REDACTED] marker: {stdout}");
}
```
**Caveat:** the existing `debug_emit_fake_secret_and_exit` in `src/cli/mod.rs:113-135` short-circuits BEFORE engine dispatch — to test the `--json` route through `run_json`, the planner must either (a) thread the fake-secret injection into `run_json` (via a `#[cfg(debug_assertions)]` test-only branch), or (b) decouple the fake-secret emission so it runs WITHIN `run_json` instead of before it. **Recommendation per RESEARCH §Architectural Responsibility Map row 8:** "Extend existing `--debug-emit-fake-secret` flag with `--json` route; do NOT add new test-only feature flag." Concretely: when `--json && cli.debug_emit_fake_secret`, the `run_json` fn builds a synthetic `JsonRoot` containing a `Secret<String>` field (via a debug-only struct extension or a synthetic provider error message) and emits it. Planner clarifies the implementation shape.

---

### 17. `tests/integration_codex.rs` (NEW — Pitfall 3 SQLite-lock guard test)

**Analog:** partial — `tests/panic_isolation.rs` for the "background process + assert AHB still works" idiom; `tests/cli_walking_skeleton.rs::setup_fake_home` for the tempdir+env injection.

**Cues for `<read_first>`:** `tests/panic_isolation.rs:1-60`, `tests/cli_walking_skeleton.rs:1-50` (setup_fake_home), CONTEXT specifics § "Codex Pitfall 3 守衛測試" (lines 295-297), RESEARCH §Codex SQLite Schema (busy_timeout 250ms binding).

**Setup pattern** (lines 16-39 of analog `tests/panic_isolation.rs`):
```rust
fn setup_fake_home() -> (tempfile::TempDir, …) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().to_path_buf();
    // Build fake ~/.codex/sessions/.../rollout-*.jsonl + ~/.codex/state_5.sqlite
    let codex_dir = home.join(".codex");
    let sessions_dir = codex_dir.join("sessions").join("2026").join("05").join("25");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::write(sessions_dir.join("rollout-...jsonl"), JSONL_FIXTURE).unwrap();
    // Create an empty SQLite file with the correct schema-stub
    rusqlite::Connection::open(codex_dir.join("state_5.sqlite")).unwrap();
    // … xdg + config setup mirrors panic_isolation
    (tmp, xdg)
}
```

**Test scenarios** (per CONTEXT specifics line 295-297):
1. **Happy path:** ~/.codex with synthetic JSONL containing non-null rate_limits → `AHB --json` returns `status: "ok"` for codex.
2. **Pitfall 3 lock guard:** open `state_5.sqlite` in another rusqlite::Connection with `BEGIN IMMEDIATE` (RESERVED lock) → run `AHB --json` 5 times → assert never hangs > 1s and never emits `ProviderError::Network` / `Internal` with "database is locked". The 250ms `busy_timeout` should produce either `Ok` (since SQLite open is read-only and Phase 2 emits no SELECT — no actual lock contention) or `Err(Internal)` if rusqlite's open itself contends. Either way the test asserts `elapsed < 1500ms`.
3. **Fallback for CI without codex CLI:** the CONTEXT says "若 CI 上無 codex CLI 安裝: fallback 用 `tempfile::tempdir` 建 fake `state_5.sqlite` + 另一個 writer process 持 RESERVED lock 模擬". Default to the fallback path; do NOT attempt to spawn real `codex` CLI in CI.

**Pattern to mirror — subprocess + env injection** (lines 41-50 of analog `tests/panic_isolation.rs`):
```rust
let output = assert_cmd::Command::cargo_bin("ahb")
    .unwrap()
    .env("HOME", home)
    .env("XDG_CONFIG_HOME", &xdg)
    .env("NO_COLOR", "1")
    .env("AHB_SECRETS_MOCK", "1")
    .arg("--json")
    .output()
    .expect("subprocess should run");
```

---

### 18. `tests/integration_output_formats.rs` (NEW — snapshot tests for 3 formats)

**Analog:** `tests/cli_walking_skeleton.rs` (entire file — same idiom of "set up fake HOME + run subprocess + assert stdout regex")

**Cues for `<read_first>`:** `tests/cli_walking_skeleton.rs:1-199` (full file), `tests/schema_drift_sentinel.rs:46-82` (env injection pattern), RESEARCH §Supporting Libraries note about `insta` ("NOT currently in dev-deps — recommend deferring until > 3 snapshot tests") — Phase 2 has exactly 3-4, so **stay with hand-rolled regex/equality assertions, do NOT pull in insta** unless planner decides otherwise.

**setup_fake_home pattern** (lines 23-53 of analog `tests/cli_walking_skeleton.rs`):
```rust
fn setup_fake_home() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().to_path_buf();
    let xdg = home.join("config_home");
    // …
    let ahb_cfg = xdg.join("ahb");
    std::fs::create_dir_all(&ahb_cfg).unwrap();
    std::fs::write(&cfg_path, "[providers.claude]\nenabled = true\n…").unwrap();
    (tmp, xdg)
}
```

**Per-format assertion pattern** — three sibling tests:

`test_compact_format_produces_expected_shape`: mirror lines 55-100 of analog (the regex `^claude  \S{10}\s+\d{1,3}%\s+.\s+resets in \d+h\d{2}m$`).

`test_detailed_format_produces_header_indented_lines`: assert
- First line: `^claude$` (header)
- Subsequent indented lines: `^  (5h|weekly)\s+\S{10}\s+\d{1,3}%\s+.\s+resets in \S+$`
- Empty line between providers (when 2+ providers).

`test_json_format_round_trips_through_jq`: assert
- `stdout` parses as JSON via `serde_json::from_str::<serde_json::Value>`.
- Top-level keys are `["schema_version", "generated_at", "providers"]`.
- `schema_version == 1`.
- `providers[].status` is one of `"ok"` / `"error"`.
- `providers[]` order matches BL-02 (Claude first if enabled).

**Critical env-injection idiom** (lines 60-72 of analog):
```rust
let assert = Command::cargo_bin("ahb")
    .unwrap()
    .env("HOME", home)
    .env("XDG_CONFIG_HOME", &xdg)
    .env_remove("APPDATA")
    .env_remove("NO_COLOR")
    .env("AHB_SECRETS_MOCK", "1")
    .arg("--detailed")  // or --compact / --json
    .assert()
    .success();
```

---

## Shared Patterns (cross-cutting concerns)

### Lint-floor banner

**Source:** every `.rs` file under `src/` opens with the same pair of attributes. Verified canonical at `src/provider/claude/mod.rs:15-16`, `src/cli/mod.rs:8-9`, `src/cli/render_text.rs` (implicit via `src/lib.rs:7-8`), `src/main.rs:9-10`.

**Pattern:**
```rust
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
```
**Apply to:** every NEW `.rs` file in Phase 2 (`src/provider/codex/{mod,jsonl,sqlite,window}.rs`, `src/cli/render_json.rs`). New integration tests `tests/integration_*.rs` should mirror the existing pattern from `tests/cli_walking_skeleton.rs` (which uses `#[allow(clippy::unwrap_used)]` at test-fn scope per `clippy.toml allow-unwrap-in-tests = true`).

### `Send + Sync` static_assertions

**Source:** `src/provider/claude/mod.rs:138-139` + `src/provider/mock.rs:70-71`

**Pattern:**
```rust
#[cfg(test)]
mod tests {
    use static_assertions::assert_impl_all;
    assert_impl_all!(ClaudeProvider: Send, Sync);
    assert_impl_all!(Box<dyn Provider>: Send, Sync);
}
```
**Apply to:** `src/provider/codex/mod.rs` test module.

### `ctx.now` clock-injection rule

**Source:** `src/provider/claude/mod.rs:125` (`fetched_at: ctx.now`), enforced by `tests/no_walltime_in_adapter.rs`.

**Pattern:** every adapter uses `ctx.now` (a `jiff::Timestamp` `Copy` value) and NEVER `jiff::Timestamp::now()`. The grep test scans `src/provider/**` for the forbidden substring.

**Apply to:** `src/provider/codex/mod.rs` AND `src/provider/codex/jsonl.rs`. The Codex twist: `resets_in_seconds` is anchored on the rollout *line's own* `timestamp` (parsed from the JSONL field), NOT `ctx.now`. The grep test does NOT forbid `line.timestamp + Span` — only the literal `Timestamp::now()` call.

### Error variant + UI-SPEC literal pattern

**Source:** `src/provider/claude/mod.rs:76-80` (Unavailable literal with `?` next-step hint) + `src/cli/render_text.rs:116-140` (SchemaDrift sentinel).

**Pattern (Unavailable literals — UI-SPEC binding: must end with `?` next-step hint):**
```rust
return Err(ProviderError::Unavailable {
    reason: "~/.claude/projects not found — is Claude Code installed?".into(),
});
```
**Apply to Codex per CONTEXT specifics + D-46:**
```rust
return Err(ProviderError::Unavailable {
    reason: "no ~/.codex/state_*.sqlite found — is Codex CLI installed?".into(),
});
// and:
return Err(ProviderError::Unavailable {
    reason: "no ~/.codex/sessions/**/rollout-*.jsonl found".into(),  // adjust to end with ?-hint
});
```
**Pattern (SchemaDrift — for `rate_limits: null`):**
```rust
return Err(ProviderError::SchemaDrift {
    missing: vec!["rate_limits".to_string()],
});
```
The render layer already paints `{label}  ▒▒▒▒▒▒▒▒▒▒ ??% • {Label} adapter may be out-of-date` once the planner generalizes the literal per Pattern 7.

### `Cow<'static, str>` for `HpWindow.label` and `ProviderState.source`

**Source:** `src/model.rs:51` + `src/model.rs:69` + `src/provider/claude/mod.rs:115,126`.

**Pattern:**
```rust
label: Cow::Borrowed("claude"),
source: Cow::Borrowed("claude-jsonl"),
```
**Apply to Codex:**
```rust
label: Cow::Borrowed("primary"),    // or "secondary" — RESEARCH-verified passthrough labels
source: Cow::Borrowed("codex-jsonl"),
```

### Cargo.toml dep comment idiom

**Source:** `Cargo.toml:40` (`# Phase 1 — JSONL file discovery for Claude adapter (REQ ADP-02).`)

**Pattern:** every dep line carries a single-line `# Phase N — purpose [+ REQ ID].` comment immediately above.

**Apply to:** the new `rusqlite` line per Pattern 14.

### Subprocess integration-test env injection

**Source:** `tests/cli_walking_skeleton.rs:60-72` (the env grid used by every Phase-1 integration test).

**Pattern:**
```rust
let assert = Command::cargo_bin("ahb")
    .unwrap()
    .env("HOME", home)
    .env("XDG_CONFIG_HOME", &xdg)
    .env_remove("APPDATA")
    .env_remove("NO_COLOR")
    .env("AHB_SECRETS_MOCK", "1")  // bypass D-41 keyring hard-error on backend-less CI
    .arg("…")
    .assert()
    .success();
```
**Apply to:** every new `tests/integration_*.rs` test in Phase 2. The `AHB_SECRETS_MOCK=1` env is the keyring-bypass per `src/secrets.rs:115-133`.

### `#[cfg(debug_assertions)]` test-injection gate

**Source:** `src/cli/mod.rs:37-39` (`debug_emit_fake_secret` field) + `src/provider/mock.rs:34-43` (`AHB_DEBUG_PANIC` env) + `src/secrets.rs:115-133` (`AHB_SECRETS_MOCK` env).

**Pattern:** all test-only injection paths are gated by `#[cfg(debug_assertions)]` so cargo-dist release builds physically cannot ship them.

**Apply to:** any Phase 2 test-injection extension (per RESEARCH §Architectural Responsibility Map row 8 + CONTEXT D-62).

---

## No Analog Found

Files with no close match in the existing codebase. The planner should rely on RESEARCH excerpts and the patterns documented above rather than searching for additional analogs.

| File | Role | Data Flow | Reason | Substitute Source |
|------|------|-----------|--------|-------------------|
| `src/provider/codex/sqlite.rs` | rusqlite open + version-sorted glob | sync file-I/O (in `spawn_blocking`) | No SQLite usage anywhere in `src/` today; `glob::glob_with` is used only in `src/provider/claude/jsonl.rs:179-197` for `*.jsonl` discovery (different filename pattern, no version-suffix sort) | RESEARCH §Codex SQLite Schema (lines 307-351) + §glob 0.3 Version Sort (lines 520-576) — both have verbatim recommended code |
| `src/cli/render_json.rs` | DTO module + `serde_json::to_writer` emitter | transform | No file in `src/` is a pure DTO module today; `src/model.rs` mixes the engine-internal types with their serde shape — Phase 2 deliberately decouples per D-49 | RESEARCH §--json schema_version: 1 Design (lines 578-674) — full DTO + converter shape verbatim |
| `tests/integration_codex.rs` (the SQLite-lock guard portion specifically) | integration test for Pitfall 3 (RESERVED-lock contention) | request-response | No existing test exercises SQLite at all; `tests/panic_isolation.rs` is the closest behavioral analog (panic + recover idiom) but doesn't touch SQLite | CONTEXT specifics § "Codex Pitfall 3 守衛測試" (lines 295-297) + RESEARCH §Codex SQLite Schema (busy_timeout binding line 351) |

---

## Metadata

**Analog search scope:**
- `src/provider/**` (all 3 existing adapter modules + trait)
- `src/cli/**` (all 3 modules: mod, render_text, tty)
- `src/engine/**` (fanout + events + mod)
- `src/model.rs`, `src/main.rs`, `src/lib.rs`, `src/config.rs`, `src/secrets.rs`
- `tests/**` (all 15 integration tests)
- `Cargo.toml`, `src/templates/default-config.toml`

**Files scanned:** 32 source files + 15 test files = 47 total. No re-reads (single Read per file).

**Key patterns identified:**
1. **Sibling-adapter mirroring** — Codex adapter follows Claude adapter's module layout (`mod.rs` + `jsonl.rs` + `window.rs` + `sqlite.rs`); Phase 2 just adds the SQLite sibling that Phase 1 didn't need.
2. **Pure-transform-with-#[cfg(test)] inline tests** — every window/render module embeds boundary tests in `#[cfg(test)] mod tests` so Phase 1's BL-03 fix proves the pattern works.
3. **UI-SPEC LOCKED literals + label-via-`id_label`** — every user-facing string either comes from `id_label(ProviderId)` (so Codex inherits Phase 1 wiring for free) or from a hardcoded string the planner must update per D-46/D-47/D-Specifics.
4. **`spawn_blocking` narrow-scope wrap** — Codex's only new async pattern; everywhere else Phase 2 reuses Phase 1 patterns (engine fan-out, panic-hook composition, secrets injection through `FetchCtx`).
5. **Debug-only injection gates** — Phase 2 SEC-03 extension reuses `#[cfg(debug_assertions)]` + `AHB_SECRETS_MOCK`/`AHB_DEBUG_PANIC`/`--debug-emit-fake-secret` pattern; no new feature flag introduced.

**Pattern extraction date:** 2026-05-25
