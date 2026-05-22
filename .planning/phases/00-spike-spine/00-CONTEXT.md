# Phase 0: Spike & Spine - Context

**Gathered:** 2026-05-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 0 is the **gating + spine** phase. It must accomplish two things, no more, no less:

1. **Resolve the largest project risk** — produce a written go/no-go memo for whether Gemini can be implemented safely in v1, by spiking the **local `gemini /stats` capture** route (NOT the web `gemini.google.com/usage` route, which carries Google account-ban risk).
2. **Lock the cross-adapter contract (`model.rs`)** and emit a runnable skeleton `AHB` binary that prints a placeholder HP bar from a `MockProvider`, so every later phase has a stable spine to plug into.

This phase delivers infrastructure and a memo — not a working provider. The user-observable artifact at the end is `cargo run --release` printing one mocked HP bar line through the locked `Provider` trait.

In scope: Gemini local spike, `model.rs` types, repo scaffold, skeleton binary, charset decision, CI skeleton, panic-hook strategy decision.

Out of scope: Any real provider adapter (Claude/Codex/Gemini all wait for later phases), TUI (Phase 1), HTTP, secrets, cache.

</domain>

<decisions>
## Implementation Decisions

### Repo Scaffolding
- **D-01 (Crate name):** `ahb`. Single crate, single binary. Both the cargo crate name and the bin name are `ahb`. `Cargo.toml` description = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance" so crates.io search hits it.
- **D-02 (Rust edition):** `2024`. Matches MSRV 1.88.
- **D-03 (License):** Dual `MIT OR Apache-2.0`. Add `LICENSE-MIT` + `LICENSE-APACHE` + SPDX `license = "MIT OR Apache-2.0"` in Cargo.toml.
- **D-04 (MSRV):** Pin `rust-version = "1.88"` in `Cargo.toml`. This is ratatui 0.30's MSRV; we accept the floor and gain let-chains + async fn in trait support.
- **D-05 (CI):** GitHub Actions on push + PR. Three OS matrix: `ubuntu-latest`, `macos-latest`, `windows-latest`. Each runs `cargo build`, `cargo test`, `cargo clippy -- -D warnings`. No fmt-check, audit, or deny in Phase 0 — defer to Phase 4. Phase 0 CI is the floor that prevents commits from breaking the spine.
- **D-06 (Project layout):** Single binary crate in v1 (NOT a workspace). Internal module split per ARCHITECTURE.md: `src/cli/`, `src/tui/`, `src/engine/`, `src/provider/`, `src/cache.rs`, `src/config.rs`, `src/secrets.rs`, `src/model.rs`. Designed to be liftable to workspace later without rewrites; not pre-emptively split.
- **D-07 (gitignore):** Standard Rust `.gitignore` (`/target`, `**/*.rs.bk`, `Cargo.lock` NOT ignored since this is a binary). `.planning/` already in tracking per project decision (commit_docs=true).

### `model.rs` Contract Shape
- **D-08 (`ProviderState` carries multiple windows):** `pub struct ProviderState { pub id: ProviderId, pub windows: Vec<HpWindow>, pub fetched_at: jiff::Timestamp, pub source: &'static str }`. One fetch returns N windows because a single provider may have multiple concurrent reset windows (Claude has 5h session AND weekly). Avoids N+1 fetches and matches REQ CORE-03 `--detailed` requirement.
- **D-09 (`HpWindow` shape):** `pub struct HpWindow { pub label: Cow<'static, str>, pub percent_remaining: f32, pub reset: ResetInfo, pub bar_color: Option<BarColor> }`. Each window has its own label (e.g., "5h session", "weekly") and reset. `bar_color` lets the adapter hint UI accent (e.g., red < 10%) — Claude's discretion to populate.
- **D-10 (`HpUnit` = percent only, NO raw fields):** Internal alias `pub type HpUnit = f32; // 0.0..=100.0`. Adapters normalize to percent themselves. Rationale: raw token counts confuse users into thinking they hit a token limit when the real limit is throttle-based; better to show only "% remaining + window label". JSON output also stays clean.
- **D-11 (`ResetInfo` = absolute jiff::Timestamp):** `pub struct ResetInfo { pub resets_at: jiff::Timestamp }`. The UI layer computes `now() - resets_at` for countdown rendering. Single source of truth, TUI's 1s tick can recompute display without re-fetching, snapshot tests freeze time deterministically. Adapter is responsible for converting any provider-side duration into an absolute jiff::Timestamp at fetch time.
- **D-12 (`ProviderError` closed enum):** `#[derive(thiserror::Error, Debug)] pub enum ProviderError { Unconfigured, Unavailable { reason: String }, SchemaDrift { missing: Vec<String> }, Network(reqwest::Error wrapper), RateLimited { retry_after: Option<jiff::Span> }, Internal(anyhow::Error) }`. Closed set so the UI can render each variant differently (SchemaDrift → yellow warning, RateLimited → cool-down, etc.). Adding a variant later is a deliberate API change, which is correct.
- **D-13 (Trait shape):** `#[async_trait] pub trait Provider: Send + Sync + 'static { fn id(&self) -> ProviderId; async fn fetch(&self, ctx: &FetchCtx) -> Result<ProviderState, ProviderError>; }`. Same trait both CLI and TUI consume — that's the spine.
- **D-14 (Wire serde, but cautiously):** `ProviderState`, `HpWindow`, `ResetInfo` derive `Serialize` + `Deserialize` (used for `--json` output and for snapshot test fixtures). `ProviderError` derives `Serialize` only (deserialization isn't needed; closed enum doesn't roundtrip cleanly through anyhow wrapper). Mark `Internal(anyhow::Error)` with `#[serde(skip)]` and emit a sentinel field in JSON.

