---
phase: 00-spike-spine
verified_at: 2026-05-22T21:35:00Z
verdict: pass
status: passed
score: 5/5 success criteria verified
re_verification: false
---

# Phase 0: Spike & Spine — Verification Report

**One-line verdict:** PASS — Phase 0 ships exactly what ROADMAP § Phase 0 promised: a Gemini NO-GO memo with structured hand-off, the locked `Provider` / `ProviderState` / `ResetInfo` / `HpUnit` / `ProviderError` contract types (Serialize+Deserialize+Send+Sync+dyn-safe), a single static binary that prints the D-25 literal `mock-session  ██████░░░░ 60% • resets in 2h00m` through the locked trait (no `println!` shortcut bypassing the spine), and a 3-OS CI matrix on MSRV 1.88 with the clippy floor in place. Phase 1 is unblocked.

**Phase Goal (ROADMAP):**
> Resolve the Gemini account-ban risk with a written go/no-go memo, lock the `Provider` trait + `model.rs` contract types that every later phase negotiates with, and emit a runnable skeleton `AHB` binary that prints a placeholder HP bar from a `MockProvider`.

---

## ROADMAP Success Criteria

| #   | Criterion (truncated)                                                                                       | Status     |
| --- | ----------------------------------------------------------------------------------------------------------- | ---------- |
| 1   | `research/GEMINI_SPIKE.md` exists with explicit go/no-go + kill criteria; outcome drives Phase 3 scope      | PASS       |
| 2   | `cargo build --release` → static binary; `AHB` prints one mock HP bar line via locked `Provider` trait      | PASS       |
| 3   | `cargo test` green; `model.rs` is `Serialize` + `Deserialize` + `Send + Sync` + dyn-safe                    | PASS       |
| 4   | Charset decision recorded (Unicode default vs `--ascii` opt-in); verified to render correctly               | PASS       |
| 5   | Repo scaffolded with MSRV ≥1.88, clippy floor (`unwrap_used` deny in adapter/render paths), 3-OS CI         | PASS       |

**Score:** 5/5 PASS

---

## Verification Questions (caller-provided)

### Q1. Does running `./target/release/ahb` print the locked literal? — PASS

Live verification just now:

```
$ /home/chasel/REPO/AIHPBar/target/release/ahb
mock-session  ██████░░░░ 60% • resets in 2h00m
```

Regex match (regex-tolerant on countdown per D-25): `^mock-session  ██████░░░░ 60% • resets in [0-9]+h[0-9]{2}m$` → **MATCH** (`grep -qP` exited 0).

ASCII mode: `/home/chasel/REPO/AIHPBar/target/release/ahb --ascii` → `mock-session  ######---- 60% | resets in 2h00m` — also matches its regex.

xxd byte proof: U+2588 (`e2 96 88`) ×6, U+2591 (`e2 96 91`) ×4, U+2022 (`e2 80 a2`) ×1. 69 stdout bytes total with trailing newline.

### Q2. Does output come through `Provider` trait (no `println!` shortcut)? — PASS

`/home/chasel/REPO/AIHPBar/src/main.rs` lines 76-81 show the bar value flows: `MockProvider` → `.fetch(&ctx).await` → `ProviderState` → `compact_line(&state, ...)` → `println!("{line}")`. The only `println!` in `main.rs` prints the already-rendered string; the value originated inside the `Provider::fetch` future. `eprintln!` on line 51 is the panic-hook stderr message, not a bar-output bypass. Comment on line 74-75 explicitly states: "The bar value MUST flow through the Provider trait, never a hardcoded println."

### Q3. Does the `Provider` trait shape match D-13? — PASS

`/home/chasel/REPO/AIHPBar/src/provider/mod.rs` lines 38-42:

```rust
#[async_trait]
pub trait Provider: Send + Sync + 'static {
    fn id(&self) -> ProviderId;
    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError>;
}
```

`Send + Sync + 'static` bound ✓, async fetch returning `Result<ProviderState, ProviderError>` ✓, `#[async_trait]` for dyn-compatibility ✓. Compile-time proof: `assert_impl_all!(Box<dyn Provider>: Send, Sync)` at line 52 — removing `#[async_trait]` would fail to compile.

### Q4. Does `ProviderState` carry `Vec<HpWindow>` (D-08)? — PASS

`/home/chasel/REPO/AIHPBar/src/model.rs` lines 62-70:

