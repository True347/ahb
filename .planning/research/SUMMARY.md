# Project Research Summary

**Project:** AHB (AI HP Bar)
**Domain:** Rust CLI + TUI single-binary multi-provider LLM-subscription usage tracker (HP-bar metaphor: reset-based session %, not only-goes-up dashboard)
**Researched:** 2026-05-22
**Confidence:** MEDIUM-HIGH overall (HIGH on stack/architecture/Claude+Codex adapters; MEDIUM-LOW on Gemini — gated behind a Phase 0 spike with kill criteria)

## Executive Summary

AHB sits in a **fragmented competitive landscape**: the single-provider Claude Code tool space is dense (ccusage, ccburn, Maciek-roboblog, claude-code-usage-bar, etc.) and sets a high feature bar (themes, daemons, statusline hooks, JSON output); the multi-provider space is essentially empty for the *subscription-session-with-reset* use case. AHB's white space is therefore **the unified multi-provider HP bar across Claude Code + Codex CLI + Gemini CLI in one view** — not feature richness within any one provider. The Core Value (HP-bar visual + reset countdown, reset-based not cumulative) is the discriminator; resist scope creep toward ccusage-style cumulative dashboards.

Architecturally the project converges on a single well-understood pattern lifted from codex-rs: **one binary, two front-ends (cli + tui), one shared UI-agnostic engine, three async `Provider` trait implementations**. The engine exposes `refresh_all()` (one-shot for CLI) and `subscribe()` (mpsc stream for TUI), runs adapters in `Vec<Result<...>>` Starship-style with per-adapter timeout + cache fallback so one provider failing degrades gracefully rather than blanking the screen. Stack picks are mostly canonical 2026 Rust: clap derive + ratatui 0.30 + tokio (lean features) + reqwest 0.13 rustls + rusqlite read-only + serde/toml + jiff + tracing + anyhow/thiserror, with snapshot tests via insta + ratatui TestBackend and HTTP mocks via wiremock. Three stack choices are non-obvious and critical-path: **`keyring-core` 1.0 (NOT the demoted `keyring` v4 facade)**, **`ratatui::crossterm` re-export (NOT a parallel direct crossterm dep — silent rendering breakage)**, and **`jiff` over chrono** for IANA-TZ-aware reset arithmetic. `tui-rs` and `async-std` are both dead; do not start there.

The dominant project risk is **Gemini account-ban exposure**: scraping `gemini.google.com/usage` at a 15s tick with the user's real Google session cookie is plausibly a ToS violation under Google's GenAI Prohibited Use Policy, with asymmetric blast radius (the whole Google identity, not just AHB). This makes **Phase 0 a non-negotiable spike with a written go/no-go memo** — Gemini ships in v1 only if the spike finds a low-rate, low-fingerprint path; otherwise Gemini is deferred to v2 behind an opt-in flag with a giant warning, or AHB prefers reading the user's own local `gemini /stats` output instead. Beyond Gemini, three secondary risks dominate Phase 1: Claude `stats-cache.json` is a cache not a source of truth (the real 5h/weekly accounting lives in `~/.claude/projects/**/*.jsonl` — naïve parsers ship wrong numbers); Codex `state_5.sqlite` is a live versioned DB that must be opened read-only with `busy_timeout` and dynamic version discovery; and OS-keyring credential storage (`keyring-core` + `Secret<T>` newtype) must land before any credential-bearing adapter — never plaintext TOML. Terminal-restore-on-panic must be wired in before any feature code so a development `unwrap()` doesn't trash the user's shell.

## Key Findings

### Recommended Stack

Mainstream Rust 2026: clap + ratatui + tokio + reqwest + rusqlite + serde, all current-major and verified via GitHub release API on 2026-05-22. The handful of non-obvious picks are load-bearing and worth surfacing upfront. See `STACK.md` for full rationale and version compatibility matrix.

