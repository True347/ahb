# Walking Skeleton — AI HP Bar (AHB)

**Phase:** 1
**Generated:** 2026-05-23

## Capability Proven End-to-End

> A developer with Claude Code installed runs `AHB` and sees one real `claude` HP-bar line that reports their actual 5h-window % remaining and reset countdown — computed live from `~/.claude/projects/**/*.jsonl`, rendered through the Phase 0 `Provider` trait + `compact_line` format, exiting cleanly.

The skeleton thread the data takes:

```
~/.claude/projects/<slug>/<uuid>.jsonl   (real file system)
  -> glob discovery                       (provider/claude/jsonl.rs)
  -> BufReader.lines() + serde_json       (D-35 streaming parser)
  -> sum cache_creation_input_tokens      (D-33 amended per L1)
  -> 5h cluster anchor + percent          (provider/claude/window.rs)
  -> ProviderState                        (model.rs — unchanged)
  -> Engine.refresh_all                   (JoinSet fan-out, 1 adapter)
  -> Vec<(ProviderId, Result<...>)>       (Phase 0 contract preserved)
  -> render_text::render_all              (compact_line per row)
  -> println!                             (stdout)
```

## Architectural Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Async runtime | `tokio` upgraded to `["rt-multi-thread","macros","fs","time","signal","sync"]` (was `["rt","macros"]` in Phase 0) | Engine needs `JoinSet` (`sync`), `fs::read_dir` (`fs`), `time::interval` (`time`), `signal::ctrl_c` (`signal`) for TUI; multi-thread runtime so a slow JSONL read does not stall the next 15s tick. Phase 3 adds nothing new. |
| Concurrency primitive | `tokio::task::JoinSet` with per-task timeout via `tokio::time::timeout(per_provider_timeout, p.fetch(...))` (D-28 + D-29) | Stronger than `join_all + timeout`: a slow adapter from the previous tick can be `abort()`-ed when a new tick fires. `HashMap<task::Id, ProviderId>` bookkeeping (RESEARCH Pitfall L4 fix) recovers the ProviderId when `JoinError::is_panic()` fires. |
| Per-adapter timeout default | `DEFAULT_PER_PROVIDER_TIMEOUT = Duration::from_secs(2)` (Phase 1) | Pure-local IO; 2s is generous. Phase 3 raises for HTTP adapters via CFG-03 (out of scope). |
| Engine result shape | `Vec<(ProviderId, Result<ProviderState, ProviderError>)>` (unchanged from Phase 0 contract) | One adapter failure cannot blank the whole bar — engine never propagates first error. |
| Config storage | Bare `serde` + `toml` 0.8, no `figment` | One TOML file, one struct (D-36). Phase 3 may revisit when env-var overrides ship. |
| Config location | `directories::ProjectDirs::from("", "", "ahb").config_dir()/config.toml` (D-39) | Cross-OS (Linux `~/.config/ahb/`, macOS `~/Library/Application Support/ahb/`, Windows `%APPDATA%\ahb\`). Six-arg form unchanged from 5.x → 6.x (Pitfall L5). |
| First-run behavior | Auto-create default `config.toml` from `include_str!("../templates/default-config.toml")` then `exit(0)` with literal `initialized {path} — enable providers and rerun` (D-37) | Pipe-safe (no interactive wizard); user sees all available keys at once. |
| Unknown-key policy | Warn + ignore via `toml::Value` pre-pass; no `#[serde(deny_unknown_fields)]` (D-38) | Forward-compat: future versions adding keys don't break older binaries. |
| Secret storage | `keyring-core` 1.0 wired end-to-end; Phase 1 never actually stores/loads a secret (Claude reads local JSONL, needs none) (D-40) | Burn down platform-specific keyring bugs (macOS prompt, Windows session, Linux dbus) BEFORE Phase 2/3 plug real credentials. |
| Keyring missing fallback | Hard error, exit code 2, stderr literal from D-41 — NEVER silent file fallback | STACK.md binding. |
| `Secret<T>` shape | Hand-rolled (~30 lines): `pub struct Secret<T: Zeroize + Clone>(T)` with `Drop`→zeroize, `Debug`→`***`, `Serialize`→`"[REDACTED]"`, no `Deserialize`, single `.expose()` unwrap path (D-42) | New dep `zeroize` only; `secrecy` crate not used. Greppable audit trail. |
| 5h window field choice | Sum `cache_creation_input_tokens` only (D-33 AMENDED per RESEARCH Pitfall L1); `input_tokens + output_tokens` are upstream-broken streaming placeholders (~75% are 0/1, undercounted 100-174×) | ccusage issue #866. README must note "best-effort estimate; upstream JSONL incomplete." |
| 5h token limit constant | `pub const CLAUDE_5H_TOKEN_LIMIT: u64 = 44_000;` (Pro tier estimate; D-44) | Anthropic does not publish exact numbers; tokenmix.ai 2026 + ccusage social-source converge on ~44k. Doc comment notes "revisit quarterly." Max5/Max20 users see undercounted bars; Phase 2 CFG-03 may add plan-tier knob. |
| File discovery | `glob` crate, pattern `~/.claude/projects/**/*.jsonl`, follow symlinks default (D-32 + L8 documented in module doc) | Matches REQ ADP-02 wording verbatim. |
| JSONL streaming | `BufReader::new(File).lines()` per-line `serde_json::from_str`; mid-file failure → `tracing::warn!` + skip; trailing-line failure → silent skip (D-35) | Tolerates Claude's in-flight append; never pulls large session into RAM. |
| Provider directory layout | `provider/claude/{mod.rs, jsonl.rs, window.rs}` — separated concerns (file IO, line parsing, cluster math) | Phase 2's `provider/codex/` mirrors this shape. |
| Engine directory layout | `engine/{mod.rs, fanout.rs, events.rs}` | Pattern set for Phase 3 (where engine grows refresh-policy logic). |
| TUI lifecycle | `ratatui::run(closure)` — NOT manual `init/restore` pair (RESEARCH Pitfall L2 overrides PITFALLS.md error) | `run()` installs panic-safe terminal restoration automatically; chains over Phase 0's `install_phase0_panic_hook`. |
| Crossterm access | `ratatui::crossterm::*` re-export only; NEVER add `crossterm` to Cargo.toml | Two crossterm versions in dep tree silently breaks ratatui rendering. `clippy.toml disallowed-types` enforces. |
| TTY detection | `std::io::IsTerminal::is_terminal(&io::stdout())` (stable since 1.70) — applied at CLI render-color decision AND at TUI entry refusal | No `atty` crate; no `libc::isatty` FFI. |
| Color decision priority | `--json` → `--color=never` → `--color=always` → `NO_COLOR` env → TTY detection → default colored (Pattern 4 / RESEARCH lines 545-572) | Four color-off paths binding (UI-SPEC Color section). |
| Wall-clock injection | Only `src/main.rs` calls `jiff::Timestamp::now()`; all adapters use `ctx.now` (Phase 0 contract extended to `claude.rs`) | Acceptance grep guards `src/provider/` (currently guards `mock.rs`; extends to `claude/**`). |
| Lint floor | `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)] + warn(pedantic)` per new module (inherited from lib.rs/main.rs) | Scoped `#[allow(...)]` with comment is the only escape hatch (e.g., `MockProvider` panic injection); tests get blanket `clippy.toml` allow. |
| Logging | `tracing` + `tracing-subscriber` with `EnvFilter::from_default_env()` (`RUST_LOG=ahb=debug`) | Phase 1 is where logs start mattering (JSONL parse warns, schema drift, keyring backend selection). |
| Mock panic injection | Env-var-gated `AHB_DEBUG_PANIC=adapter:mock` inside `MockProvider::fetch`, scoped `#[allow(clippy::panic)]` (RESEARCH A9) | Lets a release-mode integration test trigger the panic-isolation path without `#[cfg(debug_assertions)]` complications. |

## Stack Touched in Phase 1

- [x] Project scaffold — Cargo.toml gains 8 prod deps (ratatui, keyring-core, zeroize, glob, directories, toml, tracing, tracing-subscriber) + 1 cfg-gated `*-keyring-store` per OS + 4 dev deps (assert_cmd, predicates, tempfile, regex)
- [x] Routing — CLI subcommand dispatch (`AHB` default → compact; `AHB tui` → ratatui surface)
- [x] Data layer — Read path: `~/.claude/projects/**/*.jsonl` (real). Write path: `~/.config/ahb/config.toml` (first-run init only). No DB.
- [x] UI — CLI compact line (Plan 01) AND TUI fixed-frame (Plan 03), both wired to the same `Engine.refresh_all()`
- [x] Deployment — `cargo build --release && ./target/release/ahb` runs end-to-end; CI matrix (Phase 0) keeps build-test-clippy green across linux/macos/windows

## Out of Scope (Deferred to Later Slices)

> Anything not in the skeleton. Explicit here to prevent future phases from re-litigating Phase 1's minimalism.

- **Codex adapter** (`provider/codex/*`) — Phase 2 (ADP-04)
- **Gemini adapter** — Phase 3 (ADP-05, conditional on Phase 0 spike)
- **`--detailed` / `--json` output formats** — Phase 2 (CORE-03 / CORE-04)
- **Exit-code wiring (`0/1/2` per CORE-06)** — Phase 2 (Phase 1 emits 0 on success or first-run, 2 on keyring-unavailable; "all providers failed = 1" is Phase 2)
- **Per-provider `refresh_interval` config override** — Phase 3 (CFG-03)
- **Per-provider `auth_source` / cookie-path config** — Phase 3 (CFG-03 / SEC-04 extension)
- **Stale-on-error cache (moka)** — Phase 3
- **`secret_storage = "file"` 0600 backend implementation** — deferred (Phase 1 only references it in the D-41 error message)
- **`AHB_DISABLE_KEYRING=1`, `AHB_CONFIG_PATH=…` env overrides** — Phase 4 polish
- **`--strict-config` opt-in (`deny_unknown_fields`)** — declined; warn+ignore is sufficient
- **Interactive first-run TTY wizard** — Phase 4 polish or backlog
- **Plan-tier auto-detection (Max5/Max20)** — explicit PROJECT.md Out-of-Scope; would conflict with "read declared limit, don't infer"
- **Distribution (cargo-dist, binstall, Gatekeeper docs, crates.io polish)** — Phase 4

## Subsequent Slice Plan

Each later phase adds one vertical slice on top of this skeleton without altering its architectural decisions:

- **Phase 2 (Codex + Output Formats):** Add `provider/codex/` with `spawn_blocking` + read-only SQLite. Extend `render_text` with `--detailed` multi-line per provider. Add `cli/render_json.rs` emitting `schema_version: 1`. Wire exit codes per CORE-06.
- **Phase 3 (Gemini + Cache):** Add `provider/gemini/` (full HTTP or stub per Phase 0 memo). Introduce moka cache layer for stale-on-error. Wire per-provider `refresh_interval` override into engine config.
- **Phase 4 (Distribution):** `cargo-dist` GitHub Actions workflow, `cargo binstall` metadata, macOS Gatekeeper README section, `cargo deny` in CI, crates.io metadata polish.