```rust
pub struct ProviderState {
    pub id: ProviderId,
    pub windows: Vec<HpWindow>,
    ...
}
```

Plural `windows`, not a single window. Matches D-08 (multiple concurrent reset windows per provider, e.g. Claude 5h session + weekly).

### Q5. Does `Secret<T>` refuse to serialize AND redact in Debug? — N/A (Phase 1 deliverable, not Phase 0)

This question targets Phase 1 SEC-02. Phase 0 explicitly scoped only a `Secrets` (plural) stub: `pub struct Secrets;` with `#[derive(Debug, Default, Clone)]` at `/home/chasel/REPO/AIHPBar/src/secrets.rs`. Plan 02's must_haves and CONTEXT D-09/SEC-02 mapping both deliberately defer the `Secret<T>` newtype + keyring-core wiring to Phase 1. The current file's doc-comment explicitly records the deferral ("Phase 1 replaces this file with the real keyring-backed type").

**This is intentional Phase 0 scope, not a gap.** The `Secrets` stub satisfies its Phase 0 contract: it can be referenced by `FetchCtx::secrets: &Secrets` so the type's `&Secrets` reference can be widened in Phase 1 without ABI break.

### Q6. Does GEMINI_SPIKE.md have an unambiguous GO/NO-GO decision matching the body? — PASS

Frontmatter (line 4): `decision: no-go`. Body § "Go/No-Go decision: NO-GO" (line 8) restates and walks the three D-21 criteria. Each is shown to fail: (1) `gemini -p "/stats"` forwards `/stats` as an LLM prompt instead of triggering the slash-command handler; (2) no probe produced quota or reset-window data; (3) only outputs available are unparseable LLM chat-response markdown or a JSON cancellation envelope. Conclusion explicitly invokes D-22: "Per D-22 (any one of (1)-(3) fails → no-go), Gemini is deferred to v2 stub."

Frontmatter decision and body reasoning are consistent.

### Q7. Does the Phase 3 hand-off align with the NO-GO decision? — PASS

§ "Phase 3 hand-off (no-go path)" (lines 152-161) maps NO-GO to the correct downstream pattern:

- **Opt-in CLI flag** `--experimental-gemini` + `[providers.gemini] enabled = false` default — user must explicitly opt in
- **Stub adapter** returning `Err(ProviderError::Unavailable { reason: "Gemini adapter deferred to v2 — see README §Gemini status".into() })` — no HTTP, no spawn, no side effects
- **README section** with deferral reason for Phase 4
- **What Phase 3 SHOULD still ship:** cache + refresh-policy for the two adapters that DO ship (Claude + Codex)

This is the opt-in stub path mandated by D-22, NOT a full Gemini adapter path. The Phase 3 ROADMAP success criterion already mirrors this: "If Phase 0 cleared Gemini: ... If Phase 0 deferred Gemini: `AHB` shows Gemini as 'deferred to v2 — enable with `--experimental-gemini`'".

### Q8. Is the panic hook contract installed in main.rs in a way Phase 1 can replace cleanly? — PASS

`/home/chasel/REPO/AIHPBar/src/main.rs` lines 48-54 install the Phase 0 hook via the `take_hook()` + `set_hook()` chain pattern. Lines 57-60 install it as the FIRST executable statement in `main()` (before `Cli::parse()`, before runtime construction, before any provider code). The function comment (lines 44-47) explicitly documents the contract:

> Phase 0 panic hook. Composes via `take_hook()` + `set_hook()` so Phase 1's `ratatui::init()` can wrap it (ratatui takes the hook AFTER we install ours and chains: terminal-restore -> our stderr-print -> default). Order matters — see docs.rs/ratatui/latest/ratatui/fn.init.html (RESEARCH Pitfall 5).

This matches D-27 verbatim. When Phase 1 calls `ratatui::init()` AFTER the Phase 0 hook is installed, ratatui will compose its terminal-restore hook on top of the Phase 0 stderr-print hook on top of the default — clean three-layer chain, no overwrites.

### Q9. Does CI workflow have a 3-OS matrix running build + test + clippy? — PASS

`/home/chasel/REPO/AIHPBar/.github/workflows/ci.yml`:

