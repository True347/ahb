# Roadmap: AHB (AI HP Bar)

## Overview

AHB ships in five phases that each deliver an end-to-end, user-runnable binary. Phase 0 resolves the largest project risk (Gemini ToS / account-ban) via a written go/no-go spike while simultaneously locking the cross-adapter contract (`model.rs`) and emitting a runnable skeleton binary, so every later phase plugs into a stable spine. Phase 1 is the load-bearing build — engine + Claude adapter + panic-safe TUI scaffold + keyring + `Secret<T>` + per-adapter error isolation all land together because they're mutually validating. Phases 2 (Codex + output formats) and 3 (Gemini conditional + cache/refresh policy) each add a working provider while exercising a new infrastructure pattern (spawn_blocking+SQLite, then HTTP+ETag+stale-on-error). Phase 4 turns the working tool into a distributable artifact (`cargo-dist`, `cargo binstall`, Gatekeeper docs, crates.io metadata). Granularity is coarse (5 phases, broad scope each); every phase produces a binary you can run, not a horizontal layer.

## Phases

**Phase Numbering:**

- Integer phases (0, 1, 2, 3, 4): Planned milestone work
- Decimal phases (e.g., 2.1): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 0: Spike & Spine** — Gemini go/no-go memo + `model.rs` contract + scaffold + skeleton binary (completed 2026-05-22)
- [x] **Phase 1: Engine + Claude + TUI Scaffold** — load-bearing phase: engine, Claude adapter, TUI shell, keyring, panic hook, error isolation (completed 2026-05-23)
- [ ] **Phase 2: Codex + Output Formats** — Codex adapter via spawn_blocking + SQLite, lock `--detailed` / `--json schema_version:1` + exit codes
- [ ] **Phase 3: Gemini (conditional) + Cache & Refresh Policy** — Gemini adapter (full or stub per Phase 0), per-provider refresh, moka stale-on-error
- [ ] **Phase 4: Distribution & Release Polish** — cargo-dist + cargo binstall + Gatekeeper docs + crates.io metadata

## Phase Details

### Phase 0: Spike & Spine

**Goal:** Resolve the Gemini account-ban risk with a written go/no-go memo, lock the `Provider` trait + `model.rs` contract types that every later phase negotiates with, and emit a runnable skeleton `AHB` binary that prints a placeholder HP bar from a `MockProvider`.
**Mode:** mvp
**Depends on:** Nothing (first phase)
**Requirements:** ADP-00
**Success Criteria** (what must be TRUE):

  1. A written `research/GEMINI_SPIKE.md` memo exists with explicit go/no-go decision and kill criteria (rate, fingerprint, ToS reading, captcha probe); the decision determines whether Phase 3 ships full Gemini or a stub.
  2. `cargo build --release` produces a single static binary; running `AHB` prints one mocked HP bar line (e.g., `[mock ████░░ 60% / resets in 2h00m]`) using the locked `Provider` trait + `ProviderState` / `ResetInfo` / `HpUnit` / `ProviderError` types.
  3. `cargo test` runs green against `MockProvider`-based unit tests of the trait, confirming `model.rs` is `Serialize` + `Deserialize` + `Send + Sync` + dyn-safe.
  4. Charset decision is recorded (ASCII default vs. emoji opt-in) and verified to render correctly in tmux, screen, and Windows Terminal.
  5. Repo is scaffolded with MSRV pin (≥1.88), clippy config (`unwrap_used = deny` for adapter/render paths), and a CI skeleton that runs `cargo build` + `cargo test` + `cargo clippy`.

**Plans:** 5/5 plans complete
Plans:
**Wave 1**

- [x] 00-01-PLAN.md — Repo scaffold + Cargo.toml + LICENSE × 2 + clippy.toml + 3-OS CI workflow (placeholder binary; no domain code) [wave 1]

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 00-02-PLAN.md — Lock model.rs contract types + Provider trait + FetchCtx + Secrets stub + dyn-safety asserts [wave 2]
- [x] 00-04-PLAN.md — Gemini local-capture go/no-go spike memo at .planning/research/GEMINI_SPIKE.md (9 required sections per D-23) [wave 2]

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 00-03-PLAN.md — MockProvider + cli/render_text + main.rs panic hook + clap; cargo run prints the D-25 literal through the trait [wave 3]

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 00-05-PLAN.md — Phase 0 smoke + charset verification (xxd byte proof + visual eyeball) + Phase 0 success-criteria checklist [wave 4]