### Charset & HP Bar Visual
- **D-15 (Default charset):** Unicode full block `█` (filled) / `░` (empty). Example: `"████████░░ 80%"`. Verified to render in tmux, screen, and Windows Terminal during Phase 0 (success criterion #4).
- **D-16 (Bar width):** Fixed 10 cells. Snapshot tests, multi-provider alignment, and `--compact` one-line packing all depend on this being fixed. Future `--detailed` may relax to per-row.
- **D-17 (Color default):** Auto-on when stdout is a TTY, auto-off when piped. Respect `NO_COLOR` env (off) and `--color=auto|always|never` flag. Use `std::io::IsTerminal` + a color crate (likely `nu-ansi-term` or `anstyle`; researcher to pick). All `--json` output is uncolored regardless of TTY.
- **D-18 (ASCII fallback):** `--ascii` flag substitutes `#` (filled) / `-` (empty). Example: `"########-- 80%"`. No automatic detection — explicit opt-in. The point is determinism, not auto-magic.
- **D-19 (No emoji in v1):** Pace icons (🧊/🔥/🚨) and emoji-based bars are deferred to v2 (DIFF-01). Phase 0 charset decision excludes emoji to dodge tmux/screen width bugs documented in PITFALLS.

### Gemini Spike Scope & Kill Criteria
- **D-20 (Spike path):** Try **local `gemini /stats` capture FIRST** (not web `gemini.google.com/usage`). Zero account-ban risk because we read the user's own CLI output, not Google's servers. Specifically investigate whether the gemini-cli `/stats` slash command can be invoked in non-interactive mode (e.g., `gemini exec "/stats"` or equivalent), the way `codex exec "/status"` is.
- **D-21 (Go criteria — strict, ALL THREE must hold):**
  1. `/stats` can be triggered non-interactively (no manual REPL needed)
  2. The output contains both remaining-quota information AND reset time (or session window boundary)
  3. The output format is stable and parseable (JSON ideal, deterministic regex acceptable)
- **D-22 (No-go fallback):** If any of (1)-(3) fail, write the memo as `no-go` and **defer Gemini to v2 stub** — do NOT spike the web `gemini.google.com/usage` route in Phase 0. The web route's account-ban risk is asymmetric (whole Google identity) and the v1 win without Gemini still ships Claude + Codex unified HP bar, which is the white-space differentiator.
- **D-23 (Spike output location):** `.planning/research/GEMINI_SPIKE.md`. Sections required:
  1. Method (commands attempted, OS/environment)
  2. Local capture result (3 sample outputs, anonymised)
  3. Parse feasibility assessment (JSON / regex / unparseable)
  4. **Go/No-Go decision** (single line, top of doc)
  5. Kill criteria (what would invalidate this in the future)
  6. Phase 3 hand-off instructions (full adapter steps if go; stub flag wiring if no-go)
  7. Appendix: rationale for NOT spiking the web fallback in Phase 0
- **D-24 (Sample fixtures — NOT in Phase 0):** Anonymised `gemini /stats` fixtures will be checked in only during Phase 3 when the adapter is actually built. Phase 0 captures samples in the memo prose, not as separate fixture files (avoids premature commit of personal-looking quota numbers).

### Skeleton Binary Behavior
- **D-25 (MockProvider output):** Phase 0's runnable `AHB` invokes a `MockProvider` returning a hardcoded `ProviderState` with one `HpWindow` (label `"mock-session"`, `percent_remaining=60.0`, `resets_at` = now + 2h). Output line: `"mock-session  ████████░░░░░░░░░░░░ ... "` — wait, no, bar width is fixed 10 cells per D-16, so: `mock-session  ██████░░░░ 60% • resets in 2h00m`. This proves the spine works end-to-end without touching any real provider data.
- **D-26 (Charset verification):** As part of running the skeleton, the spike runner manually checks the output renders correctly under tmux, screen (if available), and Windows Terminal (best-effort — Linux dev box may not have it). Findings recorded in `GEMINI_SPIKE.md` as a small parallel "Charset verification" section (or split to a separate `CHARSET_NOTE.md` if it grows; Claude's discretion).
- **D-27 (Panic hook):** `ratatui::init()` and `ratatui::restore()` are NOT used in Phase 0 because there's no TUI yet. But the Phase 0 skeleton MUST set up `std::panic::set_hook` to do a noop in the CLI path so Phase 1's TUI integration of `ratatui::init()` panics correctly. The panic-hook contract is locked in Phase 0 even though the TUI consumer arrives in Phase 1.

### Claude's Discretion
- Choice of color crate (`nu-ansi-term` / `anstyle` / `owo-colors`) — Claude picks the one with the cleanest "skip when not TTY" API; researcher to recommend during plan-phase.
- `ProviderId` shape (`enum` with `Claude/Codex/Gemini` variants vs `String` newtype) — Claude picks. Lean toward enum because the set is closed in v1, but if the trait wants extensibility for v2 EXT-01 providers (Cursor, Windsurf, Amp), a `Cow<'static, str>` newtype is fine.
- Exact API of `FetchCtx` — Phase 0 declares the type and includes at least `now: jiff::Timestamp` and `secrets: &Secrets` (Secrets type a no-op stub in Phase 0; Phase 1 wires keyring-core). Other fields filled in when adapters need them.
- Whether to use `tokio::main` or `pollster::block_on` in Phase 0's CLI entry. Pick whichever lets the skeleton link cleanly. Phase 1 will require `tokio::main`.
- Whether to include any deps Phase 0 doesn't strictly use yet (e.g., `keyring-core`, `reqwest`). Default: include only what Phase 0 uses (`clap`, `tokio` lean features, `jiff`, `serde`, `anyhow`, `thiserror`, `async_trait`). Phase 1 adds more.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project context
- `.planning/PROJECT.md` — Core Value, Constraints, Out of Scope (locks Path A rate-limit countdown over Path B cumulative)
- `.planning/REQUIREMENTS.md` — 29 v1 requirements; Phase 0 covers ADP-00 only but the trait must serve ADP-01 through ADP-05
- `.planning/ROADMAP.md` § Phase 0 — goal, mode (mvp), 5 success criteria

### Research synthesis
- `.planning/research/SUMMARY.md` — executive synthesis; treat as required reading
- `.planning/research/STACK.md` — locked stack picks, version numbers, NOT-list (keyring v4 / async-std / tui-rs / parallel crossterm)
- `.planning/research/FEATURES.md` — table-stakes vs differentiator categorization; informs which v2 anti-features stay deferred
- `.planning/research/ARCHITECTURE.md` — Provider trait shape, engine pattern, channel-not-mutex, codex-rs reference; **the spine pattern Phase 0 must lock**
- `.planning/research/PITFALLS.md` — full pitfall list; Phase 0 specifically guards against Pitfall 1 (Gemini ToS) and Pitfall 5 (terminal restore via the deferred Phase 1 hook contract)

### External references for Phase 0 work
- ratatui 0.30 release notes — even though Phase 0 doesn't render TUI, the panic-hook contract must align with how Phase 1 uses `ratatui::init()/restore()`
- `keyring-core` 1.0 docs.rs — Phase 0 stubs the `Secrets` type but the API surface should reflect keyring-core's actual interface (verify at impl time)
- `jiff` 0.2.x docs — used for `ResetInfo` and `FetchCtx::now`; isolate behind a thin `Clock` module per STACK.md guidance against pre-1.0 churn

### Codex `/status` reference pattern
- OpenAI codex CLI source on GitHub — Phase 0 spike researches whether `gemini-cli` exposes a `/stats` non-interactive path analogous to `codex exec "/status"`

No project-internal ADRs exist yet (greenfield). When the first ADR is written (likely a Phase 1 decision around keyring-core fallback strategy), add it here.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
None — greenfield project, zero source code at Phase 0 start.

### Established Patterns
None internal to this repo. Phase 0 establishes the patterns. Referenced external patterns:
- **codex-rs** — shared engine, two front-ends, one binary (lifted into ARCHITECTURE.md). Phase 0 implements the smallest seed of this: a `MockProvider` + a tiny engine that produces output for CLI entry only.
- **Starship per-module timeout** — referenced by ARCHITECTURE.md for per-adapter error isolation. Phase 0 doesn't implement timeout yet (Phase 1) but the trait signature must NOT preclude it (e.g., must be async + cancellable).
- **`Vec<Result<...>>` aggregation** — locked at trait level. Phase 0's skeleton has one provider so this is vacuous, but the engine fn signature is `async fn refresh_all(&self) -> Vec<(ProviderId, Result<ProviderState, ProviderError>)>` from day one — not `Result<Vec<ProviderState>, _>`.

### Integration Points
- `Cargo.toml` is the canonical workspace boundary. All Phase 0 decisions are encoded there.
- GitHub Actions YAML under `.github/workflows/` is the CI surface.
- `src/main.rs` is the CLI entry; `src/lib.rs` may be added if researcher recommends a binary+library split (Claude's discretion — most CLI projects do this for testability).

</code_context>

<specifics>
## Specific Ideas

- **HP bar literal shape (per D-15, D-16):** `mock-session  ██████░░░░ 60% • resets in 2h00m`. The middle dot `•` (U+2022) is part of the locked format — used to separate "% + reset" in compact view. `--ascii` mode substitutes `|` for `•` as well.
- **Mock output for `cargo run` smoke test:** Exactly one line, exit code 0. No "welcome" header, no version banner, no help-hint footer. CLI is laconic.
- **Phase 0 success criterion #2** ("AHB prints a mocked HP bar"): the bar MUST come through the locked `Provider` trait + `ProviderState` types — NOT a hardcoded `println!`. Bypassing the trait to print faster is explicitly forbidden because it would fail to prove the spine.

</specifics>

<deferred>
## Deferred Ideas

These came up during discussion and belong in later phases or backlog:

- **`ttl_hint` field on `ProviderState`** (Area 2 follow-up, skipped) — letting adapters tell engine how long to cache. Deferred to Phase 3 when cache lands; Phase 1's Claude/Codex don't need caching so the field would be premature.
- **`bar_color: Option<BarColor>` rendering rules** — Phase 0 declares the field, but the exact rule (red < 10%, yellow < 30%, green otherwise) is a Phase 1 / Phase 2 UI concern.
- **`ProviderState::stale: bool` flag** — needed when cache fallback is used. Deferred to Phase 3 alongside moka cache.
- **Web `gemini.google.com/usage` route** — explicitly deferred: not investigated in Phase 0 even as a fallback. If local route fails AND user later wants Gemini in v1, that's a separate scope decision, not a Phase 0 problem.
- **Sample fixtures for Gemini `/stats` output** — deferred to Phase 3 (adapter implementation phase). Phase 0 memo includes samples inline as prose, not as committed fixture files.
- **Emoji / pace icon mode** — deferred to v2 (DIFF-01). Phase 0 explicitly excludes this from the charset decision.
- **`cargo fmt --check`, `cargo audit`, `cargo deny` in CI** — deferred to Phase 4 Distribution polish. Phase 0 CI floor is build + test + clippy.
- **Crate-level docs / docs.rs landing page** — deferred to Phase 4.
- **Cargo workspace split** — deferred indefinitely; only revisit if v2+ adds a daemon mode (OPS-01) or external lib consumers.

### Reviewed Todos (not folded)
None — no project todos existed pre-phase.

</deferred>

---

*Phase: 0-Spike & Spine*
*Context gathered: 2026-05-22*
