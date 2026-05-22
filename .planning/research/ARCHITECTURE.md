# Architecture Research

**Domain:** Rust CLI + TUI single-binary tool that polls heterogeneous LLM-subscription data sources (local SQLite, local JSONL, local JSON state, authenticated HTTPS endpoints) per provider, and renders a per-provider HP bar with reset countdown.
**Researched:** 2026-05-22
**Confidence:** HIGH for component boundaries, async event-loop pattern, and provider trait shape (multiple SOTA references). MEDIUM for cache / config / secrets specifics (recommendations follow community SOTA but exact crate choices are still substitutable). LOW for Gemini HTTP adapter internals — that needs a spike, see PITFALLS.md.

## Standard Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│  ENTRY LAYER  (binary `ahb` — same binary, two front-ends)              │
│                                                                          │
│   ┌─────────────────────────┐         ┌─────────────────────────────┐   │
│   │  cli::run()             │         │  tui::run()                 │   │
│   │  • parse flags          │         │  • own the event loop       │   │
│   │  • ONE refresh call     │         │  • tokio::select! ticks     │   │
│   │  • render text → stdout │         │  • redraw on ProviderUpdate │   │
│   │  • exit                 │         │  • quit on 'q'/Ctrl-C       │   │
│   └────────────┬────────────┘         └──────────────┬──────────────┘   │
│                │                                     │                   │
├────────────────┼─────────────────────────────────────┼───────────────────┤
│  CORE LAYER  (engine — `ahb-core` module/crate, UI-agnostic)             │
│                │                                     │                   │
│                ▼                                     ▼                   │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  Engine                                                          │   │
│   │   • refresh_all()       → one-shot fan-out over providers        │   │
│   │   • subscribe()         → mpsc channel of ProviderUpdate         │   │
│   │   • spawn_background()  → owns ticker + per-provider tasks       │   │
│   └────────┬───────────────────┬───────────────────┬────────────────┘   │
│            │                   │                   │                     │
│            ▼                   ▼                   ▼                     │
│   ┌────────────────┐  ┌────────────────┐  ┌─────────────────┐           │
│   │ Cache (TTL)    │  │ Config         │  │ Secrets         │           │
│   │ moka::future   │  │ figment/serde  │  │ keyring +       │           │
│   │ per provider   │  │ TOML layered   │  │ env fallback    │           │
│   └────────────────┘  └────────────────┘  └─────────────────┘           │
├──────────────────────────────────────────────────────────────────────────┤
│  ADAPTER LAYER  (`ahb-adapters` — one impl per provider)                 │
│                                                                          │
│   trait Provider { async fn fetch(&self, ctx) -> Result<ProviderState> } │
│                                                                          │
│   ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────┐    │
│   │ ClaudeAdapter    │  │ CodexAdapter     │  │ GeminiAdapter      │    │
│   │ • read           │  │ • read SQLite    │  │ • HTTPS GET        │    │
│   │   stats-cache.   │  │ • parse JSONL    │  │   usage endpoint   │    │
│   │   json           │  │ • optional       │  │ • parse JSON       │    │
│   │ • parse /usage   │  │   `codex exec    │  │ • respect          │    │
│   │   if available   │  │   /status`       │  │   Retry-After      │    │
│   └──────────────────┘  └──────────────────┘  └────────────────────┘    │
├──────────────────────────────────────────────────────────────────────────┤
│  DATA SOURCES  (external; out of process)                                │
│                                                                          │
│   ~/.claude/         ~/.codex/             gemini.google.com/usage       │
│   stats-cache.json   sessions.db (SQLite)  (HTTPS, cookie/token auth)    │
│                      *.jsonl                                             │
└──────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| `cli::run` | Parse args, call `engine.refresh_all().await`, render text, exit | clap derive Parser; `Renderer::Text` / `Json` / `Compact` enum |
| `tui::run` | Own the event loop, draw frames, subscribe to engine updates, handle key input | ratatui + crossterm + `tokio::select!` over (input, tick, update_rx) |
| `Engine` | Coordinate providers, hold cache + config + secrets, expose two surfaces: `refresh_all` (one-shot for CLI) and `subscribe` (stream for TUI) | Plain struct with `Arc`-wrapped fields; `tokio::spawn` per provider |
| `Provider` trait | Uniform async interface that each provider implements differently | `#[async_trait]` trait, `Send + Sync + 'static` bounds |
| `ClaudeAdapter` | Read local FS state, optionally invoke `claude /usage`, compute HP + reset | Implements `Provider`; uses `tokio::fs` + serde |
| `CodexAdapter` | Read SQLite + JSONL under `~/.codex/`, optionally shell out to `codex exec /status` | Implements `Provider`; uses `rusqlite` (sync, offloaded via `spawn_blocking`) + serde |
| `GeminiAdapter` | Authenticated HTTPS GET to `gemini.google.com/usage`, parse JSON | Implements `Provider`; uses `reqwest` async + secrets loader |
| `Cache` | Memoise last successful `ProviderState` per provider with TTL; serve stale on transient failures | `moka::future::Cache<ProviderId, ProviderState>` with TTL ≈ provider refresh budget |
| `Config` | Load TOML, validate, expose `Vec<ProviderConfig>` with provider-specific `extras` | `figment` or `config` crate, layered: default ← `~/.config/ahb/config.toml` ← env ← flags |
| `Secrets` | Resolve auth material (tokens, cookies) once at startup; never re-read per call | `keyring` crate primary, env var fallback; values held in `Arc<str>` / zeroizing wrapper |
| `Renderer` | Convert `Vec<ProviderState>` (or single update) into text or widget | CLI: `String`-returning fns. TUI: `impl Widget` per provider row |

