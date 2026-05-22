# Phase 0: Spike & Spine — Research

**Researched:** 2026-05-22
**Domain:** Rust CLI scaffold + cross-adapter trait contract + Gemini local-capture feasibility spike
**Confidence:** HIGH on stack picks / panic-hook composition / clippy config / GitHub Actions skeleton / `ahb` crate name availability / `serde+anyhow` sentinel pattern. MEDIUM on `gemini /stats` non-interactive viability (verified PR #8305 merged + headless JSON includes `stats`, but built-in slash commands are still gated and exact field shape unverified — the spike itself produces the final answer). LOW on `gemini /stats` *exact field structure* — the spike must capture three samples and pin the schema.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Repo Scaffolding**
- **D-01 (Crate name):** `ahb`. Single crate, single binary. Both the cargo crate name and the bin name are `ahb`. `Cargo.toml` description = `"AHB — AI HP Bar — multi-CLI subscription session usage at a glance"`.
- **D-02 (Rust edition):** `2024`. Matches MSRV 1.88.
- **D-03 (License):** Dual `MIT OR Apache-2.0`. Add `LICENSE-MIT` + `LICENSE-APACHE` + SPDX `license = "MIT OR Apache-2.0"` in Cargo.toml.
- **D-04 (MSRV):** `rust-version = "1.88"` in `Cargo.toml`.
- **D-05 (CI):** GitHub Actions on push + PR. Matrix `ubuntu-latest / macos-latest / windows-latest`. Each runs `cargo build`, `cargo test`, `cargo clippy -- -D warnings`. No fmt-check, audit, or deny in Phase 0 (deferred to Phase 4).
- **D-06 (Layout):** Single binary crate (NOT workspace). Internal module split: `src/cli/`, `src/tui/`, `src/engine/`, `src/provider/`, `src/cache.rs`, `src/config.rs`, `src/secrets.rs`, `src/model.rs`.
- **D-07 (gitignore):** Standard Rust `.gitignore` (`/target`, `**/*.rs.bk`, Cargo.lock NOT ignored).

**`model.rs` Contract Shape**
- **D-08:** `ProviderState { id, windows: Vec<HpWindow>, fetched_at: jiff::Timestamp, source: &'static str }`.
- **D-09:** `HpWindow { label: Cow<'static, str>, percent_remaining: f32, reset: ResetInfo, bar_color: Option<BarColor> }`.
- **D-10:** `HpUnit = f32  // 0.0..=100.0`. Percent only, NO raw token fields.
- **D-11:** `ResetInfo { resets_at: jiff::Timestamp }`. Absolute timestamp; UI computes countdown.
- **D-12:** `ProviderError` closed enum: `Unconfigured / Unavailable { reason } / SchemaDrift { missing: Vec<String> } / Network(reqwest-wrap) / RateLimited { retry_after: Option<jiff::Span> } / Internal(anyhow::Error)`.
- **D-13:** `#[async_trait] pub trait Provider: Send + Sync + 'static { fn id(&self) -> ProviderId; async fn fetch(&self, ctx: &FetchCtx) -> Result<ProviderState, ProviderError>; }`.
- **D-14:** Serde derive on `ProviderState`/`HpWindow`/`ResetInfo` (full); `ProviderError` is `Serialize` only; `Internal(anyhow::Error)` is `#[serde(skip)]` with a sentinel field in JSON.

**Charset & HP Bar Visual**
- **D-15:** Default `█` (U+2588) / `░` (U+2591). Verified rendering in tmux / screen / Windows Terminal is a Phase 0 success criterion.
- **D-16:** Bar width fixed 10 cells.
- **D-17:** Color auto-on when TTY, auto-off when piped. Respect `NO_COLOR`. `--color=auto|always|never`. Use `std::io::IsTerminal` + a color crate (Claude's discretion — researcher recommends).
- **D-18:** `--ascii` substitutes `#`/`-`. Example `########-- 80%`. Explicit opt-in.
- **D-19:** No emoji in v1 (deferred to v2 DIFF-01).

**Gemini Spike**
- **D-20:** Local `gemini /stats` capture FIRST. NOT web `gemini.google.com/usage`.
- **D-21 (Go criteria, ALL THREE):** non-interactive trigger; output contains quota + reset/window; format stable and parseable.
- **D-22:** No-go → write memo, defer Gemini to v2 stub. Do NOT spike web fallback.
- **D-23:** Spike output at `.planning/research/GEMINI_SPIKE.md` with required sections (Method / Result / Parse feasibility / Go/No-Go / Kill criteria / Phase 3 hand-off / Web-fallback rationale appendix).
- **D-24:** Sample fixtures deferred to Phase 3 (prose-only in Phase 0 memo).

**Skeleton Binary**
- **D-25:** `MockProvider` returns one `HpWindow` (`label="mock-session", percent_remaining=60.0, resets_at=now+2h`). Output line literal: `mock-session  ██████░░░░ 60% • resets in 2h00m`.
- **D-26:** Charset verification documented in `GEMINI_SPIKE.md` (small section, or split if it grows).
- **D-27:** `std::panic::set_hook` to a noop (or stderr-printer) in Phase 0 CLI path so Phase 1's `ratatui::init()` cleanly composes via `take_hook()`.

### Claude's Discretion

- Color crate choice (`nu-ansi-term` / `anstyle` / `owo-colors`) — researcher to recommend.
- `ProviderId` shape (enum vs `Cow<'static, str>` newtype) — researcher to recommend.
- Exact `FetchCtx` fields beyond `now: jiff::Timestamp` and `secrets: &Secrets`.
- `tokio::main` vs `pollster::block_on` in Phase 0 entry.
- Whether to include deps Phase 0 doesn't strictly need yet.

### Deferred Ideas (OUT OF SCOPE)

- `ttl_hint` field on `ProviderState` (Phase 3 with cache).
- `bar_color` rendering rules (Phase 1/2 UI).
- `ProviderState::stale: bool` (Phase 3 with cache).
- Web `gemini.google.com/usage` route (out of Phase 0 entirely).
- Sample fixtures for `gemini /stats` (Phase 3).
- Emoji / pace icons (v2 DIFF-01).
- `cargo fmt --check`, `cargo audit`, `cargo deny` (Phase 4).
- Crate-level docs / docs.rs landing page (Phase 4).
- Cargo workspace split (deferred indefinitely; revisit only on OPS-01 daemon or external lib consumers).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ADP-00 | `Provider` trait + `FetchCtx` + `ProviderState` / `ResetInfo` / `HpUnit` / `ProviderError` 統一介面，三家共用 | Trait shape, `async_trait` necessity check, FetchCtx minimal-fields recommendation, `ProviderError` Serialize-with-skip pattern, `MockProvider` test pattern (Findings 4, 5, 8, 9, 13) |

The Phase 0 trait MUST also be designed to serve ADP-01 (per-adapter timeout + `Vec<Result<...>>` + cache stale fallback), ADP-02 (Claude local FS), ADP-03 (schema-drift sentinel — already covered by `ProviderError::SchemaDrift`), ADP-04 (Codex SQLite read-only + `rate_limits: null` handling), ADP-05 (Gemini, possibly stubbed). The contract must not preclude any of these future implementations.
</phase_requirements>

## Summary

Phase 0 is a scaffolding + investigation phase, not a coding phase. Three workstreams run in parallel:

1. **Gemini local-capture spike** — verify whether `gemini -p "/stats"` or piped-stdin invocation can produce structured-enough output containing quota + reset/window data. Published evidence is **mixed-positive**: gemini-cli PR #8305 (merged 2025-09-19) added non-interactive custom-command support, and `--output-format json` returns a documented `stats` object with token counts. But (a) PR #8305 explicitly states "Built-in commands not supported", (b) the public `headless.md` docs do not enumerate what's inside `stats`, and (c) issue #16567 (filed 2026-01-14, p2) documents `gemini` hanging in non-interactive mode for shell-command flows. The spike must therefore *empirically test* on the developer's box, not infer from docs. The plan should treat the go-path as plausible but not confirmed.
2. **Lock `src/model.rs` contract** per CONTEXT.md D-08…D-14. The interesting work is the *cross-cutting details* CONTEXT didn't fully resolve: `ProviderId` shape, `FetchCtx` exact fields, `ProviderError::Internal(anyhow::Error)` serde workaround, `MockProvider` test pattern that asserts dyn-safety + Send + Sync + serde roundtrip in one shot.
3. **Repo + CI skeleton** — `cargo new ahb`, `Cargo.toml` with verified deps, `clippy.toml` with the `unwrap_used = deny` scoping that *actually works* (CONTEXT just says "for adapter/render paths" but Clippy issue #13981 shows `allow-unwrap-in-tests` does NOT cover `tests/` directory — implications for the plan), `.github/workflows/ci.yml` with the modern 3-OS matrix.

**Primary recommendation:** Three concrete picks for items CONTEXT.md left open — (a) `owo-colors` for color (CONTEXT.md options shortlist confirmed by *Rain's Rust CLI recommendations*); (b) `ProviderId` as a fixed enum (`Claude`, `Codex`, `Gemini`, `Mock`) with a `Cow<'static, str>` escape hatch added in v2 only when EXT-01 lands; (c) `actions-rust-lang/setup-rust-toolchain@v1` instead of `dtolnay/rust-toolchain` because it bundles caching + problem matchers and is the documented successor.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| CLI argument parsing | Entry layer (`src/main.rs` / `src/cli/`) | — | Per ARCHITECTURE.md: `main.rs` parses flags; CLI surface owns its own routing. |
| Provider trait & contract types | Core/Model (`src/model.rs` + `src/provider/mod.rs`) | — | Every other tier depends *down* on `model.rs`; locking the spine here is the whole point of Phase 0. |
| MockProvider | Test seam (`src/provider/mock.rs` or `tests/provider_fakes.rs`) | — | Per ARCHITECTURE.md Build Order step 3: "A `MockProvider` in `tests/provider_fakes.rs` — returns a canned `ProviderState`. Critical for testing engine and renderer in isolation before any real adapter exists." For Phase 0 the mock is also the *runtime* provider used by `cargo run`, so it lives in `src/provider/mock.rs` (production-reachable) rather than test-only. |
| HP-bar rendering (text only) | Entry layer (`src/cli/render_text.rs`) | — | One-shot CLI render in Phase 0; TUI tier doesn't exist yet. |
| Panic-hook installation | Entry layer (`src/main.rs`) | Future TUI layer (Phase 1) | Hook must be installed BEFORE any provider code per Pitfall 5; Phase 0 hook is noop/stderr-print; Phase 1 `ratatui::init()` composes via `take_hook()`. |
| Charset verification | Manual / out-of-band | Documented in `GEMINI_SPIKE.md` (or `CHARSET_NOTE.md`) | This is a Phase 0 *developer activity*, not application code. |
| `Secrets` stub | Core (`src/secrets.rs`) | — | Phase 0 declares the type as a no-op stub so `FetchCtx<'_>` can carry `&Secrets` without Phase 1 ABI breakage. No keyring-core integration yet. |
| `Config` | NOT in Phase 0 | Phase 1 (`src/config.rs`) | Phase 0 hard-codes MockProvider in `main.rs`; config loading is Phase 1. |
| CI workflow | Build infrastructure (`.github/workflows/ci.yml`) | — | Outside of `src/`; the build-and-test floor that prevents future commits from breaking the spine. |

## Standard Stack

### Core (Phase 0 scope only — Phase 1 adds more)

| Library | Verified Version | Purpose | Why Standard |
|---------|------------------|---------|--------------|
| `clap` | 4.6.1 [VERIFIED: npm-equivalent crates.io via project STACK.md cross-check 2026-05-22] | CLI arg parsing | `--ascii`, `--color`, eventual subcommands (`AHB tui`) |
| `ratatui` | 0.30.0 [VERIFIED: docs.rs 2026-05-22] | TUI rendering (NOT used in Phase 0 binary, but pulled in early so `ratatui::crossterm` re-export is consistent) | **Optional in Phase 0 — defer unless skeleton uses TestBackend snapshot tests for the mock output.** See "Phase 0 dep minimalism" note below. |
| `tokio` | 1.52.x with **lean features** `["rt-multi-thread","macros","fs","time","signal"]` [VERIFIED: STACK.md release-API audit 2026-05-22] | Async runtime | `#[tokio::main]` entrypoint; Phase 0 only needs `rt` + `macros`; lean feature set guards against the `["full"]` antipattern (PITFALLS.md "tokio = full"). |
| `async-trait` | 0.1.x [CITED: docs.rs/async-trait] | `dyn`-safe async fn in trait | Required even on Rust 1.88 — native async-fn-in-trait stabilized in 1.75 but `dyn Trait` containing async fn is **not yet `dyn`-compatible**; `async_trait` boxes the returned future to make the vtable work. See Finding 8. |
| `serde` | 1.0.228 + `derive` | Serialize `ProviderState` / `HpWindow` / `ResetInfo` / `ProviderError` | Universal; needed in Phase 0 because D-14 locks serde on these. |
| `serde_json` | 1.0 | JSON round-trip for the mock test that confirms `Serialize` + `Deserialize` | Phase 0 test fixture only; full `--json` output is Phase 2. |
| `jiff` | 0.2.24 + `serde` feature [VERIFIED: docs.rs/jiff/fmt/serde 2026-05-22] | `Timestamp` in `ResetInfo` + `FetchCtx::now` | Per CONTEXT.md D-11. Use `#[serde(with = "jiff::fmt::serde::timestamp::second::required")]` per official jiff docs. |
| `anyhow` | 1.0.102 | `main()` `Result<()>`; `ProviderError::Internal(anyhow::Error)` | Standard. |
| `thiserror` | 2.0.18 | `#[derive(thiserror::Error)] for ProviderError` | Standard. |
| `owo-colors` | 4.2.x with `supports-colors` feature [VERIFIED: docs.rs/owo-colors 2026-05-22] | Optional CLI color (Phase 0 mock output likely uncolored, but the choice is locked here to prevent re-litigation in Phase 1) | See Finding 3. |

### Supporting (test-only, Phase 0)

| Library | Purpose | When to Use |
|---------|---------|-------------|
| `tokio` (already pulled) `test` macro | `#[tokio::test]` for async MockProvider round-trip | Required for `async fn fetch` test |
| `static_assertions` 1.x (OPTIONAL) | Compile-time `assert_impl_all!(Box<dyn Provider>: Send, Sync)` | Cleaner than a no-op trait-bound `fn`; pure-compile-time so no runtime cost. **Recommended.** |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `owo-colors` | `anstyle` | Smaller, but no built-in `if_supports_color(Stream::Stdout, …)` — you wire it to `std::io::IsTerminal` yourself. Choose owo-colors specifically *because* it bundles TTY + CI + `NO_COLOR`/`FORCE_COLOR` detection. |
| `owo-colors` | `nu-ansi-term` | Older API surface (forked from `ansi_term`), no built-in `if_supports_color`, no `Stream::Stdout` abstraction. Used by Nushell internally but not the broader Rust CLI consensus pick. |
| `async_trait` | Native `async fn` in trait + `Pin<Box<dyn Future + Send>>` return | Manual `Pin<Box<…>>` is what `async_trait` generates for you. Writing it by hand earns zero clarity and one annoying line per method. Stick with `async_trait`. (Finding 8.) |
| `actions-rust-lang/setup-rust-toolchain@v1` | `dtolnay/rust-toolchain@1.88.0` | Both work. The actions-rust-lang variant is the documented successor, bundles `Swatinem/rust-cache` caching + clippy/rustfmt problem matchers. **Recommended.** |
| Single binary crate | Workspace with `ahb-core` + `ahb-cli` | CONTEXT D-06 locks single binary. Don't litigate. |

### Phase 0 dep minimalism

CONTEXT.md "Claude's Discretion" says: "Default: include only what Phase 0 uses." Strict reading: drop `ratatui` from Cargo.toml until Phase 1 — Phase 0 binary doesn't render TUI, doesn't need `TestBackend`, and pulling ratatui early adds 15+ transitive crates. The text bar render uses plain `print!` + `owo-colors`.

**Recommendation:** Phase 0 Cargo.toml deps = clap, tokio (lean), async-trait, serde+derive, serde_json, jiff (with serde feature), anyhow, thiserror, owo-colors (with supports-colors feature). Dev-deps: static_assertions. **Do not** add ratatui, reqwest, rusqlite, keyring-core, directories, tracing, insta, wiremock, tempfile, assert_cmd, predicates yet. Phase 1 adds them.

### Installation

```bash
cargo new --bin ahb && cd ahb
cargo add clap --features derive
cargo add tokio --no-default-features --features rt-multi-thread,macros
cargo add async-trait
cargo add serde --features derive
cargo add serde_json
cargo add jiff --features serde
cargo add anyhow thiserror
cargo add owo-colors --features supports-colors
cargo add --dev static_assertions
```

## Package Legitimacy Audit

slopcheck was not available in this environment (Bash sandbox prevents `pip install`). All packages below are independently verified against crates.io via WebSearch + project STACK.md release-API cross-check (2026-05-22) AND drawn exclusively from the locked stack in `.planning/research/STACK.md`. Per protocol with slopcheck unavailable: planner should still gate any first-install with a `checkpoint:human-verify` step. None of these packages was discovered via low-trust channels; all are documented in the project's own STACK.md as the SOTA picks.

| Package | Registry | Age | Downloads (approx) | Source Repo | slopcheck | Disposition |
|---------|----------|-----|--------------------|-------------|-----------|-------------|
| `clap` | crates.io | 8+ yrs | ~250M total | github.com/clap-rs/clap | n/a (slopcheck unavailable) | Approved — canonical Rust CLI lib |
| `tokio` | crates.io | 7+ yrs | ~400M total | github.com/tokio-rs/tokio | n/a | Approved — canonical async runtime |
| `async-trait` | crates.io | 6+ yrs | ~200M total | github.com/dtolnay/async-trait | n/a | Approved — dtolnay-maintained |
| `serde` | crates.io | 9+ yrs | ~600M total | github.com/serde-rs/serde | n/a | Approved — universal |
| `serde_json` | crates.io | 9+ yrs | ~500M total | github.com/serde-rs/json | n/a | Approved |
| `jiff` | crates.io | <2 yrs | growing | github.com/BurntSushi/jiff | n/a | Approved — BurntSushi-maintained; pre-1.0 (see STACK.md churn note) |
| `anyhow` | crates.io | 6+ yrs | ~300M total | github.com/dtolnay/anyhow | n/a | Approved — dtolnay-maintained |
| `thiserror` | crates.io | 6+ yrs | ~250M total | github.com/dtolnay/thiserror | n/a | Approved — dtolnay-maintained |
| `owo-colors` | crates.io | 5+ yrs | high | github.com/owo-colors/owo-colors | n/a | Approved — active, no_std, zero-alloc |
| `static_assertions` | crates.io | 7+ yrs | high | github.com/nvzqz/static-assertions | n/a | Approved — pure compile-time, no postinstall |

Packages removed due to slopcheck [SLOP] verdict: none (slopcheck unavailable; manual audit only).
Packages flagged as suspicious [SUS]: none.

## Architecture Patterns

### System Architecture Diagram

```
                ┌────────────────────────┐
                │  user                  │
                │  $ cargo run --release │
                └────────────┬───────────┘
                             │
                             ▼
        ┌────────────────────────────────────────┐
        │  main.rs                               │
        │   - install Phase 0 panic hook         │   ←  D-27 contract
        │   - parse CLI (Cli::parse)             │
        │   - construct MockProvider             │
        │   - tokio runtime starts (lean feats)  │
        └────────────┬───────────────────────────┘
                     │
                     │ engine_lite::refresh_one(&mock, &ctx).await
                     ▼
        ┌────────────────────────────────────────┐
        │  src/provider/mod.rs                   │
        │   trait Provider                       │   ←  D-13
        │   struct FetchCtx { now, secrets }     │   ←  Finding 5
        └────────────┬───────────────────────────┘
                     │ async fn fetch(&self, ctx)
                     ▼
        ┌────────────────────────────────────────┐
        │  src/provider/mock.rs                  │
        │   MockProvider::fetch returns          │   ←  D-25
        │   ProviderState { id=Mock, windows=[   │
        │     HpWindow{                          │
        │       label="mock-session",            │
        │       percent_remaining=60.0,          │
        │       reset.resets_at=now+2h,          │
        │       bar_color=None                   │
        │     }]                                  │
        │   }                                    │
        └────────────┬───────────────────────────┘
                     │ Ok(ProviderState)
                     ▼
        ┌────────────────────────────────────────┐
        │  src/cli/render_text.rs                │
        │   format!(                             │
        │     "{label}  {bar} {pct}% • {reset}", │
        │     bar = build_bar(pct, charset),     │   ←  D-15/16/18
        │     reset = format_countdown(diff)     │
        │   )                                    │
        └────────────┬───────────────────────────┘
                     │ println!("…")
                     ▼
                  stdout, exit 0
```

### Recommended Project Structure (Phase 0)

```
ahb/
├── Cargo.toml                # deps locked per "Installation" above
├── clippy.toml               # allow-unwrap-in-tests=true; disallowed-methods entries (Finding 11)
├── .gitignore                # /target, **/*.rs.bk; Cargo.lock TRACKED
├── LICENSE-MIT
├── LICENSE-APACHE
├── .github/
│   └── workflows/
│       └── ci.yml            # 3-OS matrix (Finding 10)
└── src/
    ├── main.rs               # panic hook + tokio::main + parse + dispatch
    ├── cli/
    │   ├── mod.rs            # Cli (clap derive), Args struct
    │   └── render_text.rs    # build_bar + format_one_line
    ├── model.rs              # ProviderId, ProviderState, HpWindow, ResetInfo, HpUnit, BarColor, ProviderError
    ├── provider/
    │   ├── mod.rs            # trait Provider + FetchCtx
    │   └── mock.rs           # MockProvider (production-reachable, NOT cfg(test))
    ├── secrets.rs            # pub struct Secrets;  (no-op stub; Phase 1 wires keyring-core)
    └── lib.rs                # OPTIONAL — see Finding below
```

**`lib.rs` recommendation (Claude's discretion per CONTEXT.md):** Add it. Splitting into `src/lib.rs` + `src/main.rs` lets `#[cfg(test)]` integration tests under `tests/` exercise the contract types via `use ahb::model::*;`. Without `lib.rs` you'd have to do `#[path = "../src/model.rs"]` hacks. Cost: zero. This is the standard Rust CLI pattern.

### Pattern 1: Trait-First Contract

**What:** `model.rs` and `provider/mod.rs` are written and *fully tested* before any consumer exists. The MockProvider is both the spec-by-example and the runtime provider for the Phase 0 binary.

**When to use:** Phase 0 explicitly; this is the spine pattern.

**Example:**
```rust
// src/provider/mod.rs
use async_trait::async_trait;
use crate::{model::*, secrets::Secrets};

pub struct FetchCtx<'a> {
    pub now: jiff::Timestamp,
    pub secrets: &'a Secrets,
}

#[async_trait]
pub trait Provider: Send + Sync + 'static {
    fn id(&self) -> ProviderId;
    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError>;
}
```
Source: ARCHITECTURE.md § Pattern 2 (extended for Phase 0 with FetchCtx narrowed to minimal fields).

### Pattern 2: Phase 0 Panic Hook (deferred-handoff contract)

**What:** Phase 0's `main.rs` installs a panic hook that prints to stderr and returns. Phase 1's `tui::run` later calls `ratatui::init()` which itself does `take_hook()` + wrap + `set_hook()`. The Phase 0 hook ends up as the *inner* hook that fires after terminal restore.

**Why this works:** `std::panic::set_hook` is global. Whatever was installed last "wins" but standard practice is to *compose* via `take_hook()`. ratatui's documented pattern (verified at `docs.rs/ratatui/latest/ratatui/fn.init.html`): "Ensure that this method is called *after* your app installs any other panic hooks to ensure the terminal is restored before the other hooks are called." So Phase 0 installs first; Phase 1's `ratatui::init()` wraps it later. No special contract needed beyond "Phase 0 must call set_hook".

**Example:**
```rust
// src/main.rs (Phase 0)
fn install_phase0_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Phase 0: no terminal state to restore. Just print to stderr and chain.
        eprintln!("ahb panicked: {info}");
        original(info);
    }));
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    install_phase0_panic_hook();
    // …rest of Phase 0…
    Ok(())
}
```

When Phase 1 lands, `ratatui::init()` will run `take_hook()` (returning the Phase 0 hook), wrap it with terminal-restore logic, and re-install. The Phase 0 hook then runs *after* terminal restore, exactly as Pitfall 5 requires.

### Anti-Patterns to Avoid

- **Bypass the trait with `println!` in `main.rs`** — explicitly forbidden by CONTEXT.md `<specifics>` ("the bar MUST come through the locked `Provider` trait"). The skeleton's whole purpose is to prove the spine.
- **Add `crossterm` directly to `Cargo.toml`** — PITFALLS § "What NOT to Use" + STACK.md version-compat table both call out the double-version hazard. Phase 0 doesn't need crossterm at all. If a future change wants crossterm styling, route it via `ratatui::crossterm` (Phase 1).
- **`tokio = { features = ["full"] }`** — PITFALLS.md. Use the explicit lean list above.
- **`unwrap()` / `expect()` in `MockProvider::fetch`** — Phase 0 already covers adapter-style code; clippy must catch this from day 1.
- **Reading `FetchCtx::now` from `jiff::Timestamp::now()` inside the adapter** — the whole point of injecting `now` is clock determinism for tests (ARCHITECTURE.md § Testing Seam 5). MockProvider must use `ctx.now`, not call `Timestamp::now()` itself.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async-fn-in-trait + `dyn Trait` | A custom `Pin<Box<dyn Future + Send>>` macro | `async_trait` 0.1 | Rust 1.88 still doesn't make `dyn async-trait` work natively. Finding 8. |
| Terminal-aware color | A custom `is_tty + check NO_COLOR + check FORCE_COLOR + check CI` helper | `owo-colors`'s `if_supports_color(Stream::Stdout, …)` | Built-in. Finding 3. |
| Cross-platform datetime + reset countdown | Wrap `std::time` manually | `jiff::Timestamp` + `jiff::Span` | IANA TZ + reliable arithmetic. CONTEXT D-11. |
| Crate-name availability check | Visit crates.io manually | Hit `https://crates.io/api/v1/crates/<name>` and read HTTP status | 404 = available, 200 = taken. Finding 12 (verified: `ahb` returns 404). |
| Compile-time `Send + Sync + dyn-safe` assertion | A custom `fn assert_send_sync<T: Send + Sync + 'static>()` | `static_assertions::assert_impl_all!` | Standard idiom; the inline-fn pattern works too, but `assert_impl_all` is one line and intent is explicit. Finding 13. |

**Key insight:** Phase 0 should feel small. Every "let me just hand-write this little helper" instinct in this phase is a sign you're overscoping — Phase 0 is *not* the place to invent abstractions.

## Common Pitfalls

### Pitfall 1: gemini-cli built-in slash commands NOT yet supported non-interactively (the spike's go criterion)

**What goes wrong:** gemini-cli PR #8305 (merged 2025-09-19) added non-interactive *custom* command support but explicitly states "Built-in commands not supported (requires further discussion)". `/stats` is a built-in command. A naive `gemini -p "/stats"` may currently fail through the "treat unknown command as regular prompt" fallback — meaning gemini sends literal `/stats` to the LLM as a chat prompt, which produces ChatGPT-style nonsense.

**Why it happens:** Confusing "custom commands work non-interactively" (true) with "all slash commands work non-interactively" (false).

**How to avoid:** Phase 0 spike must test exactly these three invocations on the developer's box and record verbatim output:
1. `gemini -p "/stats"` (most likely-to-work canonical form)
2. `printf '/stats\n' | gemini` (stdin pipe)
3. `gemini --output-format json -p "summarize"` (verify `stats` object is populated even without `/stats` invocation — may be the actual go path)

The third path is the most promising: every non-interactive invocation already includes the `stats` object in `--output-format json` output (per gemini-cli PR #15021 + headless.md). So even if `/stats` itself doesn't work non-interactively, AHB might invoke a *trivial* prompt (e.g., `gemini -p "ok" --output-format json`) purely to harvest the `stats` block. **This option must be evaluated as part of the spike.**

**Warning signs:** spike yields a chat response instead of stats; spike output is freeform LLM text rather than structured.

### Pitfall 2: `serde_skip` on `anyhow::Error` requires more than just `#[serde(skip)]`

**What goes wrong:** `#[derive(Serialize, Deserialize)]` on an enum variant `Internal(anyhow::Error)` fails because `anyhow::Error` doesn't implement Serialize. Adding `#[serde(skip)]` fixes the *Serialize* side but breaks *Deserialize* (skip skips both; the deserializer doesn't know how to fill in the field).

**Why it happens:** CONTEXT.md says "skip + sentinel field in JSON" but doesn't spell out the exact pattern.

**How to avoid:** CONTEXT D-14 already says `ProviderError` derives `Serialize` only (no Deserialize). So we only need the Serialize side. Pattern:

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
    #[error("network: {0}")]
    Network(#[serde(serialize_with = "serialize_display")] NetworkErr),
    #[error("rate limited, retry after {retry_after:?}")]
    RateLimited { retry_after: Option<jiff::Span> },
    #[error("internal: {0}")]
    Internal(#[serde(serialize_with = "serialize_display")] anyhow::Error),
}

fn serialize_display<T: std::fmt::Display, S: serde::Serializer>(
    val: &T, ser: S,
) -> Result<S::Ok, S::Error> {
    ser.collect_str(val)
}
```

This emits `{"kind": "internal", "0": "<the display string>"}` — which is the sentinel form CONTEXT.md asked for. No `#[serde(skip)]` needed at all once you provide `serialize_with`. Source: serde "Custom Serialization" examples + thiserror issue #161 community workarounds.

### Pitfall 3: `allow-unwrap-in-tests` does NOT cover `tests/` directory

**What goes wrong:** Setting `allow-unwrap-in-tests = true` in `clippy.toml` correctly allows unwrap in `#[cfg(test)] mod tests { … }` modules. It does **not** allow unwrap in files under `tests/` (integration tests), `examples/`, or `benches/`. Source: rust-clippy issue #13981.

**Why it happens:** Files in those directories aren't `cfg(test)` from Clippy's perspective; they're separate compilation units.

**How to avoid:** Two options:
1. **Keep Phase 0 unit tests inline** in `src/provider/mock.rs` etc. via `#[cfg(test)] mod tests { … }` — then `allow-unwrap-in-tests` works.
2. If you do add `tests/` integration tests, prefix the file with `#![allow(clippy::unwrap_used)]` at the file level. This is the documented workaround.

**Phase 0 recommendation:** Keep all Phase 0 tests inline in `src/` — there's no Phase-0 integration test that warrants `tests/`.

### Pitfall 4: Verifying charset rendering on a machine without tmux/screen/Windows Terminal

**What goes wrong:** CONTEXT.md success criterion #4 says verify rendering in tmux / screen / Windows Terminal. The dev box (Arch Linux, per `uname`) has neither tmux nor screen installed (verified during research). Windows Terminal is only on Windows. So "manual eyeball in all three" is not literally possible.

**Why it happens:** Greenfield dev machine; charset-verification target list assumes a fully-loaded multiplexer setup.

**How to avoid:** Methodology that *can* be done on the dev box, plus a documented "best-effort" carve-out for the rest:

1. **Byte-level verification (always doable):** `ahb > /tmp/out.txt; xxd /tmp/out.txt | head` — expected bytes are `e2 96 88` for `█` (U+2588) and `e2 96 91` for `░` (U+2591). Verified via the research-time shell: those exact bytes round-trip cleanly. This catches encoding bugs without needing any terminal at all.
2. **Visual eyeball in the dev box's native terminal** — alacritty / kitty / GNOME Terminal / iTerm2 / xterm. Document which.
3. **Install tmux for the spike**: `pacman -S tmux` (Arch), `brew install tmux` (macOS), or `apt install tmux` (Debian-family). Then `tmux new-session 'ahb; sleep 5'` and visually confirm.
4. **Windows Terminal: best-effort.** If the user lacks a Windows box (likely), explicitly mark this as "deferred to first Windows install attempt" in `CHARSET_NOTE.md`. The CI matrix runs Windows, so a Phase 0.5 nice-to-have is `cargo test` writing the literal bar to stdout and asserting hex bytes — the CI runner becomes the Windows charset proof.

**Definition of "renders correctly"**: (a) U+2588 displays as a single column-wide solid block, (b) U+2591 displays as a single column-wide light shade, (c) the middle dot `•` (U+2022, three bytes `e2 80 a2`) does not consume two columns, (d) no replacement character (U+FFFD `ef bf bd`) appears, (e) no "tofu" (literal hex code rendered) glyph appears. PITFALLS #8 + Tmux #647 + kitty #6108 document the failure modes.

### Pitfall 5: `ratatui::init()` panic-hook composition is order-sensitive

**What goes wrong:** If Phase 1 calls `ratatui::init()` **before** any other code installs a panic hook, the ratatui hook captures the Rust default and that's all that gets composed. If Phase 0's hook is installed AFTER `ratatui::init()`, the terminal won't be restored before the Phase 0 hook prints — which doesn't matter in Phase 0 (no terminal state), but if Phase 1 ever inverts the order, it silently breaks restore.

**How to avoid:** Document the contract explicitly in `main.rs`:

```rust
// Order matters. Per ratatui docs:
//   "Ensure that ratatui::init() is called after any other panic hooks
//    are installed to ensure the terminal is restored before they run."
// Phase 0 installs its hook here. Phase 1 will later install ratatui's
// hook (via ratatui::init()) AFTER this one.
install_phase0_panic_hook();
```

Source: `docs.rs/ratatui/latest/ratatui/fn.init.html` verbatim.

## Code Examples

Verified patterns from official sources.

### `model.rs` types with serde + jiff

```rust
// src/model.rs
use std::borrow::Cow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId { Claude, Codex, Gemini, Mock }
// Finding 4: enum chosen over Cow<'static, str>. EXT-01 (v2) will introduce
// an `Other(Cow<'static, str>)` variant when 4th provider concretely planned.

pub type HpUnit = f32; // 0.0..=100.0 per CONTEXT D-10

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarColor { Red, Yellow, Green }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetInfo {
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub resets_at: jiff::Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpWindow {
    pub label: Cow<'static, str>,
    pub percent_remaining: HpUnit,
    pub reset: ResetInfo,
    pub bar_color: Option<BarColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderState {
    pub id: ProviderId,
    pub windows: Vec<HpWindow>,
    #[serde(with = "jiff::fmt::serde::timestamp::second::required")]
    pub fetched_at: jiff::Timestamp,
    pub source: &'static str,
}
```
Source: serde docs + `jiff::fmt::serde` (https://docs.rs/jiff/latest/jiff/fmt/serde/index.html).

### MockProvider + assert dyn-safety + Send + Sync + serde roundtrip in one module

```rust
// src/provider/mock.rs
use std::borrow::Cow;
use async_trait::async_trait;
use crate::{model::*, provider::{FetchCtx, Provider}};

pub struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    fn id(&self) -> ProviderId { ProviderId::Mock }

    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        let resets_at = ctx.now + jiff::Span::new().hours(2);
        Ok(ProviderState {
            id: ProviderId::Mock,
            windows: vec![HpWindow {
                label: Cow::Borrowed("mock-session"),
                percent_remaining: 60.0,
                reset: ResetInfo { resets_at },
                bar_color: None,
            }],
            fetched_at: ctx.now,
            source: "mock",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::Secrets;
    use static_assertions::assert_impl_all;

    // Compile-time assertions: Provider must be dyn-safe, Send, Sync, 'static
    assert_impl_all!(Box<dyn Provider>: Send, Sync);
    assert_impl_all!(MockProvider: Send, Sync);

    #[tokio::test]
    async fn mock_returns_expected_shape() {
        let secrets = Secrets::default();
        let now = "2026-05-22T12:00:00Z".parse::<jiff::Timestamp>().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };

        let state = MockProvider.fetch(&ctx).await.unwrap();

        assert_eq!(state.id, ProviderId::Mock);
        assert_eq!(state.windows.len(), 1);
        let w = &state.windows[0];
        assert_eq!(w.label, "mock-session");
        assert!((w.percent_remaining - 60.0).abs() < f32::EPSILON);
        let two_hours = jiff::Span::new().hours(2);
        assert_eq!(w.reset.resets_at, now + two_hours);
    }

    #[tokio::test]
    async fn provider_state_serde_roundtrip() {
        let secrets = Secrets::default();
        let now = "2026-05-22T12:00:00Z".parse::<jiff::Timestamp>().unwrap();
        let ctx = FetchCtx { now, secrets: &secrets };
        let state = MockProvider.fetch(&ctx).await.unwrap();

        let json = serde_json::to_string(&state).unwrap();
        let back: ProviderState = serde_json::from_str(&json).unwrap();
        assert_eq!(state.id, back.id);
        assert_eq!(state.windows.len(), back.windows.len());
        assert_eq!(state.fetched_at, back.fetched_at);
    }
}
```
Source: ARCHITECTURE.md § Testing Seams + static_assertions docs + tokio test docs.

### Phase 0 `main.rs` — full skeleton

```rust
// src/main.rs
use clap::Parser;
use owo_colors::{OwoColorize, Stream::Stdout};

use ahb::{
    cli::render_text,
    model::ProviderId,
    provider::{FetchCtx, Provider},
    provider::mock::MockProvider,
    secrets::Secrets,
};

#[derive(Parser)]
#[command(version, about = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance")]
struct Cli {
    /// Force ASCII charset (uses '#' / '-' instead of '█' / '░')
    #[arg(long)]
    ascii: bool,
    /// Color mode
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    color: ColorMode,
}

#[derive(Copy, Clone, clap::ValueEnum)]
enum ColorMode { Auto, Always, Never }

fn install_phase0_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("ahb panicked: {info}");
        original(info);
    }));
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    install_phase0_panic_hook();
    let cli = Cli::parse();
    let secrets = Secrets::default();
    let ctx = FetchCtx { now: jiff::Timestamp::now(), secrets: &secrets };

    let mock = MockProvider;
    let state = mock.fetch(&ctx).await
        .map_err(|e| anyhow::anyhow!("mock provider failed: {e}"))?;

    let line = render_text::compact_line(&state, &ctx.now, cli.ascii);
    println!("{line}");
    Ok(())
}
```

Note: `flavor = "current_thread"` is the smallest tokio runtime — Phase 0 doesn't need multi-thread. This drops the `rt-multi-thread` feature requirement; can swap to `["rt","macros"]` for an even smaller dep tree if preferred. Either is correct.

### `cli/render_text.rs` — bar builder

```rust
// src/cli/render_text.rs
use crate::model::ProviderState;

const BAR_WIDTH: usize = 10; // CONTEXT D-16

pub fn compact_line(state: &ProviderState, now: &jiff::Timestamp, ascii: bool) -> String {
    debug_assert_eq!(state.windows.len(), 1,
        "Phase 0 mock returns exactly one window; multi-window rendering is Phase 1");
    let w = &state.windows[0];
    let pct = w.percent_remaining.clamp(0.0, 100.0);
    let filled = (pct * BAR_WIDTH as f32 / 100.0).round() as usize;
    let bar = if ascii {
        format!("{}{}", "#".repeat(filled), "-".repeat(BAR_WIDTH - filled))
    } else {
        format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(BAR_WIDTH - filled))
    };
    let countdown = format_countdown(now, &w.reset.resets_at);
    let sep = if ascii { '|' } else { '\u{2022}' }; // CONTEXT specifics: ASCII subs '|' for •
    format!("{label}  {bar} {pct}% {sep} resets in {countdown}",
        label = w.label, pct = pct.round() as u32)
}

fn format_countdown(now: &jiff::Timestamp, target: &jiff::Timestamp) -> String {
    let span = target.since(*now).unwrap_or_default();
    let h = span.get_hours();
    let m = span.get_minutes();
    format!("{h}h{m:02}m")
}
```

### `clippy.toml` — Phase 0 lint configuration

```toml
# clippy.toml — Phase 0
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true

# Forbid direct crossterm dependency at the source level (PITFALLS.md).
# All terminal styling goes through owo-colors or (later) ratatui::crossterm.
disallowed-types = [
    { path = "crossterm::*", reason = "use owo-colors (Phase 0) or ratatui::crossterm (Phase 1+) — see PITFALLS.md double-version hazard" },
]
```

And in `src/main.rs` (top of file):

```rust
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
```

`tests/` directory is empty in Phase 0 (Pitfall 3 above). Test code lives inline as `#[cfg(test)] mod tests { … }` where `allow-unwrap-in-tests` works correctly.

### `.github/workflows/ci.yml` — 3-OS matrix

```yaml
# .github/workflows/ci.yml
name: ci
on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    name: ${{ matrix.os }} / ${{ matrix.rust }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable]
    steps:
      - uses: actions/checkout@v6
      - name: Install toolchain
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: ${{ matrix.rust }}
          components: clippy, rustfmt
      - name: Build
        run: cargo build --all-targets
      - name: Test
        run: cargo test --all-targets
      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
```

**Windows notes (documented for Phase 3 even though Phase 0 doesn't ship rusqlite):**
- `actions/checkout@v6` defaults `core.autocrlf=true` on Windows; if any code uses byte-counted parsing, set `git config --global core.autocrlf false` before checkout.
- `rusqlite` with `bundled` feature compiles SQLite from C source; Windows needs `cc` toolchain — `actions-rust-lang/setup-rust-toolchain` doesn't install MSVC Build Tools but `windows-latest` already has them.
- Phase 0 has no Windows-specific gotchas because we have no FS adapters yet.

Sources: actions-rust-lang/setup-rust-toolchain README + rust-cli-recommendations.sunshowers.io. The `dtolnay/rust-toolchain@1.88.0` form (CONTEXT D-04 pin) is acceptable but `actions-rust-lang/setup-rust-toolchain@v1` is the documented successor with built-in `Swatinem/rust-cache` and problem matchers.

### `Cargo.toml` skeleton

```toml
# Cargo.toml — Phase 0
[package]
name = "ahb"
version = "0.0.1"
edition = "2024"
rust-version = "1.88"
license = "MIT OR Apache-2.0"
description = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance"
repository = "https://github.com/<user>/ahb"   # fill before publish
readme = "README.md"
keywords = ["claude", "codex", "gemini", "cli", "tui"]
categories = ["command-line-utilities"]

[dependencies]
clap = { version = "4.6", features = ["derive"] }
tokio = { version = "1.52", default-features = false, features = ["rt", "macros"] }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
jiff = { version = "0.2", features = ["serde"] }
anyhow = "1"
thiserror = "2"
owo-colors = { version = "4", features = ["supports-colors"] }

[dev-dependencies]
static_assertions = "1"

[profile.release]
# Phase 4 will tune more. Phase 0 default is fine.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `tui-rs` (fdehau/tui-rs) | `ratatui` 0.30 | 2023-08 archived | Not in Phase 0 deps — but documented to prevent regression in plans. |
| `keyring` v4 as library | `keyring-core` 1.0 | 2026-04 | Not in Phase 0 deps; Phase 1 will pull `keyring-core`. |
| `async-std` | `tokio` | 2025 discontinued | Phase 0 uses tokio. |
| `dtolnay/rust-toolchain` | `actions-rust-lang/setup-rust-toolchain@v1` | community-standard since ~2024 | Phase 0 CI uses the modern variant. |
| `#[async_trait]` | Native `async fn` in trait | **NOT YET** — native variant not `dyn`-compatible at 1.88 | Phase 0 still uses `async_trait`. Finding 8. |

**Deprecated/outdated (do NOT use):**
- `chrono` for new projects (jiff is the 2026 pick) — but `chrono` 0.4.44 remains the fallback per STACK.md if jiff churn becomes painful.
- `mockito` for HTTP mocks (use `wiremock` — Phase 3 concern).
- Manual `Pin<Box<dyn Future + Send>>` in trait methods (use `async_trait`).

## Plan-Level Implications

> **This section is the deliverable the planner cares about most. Suggested wave/plan breakdown — planner is free to override.**

Phase 0 cleanly decomposes into **three parallel-ish workstreams** plus a small **integration wave**. Each workstream is small enough to be a single plan; the planner may also choose to bundle them.

### Plan 01 — Repo Scaffold & CI Skeleton (no Rust code yet)
- `cargo new --bin ahb`
- `Cargo.toml` per § "Cargo.toml skeleton" above
- `LICENSE-MIT` + `LICENSE-APACHE` (standard text)
- `.gitignore` per D-07
- `clippy.toml` per § "clippy.toml — Phase 0"
- `.github/workflows/ci.yml` per § "3-OS matrix"
- Empty `src/lib.rs` + `src/main.rs` that just prints "TODO" — but compiles, tests, and clippies clean
- Push branch, verify the 3-OS matrix actually goes green before any Rust code lands
- **Done when:** CI is green on a "Hello world" binary across all three OSes, clippy with `-D warnings` is green, MSRV pin is honored.

### Plan 02 — `src/model.rs` + `src/provider/mod.rs` + `src/secrets.rs` (the spine)
- Translate § "Code Examples — `model.rs` types with serde + jiff" verbatim
- `ProviderError` with `serialize_with = "serialize_display"` per Pitfall 2
- `FetchCtx { now, secrets: &Secrets }`
- `Secrets { /* empty stub */ }` with `#[derive(Default)]`
- Unit tests inline: `assert_impl_all!(ProviderState: Send, Sync)`, `assert_impl_all!(Box<dyn Provider>: Send, Sync)`, serde round-trip for `ProviderState`
- **Done when:** the contract types compile, the dyn-safety + Send + Sync assertions hold, `ProviderState` round-trips through `serde_json`.

### Plan 03 — `src/provider/mock.rs` + `src/cli/render_text.rs` + `src/main.rs` (the skeleton binary)
- Translate § "Code Examples — MockProvider" verbatim
- Translate § "Code Examples — `cli/render_text.rs`" verbatim
- Translate § "Code Examples — Phase 0 `main.rs`" verbatim
- `cargo run` prints the literal `mock-session  ██████░░░░ 60% • resets in 2h00m` (the `2h00m` will actually be computed from `now + 2h` so it'll always be exactly `2h00m` — that's intentional for D-25's deterministic output)
- Hex-byte verification: `cargo run > /tmp/out.txt && xxd /tmp/out.txt | head` shows `e2 96 88` and `e2 96 91` bytes
- **Done when:** `cargo run --release` prints the exact CONTEXT-D-25 line and exits 0; clippy clean; tests green.

### Plan 04 — Gemini Local-Capture Spike (the gating memo)
- This is *not* code. It produces `.planning/research/GEMINI_SPIKE.md`.
- Required steps (per CONTEXT D-23 sections):
  1. **Method** — record OS / gemini-cli version / auth mode (`gemini --version`)
  2. **Attempt 1:** `gemini -p "/stats"` — capture verbatim output. Likely fails per PR #8305 limitation.
  3. **Attempt 2:** `printf '/stats\n' | gemini` — capture verbatim.
  4. **Attempt 3:** `gemini -p "what is 2+2" --output-format json` — capture verbatim, look at the `stats` object structure. **This is the most-likely-to-succeed path.** AHB invokes a trivial prompt purely to harvest the `stats` block.
  5. **Parse-feasibility assessment** — for whichever attempt yielded useful data: is it JSON (best), structured text (regex-parseable), or freeform (no-go)?
  6. **Go/No-Go decision** — single line at top of memo.
  7. **Kill criteria** — what would make this no-go in the future (e.g., `stats` object stops including `requestCount`, OAuth users lose access, etc.).
  8. **Phase 3 hand-off** — if go: list the exact command + parsing strategy for the future ADP-05 adapter. If no-go: describe the `--experimental-gemini` stub flag.
  9. **Appendix** — rationale for NOT spiking the web fallback (per D-22; ToS / account-ban asymmetry).
- **Charset verification subsection** (per D-26) — record `xxd` output of the Plan 03 binary's output, and whatever terminal-eyeball verification the developer can perform. Mark Windows Terminal as "best-effort — CI matrix run on `windows-latest` is the proof".
- **Done when:** memo committed; go/no-go decision is unambiguous; Phase 3 readers can act on it.

### Plan 05 (OPTIONAL Integration Wave) — End-to-End Smoke
- Run all CI on the final tree.
- Verify the Phase 0 success criteria checklist (5 items from ROADMAP.md § Phase 0):
  1. `GEMINI_SPIKE.md` exists with go/no-go.
  2. `cargo build --release && ./target/release/ahb` prints the locked line through the trait.
  3. `cargo test` green.
  4. Charset rendering verified (best-effort) and documented.
  5. CI skeleton runs on push/PR.
- **Done when:** the checklist is fully ticked, with audit trail (links to CI runs, byte-level verification output, memo).

### Dependencies between plans
- Plan 01 has no deps.
- Plan 02 depends on Plan 01 (needs `Cargo.toml`).
- Plan 03 depends on Plan 02 (uses the contract types).
- Plan 04 is independent of Plans 02/03 — could run in parallel. It DOES depend on Plan 01 only in the sense that the developer needs the repo to exist to commit the memo into.
- Plan 05 depends on all of the above.

### Recommendation
**Critical path:** 01 → 02 → 03 → 05. Plan 04 runs in parallel with 02+03. If solo, total work is small enough (~half a day) that linear execution 01 → 02 → 03 → 04 → 05 is fine.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | gemini-cli `--output-format json` returns a `stats` object even when the prompt is unrelated to stats (so a trivial prompt can harvest it) | Pitfall 1, Plan 04 Attempt 3 | If wrong, the spike still has Attempts 1+2 to fall back to; if all three fail, this is a clean no-go answer (D-22) — no false-positive risk. |
| A2 | `actions-rust-lang/setup-rust-toolchain@v1` is the right successor to `dtolnay/rust-toolchain` for this project | Standard Stack | LOW. Both work. If actions-rust-lang has a regression, swap to dtolnay form documented in the same section. |
| A3 | `clippy::unwrap_used` `restriction`-lint default of `allow` means we must opt in via `#![deny(clippy::unwrap_used)]`, and `allow-unwrap-in-tests = true` in clippy.toml correctly exempts `#[cfg(test)]` modules but NOT `tests/` directory files | Pitfall 3 + clippy.toml example | LOW — clippy issue #13981 confirms the limitation. We mitigate by keeping Phase 0 tests inline. |
| A4 | `jiff::Timestamp::now()` works on all three CI matrix OSes without special features | Code Examples | LOW — jiff is `std`-compatible by default; `now()` is portable. |
| A5 | `tokio` with `["rt", "macros"]` (single-threaded current_thread runtime) suffices for Phase 0 | main.rs example | LOW — Phase 0 has exactly one await chain. `current_thread` is the lightest option. |
| A6 | `owo-colors`'s `if_supports_color(Stream::Stdout, …)` correctly honors `NO_COLOR` + `IsTerminal` on all three OSes | Standard Stack | LOW — verified in owo-colors README; the `supports-colors` feature wraps the upstream `supports-color` crate which does the work. |
| A7 | `ProviderId` as a closed enum is the right v1 shape (vs. `Cow<'static, str>` newtype) | Code Examples | LOW. V1 has a closed set (Claude / Codex / Gemini / Mock). v2 EXT-01 will add `Other(Cow<'static, str>)` if more providers ship. Closed-enum gives exhaustiveness checking in match arms (the CLI renderer + JSON serializer benefit), which the newtype loses. |
| A8 | `cargo run` on first invocation will not be rate-limited or blocked by the user's terminal emulator's font lacking U+2588 | Pitfall 4 / charset verification | LOW — every modern monospaced font ships block elements. If absent, the user falls back to `--ascii` (D-18) which is explicitly designed for this. |
| A9 | The `gemini -p "/stats"` path is currently broken (per PR #8305 limitation) but Attempt 3 (`--output-format json`) is plausible. | Pitfall 1 + Plan 04 | MEDIUM. This is the largest unknown. The spike is *designed* to resolve it; if all three attempts fail, that's a clean no-go decision per D-22. |

## Open Questions

1. **Does `gemini -p "/stats"` work in the current installed version of gemini-cli on the developer's box, or does PR #8305's "built-in commands not supported" limitation block it?**
   - What we know: PR #8305 merged 2025-09-19; states custom commands work non-interactively but built-in commands do not.
   - What's unclear: whether `/stats` has been backfilled in a later release; whether `--output-format json` populates a `stats` block on any trivial prompt.
   - Recommendation: the spike (Plan 04) IS the resolution. Don't try to answer from docs.

2. **Does the gemini-cli `stats` object include a reset-window boundary or only per-call cumulative tokens?**
   - What we know: PR #15021 (raw input token counts) + headless.md (response/stats/error structure) confirm token counts exist; cached-token discrimination exists; per-model breakdowns exist.
   - What's unclear: whether any field encodes the "5-hour rolling session" or "weekly cap" that the AHB UX needs. If only cumulative tokens are exposed, the adapter must compute rolling-window math from local session-log timestamps + a known per-tier limit table — a heavier lift.
   - Recommendation: the spike captures three sample outputs (per Plan 04), spec'd in `GEMINI_SPIKE.md`. Phase 3 adapter design depends on this answer.

3. **Should Phase 0 binary's mock output ever exercise the `bar_color` field?**
   - What we know: CONTEXT D-09 declares the field; D-25 sets `bar_color = None` for the mock.
   - What's unclear: whether Plan 03's renderer should *match* on `bar_color` and skip the `None` arm, or whether the field is wholly unused in Phase 0.
   - Recommendation: declare the field, set it to `None` in the mock, *don't* wire renderer logic for it yet. Phase 1/2 UI plans (where `bar_color` actually means something) own that.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / `rustc` | All Rust work | Assumed (project is Rust) | (developer to verify ≥1.88) | None — blocking. |
| `gemini` CLI | Plan 04 spike | Unknown — developer-specific | — | If absent: install per upstream docs, OR mark spike no-go for "tool unavailable" and ship Gemini as v2 stub (D-22). |
| `tmux` | Pitfall 4 charset verification | ✗ on dev box (verified `which tmux` returned not-found) | — | (1) Install via `pacman -S tmux`; (2) Skip tmux check, rely on `xxd` byte-level verification + CI matrix run on `windows-latest` as Windows charset proof. |
| `screen` | Pitfall 4 charset verification | ✗ on dev box | — | Same as tmux: install or skip with documented carve-out. |
| Windows Terminal | Pitfall 4 charset verification | ✗ on Linux dev box | — | CI `windows-latest` matrix step provides byte-level proof; visual proof deferred to first Windows install attempt. Mark "best-effort" in `CHARSET_NOTE.md`. |
| `xxd` | Charset verification methodology | ✓ (verified during research) | (system) | `od -c` is equivalent. |
| `gh` CLI | (optional) for PR creation | ✓ assumed | — | Use web UI. |

**Missing dependencies with no fallback:**
- None blocking. Worst case: spike yields no-go (clean outcome per D-22).

**Missing dependencies with fallback:**
- tmux/screen/Windows Terminal — best-effort visual proof, byte-level proof is the binding criterion.

## Validation Architecture

`.planning/config.json` does not exist; `workflow.nyquist_validation` is therefore absent. Default = enabled. Including this section.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` (built-in) + `#[tokio::test]` for async + `static_assertions` for compile-time invariants |
| Config file | none — Phase 0 uses inline tests |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test --all-targets` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ADP-00 | `Provider` trait + types compile and are dyn-safe | compile-time | `cargo build` | ❌ Wave 0 — `src/provider/mod.rs`, `src/model.rs` |
| ADP-00 | `Box<dyn Provider>` is `Send + Sync + 'static` | compile-time (static_assertions) | `cargo test --lib` | ❌ Wave 0 — `src/provider/mock.rs` inline `#[cfg(test)]` |
| ADP-00 | `MockProvider::fetch` returns expected `ProviderState` shape | unit (tokio async) | `cargo test --lib mock_returns_expected_shape` | ❌ Wave 0 — `src/provider/mock.rs` inline `#[cfg(test)]` |
| ADP-00 | `ProviderState` round-trips through `serde_json` | unit (tokio async) | `cargo test --lib provider_state_serde_roundtrip` | ❌ Wave 0 — `src/provider/mock.rs` inline `#[cfg(test)]` |
| Phase 0 SC #2 | `cargo run --release` prints exactly the locked HP-bar line | smoke (manual + CI) | `cargo run --release | head -1` (compare against fixture) | ❌ Wave 0 — could be `tests/smoke.rs` with `assert_cmd`, but `assert_cmd` is Phase 1 dep. Phase 0 verifies manually. |
| Phase 0 SC #3 | `cargo test` green | suite | `cargo test --all-targets` | ❌ Wave 0 — tests must be written |
| Phase 0 SC #4 | Charset bytes are `e2 96 88` and `e2 96 91` | manual byte-level | `cargo run --release > /tmp/out.txt && xxd /tmp/out.txt \| head` | n/a — manual step, recorded in `GEMINI_SPIKE.md` |
| Phase 0 SC #5 | `cargo clippy -- -D warnings` is green | lint | `cargo clippy --all-targets -- -D warnings` | ❌ Wave 0 — needs `clippy.toml` + `#![deny(...)]` in `main.rs` |

### Sampling Rate
- **Per task commit:** `cargo test --lib && cargo clippy -- -D warnings`
- **Per wave merge:** `cargo test --all-targets && cargo clippy --all-targets -- -D warnings && cargo build --release`
- **Phase gate:** All 5 Phase 0 success criteria checked manually; CI 3-OS matrix green on the final commit; `GEMINI_SPIKE.md` committed.

### Wave 0 Gaps
- [ ] `src/lib.rs` — re-export `model`, `provider`, `secrets`, `cli` for integration tests
- [ ] `src/model.rs` — contract types (Plan 02)
- [ ] `src/provider/mod.rs` — trait + FetchCtx (Plan 02)
- [ ] `src/provider/mock.rs` — MockProvider + inline `#[cfg(test)] mod tests` (Plan 03)
- [ ] `src/secrets.rs` — stub `Secrets` struct (Plan 02)
- [ ] `src/cli/render_text.rs` — `compact_line` + `format_countdown` (Plan 03)
- [ ] `src/main.rs` — panic hook + tokio::main + dispatch (Plan 03)
- [ ] `clippy.toml` — `allow-unwrap-in-tests = true` + `disallowed-types` for crossterm (Plan 01)
- [ ] `.github/workflows/ci.yml` — 3-OS matrix (Plan 01)
- [ ] Framework install: nothing extra — `cargo test` is built-in; `static_assertions = "1"` and `tokio = { features = ["macros"] }` are already in dev-deps and deps respectively (Plan 01)

## Security Domain

`.planning/config.json` is absent; `security_enforcement` defaults to enabled. Phase 0 has minimal security surface (no network calls, no credential storage, no user input parsing beyond `clap` flag whitelist), but the domain checklist is included for completeness.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no — Phase 0 has no auth | n/a (Phase 1 wires `keyring-core` per CONTEXT canonical_refs) |
| V3 Session Management | no | n/a |
| V4 Access Control | no | n/a |
| V5 Input Validation | yes (minimal) | `clap` enforces `--color` enum + `--ascii` flag; no free-form user input in Phase 0 |
| V6 Cryptography | no | n/a |
| V7 Error Handling and Logging | yes | `ProviderError` is a closed enum; `Internal(anyhow::Error)` serializes only the Display string (no stack traces / secrets in JSON) — Pitfall 2 |
| V14 Configuration | yes (build-time only) | `cargo` + `clippy.toml` are the only config; no runtime config in Phase 0 |

### Known Threat Patterns for the Phase 0 stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Panic leaves terminal in unusable state | DoS-against-user | `std::panic::set_hook` per D-27 + ratatui::init composition contract (Pitfall 5) |
| Adapter `unwrap()` panic crashes whole tool | DoS | `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` in `main.rs` + tests/Phase 1 add `catch_unwind` boundary |
| `anyhow::Error` Debug-print leaks call stack to JSON output | Info disclosure | Pitfall 2 — `serialize_with = "serialize_display"` emits only the Display string |
| Slopsquatted dep at `cargo add` time | Supply chain | All Phase 0 deps independently verified against project STACK.md release-API audit (see Package Legitimacy Audit) |
| Phase 0 binary inadvertently includes test-only `MockProvider` in release build | Surface widening | `MockProvider` is intentionally in `src/provider/mock.rs` (not `tests/`) because Phase 0 *needs* it at runtime; Phase 1+ flag-gates real-vs-mock via config |

## Sources

### Primary (HIGH confidence)

- [ratatui::init() docs.rs page](https://docs.rs/ratatui/latest/ratatui/fn.init.html) — verbatim panic-hook ordering contract.
- [Ratatui Panic Hooks recipe](https://ratatui.rs/recipes/apps/panic-hooks/) — `take_hook` + `set_hook` composition pattern.
- [jiff::fmt::serde module docs](https://docs.rs/jiff/latest/jiff/fmt/serde/index.html) — `#[serde(with = "jiff::fmt::serde::timestamp::second::required")]`.
- [owo-colors README + lib.rs](https://github.com/owo-colors/owo-colors) + [lib.rs/crates/owo-colors](https://lib.rs/crates/owo-colors) — `Stream::Stdout` + `if_supports_color` + `NO_COLOR/FORCE_COLOR` semantics.
- [Rain's Rust CLI recommendations — Managing colors](https://rust-cli-recommendations.sunshowers.io/managing-colors-in-rust.html) — official-style recommendation for owo-colors.
- [actions-rust-lang/setup-rust-toolchain](https://github.com/actions-rust-lang/setup-rust-toolchain) — modern successor to dtolnay/rust-toolchain with bundled caching.
- [rust-clippy issue #13981](https://github.com/rust-lang/rust-clippy/issues/13981) — `allow-unwrap-in-tests` does not cover `tests/` directory.
- [Clippy Configuration docs](https://doc.rust-lang.org/nightly/clippy/configuration.html) — `clippy.toml` syntax + lint configuration mechanics.
- [crates.io API for `ahb` returns HTTP 404](https://crates.io/api/v1/crates/ahb) — verified during research; name is available.
- [async_trait crate docs.rs](https://docs.rs/async-trait) — `dyn`-safe async trait macro.
- [Rust async-fn-in-trait initiative — dyn_async_trait roadmap](https://rust-lang.github.io/async-fundamentals-initiative/roadmap/dyn_async_trait.html) — confirms native async-fn-in-trait still NOT `dyn`-compatible at 1.88.

### Primary (HIGH-MEDIUM) — Gemini CLI

- [gemini-cli PR #8305 (merged 2025-09-19)](https://github.com/google-gemini/gemini-cli/pull/8305) — non-interactive custom commands; explicit "built-in commands not supported".
- [gemini-cli PR #15021](https://github.com/google-gemini/gemini-cli/pull/15021) — `--output-format json` includes raw input token counts in stats.
- [gemini-cli headless docs](https://google-gemini.github.io/gemini-cli/docs/cli/headless.html) — `--output-format json` returns `response` + `stats` + `error`.
- [gemini-cli commands docs](https://google-gemini.github.io/gemini-cli/docs/cli/commands.html) — `/stats` displays "token usage, cached token savings, session duration".
- [gemini-cli issue #16567 (open, p2, filed 2026-01-14)](https://github.com/google-gemini/gemini-cli/issues/16567) — non-interactive hang regression; signals fragility in headless flows.

### Secondary (MEDIUM confidence)

- [thiserror issue #161](https://github.com/dtolnay/thiserror/issues/161) — `Serialize` workaround for enum with `#[from]` external errors; `serialize_with = "serialize_display"` pattern.
- [serde-error crate](https://docs.rs/serde-error) — pre-built option for serializing `anyhow::Error`; documented as alternative to manual `serialize_with`.
- [Schneems on clippy.toml `disallowed-methods`](https://www.schneems.com/2025/11/19/find-accidental-code-usage-with-a-custom-clippytoml/) — practical clippy.toml examples.
- [Tmux #647](https://github.com/tmux/tmux/issues/647) — emoji width pitfall (PITFALLS.md cross-ref).
- [Windows Terminal #5973](https://github.com/microsoft/terminal/issues/5973) — tmux + unicode wrap issue (Pitfall 4 cross-ref).

### Tertiary (LOW confidence — domain inference)

- The "Attempt 3" idea (harvest `stats` block from a trivial prompt) is a derivation from PR #15021 + headless.md, not a documented pattern; spike will confirm or invalidate.
- Closed-enum vs `Cow<'static, str>` for `ProviderId` — Rust API Guidelines § "Type Safety" supports closed enums for closed sets, but no canonical citation for this exact pattern.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all deps verified against project STACK.md release-API audit (2026-05-22) + this session's cross-checks.
- Architecture: HIGH — single-binary skeleton + trait-first contract is straightforward; ARCHITECTURE.md already covers the spine.
- Panic-hook + ratatui composition: HIGH — verbatim from docs.rs.
- Clippy config / CI / `Cargo.toml`: HIGH — boilerplate verified against authoritative sources.
- `ProviderId` shape recommendation: MEDIUM-HIGH — judgment call, not a documented best-practice; LOW risk of needing to reverse in v2.
- `FetchCtx` minimal fields: MEDIUM — CONTEXT.md says "at minimum now + secrets; Claude's discretion for more"; I recommend stopping at exactly those two for Phase 0 and adding `http: &reqwest::Client` in Phase 1 when the first network adapter needs it. Lower confidence because adding a field later is a breaking ABI change to any external user of `Provider::fetch` — but in v1 there are no external users (only AHB's own adapters), so additive change is safe.
- Gemini spike feasibility: MEDIUM — empirical question; this research surfaces three concrete attempts to try (Plan 04) and the most-likely-to-succeed path (Attempt 3).
- Charset verification on dev box: HIGH on the byte-level check (`xxd` works), MEDIUM on visual verification (tmux not installed; install or document carve-out).

**Research date:** 2026-05-22
**Valid until:** 2026-06-22 (30 days — stable Rust ecosystem; gemini-cli is the only moving piece and Plan 04 must be re-checked if release cadence delivers anything new in that window).

## RESEARCH COMPLETE