**Core technologies:**
- **Rust** (edition 2021, MSRV ≥ 1.88) — constrained by ratatui 0.30 MSRV; single static binary distribution.
- **clap** 4.6.1 (derive) — subcommands (`AHB`, `AHB tui`), auto-help, completions; standard pick.
- **ratatui** 0.30.0 (with `ratatui::crossterm` re-export) — the actively-maintained TUI; `tui-rs` is archived; **do not add a parallel `crossterm` dep — two versions silently break rendering**.
- **tokio** 1.52 with lean feature set (`rt-multi-thread, macros, fs, time, signal`) — required by reqwest; explicitly NOT `["full"]`.
- **reqwest** 0.13.3 with `rustls-tls, cookies, json` (no default features) — rustls keeps static binary clean; cookies needed if Gemini auth uses session cookies.
- **rusqlite** 0.39.0 with `["bundled"]` — read-only access to Codex `state_5.sqlite`; static SQLite link, no system dep.
- **serde** + **serde_json** + **toml** — JSON / JSONL / TOML config; no framework, just `from_str`.
- **jiff** 0.2.24 — new project, IANA TZ + reset-window arithmetic = jiff's sweet spot; isolate behind a `Clock` trait against pre-1.0 churn.
- **keyring-core** 1.0.0 — **NOT the `keyring` crate v4**, which has been downgraded to a demo/CLI shell. Cross-platform secret storage (macOS Keychain / Win Cred Manager / Linux Secret Service).
- **tracing** + **tracing-subscriber** — structured logging; future-proof for daemon mode.
- **anyhow** + **thiserror** — app-level errors + library-level enum.
- **insta** + **ratatui::backend::TestBackend** — snapshot tests for CLI text and TUI widget output.
- **wiremock** 0.6.5 — async-native HTTP mocking for the Gemini adapter.
- **cargo-dist** 0.32.0 — multi-arch release artifacts + GH Actions workflow + installer scripts.

**Things to actively avoid:** `tui-rs` (archived 2023), `keyring` v4 as a library dep, `async-std` (discontinued), parallel `crossterm` dep alongside ratatui, `tokio = { features = ["full"] }`, `sqlx` for read-only Codex DB, native-tls / OpenSSL.

### Expected Features

**Must have (table stakes — v1 launch):**
- Compact one-line HP-bar per provider as the default `AHB` output (the entry point)
- `--compact` / `--detailed` / `--json` flags with a stable `schema_version`-tagged JSON shape
- `AHB tui` mode with configurable auto-refresh (default 15s)
- TOML config file enumerating providers + auth sources
- Claude Code + Codex CLI + Gemini CLI adapters (Gemini conditional on Phase 0 spike outcome)
- Reset countdown rendered per window (this is half the Core Value)
- Per-adapter graceful failure — one provider down ≠ whole tool down
- Auto-detection / skip-missing for un-configured providers
- Color with auto-dark detection + `NO_COLOR` env respect
- Single static binary distribution (`cargo install` + GitHub release artifacts)
- No telemetry; no network beyond provider-direct

**Should have (differentiators, v1 if cheap or v1.x):**
- Pace indicator (behind / on-pace / too-hot) — ccburn-style behavioral nudge
- Plain-ASCII fallback mode for narrow / non-unicode terminals
- Statusline-hook compatibility (Claude Code `tui.status_line` contract via stdin JSON)
- Color-blind-friendly palette
- Tmux/Starship integration recipes (docs only)
- Per-provider window granularity (5h + weekly both shown)

**Defer (v2+):**
- Daemon mode (only if v1 inline refresh causes pain)
- Additional provider adapters (Cursor, Windsurf, Amp, Copilot CLI) — trait ready, code waits for community PRs
- Watch mode / per-window CLI filter / i18n / threshold fields