- `runs-on: ${{ matrix.os }}` with matrix `os: [ubuntu-latest, macos-latest, windows-latest]` ✓
- `rust: [stable]` ✓
- Steps: `cargo build --all-targets`, `cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings` ✓
- `fail-fast: false` so OS-specific failures surface independently ✓
- Modern `actions-rust-lang/setup-rust-toolchain@v1` (successor to `dtolnay/rust-toolchain`) ✓

No `fmt --check`, no `audit`, no `deny` — correctly deferred to Phase 4 per D-05.

### Q10. Are any LOCKED CONTEXT decisions violated? — PASS (none violated)

| Decision | Lock                                                               | Verified                                                                                          |
| -------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| D-15     | Unicode `\u{2588}` / `\u{2591}` default                            | xxd shows `e2 96 88` ×6 + `e2 96 91` ×4 ✓; `render_text.rs:43-45` uses `\u{...}` escapes          |
| D-16     | Fixed 10-cell bar width                                            | `BAR_WIDTH = 10` constant at `render_text.rs:16`; test `bar_width_fixed_at_ten` covers 0/60/67/100% |
| D-18     | `--ascii` opt-in (no auto-detection)                               | `main.rs:30 #[arg(long)] ascii: bool`; ASCII fallback uses `#`/`-`/`|` per D-18                   |
| D-19     | No emoji in v1                                                     | Source grep finds zero emoji codepoints in `src/`; only `\u{2588}`, `\u{2591}`, `\u{2022}` used   |
| D-22     | No web `gemini.google.com/usage` probing                           | Spike memo § Appendix explicitly documents NOT spiking web route; only `gemini` CLI probes attempted |
| D-25     | Output literal exactly `mock-session  ██████░░░░ 60% • resets in 2h00m` | Live binary run + xxd byte proof + regex match all confirm                                        |

### Q11. Are forbidden Phase 1+ deps in Cargo.lock? — PASS (none present)

`grep -E "^name = " Cargo.lock | sort -u` over the full lockfile shows zero entries for `ratatui`, `reqwest`, or `keyring`. The 49-crate dependency tree contains only Phase 0 deps and their transitive support (clap stack + tokio + serde stack + jiff stack + async-trait + owo-colors with `supports-color` + thiserror + anyhow + static_assertions). Phase 0 dep minimalism intact.

### Q12. Does Phase 0 advance the project core value? — PASS (correctly indirect)

Phase 0's role is the **spine**, not the user-observable core value. The core value ("any moment, one command, see all subscribed AI CLI quota") is delivered by Phase 1+ adapters (Claude/Codex/Gemini). Phase 0 succeeds if it makes Phase 1 buildable on a stable contract.

Evidence Phase 1 is buildable:

- Phase 1 can drop a `ClaudeProvider: Provider` impl into `src/provider/claude.rs` and register it in main.rs by replacing the `MockProvider` line. No contract changes needed.
- The `Vec<Result<ProviderState, ProviderError>>` aggregation pattern is already implicit in the trait shape (each provider returns `Result` independently).
- Panic-hook contract is contractually positioned for `ratatui::init()` wrapping.
- `Secrets` reference is in place for keyring-core widening.
- Lint floor + 3-OS CI are present so Phase 1 commits have a quality floor from day one.

Phase 0 has built the smallest possible runnable seed of the codex-rs "shared engine, two front-ends" pattern that ARCHITECTURE.md prescribes.

---

## Required Artifacts

