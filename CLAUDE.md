<!-- GSD:project-start source:PROJECT.md -->
## Project

**AI HP Bar (AHB)**

AHB 是一個用 Rust 寫的 CLI + TUI 工具，把多家 LLM 訂閱（Claude Code、Codex CLI、Gemini CLI 等）目前的 session 剩餘額度與 reset 倒數，用一條像遊戲血條的 HP bar 顯示出來。CLI mode `AHB` 印出緊湊狀態列、可用 flag 切換 detailed / json 輸出；TUI mode 提供固定畫面、可設定 refresh 頻率（預設 15s）。受眾是同時用多個 AI CLI 的開發者本人，最多走到 open source 分發；不是商業產品。

**Core Value:** **任何時刻、一個指令，立即看到所有訂閱的 AI CLI「現在還剩多少 session 額度、什麼時候 reset」。** 重置式血條的視覺隱喻是必要的；只顯示累積活動量（only-goes-up dashboard）不算達成 core value。

### Constraints

- **Tech stack**: Rust — 單 binary 分發、ratatui 是 TUI SOTA、未來 daemon 模式好擴。理由：對「給其他 multi-CLI 使用者用」的分發體驗最低摩擦。
- **Data source per provider**: 三家異質，預期需要 per-provider adapter，部分得靠 local state 解析 + 部分得靠 HTTP（Gemini）。
- **Distribution**: 自用為主，最多 open source；不寫 license / billing / multi-tenant 基礎建設。
- **Refresh budget**: TUI 預設 15s，避免對需要 HTTP 的 provider（Gemini）打太多；rate-limit 自保護由 adapter 內部處理。
- **Privacy**: 本機工具，不上傳任何 usage 資料到第三方。Provider 認證 token / cookie 必須只留在本機 config / OS keyring。
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## TL;DR — One-Line Picks
| Concern | Pick | Why (one line) |
|---|---|---|
| CLI parser | `clap` 4.6.1 (with `derive`) | Standard; subcommands + auto-completion + auto-help out of the box. |
| TUI | `ratatui` 0.30.0 + `crossterm` backend | The maintained successor to abandoned `tui-rs`; default backend is cross-platform. |
| Async runtime | `tokio` 1.52 (`rt-multi-thread` or `rt` only) | Required by `reqwest`; no point fighting it for a tool this small. |
| HTTP | `reqwest` 0.13.3 with `rustls-tls` + `cookies` + `json` | Cookie store for Gemini session; rustls = no OpenSSL = clean static binary. |
| Config | `serde` + `toml` (no framework) — escalate to `figment` only if env-var overrides matter | One TOML file, one struct, one `from_str`. Frameworks are overkill at v1. |
| Secrets | `keyring-core` 1.0.0 (NOT the deprecated `keyring` v4 facade) | v4 of `keyring` is now a demo/CLI shell; `keyring-core` is the library to depend on. |
| SQLite | `rusqlite` 0.39.0 with `bundled` feature | Read-only access to Codex `state_5.sqlite`; sync is fine, no async needed. |
| JSON / JSONL | `serde_json` 1 with `BufRead::lines()` | Trivial; no separate crate needed. |
| Date/time | `jiff` 0.2.24 | New project, IANA TZ + reset-window arithmetic are exactly its strengths. |
| Terminal styling (CLI mode) | Reuse `ratatui::crossterm` re-export (no separate `termcolor`) | Avoids double-version crossterm hazard. |
| Logging | `tracing` 0.1 + `tracing-subscriber` | Same crate works for daemon evolution; `RUST_LOG` env compatible. |
| Errors | `anyhow` 1.0 + `thiserror` 2.0 | Standard split: `thiserror` for library errors, `anyhow` for `main`. |
| Snapshot tests | `insta` 1.47 + `ratatui::backend::TestBackend` | Officially recommended in Ratatui docs. |
| HTTP mocks | `wiremock` 0.6.5 | Async, `tokio`-native; the de facto choice for `reqwest`-based code. |
| Distribution | `cargo-dist` 0.32.0 (v1.0.0-rc.1 already tagged) | Active, generates GH Actions + installers + multi-arch artifacts. |
| Cross-compile (optional) | `cross` 0.2.5 (if `cargo-dist`'s containerised builds aren't enough) | Last release 2023, but `cargo-dist` covers most use cases now. |
## Recommended Stack
### Core Technologies
| Technology | Version | Purpose | Why Recommended |
|---|---|---|---|
| **Rust** | edition 2021, MSRV ≥ 1.88 | Language | Constrained by ratatui 0.30 MSRV; matches stable toolchain on every distro today. |
| **clap** | 4.6.1 (released 2026-04-15) | CLI argument + subcommand parsing | De-facto Rust standard; `derive` macros give you `--help`, `--version`, completions, and `compact`/`detailed`/`json` flag switching with zero ceremony. |
| **ratatui** | 0.30.0 (released 2025-12-26) | Terminal UI rendering | The actively-maintained successor to `tui-rs` (archived 2023). 20k+ stars, recent commits, has `TestBackend` for snapshot testing. |
| **crossterm** | 0.29 (via `ratatui` re-export) | Terminal backend (input + cursor + raw mode) | Default ratatui backend; works on Linux/macOS/Windows. Ratatui 0.30 defaults to crossterm 0.29; use `ratatui::crossterm` to avoid dual-version hazard. |
| **tokio** | 1.52.x | Async runtime | Required transitively by `reqwest` and `wiremock`. Use feature flags `["rt-multi-thread", "macros", "fs", "time", "signal"]` — *not* `["full"]`. |
| **reqwest** | 0.13.3 (released 2026-04-27) | HTTPS client for Gemini usage endpoint | Cookie jar support for session-cookie auth, JSON deserialisation, automatic redirects. Now defaults to `rustls` (since v0.13), no OpenSSL. |
| **rusqlite** | 0.39.0 (released 2026-03-15) with `["bundled"]` | Read Codex `state_5.sqlite` | Codex CLI stores session metadata in `~/.codex/state_5.sqlite` (verified via `openai/codex` source). Sync API is exactly right for our short read queries; `bundled` ships SQLite statically so no system dep. |
| **serde** | 1.0.228 + `derive` | (De)serialise everything | Universal; required by `serde_json`, `toml`, `reqwest::json`. |
| **serde_json** | 1.0 | JSON + JSONL parsing | Parse Claude `stats-cache.json`, Codex JSONL rollouts (one `Value` per line via `BufRead::lines`), Gemini HTTPS responses. |
| **toml** | 0.8 | Config file format | Native Rust, serde-integrated, human-editable. |
| **jiff** | 0.2.24 (released 2026-04-23) | Date/time + reset-window math | Recommended for new projects in 2026: IANA TZ built-in, calendar arithmetic, Temporal-style API. We need "5 hours from session start" and "next weekly reset Monday 00:00 local" — exactly Jiff's wheelhouse. |
| **keyring-core** | 1.0.0 (released 2026-04-21) | Provider secret/cookie storage | Cross-platform: macOS Keychain, Windows Credential Manager, Linux Secret Service (libsecret) / Keyutils. **Critical:** do NOT use the `keyring` crate (v4 reduced it to a demo/CLI shell) — depend on `keyring-core` directly. |
| **tracing** | 0.1.41 + `tracing-subscriber` 0.3 | Structured logging | Works for both short CLI runs and the long-running TUI loop; `RUST_LOG=ai_hp_bar=debug` compatible via `EnvFilter`. Future-proof if AHB ever grows a daemon. |
| **anyhow** | 1.0.102 | App-level error handling | Standard `main()` return type and error propagation. |
| **thiserror** | 2.0.18 | Library-level error types | Use inside per-provider adapters so the orchestration layer can pattern-match on `ProviderError::AuthRequired` etc. |
### Supporting Libraries
| Library | Version | Purpose | When to Use |
|---|---|---|---|
| **directories** | 5.x | Cross-platform config/data paths | Resolve `~/.config/ai-hp-bar/config.toml`, `~/Library/Application Support/ai-hp-bar/`, `%APPDATA%\ai-hp-bar\`. |
| **figment** | 0.10.18 | Layered config (file + env + flags) | Only if v1 needs env-var overrides beyond `RUST_LOG`. Otherwise plain `toml::from_str` is fine. |
| **insta** | 1.47.2 | Snapshot testing | Pair with `ratatui::backend::TestBackend` to lock in HP-bar rendering (`ratatui.rs/recipes/testing/snapshots/`). |
| **wiremock** | 0.6.5 | HTTP mocking | Test Gemini adapter without hitting `gemini.google.com`. `tokio`-native; pairs cleanly with `reqwest`. |
| **tempfile** | 3.x | Temp dirs in tests | Mock `~/.claude/`, `~/.codex/` directory layouts. |
| **assert_cmd** + **predicates** | 2.x / 3.x | Integration test the CLI binary | Verify `AHB`, `AHB --json`, `AHB --detailed` outputs. |
### Development Tools
| Tool | Purpose | Notes |
|---|---|---|
| **cargo-dist** | 0.32.0 — generate release artifacts | Configure in `Cargo.toml` `[workspace.metadata.dist]`. Produces tarballs/zips for `x86_64-{linux,apple-darwin,pc-windows-msvc}` + `aarch64-{linux,apple-darwin}`, plus shell/PowerShell installers and a GitHub Actions workflow. v1.0.0-rc.1 already tagged — stable enough to adopt. |
| **cross** | 0.2.5 — cross-compile via Docker | Only needed if `cargo-dist` containerised builds can't reach a target (rare). Last formal release Feb 2023 but still works; project is active. |
| **cargo-edit / cargo-upgrade** | Dependency hygiene | `cargo upgrade --incompatible` periodically. |
| **cargo-deny** | License/duplicate/security audit | Run in CI; catches double-crossterm and licence violations before release. |
| **cargo-nextest** | Faster test runner | Optional but standard in 2026 CI templates. |
| **just** | Task runner | Optional; many Rust CLI projects use a `justfile` for `release`, `lint`, `fmt`, `test`. |
## Installation
# Bootstrap the crate
# Core
# Dev
# One-time tooling
## Rationale Per Choice
### CLI parser — `clap` over `argh`/`lexopt`/`pico-args`
- **clap (derive)** wins on subcommand ergonomics (`AHB`, `AHB tui`, future `AHB doctor`), auto `--help`/`--version`, and shell completion generation. The "extra dependencies + slower compile" trade is irrelevant for a single-binary tool that compiles once.
- **argh** follows Fuchsia conventions, not Unix; rejects on principle for a Unix-targeted CLI.
- **lexopt / pico-args** are right for tools where every kB of binary matters; we already pull in `ratatui` + `reqwest`, so the savings are imaginary.
- **Confidence:** HIGH.
### TUI — `ratatui` (not `cursive`, not `tui-rs`)
- **`tui-rs` is archived** (fdehau/tui-rs, last push 2023-08, marked archived) — do not start a new project on it.
- **`ratatui` 0.30** ships with `TestBackend` for snapshot testing, has `Sparkline` (useful if v2 adds an HP history graph), 20k+ stars, weekly commits.
- **`cursive`** is widget/event-callback oriented — wrong shape for a periodically-refreshing dashboard.
- 0.30 has breaking changes from 0.29 (HorizontalAlignment rename, Backend trait Error+clear_region, MSRV 1.88) — design against 0.30 from day one; do not lock to 0.29.
- **Confidence:** HIGH.
### Async runtime — `tokio` (lean features), not `smol`/`async-std`
- **`async-std` is discontinued** (officially as of 2025; README redirects to smol).
- **`smol`** is technically appealing for a tool this size, but `reqwest` and `wiremock` both target tokio. Choosing smol means writing the HTTP layer twice (smol-native crate for runtime, then a tokio compatibility shim for tests) — net loss.
- The right move: tokio with a minimal feature set (`rt-multi-thread`, `macros`, `fs`, `time`, `signal`) and *not* `tokio = { features = ["full"] }`. Binary cost is small.
- **Confidence:** HIGH.
### HTTP client — `reqwest` over `ureq`/raw `hyper`
- **Gemini auth probably needs cookie persistence.** The `~/.config/gcloud/...` or `gemini.google.com/usage` path requires session cookies; `reqwest`'s built-in cookie jar (`cookies` feature) is a deciding factor.
- **`ureq` 3.x** is genuinely lighter and sync (no tokio), but: no cookie jar middleware story, less smooth retry/middleware ecosystem, and we already need tokio for `wiremock` tests. Pick `ureq` only if Gemini auth turns out to need nothing fancier than `Authorization: Bearer …`.
- **Raw `hyper`** is wrong abstraction level for this project.
- **TLS:** `rustls-tls`, not `native-tls`. Default in reqwest 0.13+, lets us ship `musl`-static on Linux without OpenSSL.
- **Confidence:** HIGH (caveat: re-evaluate after the Gemini spike — if it's just a bearer token, `ureq` becomes viable).
### Config — bare `serde` + `toml`, escalate to `figment` if needed
- v1 has one config file (`~/.config/ai-hp-bar/config.toml`). `let cfg: Config = toml::from_str(&fs::read_to_string(path)?)?;` is two lines, no magic.
- Escalate to **`figment`** if we need: env-var overrides (`AHB_REFRESH_SECS`), CLI-flag-overrides-config layering, or per-environment profiles. Figment composes Serialize types cleanly with clap.
- **`config-rs`** (`v0.15.23`) works too, but figment has friendlier error messages and a better story for clap integration.
- **Lock-in risk:** LOW — both crates produce a `Config` struct; swap is one file's worth of code.
- **Confidence:** MEDIUM (config strategy may evolve once we know how Gemini auth wants to be configured).
### Secrets — `keyring-core` 1.0 (do NOT use the `keyring` crate)
- The `keyring` crate (`v4.0.1`, May 2026) **was downgraded to a demo/CLI tool** per its README: *"If you are writing an application that uses keyring-compatible credential stores, you should not take a dependency on this crate."*
- **The library is `keyring-core` 1.0.0** (April 2026). Different API: stores are explicitly allocated at startup and registered via `set_default_store`, rather than picked at compile time via feature flags.
- **Backends:** macOS Keychain, Windows Credential Manager, Linux libsecret (Secret Service) + Keyutils, iOS Protected Data, Android SharedPreferences.
- **Fallback:** If user explicitly disables keyring (some Linux headless setups), write to `~/.config/ai-hp-bar/secrets.toml` with `0600` permissions. *Never* fall back silently — log a warning and require the user to set `secret_storage = "file"` in config.
- **Confidence:** HIGH on the migration; MEDIUM on the exact `keyring-core` API since it's pre-1.0-era documentation evolving rapidly. Verify against `docs.rs/keyring-core` at implementation time.
### SQLite — `rusqlite` not `sqlx`
- We're read-only against Codex's own DB (`~/.codex/state_5.sqlite`). No async, no migrations, no compile-time query checks — `rusqlite` is exactly the right size.
- `rusqlite` with `["bundled"]` statically links SQLite — no system dep, no version mismatch with Codex's own SQLite.
- **`sqlx`** is wonderful for our-own-database apps; it's overkill here and pulls tokio-postgres-style baggage.
- **Lock-in risk:** Codex storage layout (`state_5.sqlite`, schema version migrations like `migration_1`) is internal to Codex CLI. Treat the schema as **unstable**; isolate all SQL in `providers/codex/state.rs` and gate behind a probe query that confirms expected columns. (See PITFALLS.md.)
- **Confidence:** HIGH on the crate choice; MEDIUM on the durability of Codex's schema.
### JSON / JSONL — no extra crate
- Claude `stats-cache.json`: `serde_json::from_str::<StatsCache>(&fs::read_to_string(...)?)?`.
- Codex JSONL rollouts: `BufReader::new(File::open(...)?).lines()` + `serde_json::from_str::<RolloutEntry>(&line?)?` per line. No `jsonlines` crate needed.
### Date/time — `jiff` over `chrono` over `time`
- New project + reset-countdown logic + IANA timezone math = `jiff`'s sweet spot.
- **`chrono`** has the bigger ecosystem (sqlx, diesel integrations) but worse TZ story by default and panicky calendar math.
- **`time`** is best for embedded/perf-critical; not what we need.
- The case against jiff: smaller ecosystem, breaking changes still possible at 0.2.x. Mitigation: isolate datetime calls behind a `Clock` trait so we can swap later if needed.
- **Confidence:** MEDIUM-HIGH. If 0.2.x churn becomes painful, fall back to `chrono` 0.4.44 + `chrono-tz`.
### Terminal styling — reuse `ratatui::crossterm`, skip `termcolor`
- The CLI mode (`AHB` one-liner) wants ANSI colour. Use the same `crossterm` already pulled in by ratatui (`ratatui::crossterm::style`). Avoids the "two versions of crossterm in the dep tree" pitfall that breaks ratatui.
- **`termcolor`** (BurntSushi) is fine standalone but not worth the duplicate.
- **`anstyle`** / **`owo-colors`** are alternative options if we want a smaller API surface; `owo-colors` is particularly nice for one-shot colouring. Acceptable choice — but again, `crossterm` is already in the tree.
- **Confidence:** HIGH.
### Logging — `tracing` over `log` + `env_logger`
- Both work for v1. `tracing` is the future-proof pick (spans across async, structured fields, OTel integration if daemon mode ever happens).
- With `tracing-subscriber` + `EnvFilter`, `RUST_LOG=ai_hp_bar=debug` works the same as it does for `env_logger`.
- **Lock-in risk:** LOW — most of the codebase will use `tracing::info!` which is structurally identical to `log::info!`; migrating either way is mechanical.
- **Confidence:** HIGH.
### Tests — `insta` snapshots + `wiremock` HTTP mocks + `assert_cmd` CLI
- **`insta` + `ratatui::backend::TestBackend`** is the documented testing recipe for ratatui apps. Snapshots lock the HP-bar layout against regressions.
- **`wiremock`** for HTTP mocking — async-native, no globals (unlike older `mockito`). Spin up a local server per test.
- **`mockito`** 1.7.x also exists; works fine but `wiremock` integrates better with reqwest's async paths.
- **`assert_cmd`** for end-to-end CLI assertions on `AHB`, `AHB --json`, etc.
- **Confidence:** HIGH.
### Distribution — `cargo-dist`
- Auto-generates a release GitHub Actions workflow that produces:
- Plus shell installer (`curl … | sh`), PowerShell installer, optional Homebrew tap, npm wrapper.
- Active project (0.32.0 released 2026-05-21, v1.0.0-rc.1 tagged).
- Pair with `cargo install ai-hp-bar` for the "I already have Rust" path — no extra work, falls out of being on crates.io.
- **Confidence:** HIGH.
## Alternatives Considered
| Recommended | Alternative | When to Use Alternative |
|---|---|---|
| `clap` derive | `pico-args` / `lexopt` | If you genuinely need < 100KB binary delta — not us. |
| `ratatui` | `cursive` | If you want widget-callback event model rather than immediate-mode redraw. Not for periodic-refresh dashboards. |
| `tokio` | `smol` | If Gemini auth turns out to need *zero* HTTP machinery beyond a one-shot `ureq` call — then drop async entirely. |
| `reqwest` | `ureq` 3.x | If Gemini auth is plain Bearer token, no cookies, no redirects → `ureq` halves your async surface area. |
| Bare `toml`+serde | `figment` | If config layering with env vars / CLI overrides matters. |
| `keyring-core` | Plain file `0600` | Linux-headless servers without dbus; explicit user opt-in only. |
| `rusqlite` | `sqlx` | If we ever build *our own* DB (caching layer); not for reading Codex state. |
| `jiff` | `chrono` 0.4.44 | If ecosystem integrations matter more than correctness (none currently do). |
| `tracing` | `log` + `env_logger` | If you're 100% sure you'll never want spans/structured logs. We probably will. |
| `wiremock` | `mockito` 1.7.x | Marginally simpler API; mockito has more global state. |
| `cargo-dist` | Hand-rolled GHA + `cross` | If you need exotic targets cargo-dist doesn't support (rare). |
## What NOT to Use
| Avoid | Why | Use Instead |
|---|---|---|
| **`tui` / `tui-rs`** (fdehau/tui-rs) | Archived August 2023; superseded by ratatui. | `ratatui` 0.30 |
| **`keyring` crate v4** (`v4.0.1`) | As of v4 this crate is a *demo binary + Python shim*, explicitly disclaiming library use. | `keyring-core` 1.0 |
| **`async-std`** | Discontinued in 2025; project redirects to smol. | `tokio` (or `smol` if going async-free in the HTTP layer) |
| **`openssl` / `native-tls` for HTTPS** | OpenSSL system dep breaks musl-static, complicates Windows + Linux distros. | `rustls-tls` (reqwest default since 0.13) |
| **Direct `crossterm` dependency parallel to ratatui** | Two crossterm versions in dep tree silently break rendering (objects from v0.28 and v0.29 aren't interchangeable). | Use `ratatui::crossterm` re-export. |
| **`tokio = { features = ["full"] }`** | Pulls in process spawning, signals, sync primitives we don't use. ~30% larger binary. | Explicit feature list. |
| **`chrono` without `--no-default-features`** | Pulls oldtime/numeric APIs and historically had a CVE around timezone offset. | If you must use chrono, set `default-features = false, features = ["clock", "std", "serde"]`. |
| **`sqlx` for read-only Codex DB** | Pulls async machinery + offline query cache we don't use; risks `libsqlite3-sys` semver conflict with `rusqlite` if both ever appear. | `rusqlite` with `["bundled"]`. |
| **`reqwest` without `rustls-tls` flag rotation** | Default-features pulls `default-tls` which on Linux pulls native-tls → OpenSSL. | `--no-default-features` + explicit `rustls-tls,cookies,json`. |
| **Plain `println!` for CLI output formatting** | Inconsistent across `--json` (machine) vs default (human) vs `--detailed`. | Use `serde_json::to_writer` for JSON path; reserve `println!` for the human path; pipe both through a `Renderer` trait. |
## Stack Patterns by Variant
- Drop `reqwest`'s `cookies` feature.
- Consider dropping `reqwest` + `tokio` entirely → `ureq` 3.x sync.
- Saves ~1.5MB binary and removes the runtime.
- **Tradeoff:** lose async-mocked tests; gain simplicity.
- Keep `tokio` (good thing we already did).
- Add `tokio::time::interval` for refresh loop.
- Add `tracing` JSON output for logfile rotation.
- `cargo-dist` handles Homebrew tap auto-publishing with one config line.
- AUR is manual but the `cargo-dist` tarball is consumed as-is.
- Build with `--release` + `strip = "symbols"` + `lto = true` + `opt-level = "z"` in `[profile.release]`.
- Drop `reqwest` for `ureq`.
- Drop `tracing` for `log` + `env_logger`.
- Use `aws_lc_rs` instead of `ring` for rustls if licensing prefers.
## Version Compatibility
| Package A | Compatible With | Notes |
|---|---|---|
| `ratatui@0.30` | `crossterm@0.29` (default) or `crossterm@0.28` (via `crossterm_0_28` feature) | **Critical:** don't add `crossterm` to your own `Cargo.toml`. Use `ratatui::crossterm`. Two crossterm versions in the tree silently break rendering. |
| `ratatui@0.30` | Rust ≥ 1.88 | MSRV bump in 0.30. Use stable rustc 1.88+ in CI. |
| `reqwest@0.13` | `tokio@1.x`, `rustls@0.23` (transitive) | Default TLS changed to rustls in 0.13. |
| `rusqlite@0.39` | `libsqlite3-sys` 0.30 | If `sqlx` ever shows up in the dep tree, both will fight over `libsqlite3-sys`. Pick one. |
| `keyring-core@1.0` | Does NOT coexist with `keyring@4.x` as a library dep | They share types historically; depend on `keyring-core` only. |
| `tokio@1.52` | `wiremock@0.6.5` | `wiremock` works with current tokio. |
| `jiff@0.2.x` | `serde@1` (with `jiff` `serde` feature) | Still pre-1.0; expect minor breaking changes through 2026. Isolate behind a thin wrapper module. |
| `cargo-dist@0.32` | Rust ≥ 1.79 (workflow runner) | Pinned via `dist-version` in `Cargo.toml` `[workspace.metadata.dist]`. |
## Confidence Summary
| Pick | Confidence | Reason |
|---|---|---|
| Rust + clap + ratatui + tokio + reqwest + rustls + rusqlite + serde + serde_json + toml + tracing + anyhow + thiserror | HIGH | All current-major, all actively maintained, all verified against GitHub release API on 2026-05-22. |
| keyring-core (vs keyring v4) | HIGH on direction, MEDIUM on exact API | The v3→v4 split is verified. The new `set_default_store` API is younger; verify against `docs.rs/keyring-core` at impl time. |
| jiff (vs chrono) | MEDIUM-HIGH | Strong technical case; pre-1.0 churn risk. Isolate behind a `Clock` trait. |
| cargo-dist | HIGH | 0.32.0 released yesterday (2026-05-21), v1.0.0-rc.1 tagged. |
| Codex SQLite path (`~/.codex/state_5.sqlite`) | HIGH (path) / MEDIUM (schema) | Path verified in `openai/codex` source. Schema is internal — treat as unstable, gate every query with a probe. |
| Gemini using reqwest + cookies | MEDIUM | Depends on auth shape, which still needs a spike. If it's bearer-token, ureq becomes viable. |
## Lock-in Risks (and Mitigations)
## Sources
- GitHub Release API (`gh api repos/.../releases/latest`), accessed 2026-05-22 — verified versions for: clap (v4.6.1, 2026-04-15), ratatui (0.30.0, 2025-12-26), tokio (1.52.x, 2026-05-08), reqwest (v0.13.3, 2026-04-27), rusqlite (v0.39.0, 2026-03-15), keyring (v4.0.1, 2026-05-12), keyring-core (v1.0.0, 2026-04-21), crossterm (0.29 tag), tracing (0.1.41 line), serde (v1.0.228), chrono (0.4.44, 2026-02-23), cargo-dist (v0.32.0, 2026-05-21, v1.0.0-rc.1 tagged), insta (1.47.2, 2026-03-30), mockito (1.7.2, 2026-02-02), wiremock-rs (v0.6.5), figment (v0.10.18), config-rs (v0.15.23), anyhow (1.0.102, 2026-02-20), thiserror (2.0.18, 2026-01-18), ureq (3.3.0).
- `ratatui.rs/concepts/backends/`, `ratatui.rs/highlights/v030/`, `ratatui.rs/recipes/testing/snapshots/` — backend and snapshot guidance. (HIGH)
- `docs.rs/jiff/latest/jiff/` and `jiff/COMPARE.md` — jiff vs chrono guidance. (HIGH)
- `docs.rs/keyring-core` + `github.com/open-source-cooperative/keyring-rs/wiki/Keyring-Core` — v3→v4 migration policy. (HIGH)
- `seanmonstar.com/blog/reqwest-v013-rustls-default/` — rustls now reqwest default. (HIGH)
- `developers.openai.com/codex/config-sample` + `github.com/openai/codex` source (`codex-rs/rollout/src/state_db.rs`, `codex-rs/thread-store/README.md`) — confirms `~/.codex/state_5.sqlite` + JSONL session rollouts. (HIGH)
- `corrode.dev/blog/async/` — async-std discontinued; tokio dominant. (HIGH)
- `rust-cli-recommendations.sunshowers.io/cli-parser.html` — clap derive standard pick. (MEDIUM)
- `github.com/axodotdev/cargo-dist` — active, v0.32 just shipped. (HIGH)
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->
## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, `.github/skills/`, or `.codex/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