## Recommended Project Structure

Single Cargo binary crate for v1 (one workspace member, but internal module split that **can** be lifted to a workspace later without rewrites). Justified below.

```
ahb/
├── Cargo.toml
├── src/
│   ├── main.rs                 # parses args, dispatches to cli:: or tui::
│   ├── cli/
│   │   ├── mod.rs              # `pub fn run(args, engine) -> Result<()>`
│   │   ├── render_text.rs      # compact + detailed formatters
│   │   └── render_json.rs      # serde-based machine output
│   ├── tui/
│   │   ├── mod.rs              # `pub async fn run(engine) -> Result<()>`
│   │   ├── app.rs              # AppState (which providers, last update, focus)
│   │   ├── event.rs            # Event enum + spawn_event_task()
│   │   ├── ui.rs               # ratatui draw fn
│   │   └── widgets/
│   │       └── hp_bar.rs       # impl Widget for HpBar — the core visual
│   ├── engine/
│   │   ├── mod.rs              # pub struct Engine, refresh_all, subscribe
│   │   ├── refresh.rs          # fan-out helper (futures::future::join_all)
│   │   └── background.rs       # ticker task for TUI subscriptions
│   ├── provider/
│   │   ├── mod.rs              # the Provider trait + ProviderState + ProviderError
│   │   ├── claude.rs           # ClaudeAdapter
│   │   ├── codex.rs            # CodexAdapter
│   │   └── gemini.rs           # GeminiAdapter
│   ├── cache.rs                # thin wrapper around moka::future::Cache
│   ├── config.rs               # serde + figment loader
│   ├── secrets.rs              # keyring lookup + env fallback
│   └── model.rs                # ProviderId, ProviderState, ResetInfo, HpUnit
└── tests/
    ├── snapshots/              # insta snapshots of CLI output
    └── provider_fakes.rs       # mock Provider impls used by both unit + integration
```

### Structure Rationale

- **One binary crate, not a workspace, in v1.** Codex-rs uses 70 crates because dozens of front-ends consume the core; AHB has two front-ends (cli, tui) and three adapters. A workspace would be premature. The module layout is structured so each top-level module (`provider/`, `engine/`, `cache.rs`, `config.rs`) is trivially extractable to its own crate later if `ahb-core` ever needs to be reused (e.g. a `daemon` or `status-line` front-end mentioned in Constraints).
- **`cli/` and `tui/` are sibling peers, both depend on `engine/`.** They never depend on each other. This is the codex-rs pattern: entry-points sit on top, talk down to the engine, never sideways.
- **`provider/` is its own module with a trait + adapters in sibling files.** Each adapter is one file in v1; promote to `provider/claude/mod.rs` if it grows multi-file (e.g. when `claude.rs` exceeds ~400 LOC or needs sub-modules for `usage_parser.rs` + `cache_io.rs`).
- **`model.rs` holds the wire types** (`ProviderState`, `ResetInfo`, `HpUnit`) so both the engine, every adapter, and every renderer can depend on it without circular module graphs. These types are `Serialize` + `Deserialize` so `--json` is free.
- **`tests/provider_fakes.rs`** is the seam for snapshot tests — see Testing Seams below.

## Architectural Patterns

### Pattern 1: Shared Engine, Two Front-Ends, One Binary (codex-rs pattern)

**What:** A `cli::run` and `tui::run` function each accept the same constructed `Engine` and differ only in their loop shape. CLI calls `engine.refresh_all().await` exactly once, formats, exits. TUI calls `engine.subscribe()` to get an `mpsc::Receiver<ProviderUpdate>`, then enters a `tokio::select!` loop over (input events, tick interval, update channel).

**When to use:** Any time the same data needs both a one-shot scriptable surface and a long-running interactive surface. Lifted directly from codex-rs (codex-cli, codex-tui, codex-exec all sit on codex-core).

**Trade-offs:**
- **Pro:** Zero duplication of fetch / cache / config / secrets logic. Bug fixes in the engine fix both surfaces.
- **Pro:** Same binary, no separate `ahb-tui` install step (Constraint: single static binary).
- **Con:** The engine must be carefully designed to support BOTH "call me once, give me everything" AND "stream me updates forever". Forces a slightly more involved API than a CLI-only tool. Resolved below in Pattern 4.

**Example:**

```rust
// main.rs
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::load(&cli.config_path)?;
    let secrets = secrets::load(&config)?;
    let engine = Engine::new(config, secrets);

    match cli.command.unwrap_or(Command::Default) {
        Command::Default | Command::Status => cli::run(&cli, &engine).await,
        Command::Tui => tui::run(engine).await,
    }
}
```

### Pattern 2: Async Provider Trait with `#[async_trait]` and Boxed Dyn

**What:** Define `Provider` as an `async_trait`-decorated trait so it is `dyn`-safe; store providers as `Vec<Arc<dyn Provider>>` inside `Engine`. Each adapter implements `fetch(&self, ctx: &FetchCtx) -> Result<ProviderState, ProviderError>` differently — local FS for Claude/Codex, HTTPS for Gemini — but the engine treats them uniformly.