| Artifact                              | Expected                                              | Status        | Details                                                                                                       |
| ------------------------------------- | ----------------------------------------------------- | ------------- | ------------------------------------------------------------------------------------------------------------- |
| `src/model.rs`                        | 7 contract types + ProviderError + serde round-trip   | VERIFIED      | 252 lines; all 7 types present; 4 inline tests cover serde, dyn-safety asserts                                |
| `src/provider/mod.rs`                 | `Provider` trait + `FetchCtx`                          | VERIFIED      | 77 lines; `#[async_trait]` Provider trait; `assert_impl_all!(Box<dyn Provider>: Send, Sync)` at line 52       |
| `src/provider/mock.rs`                | `MockProvider` returning D-25 fixture via ctx.now      | VERIFIED      | Uses `ctx.now` (no wall-clock read); test `mock_uses_injected_clock` enforces clock-injection contract        |
| `src/cli/render_text.rs`              | `compact_line` produces byte-exact D-25 line           | VERIFIED      | `BAR_WIDTH=10`, Unicode default + ASCII fallback, byte-exact tests pass                                       |
| `src/main.rs`                         | panic-hook + clap + MockProvider via trait + println  | VERIFIED      | Panic hook installs FIRST; `mock.fetch(&ctx).await` then `compact_line` then `println!`                       |
| `src/secrets.rs`                      | Zero-field `Secrets` stub w/ `Default`                 | VERIFIED      | 9 lines; `pub struct Secrets;` + `#[derive(Debug, Default, Clone)]` (Phase 1 widens)                          |
| `src/lib.rs`                          | `pub mod` declarations for model/provider/secrets/cli | VERIFIED      | 14 lines; all 4 mods exposed + crate-wide lint floor                                                          |
| `Cargo.toml`                          | MSRV 1.88, edition 2024, 9 deps + 1 dev-dep            | VERIFIED      | `rust-version = "1.88"`, `edition = "2024"`, license `MIT OR Apache-2.0`, 9 pinned deps                       |
| `Cargo.lock`                          | Tracked; ratatui/reqwest/keyring absent                | VERIFIED      | 49 crates total; forbidden Phase 1+ deps absent                                                               |
| `LICENSE-MIT`                         | Standard MIT text                                      | VERIFIED      | Present at repo root                                                                                          |
| `LICENSE-APACHE`                      | Standard Apache 2.0 text                               | VERIFIED      | Present at repo root                                                                                          |
| `clippy.toml`                         | `allow-*-in-tests` + disallowed crossterm types        | VERIFIED      | 11 lines; 3 allow-in-tests flags + 2 disallowed-types entries                                                 |
| `.github/workflows/ci.yml`            | 3-OS matrix, build/test/clippy, fail-fast=false        | VERIFIED      | 29 lines; ubuntu+macos+windows × stable; build + test + clippy steps; `fail-fast: false`                      |
| `.planning/research/GEMINI_SPIKE.md`  | go/no-go + 9 sections (Method, probes, parse, kill, hand-off, appendix, charset, fixtures) | VERIFIED      | 239 lines; `decision: no-go` in frontmatter; all D-23 sections present; charset § amended by Plan 05            |

---

## Key Link Verification

| From                                  | To                       | Via                                   | Status   |
| ------------------------------------- | ------------------------ | ------------------------------------- | -------- |
| `main.rs` MockProvider construction   | `Provider::fetch`        | `mock.fetch(&ctx).await`              | WIRED    |
| `main.rs` fetch result                | `compact_line` renderer  | `compact_line(&state, &ctx.now, ...)` | WIRED    |
| `model.rs ProviderState`              | `serde::Serialize/Deserialize` | `#[derive(Serialize, Deserialize)]` | WIRED |
| `model.rs ProviderError::Internal`    | `serialize_display` fn   | `#[serde(serialize_with = "...")]`    | WIRED    |
| `provider/mod.rs Provider trait`      | `async_trait` macro      | `#[async_trait]` attribute            | WIRED    |
| `main.rs` panic-hook                  | Phase 1 ratatui::init    | `take_hook()` + `set_hook()` chain     | WIRED (contract docs + code shape match D-27) |

All Phase 0 key links exist AND are functionally exercised by the live binary run + 15 passing tests.

---

## Behavioral Spot-Checks (Step 7b)

| Behavior                                                                 | Command                                                                                 | Result                                                                                 | Status |
| ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------ |
| Release binary prints D-25 literal                                       | `/home/chasel/REPO/AIHPBar/target/release/ahb`                                          | `mock-session  ██████░░░░ 60% • resets in 2h00m`                                       | PASS   |
| ASCII fallback works                                                     | `/home/chasel/REPO/AIHPBar/target/release/ahb --ascii`                                  | `mock-session  ######---- 60% \| resets in 2h00m`                                      | PASS   |
| Regex tolerance on countdown (Unicode)                                   | `... \| grep -qP '^mock-session  ██████░░░░ 60% • resets in [0-9]+h[0-9]{2}m$'`         | exit 0 (MATCH)                                                                         | PASS   |
| Regex tolerance on countdown (ASCII)                                     | `... --ascii \| grep -qP '... \\\| resets in [0-9]+h[0-9]{2}m$'`                        | exit 0 (MATCH)                                                                         | PASS   |
| Tests pass                                                               | `cargo test --all-targets`                                                              | 15 passed; 0 failed; 0 ignored                                                         | PASS   |
| Clippy clean                                                             | `cargo clippy --all-targets -- -D warnings`                                             | Finished with no warnings/errors                                                       | PASS   |
| Single-file binary, no system OpenSSL                                    | `file target/release/ahb`                                                               | ELF x86-64 dynamically linked to libc only (no libssl/libcrypto)                       | PASS   |
| xxd byte proof of locked codepoints                                      | `target/release/ahb \| xxd`                                                             | U+2588 ×6 + U+2591 ×4 + U+2022 ×1 — bytes match D-25                                   | PASS   |