### Phase 1: Engine + Claude + TUI Scaffold

**Goal:** Make `AHB` and `AHB tui` work end-to-end against a real Claude Code subscription, with keyring-backed secrets, panic-safe terminal restore, and per-adapter error isolation wired in BEFORE feature code so the foundation is correct from day one.
**Mode:** mvp
**Depends on:** Phase 0
**Requirements:** CORE-01, CORE-05, TUI-01, TUI-02, TUI-04, TUI-05, CFG-01, CFG-02, CFG-04, SEC-01, SEC-02, SEC-04, ADP-01, ADP-02, ADP-03
**Success Criteria** (what must be TRUE):

  1. Running `AHB` (no args) on a machine with Claude Code installed prints a compact one-line HP bar showing real Claude session % and reset countdown computed from `~/.claude/projects/**/*.jsonl` (not from `stats-cache.json` alone); piping `AHB | cat` emits no ANSI escapes.
  2. Running `AHB tui` opens a fixed full-screen view that auto-refreshes every 15 seconds, restores the terminal cleanly on `q` / Ctrl-C, and survives a deliberately-injected adapter `panic!()` without leaving the shell in raw / altscreen mode (verified by integration test).
  3. Adding a deliberately-failing fake provider alongside Claude in `~/.config/ahb/config.toml` causes `AHB` to render Claude's bar normally AND a clear error row for the broken provider — the entire tool never crashes or blanks because one adapter failed; running `AHB tui` in a non-TTY pipe prints a clear "TUI requires a terminal" error and exits.
  4. The TOML config (resolved via the `directories` crate, cross-OS) lists providers with independent enable/disable flags; un-configured providers are silently skipped (not flagged as failures); secrets are stored in OS keyring via `keyring-core` 1.0 and wrapped in a `Secret<T>` newtype whose `Debug` impl redacts the value (CI grep test confirms no plaintext credential pattern in any output).
  5. When Claude Code's JSONL schema changes and too many expected fields go missing, the Claude row renders a visible "Claude adapter may be out-of-date" sentinel instead of silently zeroing the bar.

**Plans:** 4/4 plans complete

**Wave 1**

- [x] 01-01-PLAN.md — Walking Skeleton vertical slice: engine + Claude adapter + config + CLI render-multi-row + first-run init [wave 1]

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 01-02-PLAN.md — Secret<T> + keyring-core wiring + D-43 grep test + schema-drift sentinel + panic-injection mock [wave 2]

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 01-03-PLAN.md — TUI surface: ratatui::run + 15s/1s tick loop + non-TTY refusal + panic-safe restore [wave 3]

**Wave 4** *(blocked on Wave 3 completion — gap closure from 01-VERIFICATION.md)*

- [x] 01-04-PLAN.md — BL-01 clock-injection in TUI render + BL-02 deterministic provider row order + BL-03 5h gap boundary + WR-06 cross-OS config path + WR-08 dead-code removal [wave 4]

**UI hint:** yes

### Phase 2: Codex + Output Formats

**Goal:** Add a second real provider (Codex) using the new `spawn_blocking` + read-only SQLite pattern, and lock down all CLI output formats — `--compact`, `--detailed`, `--json` with stable `schema_version: 1`, plus exit codes — before any tmux / Starship user builds on AHB's output.
**Mode:** mvp
**Depends on:** Phase 1
**Requirements:** CORE-02, CORE-03, CORE-04, CORE-06, SEC-03, ADP-04
**Success Criteria** (what must be TRUE):

  1. Running `AHB` on a machine where Codex CLI is actively writing to `~/.codex/state_*.sqlite` reads it read-only with `busy_timeout`, prefers append-only JSONL rollouts when available, treats `rate_limits: null` as "unknown" rather than 100%, and produces a Codex HP bar alongside Claude's — verified by an integration test that runs `codex` in parallel and confirms AHB doesn't crash or report locked-DB errors.
  2. `AHB --compact` forces single-line output; `AHB --detailed` prints multi-line per-provider rows showing both session and weekly bars; both work whether stdout is a TTY or piped.
  3. `AHB --json` emits a JSON document with `schema_version: 1` that round-trips cleanly through `jq` (no ANSI bytes, no escape leakage) and is consumable by tmux / Starship / shell pipelines; a CI grep test asserts no secret-shaped strings ever appear in `--json` output regardless of input.
  4. Documented exit codes work: `0` when ≥1 provider succeeded, `1` when all providers failed, `2` when config or secrets are unloadable — verified by integration tests for each path; `--help` documents them and `NO_COLOR` env + `--color=auto|always|never` flag are both honored.

