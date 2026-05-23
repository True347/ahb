# Phase 1: Engine + Claude + TUI Scaffold — Research

**Researched:** 2026-05-23
**Domain:** Rust async orchestration + ratatui 0.30 TUI + Claude Code JSONL parsing + keyring-core 1.0 + cross-OS config — walking-skeleton vertical slice
**Confidence:** HIGH on stack picks (verified on crates.io 2026-05-23) and architecture patterns (Phase 0 contract already locked); MEDIUM on Claude 5h window token math (upstream JSONL data quality is a known problem — see Pitfall L1); MEDIUM on keyring-core 1.0 backend connector status (the OS-backed stores are separately-published companion crates that only hit 1.0 alongside keyring-core itself).

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Engine concurrency & refresh loop

- **D-28 (fan-out):** `engine.refresh_all()` uses `tokio::task::JoinSet`. Each adapter is spawned into the JoinSet and drained via `.join_next()`. Strictly stronger than `join_all + timeout` because a slow adapter from the previous tick can be `abort()`-ed when a new tick fires.

- **D-29 (per-adapter timeout location):** Engine wraps every spawn in `tokio::time::timeout(per_provider_timeout, provider.fetch(ctx))`. The `Provider` trait does NOT carry a deadline. Phase 1 default value is left to the planner (suggested 2 s for Claude since it's pure local I/O).

- **D-30 (TUI refresh loop architecture):** Single `tokio::time::interval(15s)` tick triggers `engine.refresh_all()`, results pushed to mpsc as a batch. Phase 3 CFG-03 introduces per-provider intervals; Phase 1 does not pre-bake that knob.

- **D-31 (countdown re-render cadence):** TUI redraws every 1 s (its own `interval(1s)`) but only fetches every 15 s. Render task reads cached state only — never triggers fetch. Justification: countdown is the second-most-glanced field; 1 s redraw avoids "is this thing dead?" confusion at near-zero cost (ratatui diffs frames).

#### Claude JSONL parsing

- **D-32 (file discovery):** Use the `glob` crate with literal pattern `glob("~/.claude/projects/**/*.jsonl")`. Matches REQ ADP-02 / PROJECT.md wording exactly. The directory structure is one level deep (`projects/<slug>/<session-uuid>.jsonl`); glob covers it.

- **D-33 (5h rolling-window cluster anchor):**
  1. Collect every message across all `.jsonl` files, sort by `timestamp`.
  2. Walk backward from newest, find the first gap > 5 h → cluster boundary.
  3. `session_start` = earliest user-message timestamp inside the cluster.
  4. `reset_at` = `session_start + 5h`.
  5. `percent_remaining` = compare summed `message.usage` tokens in the cluster vs assumed 5 h token limit.

  > **Phase 1 RESEARCH NOTE — see Pitfall L1 below:** the formula `input_tokens + output_tokens` from CONTEXT D-33 step 5 needs revision. Upstream Claude Code logs `input_tokens` / `output_tokens` as unreliable streaming placeholders (~75 % of entries are 0 or 1); only `cache_creation_input_tokens` + `cache_read_input_tokens` are accurate to ground-truth. The planner must choose one of three escape hatches before writing the adapter (see L1 mitigation).

- **D-34 (ADP-03 schema-drift sentinel trigger):** Read the most recent N = 3 `type:"assistant"` messages. If ≥ 2 are missing `message.usage` or are missing `input_tokens` / `output_tokens` (or whichever fields the planner ends up summing — see D-33 note), trigger the sentinel. Sentinel literal (UI-SPEC LOCKED): `claude  ▒▒▒▒▒▒▒▒▒▒ ??% • Claude adapter may be out-of-date`.

- **D-35 (hot-file truncated-trailing-line tolerance):** Stream via `BufReader::new(File::open(path)?).lines()`. A mid-file `serde_json::from_str` failure → `tracing::warn!` + skip. A trailing-line failure → silently skip (treat as Claude's in-flight append). No mmap, no stat-then-read tricks.

#### Config schema & first-run UX

- **D-36 (TOML schema):** Per-provider table:
  ```toml
  [providers.claude]
  enabled = true

  [providers.codex]
  enabled = false

  [providers.gemini]
  enabled = false
  ```
  `ProviderId` is a closed enum; table headers hardcoded to the three v1 providers + `mock`.

- **D-37 (first-run UX):** Missing `~/.config/ahb/config.toml` →
  1. `mkdir -p` config dir,
  2. write the default (loaded via `include_str!("../templates/default-config.toml")`) with markdown-comment headers, all `enabled = false`,
  3. print `initialized ~/.config/ahb/config.toml — enable providers and rerun` to stdout,
  4. `exit(0)`. Reproducible, pipe-safe, no interactive wizard.

- **D-38 (unknown-key policy):** Warn + ignore. Do NOT use `#[serde(deny_unknown_fields)]`. Unknown keys → single stderr warn `unrecognized config key '{}' — see README`. Forward-compatible.

- **D-39 (ProjectDirs args):** `directories::ProjectDirs::from("", "", "ahb")` — both qualifier and organization empty, application `"ahb"`. Produces:
  - Linux: `~/.config/ahb/config.toml`
  - macOS: `~/Library/Application Support/ahb/config.toml`
  - Windows: `%APPDATA%\ahb\config.toml`

#### Secrets / keyring

- **D-40 (Phase 1 keyring use):** Wire the **entire** keyring-core code path but Phase 1 never actually stores or loads a real secret — the Claude adapter doesn't need one. `Secrets` becomes a field of `FetchCtx`; Claude receives an empty `Secrets`. CI grep test exercises `Secret<T>` via Debug/serde paths only. Goal: burn down platform-specific keyring bugs (macOS prompt, Windows session, Linux dbus) in Phase 1, before Phase 2/3 plug real adapters.

- **D-41 (Linux headless fallback policy):** keyring unavailable → **hard error**, exit code 2 (REQ CORE-06 = config/secrets unloadable), stderr message `no secret store available on this system; set [secrets].storage = "file" in ~/.config/ahb/config.toml to opt into 0600 file storage`. Phase 1 does NOT implement the file backend; only mentions it in the error. Never silently fall back (STACK.md binding).

- **D-42 (`Secret<T>` shape):** `pub struct Secret<T: Zeroize + Clone>(T)`, with:
  - `impl Drop` calling `self.0.zeroize()`
  - `impl Debug` printing `***`
  - `impl Serialize` emitting `"[REDACTED]"`
  - **No** `Deserialize` impl (secrets are read from keyring, never from TOML/JSON)
  - `Secret::new(inner)` constructor and `Secret::expose(&self) -> &T` unwrapper (grep-discoverable)
  - New dep: `zeroize` (Phase 1 only new secret-related dep — NOT `secrecy`)

- **D-43 (CI grep test):** Construct `Secret::new("deadbeefcafe1234567890abcdef".to_string())`, exercise via `format!("{:?}", …)` and `serde_json::to_string(&…).unwrap()`. Double assert: (1) literal string absent, (2) any `[A-Za-z0-9]{20,}` pattern absent (regex test-dep). Integration test mirrors: full `AHB --json` against mock fixture, grep stdout with same double assert.

### Claude's Discretion

- mpsc buffer size (suggest `unbounded` or `bounded(64)`).
- `EngineEvent` enum shape (suggest `Refresh / TickError / Shutdown`).
- Default per-adapter timeout value (suggest 2 s for Claude).
- Claude 5h token-limit numeric constant — phase-researcher to extract from official Claude docs / ccusage / `usagebar.com`; hardcode as `const CLAUDE_5H_TOKEN_LIMIT` with a config-override hook (hook NOT exposed to users in Phase 1).
- Whether `cache_read_input_tokens` counts toward usage; per-project filter? — phase-researcher to confirm against Anthropic billing docs; suggested default: count `cache_creation`, skip `cache_read`, no per-project filter.
- ratatui widget: `Gauge` vs hand-built `Paragraph + Span`.
- Panic-injection integration test mechanism (suggest env var `AHB_DEBUG_PANIC=adapter:claude` gated behind `#[cfg(debug_assertions)]`).

### Deferred Ideas (OUT OF SCOPE)

- Per-provider `refresh_interval` override (CFG-03 → Phase 3).
- Per-provider `auth_source` / cookie path (Phase 3).
- `secret_storage = "file"` 0600 file backend (later phase; Phase 1 only references it in error message).
- `AHB_DISABLE_KEYRING=1` env var override.
- `--strict-config` opt-in for `deny_unknown_fields`.
- Interactive first-run TTY wizard (Phase 4 polish or backlog).
- `AHB_CONFIG_PATH` env override (Phase 4).
- `config_dir` vs `preference_dir` macOS choice (planner confirms; recommended `config_dir`).
- `ratatui::Gauge` vs hand-built bar — Claude's discretion.
- Claude 5 h numeric token-limit constant — phase-researcher locates.
- `cache_read_input_tokens` policy — phase-researcher locates.
- Panic-injection mechanism choice — planner decides.
- mpsc buffer / `EngineEvent` shape — planner fills.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CORE-01 | `AHB` no-arg default = compact one-line HP bar for every configured provider | Reuse `cli::render_text::compact_line` (Phase 0); swap MockProvider → real ClaudeProvider through Engine fan-out (Standard Stack §Engine, Pattern 4) |
| CORE-05 | Auto-disable color on non-TTY pipe + honor `NO_COLOR` | `std::io::IsTerminal::is_terminal(&io::stdout())` (stable since 1.70); decision branch in renderer; `NO_COLOR` env-var read once at startup (Concern §4) |
| TUI-01 | `AHB tui` fixed full-screen view, one row per provider | ratatui 0.30 `DefaultTerminal` + `Layout::vertical([Length(1), Min, Length(1), Length(1)])` per UI-SPEC; widget table in Standard Stack §TUI |
| TUI-02 | TUI 15 s auto-refresh | D-30 architecture: `tokio::time::interval(Duration::from_secs(15))` triggers `engine.refresh_all()`; second `interval(1s)` for redraw cadence (Concern §2 + §10) |
| TUI-04 | Panic / Ctrl-C restores terminal cleanly | `ratatui::run()` (NOT `init/restore` pair) installs panic hook automatically; Phase 0 panic hook composes via `take_hook + set_hook` (Concern §2, exact pattern below) |
| TUI-05 | TUI refuses non-TTY with clear error | `std::io::IsTerminal::is_terminal(&io::stdout())` check at top of `tui::run`; bail with UI-SPEC literal copy + `std::process::exit(2)` (Concern §4) |
| CFG-01 | TOML lists providers with independent enable/disable | D-36 schema; bare `serde` + `toml` 0.8 (no figment) (Concern §5) |
| CFG-02 | Cross-OS config path via `directories` | `ProjectDirs::from("", "", "ahb").config_dir()` (D-39); `directories` 6.0 verified on crates.io (Concern §5) |
| CFG-04 | Un-configured providers silently skipped, not flagged | Filter `Vec<ProviderConfig>` by `enabled == true` BEFORE feeding the engine; engine has no knowledge of disabled providers; treats empty list as empty Vec, not error (UI-SPEC copy `no providers configured` only fires when list ends up empty) |
| SEC-01 | All secrets in OS keyring via `keyring-core` 1.0 | Concern §6: `set_default_store(...)` at startup, then `Entry::new("ahb", "<provider-id>")`; Linux/macOS/Windows backend connector crates listed |
| SEC-02 | `Secret<T>` newtype, redacting Debug, `#[serde(skip)]`-class | D-42 shape; new dep `zeroize` 1.8 (Concern §6 + Code Examples) |
| SEC-04 | Pure-no-secret providers (Claude) still go through identical interface | `FetchCtx<'a>` already carries `&'a Secrets` (Phase 0 lock-in); Claude adapter receives the same field, just doesn't read from it (no contract divergence) |
| ADP-01 | One adapter failing only affects that provider | D-28 + D-29 + Phase 0 `Vec<Result<...>>` aggregation; engine catches both `Err(_)` AND `JoinError::is_panic()` via JoinSet (Concern §3) |
| ADP-02 | Claude adapter computes 5 h rolling window from `~/.claude/projects/**/*.jsonl` | D-32 + D-33; **Pitfall L1 below alters the field-summing choice** — planner must pick `cache_creation_input_tokens` (reliable) over `input_tokens + output_tokens` (unreliable per ccusage issue #866) before locking the const |
| ADP-03 | Schema-drift sentinel | D-34; UI-SPEC literal locked |

</phase_requirements>

## Project Constraints (from CLAUDE.md)

CLAUDE.md is overwhelmingly a **tech-stack lock-in document** (already mirrored into STACK.md). The actionable directives that survive into RESEARCH:

- **Single-binary distribution** — no runtime dep, no OpenSSL / native-tls. Phase 1 must keep dep additions minimal (each addition documented in Cargo.toml comments per Phase 0 convention).
- **`tokio` features stay explicit** — never `features = ["full"]`. Phase 1 upgrades from Phase 0's `["rt", "macros"]` to `["rt-multi-thread", "macros", "fs", "time", "signal", "sync"]`.
- **Use `ratatui::crossterm` re-export** — do NOT add `crossterm` to `Cargo.toml`. (Phase 1 critical: ratatui 0.30 pins crossterm 0.29 transitively. A second `crossterm` somewhere in the dep tree silently breaks rendering.)
- **`keyring-core` 1.0, NEVER the `keyring` 4.x facade.** Confirmed: `keyring` v4 README explicitly disclaims library use.
- **`rustls-tls` only.** When Phase 3 lands reqwest, gate `--no-default-features` + explicit features. Phase 1 does not ship reqwest.
- **Privacy:** no telemetry, no upload, tokens never written to disk in plaintext.
- **`unwrap_used = deny` lint floor** is inherited from `src/lib.rs` and `src/main.rs`. Every new module in Phase 1 inherits it automatically.
- **`jiff::Timestamp::since` MUST pass `(Unit::Hour, *now)`** explicitly — default is `Unit::Second` which silently produces `7200s 0m 0h` instead of `2h 0m 0s` (Phase 0-03 deviation 1).
- **Wall-clock reads centralized at `src/main.rs`.** Phase 1 Claude adapter MUST use `ctx.now`, not `jiff::Timestamp::now()`. Acceptance grep guards mock.rs and will extend to claude.rs.

---

## Summary

Phase 1 is the **vertical-slice walking-skeleton** for AHB: the same `AHB` binary that today prints a mocked HP bar must, after this phase, print a real Claude session % computed from `~/.claude/projects/**/*.jsonl` AND open a panic-safe ratatui TUI that auto-refreshes every 15 s AND silently skip a disabled / unconfigured provider AND survive a deliberately-injected adapter panic without scorching the user's shell. The Phase 0 spine (Provider trait + ProviderState + FetchCtx + Vec<Result<...>> aggregation + composable panic hook) is already in place — Phase 1's job is to thread one real adapter through it, add a second front-end (TUI), and stand up the load-bearing infrastructure (keyring path, Secret<T>, config loader, file-glob, error isolation policy) so Phase 2 (Codex) and Phase 3 (Gemini) only have to write `provider/codex.rs` and `provider/gemini.rs`.

The largest planning-time risk this research surfaces is **Pitfall L1: the CONTEXT D-33 token formula is upstream-broken**. Anthropic's Claude Code emits `input_tokens` and `output_tokens` as streaming placeholders that are accurate only ~25 % of the time; the **reliable** fields are `cache_creation_input_tokens` and `cache_read_input_tokens`. The de-facto reference parser ([ccusage issue #866](https://github.com/ryoppippi/ccusage/issues/866)) measures undercount ratios of 100-174x on `input_tokens` and 10-17x on `output_tokens` (the latter omits thinking tokens entirely on Opus models). CONTEXT D-33 step 5 says "sum `input_tokens + output_tokens`" — that formula will produce wildly wrong HP bars and will silently zero out long Opus sessions. The planner must pick a Phase 1 token-summing strategy (recommended: `cache_creation_input_tokens` as the headline signal; document the limitation in README; revisit in v2) before locking the `CLAUDE_5H_TOKEN_LIMIT` constant.

**Primary recommendation:** Use `ratatui::run()` (not `init/restore`) for guaranteed panic-safe terminal restoration; use `tokio::task::JoinSet` for adapter fan-out with `JoinError::is_panic()` → `ProviderError::Internal` conversion; build the Claude adapter against the `cache_*` token fields (not the broken `input_tokens` / `output_tokens` fields); wire keyring-core 1.0 with an OS-appropriate companion store crate registered via `set_default_store()` at startup; treat the keyring backend connector ecosystem as MEDIUM confidence and add a `checkpoint:human-verify` gate before the planner locks a backend crate.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Parse Claude `~/.claude/projects/**/*.jsonl` | Adapter (`provider/claude.rs`) | — | Per-provider data-format quirks belong in the adapter; the engine sees only `ProviderState`. |
| 5 h cluster math (window-anchor + reset time) | Adapter | Engine (clock injection via `FetchCtx::now`) | Domain knowledge belongs to the adapter; the engine provides `now` so tests can freeze it. |
| Schema-drift sentinel (ADP-03) | Adapter | Renderer (UI-SPEC literal) | The adapter detects drift (its parsing decisions are what fail); the renderer just paints the locked sentinel string. |
| Per-adapter timeout | Engine (config) | — | D-29: provider trait stays simple; timeout policy lives next to the spawn site. |
| Per-adapter error isolation | Engine (`JoinSet` + `JoinError::is_panic()`) | Renderer (per-row `ERROR:` line) | Engine swallows panics into `ProviderError::Internal`; renderer paints one error row per failed provider. |
| Refresh tick (15 s) | Engine background task | TUI render loop subscribes | D-30: one source of truth for ticks; both front-ends consume identically. |
| Render tick (1 s for countdown redraw) | TUI front-end | — | D-31: redraw cadence is a UI concern only; CLI emits once and exits. |
| Terminal raw mode / altscreen lifecycle | TUI front-end (`ratatui::run`) | Engine (untouched) | TUI owns its surface; engine is UI-agnostic. |
| Panic-safe terminal restore | `ratatui::run` (auto-installed hook) composed over `install_phase0_panic_hook` (`src/main.rs`) | — | Stack-order: ratatui's hook runs first (restores terminal), then Phase 0's stderr-print, then default. Confirmed by Phase 0 D-27 comment. |
| TTY detection | CLI renderer (color decision) + TUI entrypoint (refuse-non-TTY) | — | `std::io::IsTerminal::is_terminal(&io::stdout())` at each entry point — both surfaces decide independently per UI-SPEC. |
| Config load + cross-OS path resolution | `src/config.rs` (new) | `main.rs` (single call) | `directories::ProjectDirs` evaluated once at startup; result passed by value. |
| Secret storage / retrieval | `src/secrets.rs` (Phase 0 stub → Phase 1 real) | Adapter via `FetchCtx::secrets` | Engine wires `&Secrets` once; adapters never know which backend (keyring vs file) is active. |
| Mock provider gating | `src/main.rs` (config-only flag) | — | Phase 0's `MockProvider` is the panic-injection fixture in Phase 1; user-facing builds must not invoke it unless explicitly enabled. |
| TUI input handling (`q` / Ctrl-C / resize) | TUI event loop | — | `ratatui::crossterm::event::EventStream` inside the `tokio::select!`; no shared mutex with engine (channels only). |

---

## Standard Stack

### Core (Phase 1 adds these on top of Phase 0's 9 deps)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| **ratatui** | 0.30.0 | TUI rendering (`AHB tui`) | Sole maintained TUI library; `tui-rs` archived 2023. Provides `ratatui::run()` which installs the panic-safe terminal restore hook automatically. [VERIFIED: crates.io 2026-05-23] |
| **tokio** (feature upgrade) | 1.52.x | Async runtime + `JoinSet` + `time::interval` + `signal` | Already in Phase 0 with `["rt", "macros"]`; Phase 1 adds `rt-multi-thread`, `fs`, `time`, `signal`, `sync`. Phase 3 will add nothing new. [CITED: STACK.md] |
| **keyring-core** | 1.0.0 | Secret abstraction over OS-native stores | The `keyring` crate v4.x is a demo binary, not a library; `keyring-core` is the new library home. Verified on crates.io. [VERIFIED: crates.io 2026-05-23] |
| **dbus-secret-service-keyring-store** OR **apple-native-keyring-store** OR **windows-native-keyring-store** | 1.0.0 each | Platform-specific credential store registered via `set_default_store()` | keyring-core ships only mock + sample-file backends. Real OS storage requires one of the companion crates per target OS — verified on crates.io. **See Concern §6 for `cfg`-guarded selection pattern.** [VERIFIED: crates.io 2026-05-23] |
| **zeroize** | 1.8.2 | `Drop`-time memory zero for `Secret<T>` | Canonical Rust crate for the pattern; no transitive deps; well-trusted. [VERIFIED: crates.io 2026-05-23] |
| **glob** | 0.3.3 | `glob("~/.claude/projects/**/*.jsonl")` discovery | Direct REQ ADP-02 + PROJECT.md wording; tiny, no transitive deps. [VERIFIED: crates.io 2026-05-23] |
| **directories** | 6.0.0 | Cross-OS config path resolution | STACK.md says 5.x but 6.0 is current; semver-different from 5 (verify before committing). Provides `ProjectDirs::from(...).config_dir()`. [VERIFIED: crates.io 2026-05-23] |
| **toml** | 0.8.x | Parse `~/.config/ahb/config.toml` | Bare-serde approach (no figment) per D-36. [CITED: STACK.md] |
| **tracing** + **tracing-subscriber** | 0.1.41 / 0.3.x | Structured logging with `RUST_LOG` compat | Phase 1 is where logs start mattering (parse failures, schema drift warns, keyring backend selection). Cleanly evolves to the daemon mode mentioned in Constraints. [CITED: STACK.md] |
| **regex** (dev only) | 1.12.x | Secret-leak grep test (D-43 high-entropy assertion) | Tiny test-time addition; already de-facto Rust standard. [VERIFIED: crates.io 2026-05-23] |

### Supporting (dev / test only)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **insta** | 1.47.2 | Snapshot tests of `compact_line` output + ratatui `TestBackend` frames | Use immediately for the compact-line variations (Claude OK / Claude SchemaDrift / Claude error). Ratatui frame snapshots require `ratatui::backend::TestBackend`. [VERIFIED: crates.io 2026-05-23] |
| **tempfile** | 3.27.x | Mock `~/.claude/projects/` layout for adapter tests | Build the directory structure under a `tempfile::tempdir()` and pass `base_path` into a test-only Claude adapter constructor. [VERIFIED: crates.io 2026-05-23] |
| **assert_cmd** + **predicates** | 2.2.x / 3.1.x | End-to-end CLI integration tests (panic-injection, non-TTY refusal, color-off) | The panic-injection test in TUI-04 success criteria is naturally an `assert_cmd` test that asserts (a) non-zero exit, (b) stdout does not contain raw-mode escape, (c) stderr does contain Phase-0 `ahb panicked: …` prefix. [VERIFIED: crates.io 2026-05-23] |

### Alternatives Considered (and rejected)

| Instead of | Could Use | Tradeoff (Phase 1 verdict) |
|------------|-----------|----------------------------|
| `ratatui::run()` | Manual `ratatui::init()` + `restore()` with hand-installed panic hook | `init/restore` does NOT install a panic hook; manual chaining is fragile. `ratatui::run()` documents "handles terminal initialization, restoration, **and panic hooks automatically**." Use `run()`. [CITED: docs.rs/ratatui/0.30.0] |
| `tokio::task::JoinSet` | `futures::future::join_all` + per-future `tokio::time::timeout` | D-28 already chose JoinSet because abort-on-next-tick matters. `join_all` cannot abort the previous round's slow tasks. |
| `keyring-core` + companion store crate per OS | `keyring` 4.x | `keyring` 4.x README explicitly says "do NOT depend on this crate for a library." Hard pin to `keyring-core`. |
| Bare `serde` + `toml` | `figment` | D-36 picked bare; figment buys env-var layering we don't need until CFG-03 (Phase 3). |
| `Secret<T>` hand-built (D-42) | `secrecy` crate | D-42 explicit: 30 lines of code; no new dep beyond `zeroize`; full control of Serialize behavior. |
| `dbus-secret-service-keyring-store` (Linux Secret Service via dbus) | `zbus-secret-service-keyring-store` (zbus variant) OR `linux-keyutils-keyring-store` (kernel keyring) | **MEDIUM-confidence choice — see Concern §6.** dbus is the most-deployed Linux secret store (libsecret); zbus is async-native and slightly more modern. Keyutils stores in kernel memory (lost on reboot). Default to dbus for max coverage; document the alternatives. |

### Installation (Phase 1 additions over Phase 0's Cargo.toml)

```bash
# Production
cargo add ratatui                  # 0.30.0 — TUI; pulls crossterm 0.29 transitively
cargo add tokio --features rt-multi-thread,macros,fs,time,signal,sync  # feature upgrade
cargo add keyring-core             # 1.0.0
cargo add zeroize                  # 1.8.2
cargo add glob                     # 0.3.3
cargo add directories              # 6.0.0
cargo add toml                     # 0.8.x
cargo add tracing tracing-subscriber --features tracing-subscriber/env-filter

# Platform-conditional keyring store backends — pick ONE per target OS.
# Recommended: dbus-secret-service for Linux, apple-native for macOS, windows-native for Windows.
# These go behind cfg attributes in Cargo.toml (see Concern §6 Code Examples).

# Dev / test
cargo add --dev insta --features yaml
cargo add --dev tempfile
cargo add --dev assert_cmd predicates
cargo add --dev regex             # for D-43 high-entropy leak test
```

**Total new prod deps:** 8 crates (ratatui, keyring-core, zeroize, glob, directories, toml, tracing, tracing-subscriber) + 1 platform-conditional keyring store backend per supported OS. Phase 0 was 9 prod + 1 dev; Phase 1 ends around 17 prod + 5 dev. Document each new dep with a Cargo.toml comment naming the originating phase (Phase 0 convention).

### Version verification

Verified via `cargo search` on 2026-05-23:

| Package | Latest | Notes |
|---------|--------|-------|
| ratatui | 0.30.0 | Matches STACK.md HIGH-confidence pick. |
| keyring-core | 1.0.0 | Matches STACK.md. |
| dbus-secret-service-keyring-store | 1.0.0 | Released alongside keyring-core 1.0. Linux Secret Service via dbus. |
| zbus-secret-service-keyring-store | 1.0.0 | Async-native Linux variant. |
| linux-keyutils-keyring-store | 1.0.0 | Kernel keyring (volatile). |
| apple-native-keyring-store | 1.0.0 | macOS Keychain. |
| windows-native-keyring-store | 1.0.0 | Windows Credential Manager. |
| zeroize | 1.8.2 | |
| glob | 0.3.3 | |
| directories | 6.0.0 | **STACK.md said 5.x — 6.0 is current; verify breaking-change scope before locking.** |
| tempfile | 3.27.0 | |
| insta | 1.47.2 | |
| owo-colors | 4.3.0 | Already in Phase 0 Cargo.toml. |
| wiremock | 0.6.5 | Phase 3 — not Phase 1. |
| assert_cmd | 2.2.2 | |
| predicates | 3.1.4 | |
| regex | 1.12.3 | |

## Package Legitimacy Audit

> **slopcheck unavailable at research time** (no `pip` on the research host). Per the package legitimacy gate protocol, all packages below are flagged for human verification before install. However, every package below was discovered from an authoritative source (CONTEXT.md decision lock-ins or STACK.md research synthesis, which itself cites GitHub release API), AND each was independently confirmed to exist on crates.io with a current version (2026-05-23). Where a package came in via WebSearch only, it is flagged `[ASSUMED]`.

| Package | Registry | Age | Source Repo | slopcheck | Discovery Source | Disposition |
|---------|----------|-----|-------------|-----------|------------------|-------------|
| ratatui 0.30.0 | crates.io | active (0.30 released 2025-12-26) | github.com/ratatui/ratatui (20k stars) | unavailable | STACK.md (verified via GH release API) | Approved |
| tokio 1.52 | crates.io | core ecosystem | github.com/tokio-rs/tokio | unavailable | Phase 0 dep | Approved |
| keyring-core 1.0.0 | crates.io | released 2026-04-21 | github.com/open-source-cooperative/keyring-core | unavailable | STACK.md | Approved |
| dbus-secret-service-keyring-store 1.0.0 | crates.io | released alongside keyring-core 1.0 | open-source-cooperative org | unavailable | WebSearch + cargo search confirm | [ASSUMED — companion crate, verify org/repo provenance before lock-in] |
| zbus-secret-service-keyring-store 1.0.0 | crates.io | same | open-source-cooperative org | unavailable | cargo search | [ASSUMED — same caveat] |
| linux-keyutils-keyring-store 1.0.0 | crates.io | same | open-source-cooperative org | unavailable | cargo search | [ASSUMED — same caveat] |
| apple-native-keyring-store 1.0.0 | crates.io | same | open-source-cooperative org | unavailable | cargo search | [ASSUMED — same caveat] |
| windows-native-keyring-store 1.0.0 | crates.io | same | open-source-cooperative org | unavailable | cargo search | [ASSUMED — same caveat] |
| zeroize 1.8.2 | crates.io | core ecosystem (RustCrypto org) | github.com/RustCrypto/utils | unavailable | STACK.md / Concern §6 | Approved |
| glob 0.3.3 | crates.io | rust-lang org (canonical) | github.com/rust-lang/glob | unavailable | CONTEXT D-32 | Approved |
| directories 6.0.0 | crates.io | well-known | github.com/soc/directories-rs | unavailable | STACK.md (note version bump 5→6) | Approved (verify 5→6 semver delta) |
| toml 0.8 | crates.io | toml-rs org | github.com/toml-rs/toml | unavailable | STACK.md | Approved |
| tracing / tracing-subscriber 0.1 / 0.3 | crates.io | tokio-rs org | github.com/tokio-rs/tracing | unavailable | STACK.md | Approved |
| insta 1.47.2 | crates.io | mitsuhiko (well-known) | github.com/mitsuhiko/insta | unavailable | STACK.md | Approved |
| tempfile 3.27.0 | crates.io | rust-lang-nursery | github.com/Stebalien/tempfile | unavailable | STACK.md | Approved |
| assert_cmd / predicates 2.2 / 3.1 | crates.io | clap maintainers | github.com/assert-rs | unavailable | STACK.md | Approved |
| regex 1.12.3 | crates.io | rust-lang org (canonical) | github.com/rust-lang/regex | unavailable | D-43 grep test | Approved |

**Packages removed due to slopcheck [SLOP] verdict:** none (slopcheck unavailable).
**Packages flagged as suspicious [SUS]:** none from heuristic checks. **All 5 keyring backend connector crates flagged `[ASSUMED]`** because they came in via WebSearch + cargo search but I could not retrieve the official keyring-core wiki page that authoritatively lists them. The planner must verify each backend crate's `repository` field on crates.io points to a `github.com/open-source-cooperative/*` repo (matching keyring-core's org) before adding to Cargo.toml — this is a `checkpoint:human-verify` gate.

---

## Architecture Patterns

### System Architecture Diagram

```
                        ┌──────────────────────────────────────────┐
                        │  src/main.rs                              │
                        │  • install_phase0_panic_hook() — line 1   │
                        │  • Cli::parse() — clap                    │
                        │  • config::load() ← directories+toml+glob │
                        │  • secrets::init() ← keyring-core         │
                        │      └── set_default_store(<OS-specific>) │
                        │  • Engine::new(config, secrets)           │
                        │  • dispatch:                              │
                        │      cli::run(&engine).await   ─┐         │
                        │      tui::run(engine).await     │         │
                        └────────────────────────────┬────┘         │
                                                     │              │
                  ┌──────────────────────────────────┼──────────────┘
                  │                                  │
                  ▼                                  ▼
        ┌──────────────────┐               ┌─────────────────────────┐
        │ cli::run         │               │ tui::run                │
        │ (sync-style)     │               │ ratatui::run(closure)   │
        │                  │               │   ↳ installs panic hook │
        │ refresh_all()    │               │   ↳ enters altscreen    │
        │ → render_text    │               │                         │
        │ → println!       │               │ tokio::select! over:    │
        │ → exit(0/1/2)    │               │   • event::EventStream  │
        └────────┬─────────┘               │   • interval(15s) → fetch│
                 │                         │   • interval(1s)  → draw│
                 │                         │   • mpsc<EngineEvent>   │
                 │                         └───────────┬─────────────┘
                 │                                     │
                 └──────────────┬──────────────────────┘
                                ▼
                  ┌─────────────────────────────────┐
                  │  Engine                          │
                  │   • refresh_all() → JoinSet      │
                  │     spawn one task per provider │
                  │     wrap in timeout(2s)         │
                  │     drain via join_next()       │
                  │     JoinError::is_panic() →     │
                  │       ProviderError::Internal   │
                  │   • background_loop()           │
                  │     emits EngineEvent::Refresh  │
                  └─────────────┬───────────────────┘
                                │
            ┌───────────────────┴───────────────────┐
            ▼                                       ▼
   ┌──────────────────────┐              ┌──────────────────────┐
   │ ClaudeProvider       │              │ MockProvider         │
   │ (provider/claude.rs) │              │ (provider/mock.rs)   │
   │                      │              │ • Phase 1: gated by  │
   │ • glob ~/.claude/    │              │   config flag        │
   │   projects/**/*.jsonl│              │ • used by tests +    │
   │ • BufReader.lines    │              │   panic-injection    │
   │ • parse usage fields │              └──────────────────────┘
   │ • 5h cluster anchor  │
   │ • schema-drift sniff │
   │ • ctx.now (NEVER     │
   │   Timestamp::now)    │
   └──────────────────────┘

   File system: ~/.claude/projects/<slug>/<uuid>.jsonl
                (read-only, streaming, tolerate truncated trailing line)

   OS keyring (Phase 1 wired but not exercised for Claude):
     • Linux:   dbus-secret-service-keyring-store (libsecret) — recommended
                   alt: zbus-…, linux-keyutils-…
     • macOS:   apple-native-keyring-store (Keychain)
     • Windows: windows-native-keyring-store (Credential Manager)
```

**Data flow narrative (CLI mode, the Phase 1 demo path):**
1. `main.rs` installs panic hook (line 1), parses args, loads config (creates default + exits if missing per D-37), initializes keyring (hard-error per D-41 if missing), constructs `Engine`.
2. CLI dispatch calls `engine.refresh_all().await` once.
3. Engine builds a `JoinSet`, spawns one `tokio::task` per enabled provider, each wrapped in `timeout(2s)`.
4. Engine drains the JoinSet via `join_next()`, converting `JoinError::is_panic()` into `ProviderError::Internal` (D-28 / ADP-01).
5. Result vector `Vec<(ProviderId, Result<ProviderState, ProviderError>)>` flows to `cli::render_text::render_all` (Phase 1 extension of `compact_line` from one row to N).
6. Each row printed to stdout. Exit code: per CORE-06 (Phase 2 wires properly — Phase 1 may leave as 0 unless every provider failed).

**Data flow narrative (TUI mode):**
1. Same `main.rs` prefix.
2. `tui::run` calls `std::io::IsTerminal::is_terminal(&io::stdout())` — false → print UI-SPEC literal `AHB tui requires a terminal …` to stderr, exit(2).
3. Otherwise `ratatui::run(|terminal| async move { ... })` — automatically installs panic hook that calls `disable_raw_mode` + `LeaveAlternateScreen` BEFORE running the previous hook (which prints `ahb panicked: …` from `install_phase0_panic_hook`).
4. Inside the closure, build `AppState`, spawn the engine background task (which `mpsc::send`s `EngineEvent::Refresh(Vec<...>)` every 15 s), enter the `select!` loop over (input, fetch-tick, render-tick, engine-mpsc).
5. `q` or Ctrl-C → break loop → closure returns → `ratatui::run` restores terminal.

### Recommended Project Structure (Phase 1 additions to Phase 0 layout)

```
src/
├── main.rs              # Phase 0 — extend with command dispatch (default/tui)
├── lib.rs               # Phase 0 — add `engine`, `config`, `tui` module exports
├── model.rs             # Phase 0 LOCKED — do not modify
├── secrets.rs           # Phase 0 stub → Phase 1: real keyring-core wiring + Secret<T>
├── config.rs            # NEW — TOML schema + load + first-run init + ProjectDirs
├── engine/
│   ├── mod.rs           # NEW — Engine struct + refresh_all + background_loop
│   ├── fanout.rs        # NEW — JoinSet spawn pattern + JoinError handling
│   └── events.rs        # NEW — EngineEvent enum + mpsc plumbing
├── provider/
│   ├── mod.rs           # Phase 0 LOCKED — Provider trait + FetchCtx
│   ├── mock.rs          # Phase 0 — gate behind config flag in Phase 1
│   └── claude/
│       ├── mod.rs       # NEW — ClaudeProvider struct + impl Provider
│       ├── jsonl.rs     # NEW — file glob + line-stream parser + drift sniff
│       └── window.rs    # NEW — 5h cluster anchor + token sum
├── cli/
│   ├── mod.rs           # Phase 0 — add render_all extension
│   ├── render_text.rs   # Phase 0 LOCKED format string — Phase 1 multi-row
│   └── tty.rs           # NEW — IsTerminal helper + NO_COLOR / --color resolution
├── tui/
│   ├── mod.rs           # NEW — pub async fn run(engine) -> Result<()>
│   ├── app.rs           # NEW — AppState struct (per-provider cached state)
│   ├── ui.rs            # NEW — ratatui Frame composition per UI-SPEC layout
│   └── widgets/
│       └── hp_row.rs    # NEW — one HpRow Widget impl (label + bar + countdown)
└── templates/
    └── default-config.toml  # NEW — include_str! template for first-run init
```

**Layout justification:** Single crate (no workspace) — STACK.md / ARCHITECTURE.md already decided. New `engine/` and `tui/` directories follow the codex-rs pattern. `provider/claude/` becomes a directory (not single file) because it has 3 cleanly separable concerns (file discovery + line parsing + cluster math) and CONTEXT D-32..D-35 lock decisions across all three. Phase 2's `provider/codex/` will mirror this shape.

### Pattern 1: Engine fan-out via `JoinSet` (D-28)

**What:** Each adapter runs in its own spawned `tokio::task`; results drained via `JoinSet::join_next()` until empty. A new tick can abort the previous tick's outstanding tasks.

**When to use:** Always for Phase 1+ multi-adapter refresh. Phase 0 has only one adapter so this is the first time it matters.

**Example:**
```rust
// src/engine/fanout.rs
use tokio::task::JoinSet;
use tokio::time::{timeout, Duration};

use crate::model::{ProviderError, ProviderId, ProviderState};
use crate::provider::{FetchCtx, Provider};

pub async fn refresh_all(
    providers: &[std::sync::Arc<dyn Provider>],
    ctx: &FetchCtx<'_>,
    per_provider_timeout: Duration,
) -> Vec<(ProviderId, Result<ProviderState, ProviderError>)> {
    let mut set = JoinSet::new();
    for p in providers {
        let p = p.clone();
        let ctx_owned = OwnedFetchCtx::from(ctx);  // FetchCtx<'_> → 'static for spawn
        set.spawn(async move {
            let id = p.id();
            let result = match timeout(per_provider_timeout, p.fetch(&ctx_owned.borrow())).await {
                Ok(Ok(state)) => Ok(state),
                Ok(Err(e)) => Err(e),
                Err(_elapsed) => Err(ProviderError::Unavailable {
                    reason: format!("timed out after {:?}", per_provider_timeout),
                }),
            };
            (id, result)
        });
    }

    let mut out = Vec::with_capacity(providers.len());
    while let Some(join_result) = set.join_next().await {
        match join_result {
            Ok(pair) => out.push(pair),
            Err(je) if je.is_panic() => {
                // ADP-01: panic in adapter must become an error row, not a tool crash.
                // PROBLEM: we don't know which ProviderId panicked from JoinError alone.
                // FIX: tag the task on spawn — see planner note in Pitfall L4.
                tracing::error!("adapter task panicked: {je:?}");
            }
            Err(je) => tracing::error!("adapter task cancelled: {je:?}"),
        }
    }
    out
}
```
[CITED: ARCHITECTURE.md Pattern 3 + CONTEXT D-28 + L4 caveat below]

> **Planner action item:** Either (a) use `JoinSet::spawn_with_id` (if available in tokio 1.52 — verify) to keep the `ProviderId` alongside the task handle, OR (b) make every adapter task always return `(ProviderId, Result<...>)` and convert a `JoinError::is_panic()` into a synthetic `(ProviderId::Unknown, Err(...))` via a different bookkeeping mechanism (e.g. a `HashMap<TaskId, ProviderId>` built on spawn).

### Pattern 2: TUI lifecycle via `ratatui::run` (TUI-04 binding)

**What:** Use `ratatui::run(closure)` — NOT manual `init()` + `restore()`. Per docs.rs/ratatui/0.30.0, `run` "handles terminal initialization, restoration, AND panic hooks automatically." `init/restore` does NOT install a panic hook. The Phase 0 `install_phase0_panic_hook` is preserved and runs second in the chain (after terminal restoration but before the default handler).

**When to use:** Every Phase 1+ TUI entrypoint.

**Example:**
```rust
// src/tui/mod.rs
use std::io::IsTerminal;
use ratatui::DefaultTerminal;
use tokio::time::{interval, Duration};

use crate::engine::Engine;

pub async fn run(engine: Engine) -> anyhow::Result<()> {
    // TUI-05: refuse non-TTY with UI-SPEC literal.
    if !std::io::stdout().is_terminal() {
        eprintln!(
            "AHB tui requires a terminal (stdout is not a TTY). \
             Run AHB without 'tui' for piped / non-interactive output."
        );
        std::process::exit(2);
    }

    // ratatui::run installs its panic hook on top of Phase 0's existing hook.
    // Chain order on panic: ratatui's hook (restore terminal) → Phase 0's hook
    // (eprintln!("ahb panicked: ...")) → default hook (backtrace if RUST_BACKTRACE).
    ratatui::run(|terminal: &mut DefaultTerminal| async move {
        let mut app = crate::tui::app::AppState::new(&engine);
        let mut events = ratatui::crossterm::event::EventStream::new();
        let mut fetch_tick = interval(Duration::from_secs(15));
        let mut render_tick = interval(Duration::from_secs(1));
        // initial fetch so first frame isn't empty
        app.apply_results(engine.refresh_all().await);

        loop {
            tokio::select! {
                Some(Ok(ev)) = futures_util::StreamExt::next(&mut events) => {
                    if app.handle_event(ev) { break; }
                }
                _ = fetch_tick.tick() => {
                    app.apply_results(engine.refresh_all().await);
                }
                _ = render_tick.tick() => {
                    terminal.draw(|f| crate::tui::ui::draw(f, &app))?;
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    }).await?;
    Ok(())
}
```
[CITED: docs.rs/ratatui/0.30.0 main page + Pattern 6 in ARCHITECTURE.md]

> **Verification:** Confirm `ratatui::run` signature accepts an async closure — docs page describes it as "the simplest way to initialize AND run a terminal application." Planner should `mcp__context7__get-library-docs` for `/ratatui/ratatui` topic `run` before locking the exact closure shape.

### Pattern 3: Composable panic hook chain (Phase 0 + ratatui + default)

**What:** Phase 0 installed a panic hook via `take_hook` + `set_hook` (preserves the previous hook by calling `original(info)` after `eprintln!`). When Phase 1 enters TUI mode, `ratatui::run` does the same take-and-wrap on top — its hook restores the terminal, then calls the previously-installed hook (Phase 0's), which calls the original (default backtrace) hook. Chain reads cleanly: restore → label-as-AHB → default.

**Why this matters:** The CONTEXT specifies `install_phase0_panic_hook` MUST be the FIRST line of `main()` — this is contractual. Phase 1's TUI code MUST NOT replace it; only wrap it (via ratatui's `run`).

**Anti-pattern to avoid:** Calling `std::panic::set_hook(Box::new(|_| {}))` to silence panics inside the TUI loop. This kills the chain and leaves the terminal scrambled.

### Pattern 4: TTY-aware color decision (CORE-05)

**What:** At CLI entry, decide once whether to emit ANSI bytes. The decision tree, in priority order:

1. `--json` is set → always uncolored.
2. `--color=never` → uncolored.
3. `--color=always` → colored regardless of TTY.
4. `NO_COLOR` env var set (any value) → uncolored.
5. `std::io::stdout().is_terminal() == false` → uncolored.
6. Default (`--color=auto`, env unset, TTY) → colored.

**When to use:** Exactly once at CLI startup; pass a `ColorMode` enum (or just a bool) into the renderer. Phase 0 already accepts `--color` as a `clap::ValueEnum`; Phase 1 wires it.

```rust
// src/cli/tty.rs
use std::io::IsTerminal;

pub fn should_colorize(cli_flag: ColorMode, json_mode: bool) -> bool {
    if json_mode { return false; }
    match cli_flag {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            std::env::var_os("NO_COLOR").is_none()
                && std::io::stdout().is_terminal()
        }
    }
}
```
[VERIFIED: IsTerminal stable since Rust 1.70; current MSRV is 1.88]

### Pattern 5: `keyring-core` 1.0 bootstrap (D-40 + D-41)

**What:** At startup, register an OS-appropriate credential store via `keyring_core::set_default_store(store)`. After that, every `Entry::new("ahb", "<provider-id>")` call resolves through the registered store. If registration fails on the target OS, exit code 2 with the D-41 literal stderr message.

**Why this is non-trivial in 1.0:** `keyring-core` no longer auto-selects a backend via Cargo features (that was the `keyring` 4.x pattern). The application now owns the choice. Backends are separate companion crates, each providing a `Store` type to feed `set_default_store`.

**When to use:** Once, in `main.rs` (or `secrets.rs` called from `main.rs`).

```rust
// src/secrets.rs (Phase 1 sketch — exact backend crate name pending verification)
use keyring_core::api::CredentialStore;

#[cfg(target_os = "linux")]
fn make_default_store() -> Result<Box<dyn CredentialStore>, anyhow::Error> {
    // dbus-secret-service-keyring-store 1.0.0 — Linux Secret Service via dbus
    // (Alternative: zbus-…, linux-keyutils-…)
    // Verify exact constructor signature at impl time.
    let store = dbus_secret_service_keyring_store::Store::new("ahb")?;
    Ok(Box::new(store))
}

#[cfg(target_os = "macos")]
fn make_default_store() -> Result<Box<dyn CredentialStore>, anyhow::Error> {
    let store = apple_native_keyring_store::Store::new("ahb")?;
    Ok(Box::new(store))
}

#[cfg(target_os = "windows")]
fn make_default_store() -> Result<Box<dyn CredentialStore>, anyhow::Error> {
    let store = windows_native_keyring_store::Store::new("ahb")?;
    Ok(Box::new(store))
}

pub fn init() -> Result<Secrets, anyhow::Error> {
    let store = make_default_store().map_err(|e| {
        eprintln!(
            "no secret store available on this system; \
             set [secrets].storage = \"file\" in ~/.config/ahb/config.toml \
             to opt into 0600 file storage"
        );
        anyhow::anyhow!("keyring backend unavailable: {e}")
    })?;
    keyring_core::set_default_store(store);
    Ok(Secrets::new())
}
```
[CITED: keyring-core wiki + docs.rs Entry::new error semantics ("NoDefaultStore if no default store is configured")]

> **MEDIUM-confidence area:** The exact constructor signature for each `*-keyring-store` crate (e.g. is it `Store::new("ahb")`, `Store::default()`, or something else?) needs `mcp__context7__get-library-docs` lookup at plan time. Treat the snippet above as a structural sketch.

### Pattern 6: `Secret<T>` newtype (D-42)

```rust
// src/secrets.rs
use std::fmt;
use serde::{Serialize, Serializer};
use zeroize::Zeroize;

pub struct Secret<T: Zeroize + Clone>(T);

impl<T: Zeroize + Clone> Drop for Secret<T> {
    fn drop(&mut self) { self.0.zeroize(); }
}

impl<T: Zeroize + Clone> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

impl<T: Zeroize + Clone> Serialize for Secret<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("[REDACTED]")
    }
}
// Intentionally no Deserialize impl: secrets come from the keyring, not from TOML/JSON.

impl<T: Zeroize + Clone> Secret<T> {
    pub fn new(inner: T) -> Self { Self(inner) }
    /// The ONLY way to read the underlying value. Greppable for audits.
    pub fn expose(&self) -> &T { &self.0 }
}
```
[CITED: CONTEXT D-42 verbatim]

### Anti-Patterns to Avoid

- **`ratatui::init()` + `restore()` without `run()` wrapper.** Skips panic-hook install; one `unwrap()` panic anywhere in the loop wrecks the terminal. Always use `run`.
- **Adding `crossterm` directly to Cargo.toml.** Two crossterm versions silently break ratatui rendering. Use `ratatui::crossterm::*` re-exports exclusively.
- **`Arc<Mutex<AppState>>` shared between engine and TUI.** ARCHITECTURE.md Anti-Pattern 3. Channels only.
- **`unwrap()` / `expect()` / `panic!()` anywhere in adapter code or render loop.** `src/lib.rs` already denies all three at the crate root; new modules inherit. Don't `#[allow]` it.
- **Synchronous `std::fs` calls inside an async `Provider::fetch`.** Use `tokio::fs::read_to_string` or `tokio::task::spawn_blocking` for sync APIs. (Claude adapter is local-file-read with low ms cost — `tokio::fs` is correct.)
- **`Provider::fetch` reading `jiff::Timestamp::now()` directly.** Phase 0 acceptance grep already guards `mock.rs`; Phase 1 must extend the grep to `claude.rs`. Use `ctx.now`.
- **Calling `jiff::Timestamp::since(*now)` without `(Unit::Hour, *now)`.** Default unit is Second; you get `7200s` instead of `2h 0m 0s`. Phase 0-03 deviation 1.
- **Using `#[serde(deny_unknown_fields)]` on the Config struct.** D-38: warn + ignore. Forward-compatibility matters.
- **Falling back to file-storage when keyring is unavailable.** D-41: hard error with next-step hint. Phase 1 doesn't even implement the file backend.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cross-OS config path resolution | Hand-pattern matching on `cfg(target_os)` | `directories::ProjectDirs::from("", "", "ahb")` | Macos `~/Library/Application Support/...` and Windows `%APPDATA%\...` rules are non-obvious; directories handles XDG fallbacks and per-user cache dir for free. [CITED: CFG-02 + STACK.md] |
| Terminal raw mode + altscreen lifecycle | Manual `enable_raw_mode` + `LeaveAlternateScreen` calls | `ratatui::run(closure)` | Panic hook installation, signal handling for resize, and double-restore prevention are all there for free. [CITED: docs.rs/ratatui/0.30.0] |
| Per-task cancellation on next tick | `tokio::sync::Notify` + manual handle bookkeeping | `tokio::task::JoinSet::abort_all()` | JoinSet was designed for exactly this; one method call. [CITED: tokio docs] |
| TTY / pipe detection | `libc::isatty` direct FFI | `std::io::IsTerminal::is_terminal` | Stable since 1.70, in std, no deps. [VERIFIED] |
| Glob over `~/.claude/projects/**/*.jsonl` | `walkdir` + manual `.jsonl` filter | `glob` crate | Direct pattern from REQ ADP-02 text. Tiny crate (no transitive deps). [CITED: CONTEXT D-32] |
| Stream JSONL with trailing-line tolerance | `read_to_string` + `split('\n')` | `std::io::BufReader::new(File).lines()` + per-line `serde_json::from_str` + log+skip | Idiomatic, handles truncated files gracefully, doesn't pull large sessions into RAM. [CITED: CONTEXT D-35 + ARCHITECTURE.md] |
| OS keyring abstraction | Per-OS `cfg`-gated FFI bindings | `keyring-core` 1.0 + `*-keyring-store` companion crate | Three OS-specific APIs (Keychain Services, Credential Manager, libsecret/dbus) hidden behind one trait. [CITED: STACK.md + SEC-01] |
| Memory-zeroing secret wrapper | Hand `unsafe { std::ptr::write_volatile }` | `zeroize` crate | `zeroize::Zeroize` derive macro and `Drop` discipline get it right; canonical crate. [CITED: D-42] |
| 5h cluster math (gap detection + window sum) | Custom datetime arithmetic | `jiff::Timestamp::since((Unit::Hour, *now))` for spans + manual sort+walk for gaps | Cluster-anchor algorithm itself IS the custom logic — but anchor on `jiff` for time math because chrono is panic-prone and time-rs is overkill. [CITED: STACK.md + CONTEXT D-33] |
| Wall-clock injection for testing | `Box<dyn Fn() -> Timestamp>` clock trait | Pass `now: jiff::Timestamp` (Copy) via `FetchCtx` | Phase 0 ALREADY did this — `FetchCtx::now`. Phase 1 just keeps using it. [CITED: Phase 0 lock-in] |
| Per-adapter timeout enforcement | Manual `tokio::select! { _ = adapter, _ = sleep }` | `tokio::time::timeout(duration, future)` | One function call; returns `Result<T, Elapsed>` cleanly. [CITED: D-29] |

**Key insight:** Phase 1 is mostly **wiring existing tools together correctly** — the custom code that ships is small (5h cluster algorithm, schema-drift sniff, mock-injection-for-tests gating, error-row rendering). Anything outside those concerns should be a library call.

---

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | **None — verified by inspection.** Phase 1 reads `~/.claude/projects/**/*.jsonl` (read-only) and writes only to `~/.config/ahb/config.toml` on first run. AHB itself stores no persistent state in Phase 1 (no cache, no usage history). | none |
| Live service config | **None — verified.** Phase 1 doesn't manage external services. The OS keyring is touched only in Phase 1 wire-up tests; no real credentials stored under our service name yet. | none |
| OS-registered state | **None — verified.** No launchd plist, no systemd unit, no Windows scheduled task. (Phase 4 may add release-time signing artifacts; out of Phase 1 scope.) | none |
| Secrets / env vars | **Read but not written:** `NO_COLOR` (env). `AHB_DEBUG_PANIC` may be added per Claude's discretion (panic-injection test mechanism). `RUST_LOG` via `tracing-subscriber::EnvFilter`. | none — these are read-only env reads |
| Build artifacts | **None for Phase 1.** Phase 4 will produce release binaries via cargo-dist (out of scope). | none |

**This is a greenfield phase.** Runtime state inventory ran clean.

---

## Common Pitfalls

### Pitfall L1: CONTEXT D-33 token formula is upstream-broken (CRITICAL — blocks ADP-02)

**What goes wrong:** D-33 step 5 says "sum `message.usage.input_tokens + output_tokens`" across the cluster. Per [ccusage issue #866](https://github.com/ryoppippi/ccusage/issues/866) (the de-facto reference parser's own bug tracker): `input_tokens` is undercounted **100-174x** because ~75 % of entries are streaming placeholders (0 or 1) that never finalize. `output_tokens` is undercounted **10-17x** on Opus models because thinking tokens are omitted. Reliable fields are `cache_creation_input_tokens` (~0.9x ground truth) and `cache_read_input_tokens` (~1.1x ground truth).

**Why it happens:** Claude Code logs usage from early streaming events and never re-writes them when the streaming finalizes. Multiple upstream issues open ([anthropics/claude-code#24147](https://github.com/anthropics/claude-code/issues/24147), [#22686](https://github.com/anthropics/claude-code/issues/22686)); no fix landed as of 2026-05-23.

**How to avoid:** The planner must pick ONE of:
1. **Recommended:** Use `cache_creation_input_tokens` as the headline token-sum field (this is what gets billed against the 5 h budget; cache reads are essentially free). Add `output_tokens` only as supplement, explicitly documenting it under-counts. README note: "AHB's Claude % is a best-effort estimate; upstream JSONL data is partially incomplete — see issue #X."
2. Fallback: Match ccusage's exact formula (whatever it currently uses); brittle because ccusage may change.
3. Defer the question: ship Phase 1 with explicit "approximate" labeling on the bar and revisit in v2.

The real `CLAUDE_5H_TOKEN_LIMIT` constant must be derived against whichever field-set we choose. Suggested anchor: Pro plan is ~44k tokens/window per [tokenmix.ai blog 2026](https://tokenmix.ai/blog/complete-claude-limits-guide-2026-tokens-uploads-5-hour) — but Anthropic explicitly no longer publishes hard token numbers; this is a moving target.

**Warning signs:** Pre-launch sanity check: spend a known number of tokens in a fresh Claude Code session, then read AHB's % — if it's off by >10x, the formula is using the broken fields.

### Pitfall L2: ratatui `init()` + `restore()` does NOT install a panic hook (CRITICAL — blocks TUI-04)

**What goes wrong:** A natural read of older ratatui docs leads developers to `let mut t = ratatui::init(); ...; ratatui::restore();`. This skips the panic-hook installation — one `unwrap()` panic anywhere in the loop leaves raw mode on, altscreen active, cursor hidden, user's shell wrecked.

**Why it happens:** STACK.md and ARCHITECTURE.md sketches both reference `init` / `restore` as the entry pattern. The Pitfall 5 entry in PITFALLS.md says "Use `ratatui::init()` and `ratatui::restore()` — the init function installs a panic hook" — **this is incorrect for ratatui 0.30.** Verified via docs.rs/ratatui/0.30.0: only `run()` installs the panic hook.

**How to avoid:** Use `ratatui::run(|terminal| async move { ... })`. The TUI body lives inside the closure. ratatui handles init/restore/panic-hook chain in one call. See Pattern 2 above.

**Warning signs:** Phase 0's existing panic hook stays installed (good). If Phase 1's TUI panics during dev and the dev's terminal is fine afterward, `run` is wired correctly. If terminal is scrambled, the closure wrapping is wrong.

### Pitfall L3: `keyring-core` 1.0 fails with `NoDefaultStore` if `set_default_store` isn't called

**What goes wrong:** `Entry::new("ahb", "claude")` returns `Err(Error::NoDefaultStore)` if `set_default_store` hasn't run yet. The error is silent unless explicitly handled — adapters could see `secrets.get("foo")?` as "no credential" and skip silently.

**Why it happens:** keyring-core 1.0 removed the auto-backend-selection that `keyring` 4.x had. Application code OWNS the choice.

**How to avoid:** Make `secrets::init()` infallible-or-exit at `main.rs` startup (D-41 binding). If `set_default_store` cannot register a backend on this OS, exit(2) with the literal D-41 message. Never let the app proceed with an uninitialized keyring.

**Warning signs:** Unit test that calls `Entry::new` BEFORE `set_default_store` should reproduce the `NoDefaultStore` error — make this a regression test so future changes don't reorder init.

### Pitfall L4: `JoinSet::join_next()` loses `ProviderId` when a task panics

**What goes wrong:** If the spawned task panics, `JoinSet::join_next` returns `Err(JoinError)`. The error tells you it was a panic but NOT which provider's task it was. ADP-01's "render an error row for the panicked adapter" requires the ID.

**Why it happens:** `JoinSet::spawn` returns an opaque `AbortHandle`; the task's logical identity is whatever you put in the closure return value — which never happens on panic.

**How to avoid:** Two options:
1. Use `JoinSet::spawn_with_id` if tokio 1.52 has it. (Verify at plan time.)
2. Maintain a `HashMap<tokio::task::Id, ProviderId>` populated at spawn time, looked up on `JoinError`. The `tokio::task::Id` IS available via `JoinError::id()`.

**Warning signs:** Panic-injection integration test (TUI-04 success criterion) must assert the rendered output contains both `ERROR:` AND the panicked provider's name. If it shows `unknown ERROR:` or no row at all, this pitfall struck.

### Pitfall L5: `directories` crate 5 → 6 may have breaking changes

**What goes wrong:** STACK.md locks "directories 5.x" but the current crates.io latest is 6.0. The Phase 1 planner copies STACK.md's `cargo add directories@5` and silently locks an old version with different APIs.

**Why it happens:** STACK.md was researched 2026-05-22; directories 6.0 was released after.

**How to avoid:** Use `cargo add directories` (no version pin) to get 6.0, then verify the `ProjectDirs::from("", "", "ahb")` signature still works (the qualifier/organization/application three-arg form is the canonical entry point; unlikely to have changed). If 6.0 has unrelated breaking changes that hurt, downgrade to 5.x — but bias toward the current version.

**Warning signs:** Build error on `ProjectDirs::from` call shape. Fix forward.

### Pitfall L6: Phase 0's `MockProvider` panic injection requires explicit gating

**What goes wrong:** The TUI-04 success criterion requires an integration test that injects a panic into an adapter. The natural way is to extend `MockProvider` with a "panic on Nth call" variant. If that variant is reachable in production builds, a malicious config could crash the user's tool.

**Why it happens:** `MockProvider` is currently `pub` and not `#[cfg(test)]`.

**How to avoid:** Either (a) gate the panic variant behind `#[cfg(debug_assertions)]` so release builds can't trigger it, or (b) gate behind an `AHB_DEBUG_PANIC` env var that's read once at startup and ignored otherwise. CONTEXT defers the choice to the planner; recommend the env-var approach because it lets the panic-injection integration test work on a release build too.

### Pitfall L7: `tracing-subscriber` initialization races the panic hook

**What goes wrong:** If `tracing_subscriber::fmt::init()` runs AFTER `install_phase0_panic_hook`, any `tracing::error!` inside the panic hook (e.g. logging the panic site) emits to a fresh stderr that may swallow the panic message.

**Why it happens:** Most examples show `tracing_subscriber::fmt::init()` as the first line of main; Phase 0's contract says `install_phase0_panic_hook()` is the first line.

**How to avoid:** Install the panic hook first (Phase 0 contract is binding), then initialize tracing. The panic hook uses plain `eprintln!`, not `tracing`, so the order is decoupled. Verify in a smoke test.

### Pitfall L8 (carried from PITFALLS.md): `glob` follows symlinks by default

**What goes wrong:** `glob("~/.claude/projects/**/*.jsonl")` on a user with a symlinked `~/.claude` (e.g. dotfile sync) may traverse outside the expected tree, or worse loop if there's a symlink cycle. Phase 1's plan must decide.

**How to avoid:** Use `glob::glob_with(pattern, MatchOptions { case_sensitive: true, require_literal_separator: false, require_literal_leading_dot: false })` and consider explicitly stating symlink behavior. Default `glob::glob` does follow symlinks. Document the decision in the adapter's module doc.

---

## Code Examples

### Example 1: Claude adapter cluster anchor (D-33, with L1 caveat)

```rust
// src/provider/claude/window.rs
use jiff::{Timestamp, Span, Unit};
use crate::provider::claude::jsonl::AssistantEntry;

const FIVE_HOURS: Span = Span::new().hours(5);
// L1 NOTE: this constant must be chosen carefully; see L1 for the field-choice debate.
const CLAUDE_5H_TOKEN_LIMIT: u64 = 44_000;  // Pro plan estimate per tokenmix.ai 2026

pub struct Cluster {
    pub session_start: Timestamp,
    pub reset_at: Timestamp,
    pub used_tokens: u64,
}

pub fn find_active_cluster(
    sorted_msgs: &[AssistantEntry],
    now: Timestamp,
) -> Option<Cluster> {
    if sorted_msgs.is_empty() { return None; }
    // Walk newest → oldest, find first gap > 5h
    let mut cluster_start_idx = 0;
    for i in (1..sorted_msgs.len()).rev() {
        let gap = sorted_msgs[i].timestamp
            .since((Unit::Hour, sorted_msgs[i - 1].timestamp))
            .unwrap_or_else(|_| Span::new());
        // L0 reminder: pass (Unit::Hour, *now) explicitly per Phase 0-03 deviation 1
        if gap.get_hours() >= 5 {
            cluster_start_idx = i;
            break;
        }
    }
    let cluster = &sorted_msgs[cluster_start_idx..];
    let session_start = cluster.first()?.timestamp;
    let reset_at = session_start.checked_add(FIVE_HOURS).ok()?;
    // L1 NOTE: choose cache_creation_input_tokens, NOT input_tokens + output_tokens.
    let used: u64 = cluster.iter()
        .map(|m| m.cache_creation_input_tokens)
        .sum();
    Some(Cluster { session_start, reset_at, used_tokens: used })
}

pub fn percent_remaining(used: u64, limit: u64) -> f32 {
    if limit == 0 { return 0.0; }
    let remaining = limit.saturating_sub(used) as f32;
    (remaining / limit as f32 * 100.0).clamp(0.0, 100.0)
}
```
[SOURCE: CONTEXT D-33, with L1-recommended field choice]

### Example 2: Streaming JSONL parser with trailing-line tolerance (D-35)

```rust
// src/provider/claude/jsonl.rs
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum JsonlEntry {
    #[serde(rename = "assistant")]
    Assistant(AssistantEntry),
    #[serde(other)]
    Other,  // file-history-snapshot, user, etc — ignored for token sum
}

#[derive(Deserialize)]
pub struct AssistantEntry {
    pub timestamp: jiff::Timestamp,
    pub message: ClaudeMessage,
}

#[derive(Deserialize)]
pub struct ClaudeMessage {
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
pub struct Usage {
    #[serde(default)] pub input_tokens: u64,           // L1: unreliable, do not sum
    #[serde(default)] pub output_tokens: u64,           // L1: unreliable, do not sum
    #[serde(default)] pub cache_creation_input_tokens: u64,  // L1: reliable, sum this
    #[serde(default)] pub cache_read_input_tokens: u64,      // L1: reliable
}

pub fn read_assistant_entries(path: &Path) -> Vec<AssistantEntry> {
    let f = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("cannot open jsonl {}: {e}", path.display());
            return Vec::new();
        }
    };
    let reader = BufReader::new(f);
    let mut out = Vec::new();
    let mut lines = reader.lines().peekable();

    while let Some(line_res) = lines.next() {
        let line = match line_res {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("io error in {}: {e}", path.display());
                continue;
            }
        };
        let is_last = lines.peek().is_none();
        match serde_json::from_str::<JsonlEntry>(&line) {
            Ok(JsonlEntry::Assistant(a)) => out.push(a),
            Ok(JsonlEntry::Other) => {}
            Err(e) if is_last => {
                // D-35: trailing-line truncation is normal during active session — silent.
            }
            Err(e) => {
                tracing::warn!("malformed jsonl line in {}: {e}", path.display());
            }
        }
    }
    out
}
```
[SOURCE: CONTEXT D-35 + observed JSONL schema from `~/.claude/projects/-home-chasel-REPO-AIHPBar/*.jsonl`]

### Example 3: Real JSONL schema observed in this project (2026-05-23)

The user-message envelope (`type:"user"`):
```json
{"parentUuid":null, "isSidechain":false, "promptId":"…", "type":"user",
 "message":{"role":"user","content":"…"}, "isMeta":true, "uuid":"…",
 "timestamp":"2026-05-22T15:23:06.964Z", "userType":"external", "entrypoint":"cli",
 "cwd":"/home/chasel/REPO/AIHPBar", "sessionId":"bcae7970-…", "version":"2.1.148",
 "gitBranch":"HEAD"}
```

The assistant-message envelope (`type:"assistant"` — token usage lives here):
```json
{"parentUuid":"…", "isSidechain":false, "type":"assistant",
 "message":{"model":"claude-opus-4-7", "id":"msg_…", "type":"message", "role":"assistant",
            "content":[…],
            "stop_reason":"tool_use",
            "usage":{"input_tokens":5,
                     "cache_creation_input_tokens":41630,
                     "cache_read_input_tokens":15958,
                     "output_tokens":199,
                     "service_tier":"standard",
                     "cache_creation":{"ephemeral_1h_input_tokens":41630,
                                       "ephemeral_5m_input_tokens":0},
                     "iterations":[…]}},
 "uuid":"…", "timestamp":"2026-05-22T15:23:12.791Z",
 "sessionId":"bcae7970-…", "version":"2.1.148"}
```
[VERIFIED: read directly from `~/.claude/projects/-home-chasel-REPO-AIHPBar/bcae7970-3240-4f63-9c97-fa6392ee9e21.jsonl` on 2026-05-23]

Note: there are also `type:"file-history-snapshot"` entries with neither `timestamp` at top level nor `message.usage` — the parser's `#[serde(other)] Other` arm handles these.

### Example 4: Provider that doesn't need secrets (SEC-04 contract preservation)

```rust
// src/provider/claude/mod.rs
use async_trait::async_trait;
use std::sync::Arc;

use crate::model::{ProviderError, ProviderId, ProviderState};
use crate::provider::{FetchCtx, Provider};

pub struct ClaudeProvider {
    base_path: std::path::PathBuf,  // overridable for tests; default ~
    token_limit: u64,                // const for v1, hook for v2
}

impl ClaudeProvider {
    pub fn new(home_dir: &std::path::Path, token_limit: u64) -> Self {
        Self {
            base_path: home_dir.join(".claude").join("projects"),
            token_limit,
        }
    }
}

#[async_trait]
impl Provider for ClaudeProvider {
    fn id(&self) -> ProviderId { ProviderId::Claude }

    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        // SEC-04 binding: we receive ctx.secrets but don't use it. Contract preserved.
        let _ = ctx.secrets;
        let now = ctx.now;  // Wall-clock injection — NEVER call jiff::Timestamp::now() here.

        // ... file discovery + parse + cluster math + emit ProviderState ...
        unimplemented!("Phase 1 task")
    }
}
```
[CITED: Phase 0 Provider trait + SEC-04 + CONTEXT clock-injection contract]

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `keyring` 4.x crate as library dep | `keyring-core` 1.0 + companion store crate | 2026-04 | Apps now own backend selection; cleaner separation; small migration cost. |
| `ratatui::init()` + `restore()` with manually-installed panic hook | `ratatui::run(closure)` with auto-installed hook | ratatui 0.30 | One line instead of three; panic-safety by default. |
| `chrono` for date/time in new CLI tools | `jiff` 0.2 | 2025 | Better TZ defaults, panic-free arithmetic, Temporal-style API. Already locked. |
| `async-std` runtime | `tokio` (async-std discontinued 2025) | 2025 | Phase 0 already on tokio. |
| Hand-built ANSI color via raw escape codes | `std::io::IsTerminal` (stable 1.70) + `crossterm::style` via ratatui re-export | Rust 1.70+ | IsTerminal removes the `atty` crate dep entirely. |
| Sum `input_tokens + output_tokens` from Claude JSONL | Sum `cache_creation_input_tokens` (with caveat docs) | ccusage issue #866 surfaced 2026 | THE Phase 1 decision shift; see Pitfall L1. |
| Manual `cfg(target_os)` for config paths | `directories` (6.0.0 current) | Stable for years | Cross-OS done right; tiny dep. |

**Deprecated / outdated to avoid:**
- The `keyring` 4.x crate as a library dep (now demo-only).
- `tui-rs` (archived 2023).
- `atty` (replaced by `std::io::IsTerminal`).
- `time` 0.1 (very old; not relevant here).
- Manually-summed `input_tokens + output_tokens` for Claude session %.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `dbus-secret-service-keyring-store` is the right Linux backend choice over `zbus-…` and `linux-keyutils-…` | Standard Stack §Core / Pattern 5 | Low — all three work; dbus is the most-deployed Linux secret store (gnome-keyring, KWallet via libsecret), so coverage is best. If a target user runs a non-libsecret system (rare), pick zbus or document the keyutils fallback. |
| A2 | The three companion crates (`dbus-secret-service-keyring-store` 1.0, `apple-native-keyring-store` 1.0, `windows-native-keyring-store` 1.0) are published by `github.com/open-source-cooperative` (same org as keyring-core) and not slopsquatted | Package Legitimacy Audit | HIGH if wrong — keyring is a credential surface; a slopsquatted store would silently exfiltrate. Planner MUST verify each crate's `repository` field on crates.io BEFORE adding to Cargo.toml. Recommended `checkpoint:human-verify` gate. |
| A3 | The exact constructor signature for each `*-keyring-store` crate is `Store::new("ahb")` | Pattern 5 Code Example | Low — structural sketch; planner verifies via Context7 / docs.rs at impl time. |
| A4 | `tokio::task::JoinSet::spawn_with_id` exists in tokio 1.52 (OR `JoinError::id()` is sufficient for the panic-recovery bookkeeping) | Pitfall L4 | Low — both APIs have existed in tokio for some time; verify at impl. |
| A5 | `directories` 6.0's `ProjectDirs::from("", "", "ahb")` signature is unchanged from 5.x | Pitfall L5 | Very low — three-arg form is canonical and stable across major versions. |
| A6 | The Pro plan's ~44 k token estimate is current as of 2026-05 | Code Example 1 + L1 mitigation | Medium — Anthropic explicitly stopped publishing hard numbers. Planner should mark this constant as a "best-effort estimate; revisit quarterly." |
| A7 | `cache_creation_input_tokens` is the right headline summing field; `cache_read_input_tokens` is ignored | L1 mitigation recommendation | Medium — based on (a) ccusage's "reliable" rating and (b) the intuition that cache_read tokens are amortized over many sessions and shouldn't count against the live window. Planner should sanity-check by running a real Claude Code session and comparing AHB's output to in-Claude `/status`. |
| A8 | `ratatui::run` accepts an async closure | Pattern 2 Code Example | Very low — fetched from docs.rs/ratatui/0.30.0; the docs page describes it as "the simplest way to initialize and run." Verify exact closure signature at impl. |
| A9 | The `MockProvider` panic-injection lever should be env-var gated (`AHB_DEBUG_PANIC`) rather than `#[cfg(debug_assertions)]` | Pitfall L6 | Very low — both work; env-var is more flexible. CONTEXT explicitly delegates choice to planner. |
| A10 | `tracing-subscriber::fmt::init()` after `install_phase0_panic_hook` does not race the hook | Pitfall L7 | Very low — the panic hook uses `eprintln!` directly, not `tracing`, so there's no actual race. The advice is "init in this order to avoid future confusion." |

> **Action for `discuss-phase` or `planner`:** Items A2, A6, A7 deserve user confirmation before lock-in. A2 is the highest risk (keyring backend selection touches credentials). A6 and A7 are about Claude observability quality — user may have opinions on the user-facing accuracy story.

---

## Open Questions

1. **Which Linux keyring backend?** (A1 / A2.) Recommended: `dbus-secret-service-keyring-store`. But the planner should call this out as a user-visible product decision — if AHB ships to users on niche distros without libsecret (Alpine? NixOS without home-manager wiring?), `linux-keyutils-keyring-store` is more portable but volatile across reboots.
   - What we know: three backend connector crates at 1.0 each.
   - What's unclear: which is the de-facto recommended pick for end-user CLI tools.
   - Recommendation: Default dbus; gate at compile time per `cfg(target_os = "linux")` so the choice is uniform within Linux builds. Document the alternatives in README.

2. **Claude 5h `CLAUDE_5H_TOKEN_LIMIT` constant value.** (L1 + A6.) The ~44 k Pro / ~88 k Max5 / ~220 k Max20 numbers come from third-party blog posts. Anthropic does not publish these. Phase 1 has no plan-detection (and PROJECT.md explicitly excludes "plan-tier auto-detection"), so AHB must hardcode one value.
   - What we know: Pro ~44 k is the most-cited 2026 number.
   - What's unclear: how stable that number is; whether Max20 users will see wildly-wrong %.
   - Recommendation: Hardcode `44_000` for v1 (Pro is the default subscription) and document under-counting for Max users. Phase 3 CFG-03 could plausibly add a `[providers.claude].plan_tier = "max20"` knob.

3. **Should the panic-injection integration test live in `tests/` or `examples/`?** (Pitfall L6.) `assert_cmd` tests can run a release-mode binary, but launching a subprocess that deliberately panics generates noise in CI logs.
   - What we know: `assert_cmd` + `AHB_DEBUG_PANIC=adapter:claude` is the recommended mechanism (env var keeps prod builds safe).
   - What's unclear: whether to redirect stderr in the test (clean CI output) vs assert against the panic message in stderr (richer signal but noisy).
   - Recommendation: assert against stderr; CI noise is acceptable for one test.

4. **Should Phase 1's TUI ship the `q` and Ctrl-C handlers, or only `q`?** UI-SPEC says both. ratatui's `EventStream` already delivers Ctrl-C as `KeyEvent { code: Char('c'), modifiers: CONTROL }` — handling is one match arm. Recommend: ship both per UI-SPEC.

5. **Per-provider tokio task tagging for `JoinError` recovery.** (L4.) Two implementation options; both work.
   - Recommendation: pick `HashMap<task::Id, ProviderId>` because `spawn_with_id` may not be public-API yet in tokio.

6. **`glob` symlink behavior in Phase 1.** (L8.) Default is follow-symlinks; for `~/.claude/projects`, this is almost certainly correct, but worth a one-line module doc and a planner note.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (cargo + rustc) | Phase 1 build | ✓ | cargo 1.94.0 / rustc 1.94.0 | — (MSRV is 1.88; we're ahead) |
| `~/.claude/projects/` directory | ADP-02 manual verification | ✓ | populated by Claude Code v2.1.148 on this machine | — |
| `~/.claude/projects/<slug>/<uuid>.jsonl` files | Adapter testing (real data) | ✓ | observed multiple sessions | — |
| `sqlite3` CLI (for Codex Phase 2 sanity checks) | Not Phase 1 — Phase 2 | ✓ | /usr/bin/sqlite3 (irrelevant here) | — |
| Internet access | Not Phase 1 (Gemini is Phase 3) | ✓ (not exercised) | — | — |
| OS keyring backend | Phase 1 wire-up smoke (D-40) | unknown — depends on planner's backend choice and target machines | — | Hard-error per D-41 (no fallback in Phase 1) |
| `pip` for `slopcheck` | Package legitimacy audit | ✗ | — | All packages flagged for human verify; planner uses `checkpoint:human-verify` |

**Missing dependencies with no fallback:** none for Phase 1 implementation. The keyring backend question is a runtime concern for end-users, not a build-time blocker.
**Missing dependencies with fallback:** slopcheck — substituted with manual cargo-search verification + provenance flagging in the Package Legitimacy Audit.

---

## Security Domain

> `security_enforcement` is not explicitly disabled in `.planning/config.json` — treating as enabled. Phase 1 touches credentials surface (keyring-core wire-up + `Secret<T>`), so this section is binding.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | partial | Not user-auth; keyring acts as cred store for downstream provider auth. Phase 1 wires the path but doesn't auth any provider. |
| V3 Session Management | no | No sessions; this is a local CLI. |
| V4 Access Control | yes | Config file at `~/.config/ahb/config.toml` is per-user. Keyring entries are scoped to the user. No multi-user concerns. |
| V5 Input Validation | yes | TOML config (D-38 warn+ignore unknown keys); JSONL parser (D-35 tolerate malformed lines); CLI args (clap). |
| V6 Cryptography | partial | We hold credential plaintext briefly via keyring round-trip; `Secret<T>` + zeroize ensure memory hygiene. We never roll crypto; OS keyring owns at-rest encryption. |
| V7 Error Handling & Logging | yes | `tracing` JSON output and `--json` must never echo secret values (D-43 CI grep test). |
| V14 Configuration | yes | Default-deny config (all providers `enabled = false` per D-37); forward-compat unknown-key handling. |

### Known Threat Patterns for the Phase 1 stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Plaintext credential in config | Information disclosure | Keyring-core only; never persist via toml/json (D-42 omits `Deserialize` on `Secret<T>` for this reason). |
| Secret leaking through `Debug` impl | Information disclosure | `Secret<T>` `Debug` prints `***`; CI grep test asserts. |
| Secret leaking through `Serialize` impl (e.g., `--json` output) | Information disclosure | `Secret<T>` `Serialize` emits `"[REDACTED]"`; D-43 integration test asserts via stdout grep. |
| Adapter panic crashes whole tool (DoS surface for malicious adapters) | DoS | JoinSet + JoinError::is_panic → ProviderError::Internal (ADP-01); engine never propagates panic. |
| Terminal left in raw mode after panic (UX harm; not security-classifiable) | — | `ratatui::run` panic hook (TUI-04). |
| Slopsquatted keyring backend crate | Spoofing / Tampering | `checkpoint:human-verify` before adding each `*-keyring-store` crate (A2). |
| `keyring` v4 mistakenly added by autoformatter | Dependency confusion | `cargo deny` (Phase 4) catches it; for now, comment in Cargo.toml warns. |
| Malicious JSONL injects `"\u{001b}[...m"` escape via embedded prompt → leaks into terminal | Information disclosure (via rendering) | UI-SPEC bans direct emission of any byte outside the locked codepoint set; the renderer paints provider labels (which are closed-enum `snake_case`), not arbitrary content from JSONL. |
| Bogus glob expansion (symlink trickery in `~/.claude`) | Tampering / IDOR | Plain `glob` follows symlinks but stays within user-owned paths; not a Phase 1 attack surface. |

---

## Sources

### Primary (HIGH confidence)

- [docs.rs/ratatui/0.30.0](https://docs.rs/ratatui/0.30.0/ratatui/index.html) — `run()` vs `init()` panic-hook behavior (Pattern 2, Pitfall L2). Confirms `run()` "handles terminal initialization, restoration, **and panic hooks automatically**" while `init()` does NOT.
- [docs.rs/keyring-core/1.0.0/keyring_core/struct.Entry.html](https://docs.rs/keyring-core/latest/keyring_core/struct.Entry.html) — Entry constructors, methods, `NoDefaultStore` error (Pattern 5, Pitfall L3).
- [github.com/open-source-cooperative/keyring-rs (wiki)](https://github.com/open-source-cooperative/keyring-rs) — keyring-core ecosystem architecture, set_default_store pattern.
- crates.io live `cargo search` runs on 2026-05-23 — version confirmations for all 8 Phase 1 prod additions + 5 keyring backend connector crates + 5 dev deps.
- `/home/chasel/REPO/AIHPBar/.planning/research/STACK.md` — primary stack lock-in (Phase 0 research).
- `/home/chasel/REPO/AIHPBar/.planning/research/ARCHITECTURE.md` — engine/cli/tui split, Pattern catalog.
- `/home/chasel/REPO/AIHPBar/.planning/research/PITFALLS.md` — base pitfall list (extended here as L1-L8).
- `/home/chasel/REPO/AIHPBar/.planning/phases/01-engine-claude-tui-scaffold/01-CONTEXT.md` — Decisions D-28..D-43.
- `/home/chasel/REPO/AIHPBar/.planning/phases/01-engine-claude-tui-scaffold/01-UI-SPEC.md` — locked literals (compact-line format, schema-drift sentinel, TUI non-TTY refusal, quit hint).
- `/home/chasel/REPO/AIHPBar/.planning/REQUIREMENTS.md` — 15 phase-1 REQ IDs with traceability.
- Direct inspection of `~/.claude/projects/-home-chasel-REPO-AIHPBar/bcae7970-…jsonl` on 2026-05-23 — confirms actual JSONL schema (Example 3).

### Secondary (MEDIUM confidence — WebSearch verified against multiple sources)

- [github.com/ryoppippi/ccusage/issues/866](https://github.com/ryoppippi/ccusage/issues/866) — Pitfall L1 source for token-field reliability (input_tokens / output_tokens unreliable; cache_* reliable). Cross-referenced against [anthropics/claude-code#24147](https://github.com/anthropics/claude-code/issues/24147) and [#22686](https://github.com/anthropics/claude-code/issues/22686).
- [tokenmix.ai/blog/complete-claude-limits-guide-2026-tokens-uploads-5-hour](https://tokenmix.ai/blog/complete-claude-limits-guide-2026-tokens-uploads-5-hour) — Pro ~44 k / Max5 ~88 k / Max20 ~220 k token estimates (best-available source; Anthropic does not publish).
- [Claude Help Center: usage and length limits](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work) — 5h rolling window + weekly caps confirmation (no hard token numbers).
- [usagebar.com/blog/claude-code-weekly-limit-vs-5-hour-lockout](https://usagebar.com/blog/claude-code-weekly-limit-vs-5-hour-lockout) — community parsing of the 5h + weekly rules.

### Tertiary (LOW confidence — single source, flagged for confirmation)

- Specific Linux backend connector crate names (`dbus-secret-service-keyring-store` 1.0.0, etc.) — discovered via `cargo search`; org provenance not yet verified against github.com/open-source-cooperative. Tagged [ASSUMED] in package audit; planner verifies before lock-in.
- Exact constructor signatures for the three `*-keyring-store` crates — sketched in Pattern 5 but should be Context7-confirmed at impl time.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all 8 Phase 1 additions verified on crates.io 2026-05-23; STACK.md already did the primary research.
- Architecture: HIGH — Phase 0 already locked the spine; Phase 1 just threads one real adapter through it. JoinSet + ratatui::run patterns are both documented current SOTA.
- Pitfalls: MIXED — L1 (token field reliability) is HIGH-confidence and reverses a CONTEXT decision; L2 (ratatui run vs init) is HIGH and corrects a PITFALLS.md error; L3-L8 are MEDIUM operational.
- Keyring backend selection: MEDIUM — companion crate ecosystem is new (April 2026), connector crate names need provenance check before lock-in.
- Claude `CLAUDE_5H_TOKEN_LIMIT` numeric value: MEDIUM — best-available estimate, Anthropic no longer publishes.

**Research date:** 2026-05-23
**Valid until:** 2026-06-22 (30 days) for stack picks; **2026-06-06 (14 days)** for the Claude token-field guidance (upstream Anthropic issues are actively being discussed and may land a fix that shifts L1).