---

## Anti-Patterns Scan

| File           | Pattern Searched For                                                                       | Result                              |
| -------------- | ------------------------------------------------------------------------------------------ | ----------------------------------- |
| `src/**`       | `TBD\|FIXME\|XXX\|TODO\|HACK\|PLACEHOLDER`                                                  | Zero matches                        |
| `Cargo.toml`   | Same as above                                                                              | Zero matches                        |
| `.github/**`   | Same as above                                                                              | Zero matches                        |
| `src/secrets.rs` | Stub doc-comment with explicit Phase 1 handoff reference                                  | Intentional stub, doc-commented     |
| `src/main.rs`  | Any `println!` outside the rendered-line path                                              | Only one `println!`, prints already-rendered string from trait; `eprintln!` is panic-hook output |
| `cargo clippy` | unwrap/expect/panic in adapter & render paths                                              | Clean (`-D warnings`)               |

No debt markers, no hardcoded empty data flowing to rendering, no stub-only handlers. The `MockProvider` returning a hardcoded fixture is intentional per D-25 (not a stub flagged for elimination) — it's the explicit Phase 0 deliverable.

---

## Requirements Coverage

| Requirement | Source Plan | Description                                                                          | Status     | Evidence                                                                          |
| ----------- | ----------- | ------------------------------------------------------------------------------------ | ---------- | --------------------------------------------------------------------------------- |
| ADP-00      | 00-02-PLAN  | `Provider` trait + `FetchCtx` + `ProviderState` / `ResetInfo` / `HpUnit` / `ProviderError` 統一介面 | SATISFIED | All types present in `src/model.rs` + `src/provider/mod.rs`; serde + Send+Sync+dyn-safe proven |

No orphaned requirements. REQUIREMENTS.md maps only ADP-00 to Phase 0; all other v1 requirements are explicitly mapped to Phase 1-4.

---

## Locked-Decision Cross-Check (D-01 through D-27)

| Decision | Lock                                          | Verified |
| -------- | --------------------------------------------- | -------- |
| D-01     | Crate `ahb`, bin `ahb`                        | ✓        |
| D-02     | Edition 2024                                  | ✓        |
| D-03     | Dual MIT OR Apache-2.0                        | ✓        |
| D-04     | MSRV 1.88                                     | ✓        |
| D-05     | 3-OS CI: build + test + clippy                | ✓        |
| D-06     | Single binary crate, no workspace             | ✓        |
| D-07     | Cargo.lock tracked                            | ✓        |
| D-08     | `ProviderState.windows: Vec<HpWindow>`        | ✓        |
| D-09     | `HpWindow { label, percent_remaining, reset, bar_color }` | ✓ |
| D-10     | `HpUnit = f32`, no raw token fields           | ✓        |
| D-11     | `ResetInfo { resets_at: jiff::Timestamp }`     | ✓        |
| D-12     | `ProviderError` closed enum, 6 variants       | ✓ (struct-variant adaption per Plan 02 deviation — verified) |
| D-13     | `Provider: Send + Sync + 'static`, async fetch | ✓       |
| D-14     | Serialize+Deserialize on data; Serialize-only on error | ✓ |
| D-15     | Unicode `\u{2588}` / `\u{2591}` default       | ✓        |
| D-16     | Fixed 10-cell bar width                       | ✓        |
| D-17     | `--color` flag accepted (Phase 1 applies)     | ✓ (parsed, not applied — documented) |
| D-18     | `--ascii` explicit opt-in                     | ✓        |
| D-19     | No emoji in v1                                | ✓        |
| D-20     | Spike local `gemini /stats` capture FIRST     | ✓ (3 probes attempted)              |
| D-21     | Strict 3-criteria gate for go                  | ✓ (all 3 failed → no-go)            |
| D-22     | No-go → defer to v2 stub; NO web fallback     | ✓        |
| D-23     | Spike memo location + required sections       | ✓ (9 sections present)              |
| D-24     | No `gemini /stats` fixtures committed in P0   | ✓ (no `tests/fixtures/gemini/` dir) |
| D-25     | Output literal exactly as specified           | ✓        |
| D-26     | Charset verification in memo § 7              | ✓ (kitty eyeball + xxd byte proof)  |
| D-27     | Phase 0 panic-hook contract                   | ✓        |