**When to use:** When you need polymorphism over async operations with a small, fixed (~3-10) set of implementations. `async_trait` adds one allocation per call (negligible at 15s refresh budget). Native `async fn in trait` exists in stable Rust as of 1.75, but is not `dyn`-safe yet without workarounds, so `async_trait` remains the pragmatic SOTA for plugin-style polymorphism. ([Comprehensive Rust async traits](https://google.github.io/comprehensive-rust/concurrency/async-pitfalls/async-traits.html))

**Trade-offs:**
- **Pro:** Trivial to add a new provider in v2 (new file, new impl, register in config).
- **Pro:** Each adapter is independently testable.
- **Con:** `async_trait` requires `Send + Sync + 'static` discipline on adapter state. `ProviderError` must be `Send + Sync` (use `Box<dyn Error + Send + Sync>` or, better, a concrete `thiserror` enum). ([Tokio dyn Error Send Sync issue](https://nathanleclaire.com/blog/2021/11/06/tokio/rust-dyn-stderrorerror-cannot-be-sent-between-threads-safely/))

**Example:**

```rust
// src/provider/mod.rs
use async_trait::async_trait;

#[async_trait]
pub trait Provider: Send + Sync + 'static {
    fn id(&self) -> ProviderId;
    /// Read the provider's current session/subscription state.
    /// MUST NOT panic. MUST return within ctx.timeout (default 3s).
    async fn fetch(&self, ctx: &FetchCtx) -> Result<ProviderState, ProviderError>;
}

pub struct FetchCtx<'a> {
    pub now: chrono::DateTime<chrono::Utc>,
    pub timeout: std::time::Duration,
    pub secrets: &'a Secrets,
    pub http: &'a reqwest::Client,
}
```

### Pattern 3: Per-Provider Error Isolation via `Vec<Result<...>>` (Starship-style)

**What:** `refresh_all` runs all providers concurrently with `futures::future::join_all` (NOT `try_join_all`). The result is `Vec<Result<ProviderState, ProviderError>>` — never a single early-return. The renderer is responsible for showing failed providers in a degraded state (greyed-out bar, "fetch failed" label, last-known-good from cache if available) while successful providers render normally. This is exactly the model Starship uses: when a module's command times out, "the module then skips rendering that specific information, but the rest of the prompt continues to render normally". ([Starship timeout architecture](https://instagit.com/starship/starship/how-does-starships-command-execution-timeout-system-prevent-slow-commands-from-delaying-prompt-rendering/))

**When to use:** Multi-source aggregators where source independence is a product requirement. Quality-gate explicit: "one provider failing must not break the others."

**Trade-offs:**
- **Pro:** True isolation — Gemini's HTTP endpoint going down does not blank the Claude / Codex bars.
- **Pro:** Combines naturally with cache fallback (Pattern 5).
- **Con:** No "fail-fast" mode. Acceptable for this product — there's no "transactional" semantics to preserve.

**Example:**

```rust
// src/engine/refresh.rs
pub async fn refresh_all(
    providers: &[Arc<dyn Provider>],
    ctx: &FetchCtx<'_>,
) -> Vec<(ProviderId, Result<ProviderState, ProviderError>)> {
    let futures = providers.iter().map(|p| {
        let id = p.id();
        let fut = tokio::time::timeout(ctx.timeout, p.fetch(ctx));
        async move {
            let r = match fut.await {
                Ok(Ok(s)) => Ok(s),
                Ok(Err(e)) => Err(e),
                Err(_)    => Err(ProviderError::Timeout),
            };
            (id, r)
        }
    });
    futures::future::join_all(futures).await
}
```

### Pattern 4: `Engine` Exposes Two Surfaces — `refresh_all` and `subscribe`

**What:** The engine offers two distinct entry points so CLI and TUI consume it without duplication:

```rust
impl Engine {
    /// One-shot. Used by CLI. Returns when all providers have settled.
    pub async fn refresh_all(&self) -> Vec<(ProviderId, Result<ProviderState, ProviderError>)>;

    /// Long-running. Used by TUI. Spawns a background task that ticks every
    /// `config.refresh_interval` and publishes `ProviderUpdate` per provider
    /// as each settles (not batched — first ready, first rendered).
    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<ProviderUpdate>;
}
```

**Why this shape:** CLI users want "give me everything now, then exit" — an awaitable fn. TUI users want "tell me when things change" — a stream. Both surfaces are thin wrappers over the same `refresh_all` internals; `subscribe` just runs `refresh_all` on a tokio interval and forwards each `(id, result)` as it lands. Asyncgit uses the same shape: long-running git ops dispatched off-thread, results delivered via crossbeam channels. ([asyncgit README](https://github.com/gitui-org/gitui/blob/master/asyncgit/README.md))

**When to use:** Always, when one tool serves both batch and interactive use cases.

**Trade-offs:**
- **Pro:** Adapters are written once and serve both surfaces identically — they never know whether a CLI or TUI invoked them.
- **Pro:** Easy to add a third surface later (`daemon`, `status-line`) without changing the engine.
- **Con:** Slightly more API surface than a CLI-only engine. Worth it.

### Pattern 5: TTL Cache with Stale-On-Error Fallback

**What:** Wrap `moka::future::Cache<ProviderId, ProviderState>` in a thin `Cache` struct. On every fetch, the engine first calls the adapter; if the adapter succeeds, it writes the fresh state to cache. If the adapter fails AND a cached entry exists, the engine returns the stale state with an `is_stale: true` flag instead of an error — the renderer dims the bar and shows "(cached Ns ago)". ([Moka TTL docs](https://docs.rs/moka/latest/moka/future/struct.Cache.html))

**When to use:** Network-backed providers (Gemini) where transient HTTP errors should not blank the display.

**Trade-offs:**
- **Pro:** Smooths over transient failures invisibly to the user; meets the "degraded UX, not crash" quality gate.
- **Pro:** Decouples refresh interval from cache TTL — TUI can tick every 15s but cache TTL can be 5min if Gemini endpoint is flaky.
- **Con:** Stale data is shown without explicit user action. Mitigated by visible "(stale)" indicator. Document in README.

### Pattern 6: Channels for Background → UI Communication (ratatui async pattern)

**What:** The TUI's main loop is built on `tokio::select!` over three streams: terminal input events (from `crossterm::event::EventStream`), a tick interval (`tokio::time::interval(Duration::from_millis(250))` for redraw cadence), and the engine's update channel. The engine's background task pushes `ProviderUpdate` events as fetches complete. ([Ratatui full async events](https://ratatui.rs/tutorials/counter-async-app/full-async-events/))

**When to use:** Any TUI that needs non-blocking I/O. This is THE ratatui SOTA pattern.

**Example:**

```rust
// src/tui/mod.rs (sketch)
pub async fn run(engine: Engine) -> Result<()> {
    let mut terminal = init_terminal()?;
    let mut app = AppState::new(&engine.config());
    let mut updates = engine.subscribe();
    let mut input  = crossterm::event::EventStream::new();
    let mut tick   = tokio::time::interval(Duration::from_millis(250));

    loop {
        tokio::select! {
            Some(Ok(ev)) = input.next() => {
                if app.handle_input(ev)?.is_quit() { break; }
            }
            Some(upd) = updates.recv() => {
                app.apply(upd);
            }
            _ = tick.tick() => {
                terminal.draw(|f| ui::draw(f, &app))?;
            }
        }
    }
    restore_terminal()?;
    Ok(())
}
```

## Data Flow

### CLI One-Shot Flow

```
$ ahb --detailed
    │
    ▼
main.rs ─→ Cli::parse() ─→ config::load() ─→ secrets::load() ─→ Engine::new()
                                                                       │
                                                                       ▼
                                                       engine.refresh_all().await
                                                                       │
                              ┌────────────────────────────────────────┼────────────────────────────────────────┐
                              │                                        │                                        │
                              ▼                                        ▼                                        ▼
                    ClaudeAdapter.fetch()                    CodexAdapter.fetch()                    GeminiAdapter.fetch()
                       (tokio::fs)                              (spawn_blocking                          (reqwest http)
                                                                + rusqlite)
                              │                                        │                                        │
                              ▼                                        ▼                                        ▼
                       Ok(ProviderState)                         Ok(ProviderState)                        Err(Timeout)
                              │                                        │                                        │
                              └────────────────────────────────────────┴────────────────────────────────────────┘
                                                                       │
                                                                       ▼
                                                  Vec<(ProviderId, Result<ProviderState, _>)>
                                                                       │
                                                                       ▼
                                                            renderer::text() / json()
                                                                       │
                                                                       ▼
                                                                    stdout
                                                                       │
                                                                       ▼
                                                                    exit(0)
```

### TUI Long-Running Flow

```
$ ahb tui
    │
    ▼
main.rs ─→ Engine::new() ─→ tui::run(engine).await
                                       │
              ┌────────────────────────┼────────────────────────┐
              │                        │                        │
              ▼                        ▼                        ▼
        spawn input task         spawn tick task          engine.subscribe()
        crossterm EventStream    250ms redraw cadence            │
              │                        │                         ▼
              │                        │                  spawn engine bg task
              │                        │                  every 15s:
              │                        │                    refresh_all()
              │                        │                    for each settled provider:
              │                        │                      send ProviderUpdate
              │                        │                         │
              ▼                        ▼                         ▼
    ┌───────────────────────────────────────────────────────────────────┐
    │                  tokio::select! main loop                          │
    │                                                                    │
    │   on input    → app.handle_input()  (may set quit flag)            │
    │   on tick     → terminal.draw(ui::draw)                            │
    │   on update   → app.apply(update)  (per-provider state mutation)   │
    └───────────────────────────────────────────────────────────────────┘
                                       │
                                       ▼
                            (loop until quit)
                                       │
                                       ▼
                              restore_terminal()
```

### Refresh Cycle Detail (tick → fresh state → rendered bar)

```
T = 0s          tick fires in engine background task
                  │
                  ▼
                refresh_all() invoked
                  │
       ┌──────────┼──────────┐
       │          │          │
       ▼          ▼          ▼
  Claude     Codex      Gemini
  (10ms)    (40ms)     (800ms HTTP)
       │          │          │
       │ ◄── on settle, send ProviderUpdate { id, result } ─── │
       │          │          │
       ▼          ▼          ▼
  (sent at    (sent at    (sent at
   T=10ms)    T=40ms)     T=800ms)
                  │
                  ▼
       TUI's update channel receives THREE messages, possibly out of order
                  │
                  ▼
       AppState.apply() updates per-provider entry
                  │
                  ▼
       Next 250ms tick → ui::draw() → only changed rows visually diff
                  │
                  ▼
       Cache.put(provider_id, state) for each Ok result
       (failed providers retain prior cached state, marked is_stale)
```

### State Management

The engine owns the source-of-truth state. The TUI keeps a view-model (`AppState`) that mirrors it, mutated only via `ProviderUpdate` messages. This is the unidirectional flow ratatui's async-template recommends. No shared `Arc<Mutex<State>>` between engine and UI — channels only. ([ratatui async-template structure](https://ratatui.github.io/async-template/02-structure.html))

### Key Data Flows

1. **Provider state delivery:** `Provider::fetch` → `Engine::refresh_all` → (CLI: returned Vec) / (TUI: per-provider mpsc message) → renderer.
2. **Config bootstrap:** TOML file → `figment` merge with env vars + flags → `Config` struct → passed into `Engine::new` and `Cli::parse`. One-time, at startup.
3. **Secret resolution:** `Secrets::load` reads keyring (and env fallback) ONCE at startup. Adapters get `&Secrets` reference via `FetchCtx`. No re-read per fetch — minimises keyring prompts and reduces blast radius if process is dumped.
4. **Cache update:** Successful `ProviderState` → `Cache.put` → subsequent failures within TTL serve `is_stale: true` from cache → renderer dims the row but does not blank it.

## Scaling Considerations

This is a single-user local CLI, so "scale" means "how many providers and how often", not users.

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 3 providers, 15s refresh (v1 target) | Current design works as-is. No adjustments needed. |
| 10 providers, 15s refresh | No changes. `join_all` over 10 futures is trivial. |
| 30+ providers OR sub-5s refresh | Consider per-provider refresh intervals (some providers cheap, some expensive). Cache TTL becomes critical. |
| Daemon mode (Constraints mentions "daemon 模式好擴") | Lift `engine/` to a separate crate, add `daemon::run` as a third entry point that listens on a unix socket. CLI becomes a thin client. No engine rewrite needed if Pattern 4 is followed. |

### Scaling Priorities

1. **First bottleneck:** Network-backed adapters (Gemini today; future cloud-quota providers). Fix: per-provider `refresh_interval` in config, cache TTL ≥ refresh interval, respect `Retry-After`.
2. **Second bottleneck:** Adapters that shell out (`codex exec /status`). Fix: prefer parsing files directly over shelling; shell out only as fallback. Use `tokio::process::Command` with `kill_on_drop`.
3. **Third bottleneck (only if it materialises):** SQLite reads if Codex's session DB grows large. Fix: prepared statements, index hints, or move to `spawn_blocking` if read latency exceeds 100ms.

## Anti-Patterns

### Anti-Pattern 1: Duplicating Fetch Logic Between CLI and TUI

**What people do:** Write a `cli/fetch.rs` and a `tui/fetch.rs` that each call the providers, because "CLI is sync and TUI is async". Or worse, write the CLI synchronously and re-implement async fetch for the TUI.
**Why it's wrong:** Bug fixes have to be made twice. The "compact" CLI output starts drifting from the TUI's row format. Cache semantics diverge.
**Do this instead:** Pattern 1 + Pattern 4. The engine is async everywhere; CLI just runs `tokio::main` and awaits `refresh_all()` once. `cargo install` users will not notice the tokio startup cost (< 5ms on warm filesystem).

### Anti-Pattern 2: `Result<Vec<ProviderState>, Error>` for Multi-Provider Refresh

**What people do:** `refresh_all() -> Result<Vec<ProviderState>, Error>` — one failure poisons the whole batch.
**Why it's wrong:** Violates the explicit quality gate: "one provider failing must not break the others". Also makes `?` propagation accidentally lethal.
**Do this instead:** `Vec<(ProviderId, Result<ProviderState, ProviderError>)>` (Pattern 3). The Vec itself is infallible; per-provider results carry the per-provider error.

### Anti-Pattern 3: Sharing State via `Arc<Mutex<AppState>>` Between Engine Task and UI

**What people do:** Spawn the refresh task with a clone of `Arc<Mutex<AppState>>`, mutate it directly from the refresh task, and have the UI loop read it on every redraw.
**Why it's wrong:** Mutex contention between refresh and redraw, lock-ordering bugs, no audit trail of state transitions, no way to test in isolation.
**Do this instead:** Channels. Engine task owns its own state, sends `ProviderUpdate` messages, UI task owns `AppState` and mutates it only in response to messages. Same pattern as Elm/redux. ([ratatui best practices discussion](https://github.com/ratatui/ratatui/discussions/220))

### Anti-Pattern 4: Loading Secrets Per Fetch

**What people do:** Each adapter reads its API token from keyring/file at the start of `fetch()`.
**Why it's wrong:** macOS keyring prompts the user on every call. Linux `secret-service` may not be running. Files may have been rotated mid-run.
**Do this instead:** Pattern in `Data Flows` #3 — load once at startup into `Secrets`, pass `&Secrets` via `FetchCtx`. If a secret is missing at startup, fail loud (clear error message naming the provider and the expected secret), not silent.

### Anti-Pattern 5: Blocking Filesystem / SQLite Reads on the Async Runtime

**What people do:** Use `std::fs::read_to_string` or `rusqlite` directly inside an async `fetch` impl.
**Why it's wrong:** Blocks a tokio worker thread. With a 15s tick across 3 providers it usually works by accident, but under load (or with a slow disk) the TUI input loop stutters.
**Do this instead:** Use `tokio::fs` for filesystem; wrap `rusqlite` calls in `tokio::task::spawn_blocking`. ClaudeAdapter is mostly async-native; CodexAdapter needs `spawn_blocking` for its SQLite read.

### Anti-Pattern 6: Coupling Renderer to Provider Identity

**What people do:** `if state.provider == ProviderId::Claude { /* special claude rendering */ }` scattered through the renderer.
**Why it's wrong:** Adding a fourth provider in v2 requires touching the renderer in 5 places.
**Do this instead:** The renderer reads `ProviderState` polymorphically — name, hp_percent, reset_at, optional sub-bar entries. Provider-specific quirks live in the adapter's mapping into `ProviderState`, not in the renderer.

## Build Order

Order is dictated by what unblocks the next stage's tests. Phase boundaries roughly map to roadmap phases.

### Foundation (cannot ship without)

1. **`model.rs`** — `ProviderId`, `ProviderState`, `ResetInfo`, `HpUnit`, `ProviderError`. Serializable, no deps beyond serde + chrono. This is the contract; everything else negotiates with it. **Build first.**
2. **`provider::Provider` trait + `FetchCtx`** — the async trait shape, with `#[async_trait]`. No implementations yet.
3. **A `MockProvider` in `tests/provider_fakes.rs`** — returns a canned `ProviderState`. Critical for testing engine and renderer in isolation before any real adapter exists.
4. **`engine::refresh_all`** — fan-out helper. Tested against MockProvider. No cache yet.
5. **`config.rs`** + **`secrets.rs`** — parse TOML, resolve secrets. Minimal: just enough to wire 1 provider.
6. **`cli/render_text.rs` (compact format)** — the simplest renderer. Tested against MockProvider via engine.
7. **`main.rs` with `Command::Default` only** — minimal viable CLI. Smoke-test: `ahb` prints one MockProvider's HP bar. **First demo-able milestone.**

### First Real Adapter (validates the trait shape)

8. **`provider/claude.rs`** — the easiest adapter (pure local FS, no auth). Implementing this first surfaces any holes in the trait. If `Provider::fetch` is wrong, you discover it now, not after 3 adapters are written.
9. **Snapshot tests for CLI output with real ClaudeAdapter** — `insta` snapshots of compact + detailed + json output. Use a fixture `~/.claude/stats-cache.json` checked into `tests/fixtures/`.

### Network Adapter (validates HTTP + secrets path)

10. **`provider/gemini.rs`** — the hardest adapter (auth, HTTP, JSON parse, retry, timeout). Build before Codex so the network / secrets / cache plumbing is proven before adding a third pattern.
11. **`cache.rs`** — only NOW is cache needed; Claude reads are cheap enough that v1 could ship without cache for local-FS adapters. Gemini needs it.
12. **Stale-on-error fallback wiring** — extend the engine to consult cache when an adapter errors.

### Second Real Adapter (validates spawn_blocking + SQLite)

13. **`provider/codex.rs`** — exercises `spawn_blocking` for SQLite. Pattern proven by now.

### TUI Mode (builds on everything above)

14. **`tui/event.rs`** — input + tick channels. Trivial standalone.
15. **`engine::subscribe`** + `engine::background` — the streaming surface. Tested by sending Mock updates.
16. **`tui/widgets/hp_bar.rs`** — the visual core. Snapshot-tested with `ratatui::backend::TestBackend`.
17. **`tui/ui.rs` + `tui/app.rs`** — assemble widgets, handle layout.
18. **`tui/mod.rs::run`** — the `tokio::select!` loop wires everything.

### Polish

19. **`cli/render_json.rs`** — trivial given `ProviderState: Serialize`.
20. **`--detailed` formatter** — multi-line per provider.
21. **Error renderer polish** — `(fetch failed: timeout)` / `(stale 2m ago)` indicators.
22. **Config file scaffold + `ahb init` command** — generates a starter `~/.config/ahb/config.toml`.

### Defer Until Proven Needed

- Workspace split (only if a second binary appears).
- Daemon mode (Pattern from Constraints; not v1).
- Per-provider refresh intervals (until 3+ providers prove uniform 15s is wrong).
- Cache persistence to disk (in-memory is enough at process scope).
- Plugin loading at runtime (compile-time provider registration is fine for 3 providers).

## Error / Degraded-State Model

### Single-Provider Failure Isolation

Three-layer defence:

1. **Timeout wrapper** (Pattern 3): every `Provider::fetch` is wrapped in `tokio::time::timeout(ctx.timeout, ...)`. Default `ctx.timeout = 3s` for local adapters, `10s` for HTTP adapters; configurable per provider in TOML.
2. **`Vec<Result<...>>`** (Pattern 3): the engine never collapses one provider's failure into a fatal error.
3. **Cache fallback** (Pattern 5): when an adapter returns `Err`, the engine consults the cache; if a non-expired entry exists, the renderer shows it with `is_stale = true`.

### `ProviderError` Enum

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider timed out after {0:?}")]
    Timeout(Duration),
    #[error("auth failed: {0}")]
    Auth(String),
    #[error("data source missing: {0}")]
    MissingSource(PathBuf),
    #[error("rate limited; retry after {0:?}")]
    RateLimited(Duration),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

The renderer maps each variant to a short user-facing label (`"timeout"`, `"unauthorized"`, `"missing"`, `"rate-limited (retry 12m)"`, `"parse error"`, `"offline"`, `"failed"`).

### Degraded UX Rules

- **No panics.** Adapter `panic`s would unwind a tokio task and could leave terminal in raw mode. Use `catch_unwind` at the engine boundary OR — preferred — code adapters to never panic.
- **Failed provider row is never blank.** Always shows a row with provider name and either (a) last cached value dimmed with stale indicator, or (b) a clear error label.
- **Failed provider never blocks others.** Independent timeouts. `join_all`, never `try_join_all`.
- **TUI never exits on fetch error.** Only on user keypress (`q`, Ctrl-C) or unrecoverable terminal state.
- **CLI exit code semantics:** `0` if ≥1 provider succeeded; `1` if ALL providers failed; `2` if config/secrets failed to load (couldn't even try). Documented in `--help`.

## Testing Seams

### Seam 1: `Provider` Trait → Mock Adapters

The `Provider` trait is the single biggest testing seam. Tests inject `MockProvider` (or `FlakyMockProvider`, `SlowMockProvider`, etc.) via the same `Vec<Arc<dyn Provider>>` that the production `Engine` accepts.

```rust
// tests/provider_fakes.rs
pub struct MockProvider {
    pub id: ProviderId,
    pub responses: Mutex<VecDeque<Result<ProviderState, ProviderError>>>,
}

#[async_trait]
impl Provider for MockProvider {
    fn id(&self) -> ProviderId { self.id }
    async fn fetch(&self, _: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        self.responses.lock().await.pop_front()
            .unwrap_or(Err(ProviderError::Other(anyhow!("no canned response"))))
    }
}
```

### Seam 2: Filesystem Fixtures for Real Adapters

`tests/fixtures/claude/.claude/stats-cache.json`, `tests/fixtures/codex/.codex/sessions.db`, etc. Adapters accept a `base_path` in test-only constructors (`ClaudeAdapter::for_test(path)`) so unit tests don't depend on `$HOME`.

### Seam 3: HTTP via `wiremock` for Gemini

`wiremock::MockServer` simulates `gemini.google.com/usage` responses (success, 401, 429 with `Retry-After`, 500, slow response). GeminiAdapter accepts the base URL in config so tests can point it at the mock server.

### Seam 4: Snapshot Tests via `insta`

CLI output is plain text → `insta::assert_snapshot!`. TUI widgets are drawn into `ratatui::backend::TestBackend` (an in-memory buffer) → `insta::assert_snapshot!(buffer)`. Snapshots capture the HP bar's exact column layout, so any regression is visible in PR diff.

### Seam 5: Clock Injection for Reset Countdown

`ProviderState::reset_at: DateTime<Utc>` is computed against `FetchCtx::now`. Tests construct `FetchCtx` with a fixed `now` to get deterministic countdown output.

### Seam 6: Channel-Based TUI Tests

Because the engine→TUI boundary is an `mpsc::Receiver<ProviderUpdate>`, TUI logic can be unit-tested by hand-constructing updates and pushing them through a channel — no need to spawn the real engine.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| `~/.claude/stats-cache.json` | `tokio::fs::read_to_string` + `serde_json` | File may be missing if Claude Code never launched; treat as `MissingSource` not crash. |
| `claude /usage` (optional fallback) | `tokio::process::Command::new("claude")` with `kill_on_drop(true)` and a hard timeout | Only invoke if local stats file is insufficient. Slow (`claude` startup ≈ 200ms). |
| `~/.codex/sessions.db` (SQLite) | `rusqlite::Connection::open` inside `tokio::task::spawn_blocking` | Read-only open. Use `OpenFlags::SQLITE_OPEN_READ_ONLY`. |
| `~/.codex/*.jsonl` | Line-by-line `tokio::io::BufReader` over `tokio::fs::File` | Tail-read newest file; cache file inode + offset between calls. |
| `codex exec /status` (optional fallback) | `tokio::process::Command` with `kill_on_drop` and 5s timeout | Out-of-process; expensive; only as fallback. |
| `gemini.google.com/usage` (HTTPS) | `reqwest::Client` with `timeout(10s)` and `User-Agent: ahb/<ver>` | Auth: cookie or bearer token from secrets. Respect `Retry-After`. |
| OS keyring | `keyring` crate (`Entry::new("ahb", &provider_id)`) | macOS: Keychain. Linux: Secret Service. Windows: Credential Manager. Env-var fallback in CI. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `main` ↔ `cli` / `tui` | Direct function call | `tui::run` is `async`; `cli::run` is `async`; both awaited from `#[tokio::main]`. |
| `cli` / `tui` ↔ `engine` | Direct call (`refresh_all`) + mpsc channel (`subscribe`) | Pattern 4. |
| `engine` ↔ `Provider` adapters | `async_trait` method calls through `Arc<dyn Provider>` | Pattern 2. |
| `engine` ↔ `Cache` | Direct method calls | Cache is a struct field; not a trait (no need to mock independently in v1). |
| `engine` ↔ `Config` / `Secrets` | Read-only at startup; `Arc<Config>` / `Arc<Secrets>` passed by reference | Patterns 3, 4. |
| Background task ↔ TUI | `tokio::sync::mpsc::UnboundedSender<ProviderUpdate>` | Pattern 6. Unbounded because update volume is low (3 providers × 1/15s). |
| Provider adapters ↔ external sources | Adapter-specific (fs, sqlite, http) | Each adapter encapsulates its own integration; engine is unaware. |

## How the Same Provider Adapter Serves Both CLI and TUI Without Duplication

This is the explicit downstream-consumer ask, so summarised clearly:

1. **Adapters never know about CLI vs TUI.** They implement `Provider::fetch(&self, ctx: &FetchCtx) -> Result<ProviderState>`. That's it.
2. **CLI path:** `cli::run` calls `engine.refresh_all().await` — engine fans out over all adapters, collects results, returns the `Vec`. Single invocation. Single execution of each adapter.
3. **TUI path:** `tui::run` calls `engine.subscribe()` — engine spawns a background task that calls `refresh_all()` on a `tokio::time::interval`, and forwards each settled `(ProviderId, Result<...>)` over an mpsc channel. The TUI loop receives, mutates `AppState`, redraws on the next 250ms tick.
4. **The adapter is invoked identically in both paths.** Same trait method, same `FetchCtx`, same return type. The ONLY difference is who collects the results: a `Vec` (CLI) vs an `mpsc::Sender` (TUI). The engine itself contains one `refresh_all` implementation; the streaming version is just `refresh_all` called repeatedly with a sender.
5. **Cache, secrets, config, and timeout policy are applied at the engine layer**, not the adapter layer, so they apply uniformly. An adapter author writes one `fetch` impl and gets caching, timeouts, error isolation, and stream/batch support for free.

This is the codex-rs lesson distilled down to AHB's scale: keep the core engine UI-agnostic, expose two clean surfaces (`refresh_all` for batch, `subscribe` for stream), and let front-ends choose their cadence.

## Sources

### High confidence (official docs, primary architecture references)

- [codex-rs Architecture: How OpenAI Rewrote Codex CLI in Rust](https://codex.danielvaughan.com/2026/03/28/codex-rs-rust-rewrite-architecture/) — shared-core multi-entrypoint pattern, submit/event model, workspace layering.
- [Ratatui — Full Async Events tutorial](https://ratatui.rs/tutorials/counter-async-app/full-async-events/) — `tokio::select!` event loop, channel-based event distribution, tick/render interval pattern.
- [Ratatui — Async Event Stream tutorial](https://ratatui.rs/tutorials/counter-async-app/async-event-stream/) — background task spawning, mpsc channel pattern for input events.
- [Ratatui async-template structure docs](https://ratatui.github.io/async-template/02-structure.html) — unidirectional state flow recommendation.
- [Ratatui Event Handling concepts](https://ratatui.rs/concepts/event-handling/) — input + tick stream multiplexing.
- [gitui asyncgit README](https://github.com/gitui-org/gitui/blob/master/asyncgit/README.md) — thread-pool + crossbeam-channel pattern for offloading slow ops while keeping UI responsive.
- [Starship — Command Execution Timeout architecture](https://instagit.com/starship/starship/how-does-starships-command-execution-timeout-system-prevent-slow-commands-from-delaying-prompt-rendering/) — per-module timeout with graceful degradation; module returns `None` instead of failing the prompt.
- [Moka — `future::Cache` docs](https://docs.rs/moka/latest/moka/future/struct.Cache.html) — TTL configuration, async cache API.

### Medium confidence (community recommendations)

- [Ratatui best practices discussion #220](https://github.com/ratatui/ratatui/discussions/220) — channels-not-mutex for state, view-model pattern.
- [Comprehensive Rust — Async Traits](https://google.github.io/comprehensive-rust/concurrency/async-pitfalls/async-traits.html) — `async_trait` rationale and `dyn` safety.
- [Tokio dyn Error Send Sync blog post](https://nathanleclaire.com/blog/2021/11/06/tokio/rust-dyn-stderrorerror-cannot-be-sent-between-threads-safely/) — `Send + Sync` discipline for `tokio::spawn`-friendly errors.
- [Rain's Rust CLI recommendations — Handling arguments](https://rust-cli-recommendations.sunshowers.io/handling-arguments.html) — clap subcommand patterns.
- [How to Set a Default Subcommand with Clap Derive](https://www.w3tutorials.net/blog/how-to-make-a-default-subcommand-with-clap-and-derive/) — `subcommand_negates_reqs` and default-command idioms.
- [Building High-Performance CLIs: Rust & TUI for Monitoring](https://techbytes.app/posts/rust-tui-high-performance-cli-monitoring/) — TUI-for-monitoring architecture survey.
- [Ratatui main repo ARCHITECTURE.md](https://github.com/ratatui/ratatui/blob/main/ARCHITECTURE.md) — workspace organisation reference (for future workspace split decision).

### Lower confidence (background reading)

- [Trait-Driven Rust Architecture](https://dev.to/raminfp/trait-driven-rust-architecture-1ife) — trait composition patterns; AHB uses a single core trait, not deep composition.
- [How to Implement Caching Strategies in Rust](https://oneuptime.com/blog/post/2026-02-01-rust-caching-strategies/view) — caching options survey.

---
*Architecture research for: AHB (AI HP Bar) — Rust CLI+TUI multi-provider LLM-subscription monitor*
*Researched: 2026-05-22*