**Plans:** TBD

### Phase 3: Gemini (conditional) + Cache & Refresh Policy

**Goal:** Wire the third provider per Phase 0's outcome — full Gemini HTTP adapter if the spike cleared, opt-in stub otherwise — and introduce the per-provider refresh-interval mechanism + moka stale-on-error cache that smooths transient network failures without blanking the bar.
**Mode:** mvp
**Depends on:** Phase 2
**Requirements:** TUI-03, CFG-03, ADP-05
**Success Criteria** (what must be TRUE):

  1. If Phase 0 cleared Gemini: running `AHB` on a Gemini-configured machine produces a Gemini HP bar fetched from the chosen data source (local `gemini /stats` capture or HTTP endpoint), with refresh interval clamped to ≥5 minutes regardless of the TUI's 15s tick, ETag / If-Modified-Since respected, and a hard daily request ceiling enforced. If Phase 0 deferred Gemini: `AHB` shows Gemini as "deferred to v2 — enable with `--experimental-gemini`" and the README documents the deferral reason.
  2. Either way, `README.md` contains an explicit warning section about Gemini's unofficial endpoint / ToS risk (or the deferral note), and the `--json` output never echoes any Gemini credential.
  3. Per-provider `refresh_interval` in `config.toml` overrides the global TUI tick; setting Gemini to 600s while Claude/Codex stay at 15s produces the expected polling cadence (verified by wiremock-based integration tests covering 200 / 304 / 401 / 429+Retry-After / 500 / slow-response paths).
  4. When a network adapter errors transiently, the engine serves the last successful `ProviderState` from cache with a visible "(stale Ns ago)" indicator instead of blanking the row; the cache TTL is decoupled from the refresh interval.

**Plans:** TBD

### Phase 4: Distribution & Release Polish

**Goal:** Turn the working tool into something a non-Rust developer can install in one command across macOS / Linux / Windows, with all the "looks done but isn't" distribution pitfalls (Gatekeeper, slow `cargo install`, undiscoverable crate metadata) closed.
**Mode:** mvp
**Depends on:** Phase 3
**Requirements:** DIST-01, DIST-02, DIST-03, DIST-04
**Success Criteria** (what must be TRUE):

  1. A tagged GitHub release ships pre-built static binaries (no OpenSSL / native-tls / runtime deps) for `x86_64-{linux,apple-darwin,pc-windows-msvc}` + `aarch64-{linux,apple-darwin}` via `cargo-dist`, plus a shell installer and a PowerShell installer.
  2. All three install paths work from a clean machine: `cargo install ahb`, `cargo binstall ahb`, and downloading + running the GitHub-release artifact directly — each is documented in `README.md` with copy-pasteable commands.
  3. `README.md` contains a macOS Gatekeeper workaround section (`xattr -d com.apple.quarantine ./ahb` and equivalents) so users who hit "developer cannot be verified" can recover without giving up.
  4. The crate is published to crates.io with a discoverable `description` (e.g., "AHB — AI HP Bar — multi-CLI subscription session usage at a glance"), `keywords` covering claude/codex/gemini/cli/tui, and `repository` set so `cargo binstall` auto-discovers releases; searching crates.io for "ai hp bar" or "claude codex usage" finds it.

**Plans:** TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 0 → 1 → 2 → 3 → 4 (with decimal phases inserted between integers as needed).

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 0. Spike & Spine | 5/5 | Complete   | 2026-05-22 |
| 1. Engine + Claude + TUI Scaffold | 4/4 | Complete   | 2026-05-23 |
| 2. Codex + Output Formats | 0/TBD | Not started | - |
| 3. Gemini (conditional) + Cache & Refresh Policy | 0/TBD | Not started | - |
| 4. Distribution & Release Polish | 0/TBD | Not started | - |

---
*Roadmap created: 2026-05-22*
*Coverage: 29/29 v1 requirements mapped (no orphans)*