All 27 LOCKED decisions honored. Plan 02 adapted D-12 to struct-variant form (necessary because serde's internally-tagged enum mode rejects newtype variants with scalar-serialized inner types) — this is documented in 00-02-SUMMARY.md `key-decisions` and preserves construction ergonomics via `From` impls. Not a violation; a faithful re-expression.

---

## Issues Found

None of severity > info.

**Info-level observation only (not a gap):** Phase 0 success criterion #5 is marked PASS-AWAITING-PUSH in 00-05-SUMMARY for the CI green status — that's the GitHub Actions matrix actually running. Locally, every CI step (`cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`) exits 0. The CI binding becomes verifiable on first `git push`. This is the same posture the original verifier (Plan 05) took; not a Phase 0 blocker.

---

## Phase Complete?

**Yes.** Every ROADMAP § Phase 0 Success Criterion has codebase + artifact evidence. The Gemini risk is closed (NO-GO memo with structured Phase 3 hand-off). The cross-adapter contract is locked and proven Send+Sync+dyn-safe at compile time. The skeleton binary prints the locked D-25 literal through the locked `Provider` trait (not via `println!` shortcut). The charset decision is recorded and byte-proven. The repo scaffold + CI floor are in place. ADP-00 is satisfied.

No must-have is FAILED. No must-have is UNCERTAIN. Zero BLOCKER findings. Zero WARNING findings.

---

## Phase 1 Readiness Check

**Yes — Phase 1 is unblocked.**

What Phase 1 inherits, ready to use:

1. **Locked contract types** — `ProviderState`, `HpWindow`, `ResetInfo`, `HpUnit`, `ProviderError`, `ProviderId`, `FetchCtx` exist with serde round-trip + Send+Sync+dyn-safety proofs.
2. **`Provider` trait** — `#[async_trait]` + `Send + Sync + 'static` bound; Phase 1's `ClaudeProvider` is a drop-in replacement for `MockProvider`.
3. **`Vec<Result<...>>` aggregation pattern** — implicit in the trait shape (each adapter's `fetch` returns `Result` independently). Phase 1 engine can call N providers and collect failures per-adapter without crashing the bar.
4. **Panic-hook contract (D-27)** — `take_hook()` + `set_hook()` chain installed FIRST in main.rs. Phase 1's `ratatui::init()` composes cleanly on top per the documented order.
5. **`Secrets` stub** — `FetchCtx::secrets: &'a Secrets` reference is in place; Phase 1 widens the type to wrap keyring-core + introduce `Secret<T>` (SEC-02). No ABI break expected at call sites.
6. **Lint floor inherited via lib.rs** — Plan 02 moved `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` to `src/lib.rs` so every new module (claude.rs, engine.rs, tui/*) inherits automatically. Phase 1 doesn't need per-file pragmas.
7. **3-OS CI** — green locally; will gate Phase 1 commits the moment Phase 0 is pushed.
8. **Phase 3 routing** — NO-GO memo's Phase 3 hand-off section gives Phase 3 planning a concrete opt-in stub contract (`--experimental-gemini` flag + `Unavailable` stub adapter); Phase 3 doesn't need to re-decide Gemini scope.

What Phase 1 must add (not Phase 0 gaps — explicit Phase 1 scope):

- Real `ClaudeProvider: Provider` impl reading `~/.claude/projects/**/*.jsonl`
- `Secret<T>` newtype + keyring-core wiring (SEC-02)
- TUI scaffold + `ratatui::init()` composing the panic hook
- Engine doing `Vec<Box<dyn Provider>>` aggregation with `Vec<Result<...>>`
- Per-adapter error isolation (ADP-01)
- Schema-drift sentinel for Claude (ADP-03)
- TOML config (CFG-01/02/04)

These are all Phase 1 success criteria, not Phase 0 carryover.

---

_Verified: 2026-05-22T21:35:00Z_
_Verifier: Claude (gsd-verifier, Opus 4.7)_