**Anti-features — explicitly do NOT build:**
- Cumulative cost / token dashboards (violates Core Value; ccusage does this excellently — link to it)
- ML/P90 limit prediction (% remaining + pace indicator already covers the value)
- Historical trend graphs (requires persistence we don't otherwise need)
- Desktop notifications (compose with code-notify, don't bundle)
- Web dashboard / GUI / OBS overlay (wrong form factor; out of scope per PROJECT.md)
- API-key/quota tracking (different use case from subscription-session)
- Plan auto-detection (read declared limits from each provider; don't infer)

See `FEATURES.md` for full prioritization matrix and competitor analysis.

### Architecture Approach

**Shared engine, two front-ends, one binary** (codex-rs pattern, downscaled). Single Cargo binary crate in v1 with internal module split (`cli/`, `tui/`, `engine/`, `provider/`, `cache.rs`, `config.rs`, `secrets.rs`, `model.rs`) structured so any of those can be lifted to a workspace member later without rewrites. The CLI and TUI never depend on each other; both depend down on `engine/`.

**Major components:**
1. **Entry layer (`cli::run` + `tui::run`)** — CLI calls `engine.refresh_all().await` once, formats, exits. TUI subscribes to an mpsc stream and runs a `tokio::select!` loop over (crossterm input, tick interval, engine updates).
2. **Engine** — UI-agnostic core. Owns `Vec<Arc<dyn Provider>>`, cache, config, secrets. Exposes `refresh_all` (batch) and `subscribe` (stream) — both surfaces wrap the same fan-out logic.
3. **`Provider` trait** — `#[async_trait]` interface `fetch(&self, ctx: &FetchCtx) -> Result<ProviderState, ProviderError>`. **Each adapter is invoked identically by both CLI and TUI** — that's how one adapter serves both surfaces without duplication.
4. **Per-provider adapters** — `ClaudeAdapter` (tokio::fs + JSONL streaming + stats-cache hint), `CodexAdapter` (rusqlite read-only inside `spawn_blocking` + JSONL rollouts), `GeminiAdapter` (reqwest + cookie/bearer auth, conditional on spike).
5. **`Cache` + `Secrets` + `Config`** — moka TTL cache with stale-on-error fallback; keyring-core secrets loaded once at startup; figment/serde TOML config with env+flag overlay.

**Critical patterns (all from ARCHITECTURE.md):**
- Per-provider `Vec<Result<ProviderState, ProviderError>>` — never `try_join_all`. Starship-style isolation.
- Per-adapter timeout (3s local, 10s HTTP) wrapped via `tokio::time::timeout`.
- Channels (mpsc), not `Arc<Mutex<AppState>>`, between engine background task and UI.
- `spawn_blocking` for `rusqlite`; `tokio::fs` everywhere else.
- Secrets loaded ONCE at startup into `Secrets`, passed via `&FetchCtx` — never re-read per fetch (avoids keyring prompts).
- `ratatui::init()` / `ratatui::restore()` installs a panic hook BEFORE any adapter code runs — terminal restoration is non-negotiable.

See `ARCHITECTURE.md` for the full diagram, data flow, anti-patterns, and testing seams.

### Critical Pitfalls

1. **Gemini account-ban risk (Pitfall 1, HIGH).** Scraping `gemini.google.com/usage` with the user's real Google session cookie at a 15s tick = 5,760 req/day to an undocumented consumer surface, plausibly a ToS violation with whole-Google-account blast radius. **Mitigation: Phase 0 spike with written go/no-go memo. If shipped, >=5min refresh, ETag/If-Modified-Since, daily ceiling, README warning. Prefer capturing local `gemini /stats` output over web scraping if at all possible.**
2. **Claude `stats-cache.json` is a cache, not a source of truth (Pitfall 2, HIGH).** Naïve readers ship wrong numbers — the real 5h/weekly accounting lives in `~/.claude/projects/**/*.jsonl`. **Mitigation: sum `usage.input_tokens + usage.output_tokens` from `assistant` JSONL entries within the rolling 5h window; defensive `Option<T>` deserialization; check-in anonymised snapshot fixtures so CI catches Claude Code schema drift before users do; add a visible "Claude adapter may be out-of-date" sentinel when too many fields are missing.**
3. **Codex `state_5.sqlite` is a live, versioned, mutating DB (Pitfall 3, HIGH).** Default rusqlite open is read-write; live writers cause `SQLITE_BUSY`; the `_5` suffix changes on Codex migrations. **Mitigation: `OpenFlags::SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`, `busy_timeout(250ms)`, glob `state_*.sqlite` and pick highest version, treat `SQLITE_BUSY` as transient (show waiting indicator, retry next tick), prefer append-only JSONL rollouts over SQLite when the data exists in both.**
4. **Plaintext credential storage (Pitfall 4, HIGH).** TOML config storing a Google session cookie = whole-Google-account exfiltration via dotfile sync, backups, screen-share. **Mitigation: `keyring-core` from day 1, `Secret<T>` newtype with redacting `Debug`, `#[serde(skip)]` on all secret fields, CI grep test for plaintext-credential patterns, audit `--json` output to confirm no secret echoes.**
5. **Terminal left in altscreen/raw mode on panic (Pitfall 5, HIGH).** A single `unwrap()` panicking 6 months from now leaves the user's shell unusable. **Mitigation: `ratatui::init()` / `ratatui::restore()` (which install a panic hook); install the hook BEFORE any adapter code; clippy `unwrap_used = deny` in adapter/render code; integration test that deliberately panics inside the render loop.**
6. **One adapter failing crashes the whole tool (Pitfall 6, HIGH).** Naïve `?` propagation in the orchestration layer makes a Gemini network blip kill the Claude and Codex bars too — violates Core Value. **Mitigation: `Vec<(ProviderId, Result<ProviderState, ProviderError>)>` return shape, `join_all` not `try_join_all`, per-adapter `tokio::time::timeout`, cache fallback with `is_stale = true` indicator, `catch_unwind` boundary around each adapter call.**

See `PITFALLS.md` for the full list including moderate (ANSI leak to pipes, tmux emoji width, blocking render loop, macOS Gatekeeper, cargo install slowness, excessive Gemini polling, inadequate exit codes) and minor (TTY detection, cross-OS config paths, AHB name discoverability) plus the technical-debt table, integration gotchas, and the "looks done but isn't" verification checklist.

## Implications for Roadmap

Research surfaces a clear phase decomposition driven by three cross-cutting forces:

- **Phase 0 spike is non-negotiable and gating.** The Gemini ToS/account-ban risk (Pitfall 1) must be resolved with a written go/no-go memo BEFORE any Gemini adapter code is written. The spike output also determines whether Phase 1 includes Gemini at all, or whether Gemini ships behind a v2 opt-in flag.
- **The adapter trait must land in Phase 1 even though only Claude uses it initially.** Designing it for one provider then retrofitting for three causes painful rewrites — getting it right with all three in mind from day one is much cheaper than discovering trait holes after three adapters exist (architecture build-order step 2-3, before any real adapter). The `model.rs` types (`ProviderState`, `ResetInfo`, `HpUnit`, `ProviderError`) are the contract everything else negotiates with.
- **Gemini scope is conditional on spike outcome.** If the spike clears with a low-risk path, Gemini ships in v1 with aggressive >=5min refresh + ETag + README warning. If not, Gemini is deferred to v2 behind an explicit opt-in, AHB v1 ships with Claude + Codex only, and the README documents the deferral honestly.

The architecture's build order (`ARCHITECTURE.md` § Build Order) suggests the natural phase decomposition below. Phase numbering matches the pitfall-to-phase mapping in `PITFALLS.md`.

### Phase 0: Gemini Feasibility Spike + Foundation Decisions

**Rationale:** The Gemini account-ban risk is the single largest project risk; everything downstream is gated on its outcome. Bundling it with a few non-coding foundational decisions (model.rs contract, charset choice, panic-hook strategy) keeps it from being a pure investigation phase.
**Delivers:**
- Written Gemini go/no-go memo with kill-criteria (rate limits, fingerprint, ToS reading, captcha probe)
- `model.rs` contract types (`ProviderId`, `ProviderState`, `ResetInfo`, `HpUnit`, `ProviderError`) — serde-derived, no deps beyond serde+jiff
- Charset decision (ASCII default vs. unicode/emoji opt-in) verified under tmux/screen/Windows Terminal
- Repo scaffold: `cargo new`, MSRV pin, clippy config (`unwrap_used = deny` for render/adapter paths), CI skeleton

**Addresses:** Implicit requirement to know what we're building before building it; locks the contract before any adapter exists.
**Avoids:** Pitfall 1 (Gemini ban), Pitfall 8 (unicode width).
**Research flag:** **YES — this phase is itself research.** No deeper `/gsd:plan-phase --research-phase` needed inside it; the spike IS the research.

### Phase 1: Core Engine + Claude Adapter + TUI Scaffold (the spine)

**Rationale:** This is the load-bearing phase. The engine + adapter trait + first real adapter + panic-safe TUI shell all need to land together because they're mutually validating — the adapter shape isn't proven until a real adapter implements it, and the TUI scaffold isn't proven until it survives a panic. Claude is the easiest adapter (pure local FS, no auth) so it surfaces trait-shape problems cheaply. Per-adapter error isolation (Pitfall 6) and terminal-restore-on-panic (Pitfall 5) are wired in before any feature code — they're not optional polish.
**Delivers:**
- `Provider` trait + `FetchCtx` + `MockProvider` for tests
- `Engine::refresh_all` (CLI surface) + per-adapter timeout + `Vec<Result<...>>` isolation
- `engine::subscribe` (TUI surface) + background ticker
- `cli::run` with compact text output (default `AHB` command works end-to-end)
- `tui::run` with `tokio::select!` loop, `ratatui::init()/restore()` panic hook, single-provider HP bar widget
- `ClaudeAdapter` reading JSONL rollups (not just stats-cache) with snapshot-test fixtures + schema-drift sentinel
- `config.rs` (TOML via `directories` crate, cross-OS paths) + `secrets.rs` (keyring-core + `Secret<T>` newtype, even though Claude doesn't need credentials — wires the contract before Gemini)
- Insta snapshot tests for CLI compact output + ratatui TestBackend snapshot for HP bar widget

**Addresses:** Most P1 features (compact one-line, Claude adapter, TUI with auto-refresh, config file, graceful per-adapter failure, color/TTY, single binary).
**Avoids:** Pitfalls 2 (Claude schema drift), 4 (plaintext credentials), 5 (terminal restoration), 6 (one adapter crashing all), 8 (unicode width), 9 (blocking render loop), 16 (cross-OS config paths).
**Research flag:** Optional. Architecture patterns are well-documented (`ARCHITECTURE.md` covers it). Worth a small `/gsd:plan-phase --research-phase` if the planner wants to validate ratatui async-template details before TUI scaffolding.

### Phase 2: Codex Adapter + Output Format Polish

**Rationale:** Codex exercises `spawn_blocking` + read-only SQLite + JSONL rollouts — patterns not yet proven by Claude's pure-async-fs adapter. Pairing it with CLI output-format polish (`--detailed`, `--json` with `schema_version`, TTY detection, exit codes, `NO_COLOR`) gives the phase a coherent shape: "AHB now reads two providers reliably and outputs them in shell-pipeable ways." Codex schema versioning (`state_*.sqlite` glob) lands here.
**Delivers:**
- `CodexAdapter` (read-only SQLite open, `busy_timeout`, dynamic version glob, JSONL rollouts preferred, `rate_limits: null` handled as "unknown")
- `--detailed` multi-line per-provider formatter
- `--json` output with `schema_version: 1` (locked schema before any tmux/Starship users build on it)
- `IsTerminal::is_terminal()` detection + `NO_COLOR` + `--color=auto|always|never`
- Documented exit codes (0 = >=1 provider OK; 1 = all failed; 2 = config/secrets fatal; 64+ for sysexits-style usage errors)
- Per-adapter integration test: run codex actively in parallel terminal, verify AHB doesn't crash

**Addresses:** Codex adapter (P1), `--detailed` / `--json` (P1), stable schema (P1), exit codes (P2).
**Avoids:** Pitfalls 3 (Codex SQLite locking), 7 (ANSI in pipes), 13 (exit codes), 15 (TUI in non-TTY).
**Research flag:** Possibly — Codex's `state_5.sqlite` schema is unstable and undocumented; `/gsd:plan-phase --research-phase` worth running to scrape openai/codex source for current schema before coding the adapter.

### Phase 3: Gemini Adapter (conditional) + Refresh Policy

**Rationale:** Conditional phase — content depends on Phase 0 outcome. Either: (a) full Gemini HTTP adapter with per-provider refresh interval (>=5min for network adapters, decoupled from 15s TUI tick), wiremock-based tests, cache stale-on-error fallback, browser-realistic-but-conservative request shape, README ToS warning — OR — (b) feature-flagged stub `GeminiAdapter` that returns `ProviderError::OptIn` by default, with v2 marker for future revisit. Either way, the per-provider refresh-interval mechanism lands here because Gemini is the first network adapter that needs it.
**Delivers:**
- `GeminiAdapter` (full or stub depending on Phase 0)
- Per-provider `refresh_interval` config field; engine respects it
- `Cache` (moka) with stale-on-error fallback wired into the engine
- wiremock-based test suite for HTTP success, 401, 429 + Retry-After, 500, slow response
- Battery/idle detection hooks (optional — pause network adapters when on battery)
- README "Gemini support uses an unofficial endpoint" warning (or "Gemini deferred to v2" note if stub)

**Addresses:** Gemini adapter (P1, conditional), cache/refresh-policy infrastructure.
**Avoids:** Pitfalls 1 (already mitigated by Phase 0 gate; this phase enforces the mitigations), 12 (excessive Gemini polling).
**Research flag:** **DEPENDS on Phase 0 outcome.** If Gemini ships, `/gsd:plan-phase --research-phase` is critical here — the Gemini endpoint shape, auth flow, and rate-limit behavior need fresh investigation right before coding. If Gemini is stubbed, no research needed.

### Phase 4: Distribution + Release Polish

**Rationale:** Distribution is its own concern — Gatekeeper, `cargo install` slowness vs. pre-built binaries, `cargo binstall` discovery, optional Homebrew tap — and consolidating it into one phase avoids it being a perpetual half-done backlog. Also includes any "looks done but isn't" items deferred from earlier phases.
**Delivers:**
- `cargo-dist` configured in `Cargo.toml [workspace.metadata.dist]` — multi-arch GH Actions workflow, tarballs, shell installer, PowerShell installer
- `cargo binstall` works (set `repository` metadata)
- README install path: brew (if tap published) → `cargo binstall` → `cargo install` → manual download with macOS Gatekeeper workaround documented
- Crate `description` is discoverable ("AHB — AI HP Bar — multi-CLI session usage at a glance")
- README sections: Core Value, Install (all paths), Configuration (with keyring setup), Gemini warning (if applicable), Troubleshooting (terminal stuck after panic), tmux/Starship recipes
- Final "looks done but isn't" pass against PITFALLS.md checklist

**Addresses:** Single binary distribution (P1), AHB name discoverability (P3), tmux/Starship recipes (P2 docs).
**Avoids:** Pitfalls 10 (Gatekeeper), 11 (cargo install slowness), 14 (name discoverability).
**Research flag:** None. cargo-dist is well-documented.

### Phase Ordering Rationale

- **Phase 0 first because Gemini risk is gating.** Doing it last would mean either rewriting Phase 1's adapter-orchestration assumptions after the spike, or shipping v1 without Gemini at all without having ever asked the question. Front-loading it preserves optionality.
- **Phase 1 is the spine.** Adapter trait + engine + first adapter + TUI scaffold are mutually validating; they can't be cleanly split into smaller phases without leaving each piece half-tested. Pitfalls 4, 5, 6 mitigations (keyring/Secret, panic hook, error isolation) MUST land here, not later, because retrofitting them is painful.
- **Phase 2 (Codex) comes before Phase 3 (Gemini)** because Codex exercises new infrastructure (`spawn_blocking`, SQLite, version glob) that's an internal pattern; Gemini exercises external infrastructure (HTTP, auth, cache fallback) that's a different pattern. Doing the internal pattern first keeps Phase 3's variables (network + ToS risk + cache) isolated.
- **Phase 2 also bundles output-format polish** because once two providers exist the JSON schema is real (vs. theoretical with one provider) and locking it before any third-party tmux/Starship user builds on it is much cheaper.
- **Cache lands in Phase 3, not Phase 1**, because Claude and Codex read local FS and are cheap enough to not need caching; Gemini is the first adapter where stale-on-error semantics matter. (Architecture build-order confirms this: step 11.)
- **Phase 4 last because distribution is downstream of "the thing actually works".** Investing in cargo-dist + signing + Homebrew before the tool stabilizes wastes signing certificates and tap rename overhead.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 0:** The phase IS research. Spike Gemini endpoint behavior, ToS reading, rate-limit/fingerprint testing. Output is a memo, not code.
- **Phase 2 (Codex):** `/gsd:plan-phase --research-phase` recommended — scrape openai/codex source on GitHub for the current `state_*.sqlite` schema before coding. Schema is undocumented and changes; latest-version-on-master is the only source of truth.
- **Phase 3 (Gemini, conditional):** `/gsd:plan-phase --research-phase` critical if Gemini ships — endpoint shape and auth flow need fresh verification immediately before coding. Spike findings from Phase 0 may have aged.

Phases with standard patterns (skip research-phase):
- **Phase 1:** ARCHITECTURE.md fully covers the patterns (codex-rs-style engine + ratatui async template + `Provider` trait). Optional light research if the planner wants to double-check ratatui 0.30 breaking changes vs. 0.29, but not required.
- **Phase 4:** cargo-dist + cargo-binstall + Homebrew are all well-documented; standard patterns.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions verified against GitHub release API on 2026-05-22; non-obvious picks (keyring-core, ratatui::crossterm re-export, jiff isolation) are well-justified. Caveats: `keyring-core` 1.0 API is young (verify against docs.rs at impl time); `jiff` is pre-1.0 (isolate behind a `Clock` trait); ratatui 0.30 has breaking changes from 0.29 (design against 0.30 directly). |
| Features | HIGH-MEDIUM | HIGH on Claude Code ecosystem — dense competitive landscape, many real tools verified (ccusage, ccburn, Maciek-roboblog, claude-code-usage-bar, etc.). MEDIUM on Codex/Gemini — fewer existing tools, but provider docs + native `/status` and `/stats` commands verified. The "no existing multi-provider HP-bar tool" claim is HIGH confidence (searched and confirmed). |
| Architecture | HIGH | Component boundaries, async event-loop pattern, and `Provider` trait shape all triangulated against multiple SOTA references (codex-rs, ratatui async-template, gitui asyncgit, Starship per-module timeout). MEDIUM on exact crate choices for cache/config (moka, figment are recommendations but substitutable). LOW on Gemini HTTP adapter internals — that's exactly what the Phase 0 spike is for. |
| Pitfalls | HIGH | Gemini ToS / account-ban risk is HIGH confidence — Google's GenAI Prohibited Use Policy is explicit, real ban incidents documented (gemini-cli #20632). Claude schema instability HIGH confidence (Anthropic ships weekly, ccusage has tracked changes). Codex SQLite fragility HIGH confidence (open issues #21750, #23848, #14880). Distribution pitfalls MEDIUM-HIGH — best practices established, project-specific risk varies. |

**Overall confidence:** MEDIUM-HIGH. The high-confidence findings (stack, architecture, Claude/Codex pitfalls) are well-grounded; the medium-confidence area (Gemini feasibility) is explicitly gated behind Phase 0 — confidence is structured to be honest about it rather than handwaved.

### Gaps to Address

- **Gemini endpoint shape, auth flow, rate-limit behavior** — only resolvable via Phase 0 spike. Phase 0 must produce a written memo with kill criteria, not a "we'll figure it out" handwave.
- **Codex `state_*.sqlite` schema** — undocumented and unstable. Phase 2 research-phase must scrape current openai/codex source before coding adapter; build with the assumption that schema will change and gate every query with a probe.
- **Claude Code session-state shape across versions** — `stats-cache.json` and `~/.claude/projects/**/*.jsonl` schemas evolve. Phase 1 must check in anonymised snapshot fixtures from the current Claude Code version as test corpus, and include a schema-drift sentinel that surfaces a visible warning in the HP bar when too many expected fields are missing.
- **Gemini CLI local `/stats` command capture** — feature research notes Gemini CLI has a native `/stats` command, but its non-interactive capture path (via `gemini exec` or similar) is unverified. Phase 0 spike should check whether reading local Gemini CLI output is viable as a safer alternative to web scraping.
- **`keyring-core` 1.0 exact API** — just hit 1.0 (April 2026), surrounding connector crates still settling. Phase 1 must abstract behind a `trait SecretStore` so file-backed fallback (with explicit user opt-in for headless Linux) is swappable.
- **`jiff` pre-1.0 churn** — minor breakages possible through 2026. Phase 1 isolates all datetime calls behind a thin `Clock` module so we can swap to chrono+chrono-tz if needed.
- **Per-provider refresh interval mechanism timing** — Phase 1 docs cite uniform 15s tick; Phase 3 introduces per-provider intervals. Phase 1 should at minimum stub the config field even if the engine ignores it initially, to avoid a config breaking change in v1.0.1.

## Sources

### Primary (HIGH confidence)

- GitHub Release API (verified versions on 2026-05-22) — clap, ratatui, tokio, reqwest, rusqlite, keyring-core, crossterm, tracing, serde, jiff, cargo-dist, insta, wiremock, anyhow, thiserror.
- `ratatui.rs` — backends, snapshot testing, full async events, panic hooks, init/restore. Architectural authority for TUI patterns.
- `codex-rs Architecture: How OpenAI Rewrote Codex CLI in Rust` (codex.danielvaughan.com) — shared-core multi-entrypoint pattern, the spine of AHB's architecture.
- `developers.openai.com/codex` + `github.com/openai/codex` source — `~/.codex/state_5.sqlite` + JSONL rollouts confirmed in upstream code.
- `docs.rs/keyring-core` + open-source-cooperative wiki — `keyring` v4→`keyring-core` 1.0 migration policy.
- Google GenAI Prohibited Use Policy (`support.google.com/gemini/answer/16625148`) — basis for Pitfall 1 Gemini ToS risk.
- Codex GitHub issues #21750, #23848, #14880, #17537 — SQLite corruption, init failure, `rate_limits: null`, quota inconsistency — real evidence for Codex adapter pitfalls.
- ccusage (`github.com/ryoppippi/ccusage`) — reference parser for Claude JSONL schema; sets the bar for Claude single-provider feature richness.
- `seanmonstar.com/blog/reqwest-v013-rustls-default/` — rustls is now reqwest default.
- `corrode.dev/blog/async/` — `async-std` discontinued.
- Starship per-module timeout architecture write-up — per-provider error-isolation reference pattern.

### Secondary (MEDIUM confidence)

- Ratatui best practices discussion #220 — channels-not-mutex for state.
- `Comprehensive Rust — Async Traits` — `async_trait` vs. native async-fn-in-trait `dyn` safety.
- gitui asyncgit README — thread-pool + crossbeam-channel offload pattern.
- `rust-cli-recommendations.sunshowers.io` — clap derive as standard pick.
- `docs.rs/moka` — async TTL cache API.
- Maciek-roboblog Claude-Code-Usage-Monitor, ccburn, claude-code-usage-bar, claude-statusbar, ccstatusline — competitive feature analysis.
- `b3nw/gemini-cli-usage`, codex-cli-usage PyPI, Codex CLI `/status` issue #15281 — Codex + Gemini ecosystem context.
- code-notify, ai-cli-complete-notify — multi-provider notification tools (notify-only, not display) — confirms AHB's white space.
- ratatui issues #1271, #1438; tmux #647 — unicode width / emoji rendering hazards.
- cargo-dist (`github.com/axodotdev/cargo-dist`), cargo-binstall — distribution best practices.

### Tertiary (LOW confidence — domain inference)

- Refresh-rate / abuse-detection asymmetry argument in Pitfall 1 — synthesised from Google ToS reading + general abuse-detection literature; no single canonical citation.
- `Secret<T>` newtype pattern — common Rust convention, no canonical citation.
- Adapter isolation contract framing — derived from health-state literature + Starship pattern; not a single canonical source.
- "AHB white space is multi-provider, not single-provider richness" — synthesis from competitor analysis; no single source confirms this is the right strategic frame.

---
*Research completed: 2026-05-22*
*Ready for roadmap: yes*
