# Pitfalls Research

**Domain:** Multi-provider LLM-CLI usage tracker (Rust CLI + TUI, polling local state + web endpoints)
**Researched:** 2026-05-22
**Confidence:** HIGH (data-extraction + Gemini ToS); MEDIUM (TUI specifics — verified against ratatui docs); MEDIUM (distribution — best practices well-established but project-specific risk varies)

---

## Critical Pitfalls

### Pitfall 1: Scraping `gemini.google.com/usage` with the user's real Google session cookie → account ban

**What goes wrong:**
The PROJECT.md spike plan is "curl `gemini.google.com/usage` with Google session cookies." This is the highest-risk pitfall in the entire project. `gemini.google.com` is a consumer surface — there is no public, documented "usage" REST endpoint. Any automation that re-uses a logged-in Google session cookie to poll a `myaccount.google.com` / `gemini.google.com` internal endpoint is, under Google's current GenAI Prohibited Use Policy, "using third-party software to piggyback on Gemini CLI's OAuth authentication to access backend services" — a direct ToS violation that can result in temporary suspension on first detection and **permanent account ban on a second flag.**

A 15-second TUI refresh = 5,760 requests/day per user against an endpoint not designed to be polled. That trips:
- Cloudflare/Akamai-style cookie-bound rate limits (account-level, not IP)
- Google T&S automated abuse heuristics (consistent pattern + non-browser User-Agent)
- Manual review escalation if a single account hits the heuristic repeatedly

The damage is asymmetric: even if the tool works perfectly, the cost of being wrong is the user's Google account (Gmail, Drive, Workspace, etc.), not just the AHB tool.

**Why it happens:**
- "It works in curl from the browser cookies" feels like proof of an API.
- The HP-bar UX needs all three providers; dropping Gemini feels like product failure.
- The risk is invisible until enforcement happens.

**How to avoid:**
1. Treat the Gemini adapter as an explicit Phase 0 spike with a written kill-criterion. The spike must answer: *is there a stable, low-rate path that doesn't violate ToS?* If no, Gemini is deferred to v2 behind an opt-in flag with a giant warning.
2. If shipped at all: aggressive default refresh (≥5 min for Gemini specifically, decoupled from TUI 15s tick), conditional GET / ETag, and a hard daily request ceiling per account.
3. Never bundle the cookie in the repo, never log the cookie, never include it in `--json` output, never send it in any User-Agent fingerprint.
4. README must explicitly state: "Gemini support uses an unofficial endpoint. Use at your own risk. May violate Google ToS."
5. Prefer Gemini CLI's own `/stats` slash command output (if it can be captured non-interactively via `gemini exec` or similar) over web scraping. Reading the user's own local CLI output is far less risky than authenticating to web properties.

**Warning signs:**
- Spike finds the endpoint returns JSON readily → tempting to ship. Treat as a *signal to slow down*, not green-light.
- "Just refresh every 30s, it'll be fine" — Google heuristics are account-level, not request-rate-level. One account doing this for 6 months is suspicious regardless of rate.
- Response includes a `set-cookie` rotating the session, or a `__Secure-` cookie shape → server is actively session-tracking.
- 200 response that contains an interstitial / captcha HTML rather than JSON → already shadow-throttled.

**Phase to address:** Phase 0 (spike) — gate the entire Gemini adapter on a written go/no-go decision. **Do not start Phase 1 implementation of Gemini adapter without this gate cleared.**

---

### Pitfall 2: Parsing `~/.claude/stats-cache.json` as if its schema were stable

**What goes wrong:**
`stats-cache.json` is an undocumented internal cache file owned by Anthropic's Claude Code CLI. It has no API stability contract. Field names, units (tokens vs. requests vs. messages), nesting, and even file location have shifted over Claude Code versions (the team has been moving fast — the ccusage project tracks several schema changes since launch). Worse, the actual rate-limit countdown (5-hour rolling window + weekly hard cap) is **not** straightforwardly in `stats-cache.json` — that file is aggregated/cache stats; per-session token accounting lives in `~/.claude/projects/<project>/<session-id>.jsonl` and the 5h/weekly state is computed client-side from those JSONL files plus plan metadata. A naïve parser reading only `stats-cache.json` will silently show wrong numbers for the headline metric of the entire tool.

**Why it happens:**
- The name "stats-cache" sounds like *the* stats. It is not — it's a cache, not source of truth.
- Searching "claude code stats-cache.json" returns tutorials that build dashboards from it; those dashboards mostly show cumulative usage (Path B), not the reset countdown (Path A, the AHB Core Value).
- Anthropic ships Claude Code updates weekly; the schema can change at any release.

**How to avoid:**
1. **Read the JSONL files**, not just the cache. The 5-hour window must be computed by summing `usage.input_tokens + usage.output_tokens` from `assistant` entries within the last 5 hours. Plan limits (Pro/Max5/Max20) come from config or are hard-coded with override.
2. **Defensive deserialization:** every field is `Option<T>`, every numeric field handles both `u64` and `f64`, every parser returns a `ClaudeUsage` struct with explicit "missing/unknown" variants rather than panicking.
3. **Read-only access:** open the JSONL files with read-only file handles (`OpenOptions::read(true)`), never `read_to_string` an entire huge JSONL file (sessions can be MB-scale); stream line-by-line, drop partial lines.
4. **Version sniff:** before parsing, read Claude Code's version (`claude --version` or `~/.claude/CLAUDE.md` if exposed) and log a warning if version is newer than the latest known-good. Don't fail; warn.
5. **Snapshot test corpus:** check in a small set of anonymised real `stats-cache.json` + JSONL samples from the current Claude Code version as test fixtures. CI runs the parser against them. When Claude Code updates and breaks parsing, the test fails *before* a user reports a wrong HP bar.
6. **Format-drift sentinel:** when the parser falls back to "unknown" for >X% of expected fields, surface a visible "⚠ Claude adapter may be out-of-date" indicator in the HP bar instead of silently zero-ing.

**Warning signs:**
- HP bar shows 100% or 0% for Claude even though `/usage` inside claude code shows a non-extreme number.
- New Claude Code release lands and AHB silently starts showing stale or default values.
- Parser logs "field X not found" but exit code is still 0.

**Phase to address:** Phase 1 (Claude adapter) — schema sniffing + snapshot fixtures from day 1.

---

### Pitfall 3: Reading Codex CLI's SQLite (`state_5.sqlite`) while Codex is actively writing

**What goes wrong:**
`~/.codex/state_5.sqlite` (and `logs_2.sqlite`) are live SQLite databases owned by an actively-running Codex CLI process. Two failure modes:

1. **Lock contention:** Without WAL mode, a writer holding `RESERVED`/`PENDING`/`EXCLUSIVE` blocks readers. Even with WAL, a reader that opens during a checkpoint or a non-WAL transaction can hit `SQLITE_BUSY` ("database is locked"). A 15-second TUI tick that occasionally fails with "database is locked" looks like an AHB bug to the user.
2. **Corruption risk:** If AHB opens the DB in read-write mode (default for some Rust wrappers) and the process is interrupted mid-write, the journal file gets confused. There are open Codex issues (#21750, #23848) about `state_N.sqlite` corruption — adding another reader that occasionally fails to clean up is making a known fragile situation worse.

Additionally, `state_N.sqlite` is **versioned** (the `_5` suffix is a schema version) — Codex bumps to `state_6.sqlite` etc. when it migrates. Hard-coding the filename will break on upgrade.

**Why it happens:**
- Rust's `rusqlite` defaults to opening in read-write mode unless you ask otherwise.
- "I'll just `SELECT * FROM threads`" feels safe — it's read-only intent. But intent ≠ open mode.
- Schema version drift is invisible until it isn't.

**How to avoid:**
1. **Open read-only and with WAL-aware flags:**
   ```rust
   Connection::open_with_flags(
       path,
       OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
   )
   ```
2. **Set `busy_timeout` to a small value** (e.g. 250ms) — fail fast on lock contention rather than hanging the TUI tick.
3. **Catch `SQLITE_BUSY` explicitly** and treat it as transient: show "⌛" in the HP bar for that tick, retry next tick. Never crash the tool.
4. **Prefer the JSONL rollouts (`~/.codex/sessions/**/rollout-*.jsonl`)** when possible — they're append-only files and contain `token_count` events. The SQLite is mostly thread metadata; the actual usage signal is in the rollouts. Reading append-only JSONL is dramatically safer than reading a live SQLite.
5. **Discover state DB dynamically:** glob `~/.codex/state_*.sqlite` and pick the highest version number, with a fallback warning if multiple coexist (mid-migration).
6. **Never write** to the DB. Never run `PRAGMA journal_mode=WAL` from AHB (that's a write).
7. **Beware empty `rate_limits` field:** Codex issue #14880 documents `rate_limits` being null in rollout files. The parser must treat missing rate-limit info as "unknown" and fall back to other signals (e.g. last token_count timestamp), not "fully available".

**Warning signs:**
- Intermittent "database is locked" on TUI tick.
- Codex updates and `state_5.sqlite` becomes `state_6.sqlite` — AHB shows Codex as offline.
- `rate_limits: null` everywhere in rollouts → adapter shows phantom 100%.

**Phase to address:** Phase 1 (Codex adapter) — read-only + busy_timeout + JSONL-first strategy.

---

### Pitfall 4: Storing Google session cookies (or any provider token) in plaintext config

**What goes wrong:**
If `~/.config/ahb/config.toml` contains `gemini_cookie = "..."`, that file becomes a high-value credential at rest. Any process running as the user can read it. Backup tools, dotfile sync (Syncthing, Dropbox, GitHub dotfiles), tmpfs leaks, and screen-shares all become exfiltration vectors. The cookie is full Google session authority — read Gmail, read Drive, change account settings. The blast radius is the user's entire Google identity, not just LLM usage.

Token-in-config is the most common mistake CLI tools make and it's almost always done because keyring integration "felt heavy" at MVP time.

**Why it happens:**
- TOML/JSON config is easy; OS keyring requires conditional compilation across Linux/macOS/Windows.
- "It's just on my machine" — until the user dotfile-syncs to a public repo, which has happened many times in the Anthropic API key + GitHub leak corpus.

**How to avoid:**
1. **Use `keyring-rs` from day 1.** Supports macOS Keychain, Windows Credential Manager, Linux Secret Service. Single crate, single API.
2. **Config file stores a *reference* (service name + account name), not the secret.** Example: `gemini.cookie_keyring_service = "ahb.gemini"`, then AHB asks the keyring for "ahb.gemini" / "<user>".
3. **First-run bootstrap:** AHB prompts for the cookie interactively, stores in keyring, never writes to disk.
4. **`Debug` redaction:** wrap any token/cookie type in a `Secret<String>` newtype whose `Debug` impl prints `"[REDACTED]"`. Forbid plain `String` for credentials at the type level.
5. **Forbid logging secret types:** integrate with `tracing` such that any field marked `secret` is redacted before emission.
6. **No `--json` leak:** the JSON output mode must never echo the source credential. Audit the serde derives — `#[serde(skip)]` on every secret field, with a test that proves it.

**Warning signs:**
- Code review finds `String` typed credentials.
- `log::info!("config: {:?}", cfg)` printing a struct that contains a token.
- A user reports "I committed my AHB config to dotfiles and now…"

**Phase to address:** Phase 1 (config + first adapter) — keyring + `Secret<T>` newtype must land before any adapter that reads credentials.

---

### Pitfall 5: Terminal left in altscreen / raw mode / no cursor on panic

**What goes wrong:**
If the TUI panics (or `process::exit`s, or is killed mid-render), the terminal stays in alternate-screen mode with raw mode enabled and cursor hidden. The user's shell becomes unusable — typed input is invisible, Ctrl-C doesn't echo, `reset` is the only escape. This is a well-known ratatui pitfall — there's a dedicated issue (#2087) and recipe page about it.

For a tool the user runs constantly (a 15s-refresh HP bar), the chance of a panic over a year of usage is nearly 1.0. Every panic that breaks the terminal is a maximum-pain bug.

**Why it happens:**
- Default `panic_hook` doesn't know about terminal state.
- Manual `Terminal::new()` (instead of `ratatui::init()`) skips the bundled panic-hook restoration.
- `unwrap()` / `expect()` in adapter code paths panics propagate up the render loop.

**How to avoid:**
1. **Use `ratatui::init()` and `ratatui::restore()`** — the init function installs a panic hook that restores the terminal before the default panic handler runs.
2. **Install the panic hook *before* any adapter code runs** — adapter panics during startup must also restore.
3. **Chain with `color_eyre` / `human-panic`** so users see a useful report instead of a raw backtrace.
4. **No `unwrap()` in render loop or adapter code paths.** Treat `unwrap` in adapter code as a CI lint failure (clippy `unwrap_used` deny).
5. **Test the panic path:** have an integration test that deliberately panics inside the render loop and verifies the terminal is restored (set `TERM=dumb`, run, exit code != 0 is fine, but no leftover altscreen escape in stderr).
6. **SIGINT/SIGTERM handler** that calls `ratatui::restore()` before exiting.

**Warning signs:**
- During development, an `unwrap()` panic leaves your shell broken.
- Issue tracker gets "my terminal is stuck after AHB crashed" reports.

**Phase to address:** Phase 1 (TUI scaffolding) — panic hook installed before any feature code.

---

### Pitfall 6: One provider adapter failing crashes the whole tool

**What goes wrong:**
If `gemini_adapter::fetch()` returns `Result<Usage, Error>` and the main loop does `let gemini = gemini_adapter::fetch()?;`, then a Gemini network blip kills the entire tool — Claude and Codex go dark too, even though their data is sitting on local disk and would have rendered fine. This violates the Core Value (always show *something* for every configured provider) and conflates "I can't reach Gemini right now" with "AHB is broken."

**Why it happens:**
- The `?` operator is ergonomic and propagates errors by default.
- "Fail fast" is good advice in libraries; it's terrible advice in a status-line aggregator where partial data is the whole point.

**How to avoid:**
1. **Each adapter returns `Result<Usage, AdapterError>` and the orchestrator catches per-adapter**, never propagating up to the render loop.
2. **Use `tokio::task::JoinSet`** (or `std::thread::spawn` if avoiding async) — each adapter runs in its own task, and one task's panic does not bring down others.
3. **The Usage model has explicit "error" / "unknown" variants** with a reason string. The TUI renders `[claude ▰▰▰▰▱ 78% / 2h12m] [codex ⌛ locked] [gemini ⚠ unreachable]`.
4. **Per-adapter timeout** (e.g. 3s) — never let a slow adapter block the render loop. The TUI's 15s tick must remain a 15s tick even if Gemini hangs.
5. **`catch_unwind`** around adapter calls so a panic in one adapter shows as an error tile, not a crash.

**Warning signs:**
- Pulling network cable → AHB exits.
- Killing `codex` mid-write → AHB exits.
- One adapter's `unwrap()` panics → whole TUI dies.

**Phase to address:** Phase 1 (orchestration layer) — error-isolation contract defined before the second adapter is written.

---

## Moderate Pitfalls

### Pitfall 7: ANSI color codes leak when piped to non-TTY

**What goes wrong:**
`AHB | grep claude` outputs `\x1b[32m▰▰▰▰▱\x1b[0m` — color escapes pollute downstream pipes, breaking grep/awk/jq integration. Worse, `AHB --json | jq` may break if the JSON contains stray escape bytes.

**Why it happens:**
- Color crates emit unconditionally unless you tell them otherwise.
- `println!("\x1b[32mok\x1b[0m")` doesn't know about the pipe.

**How to avoid:**
1. Use `std::io::IsTerminal::is_terminal()` on stdout to detect TTY.
2. Respect `NO_COLOR` env var (no-color.org convention).
3. Provide explicit `--color=auto|always|never` flag.
4. `--json` mode must *never* emit ANSI codes regardless of TTY.

**Phase to address:** Phase 2 (CLI output formats).

---

### Pitfall 8: Emoji / unicode width breaking the HP bar layout in tmux

**What goes wrong:**
The HP-bar metaphor invites emoji ❤️ / 🩸 / 💊. But ratatui uses `unicode-width` to compute column count, and `unicode-width` disagrees with terminal emulators on emoji width. Result: in tmux/screen, emoji renders as 1 column but ratatui assumed 2 → box borders go off-by-one → entire layout shifts → unreadable.

**Why it happens:**
- "Width" depends on font + terminal renderer, not just the unicode codepoint.
- Tmux specifically (issue #647, #1057) miscounts emoji widths.

**How to avoid:**
1. Default to ASCII-only block characters (`▰▱`, `█░`, `=-`) for the bar itself.
2. Allow emoji in optional `--style fancy` mode with explicit "may break in tmux" warning.
3. Test rendering in: macOS Terminal.app, iTerm2, Windows Terminal, plain xterm, tmux, screen.
4. Use `unicode-display-width` (not `unicode-width`) where available for better emoji handling.

**Phase to address:** Phase 1 (rendering primitives) — pick a charset and stick with it.

---

### Pitfall 9: Blocking I/O in the ratatui render loop

**What goes wrong:**
If `fetch_claude_usage()` takes 800ms and is called synchronously inside the render loop, the TUI freezes for 800ms every tick. User keystrokes are queued. Resize events are missed. It feels broken.

**Why it happens:**
- Easy to write: call adapter, render, sleep, repeat.
- Synchronous code is harder to misuse than async, but composes poorly with rendering.

**How to avoid:**
1. **Render loop owns only display state.** Adapters run on separate tasks/threads and post results to a channel.
2. Use `tokio::select!` over `crossterm::event::EventStream`, an interval ticker, and the adapter result channel — render whenever any of them fires.
3. Render must be sub-16ms (60fps target, though 15s data refresh — render is still fast for resize / scroll).
4. Adapter timeouts (see Pitfall 6) ensure no adapter monopolises.

**Warning signs:**
- Resize causes visible lag.
- Quit key has 200ms+ latency.

**Phase to address:** Phase 1 (TUI scaffolding) — render/data separation from the start.

---

### Pitfall 10: macOS Gatekeeper blocks the binary on first run

**What goes wrong:**
User downloads `ahb-macos-arm64` from GitHub release, runs it: `"ahb" cannot be opened because the developer cannot be verified.` 90% of users give up there. The remaining 10% hit System Settings → Privacy & Security → Open Anyway, which is hostile UX.

**Why it happens:**
- Apple requires Developer ID signing + notarization for binaries downloaded from the web.
- `cargo install` from source compiles locally and avoids quarantine; pre-built binaries do not.
- Apple Developer Program costs $99/year — pure friction for an OSS project.

**How to avoid:**
1. **Recommend `cargo install ahb`** as the primary install path in README — sidesteps Gatekeeper entirely.
2. **Provide `cargo binstall ahb`** for users who want speed — `cargo-binstall` downloads from GitHub releases and bypasses Gatekeeper because the binary is launched via cargo, not Finder.
3. **Provide `brew install ahb` via a tap** — Homebrew handles quarantine flag stripping (`xattr -d com.apple.quarantine`).
4. **Document the manual workaround** in README for users who download the raw release: `xattr -d com.apple.quarantine ./ahb` or `chmod +x ./ahb && spctl --add ./ahb`.
5. If the project gains traction → invest in Developer ID signing + notarization via `cargo-dist` (handles the full pipeline).

**Phase to address:** Phase 4 (distribution / release).

---

### Pitfall 11: `cargo install` is slow / fails on user machines without rustup

**What goes wrong:**
The Active requirement says "single static binary (cargo install OR release artifact)." `cargo install ahb` for a project with ratatui + tokio + rusqlite + reqwest pulls 200+ transitive crates and takes 2-5 minutes on a developer's first install — and *fails entirely* if the user has no Rust toolchain. Some users won't have rustup; the "Rust dev" assumption may not match the "multi-CLI dev" reality (they may be primarily Node/Python devs who use Claude Code).

**Why it happens:**
- Rust's default install story is `cargo install`, which assumes rustup is present.
- Release-mode link time on macOS is particularly slow (search results note OSX linking is a major cost).

**How to avoid:**
1. **Pre-built binary releases as primary path**, not fallback. Use `cargo-dist` to automate GitHub Actions cross-compilation for `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`.
2. **`cargo binstall` integration** — set `repository` in Cargo.toml so binstall auto-discovers GitHub releases.
3. **`brew` tap** (Phase 4+).
4. **README install order:** `brew` → `cargo binstall` → `cargo install` → manual download. Lead with the fastest path.

**Phase to address:** Phase 4 (distribution).

---

### Pitfall 12: TUI hits Gemini endpoint 5,760 times/day during idle

**What goes wrong:**
Default 15s refresh × 24h × 3600 / 15 = 5,760 requests/day to Gemini per user. Even if not banned (see Pitfall 1), this wastes bandwidth, drains laptop battery (radio on every 15s), and accumulates load on Google's side. The HP-bar doesn't *need* 15s for Gemini — the rate-limit windows are hours-long.

**Why it happens:**
- Uniform refresh policy across providers feels clean.
- "More frequent = more up-to-date" intuition.

**How to avoid:**
1. **Per-adapter refresh intervals.** Claude/Codex (local disk reads) at 15s; Gemini (network) at 5-15 minutes.
2. **Pause network adapters when laptop is on battery / lid is closed** (use `battery` crate or platform APIs).
3. **Exponential backoff on errors** — if Gemini returns 429 or 5xx, back off to 30 min, then 1h, until success.
4. **`If-Modified-Since` / `ETag`** if endpoint supports them.

**Phase to address:** Phase 1 (Gemini adapter, if it ships) + Phase 3 (refresh policy).

---

### Pitfall 13: Exit codes are 0/1 only, breaking shell monitoring use cases

**What goes wrong:**
A user wants `AHB --json && do-something` to fire only if all providers reported healthy data. If AHB exits 0 whether or not adapters succeeded, the integration is broken. Conversely, exit 1 on partial failure breaks "show me whatever you have" use cases.

**Why it happens:**
- Default Rust exits with 0 on success, 1 on `Err` from `main`.
- The middle states ("partial data," "all adapters failed," "config error") aren't expressed.

**How to avoid:**
1. Follow sysexits.h conventions (loosely): 0 = all OK, 64 = usage error, 65 = data error, 69 = service unavailable (all adapters down), 70 = internal panic.
2. Document exit codes in `--help` and README.
3. Provide `--exit-code-strict` (exit non-zero on any adapter error) and `--exit-code-lax` (exit 0 if at least one adapter succeeded) for different integration scenarios.
4. JSON output always includes per-adapter status regardless of exit code, so scripts can parse rather than relying on exit code alone.

**Phase to address:** Phase 2 (CLI ergonomics).

---

## Minor Pitfalls

### Pitfall 14: "AHB" is unfamiliar / undiscoverable

**What goes wrong:**
"AHB" doesn't suggest "AI usage tracker." `brew search ahb` finds it but users have to be told. SEO is impossible.

**Why it happens:**
- Short names are nice for typing.
- The acronym is meaningful internally but opaque externally.

**How to avoid:**
- Tagline in README/help: "AHB — AI HP Bar — multi-CLI session usage at a glance."
- Crate metadata `description` is the discoverability lever — make it descriptive, not cute.
- Consider `aihpbar` as a longer alias.
- Don't hard-block on this — it's recoverable.

**Phase to address:** Phase 4 (release prep).

---

### Pitfall 15: TUI doesn't honor `$TERM=dumb` or `CI=true`

**What goes wrong:**
Running `AHB tui` inside a CI log or a non-interactive terminal renders garbage escape sequences into the log.

**How to avoid:**
- Detect `IsTerminal` on stdin/stdout/stderr at startup; if not a TTY, refuse to enter TUI mode and print a helpful "run `AHB` (CLI mode) for non-interactive use" message.
- Respect `$TERM=dumb` and `$CI=true`.

**Phase to address:** Phase 2 (TUI hardening).

---

### Pitfall 16: Config file location varies across OSes

**What goes wrong:**
Hard-coding `~/.config/ahb/` works on Linux, wrong on macOS (`~/Library/Application Support/ahb/`) and Windows (`%APPDATA%\ahb\`).

**How to avoid:**
- Use the `directories` crate (`ProjectDirs::from("dev", "yourname", "ahb")`) — gives correct path per OS.
- Allow `$AHB_CONFIG` env var override.

**Phase to address:** Phase 1 (config).

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hard-code Claude/Codex/Gemini in `main.rs` instead of trait-based adapter | Ships v1 faster; no premature abstraction | Adding 4th provider requires refactor | **Acceptable for v1** — extract trait when 4th provider is concretely planned, not before |
| Use synchronous `std::thread` per adapter instead of full tokio | Simpler reasoning, no async coloring | Network adapters harder if HTTP/2 / streaming needed | Acceptable initially — the data sizes are tiny and 3 threads is fine. Re-evaluate if Gemini ships and needs HTTP/2 |
| Plain TOML config without keyring | Faster MVP | Token-leak risk (see Pitfall 4) | **Never acceptable for credentials**. Acceptable for non-secret config |
| `unwrap()` in adapter prototype code | Faster iteration | Crashes in production (see Pitfall 5) | Only in throwaway spike binaries — never in code that ships |
| Skip TTY detection, always emit ANSI | Faster MVP | `--json` mode pollution | Never — `IsTerminal` is one line |
| One-file `main.rs` with everything | Faster start | Adapter contract emerges by accident | Acceptable through Phase 1; extract `adapter` module by Phase 2 |
| Skip snapshot tests for adapter parsers | Faster Phase 1 | Silent breakage on upstream changes (Pitfall 2) | **Never acceptable** for adapters that parse undocumented schemas |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Claude Code `stats-cache.json` | Treating it as source-of-truth for session limits | Compute from JSONL session files; treat cache as hint only |
| Claude Code JSONL session files | Reading entire file into memory | Stream line-by-line with `BufReader::lines()`; skip malformed lines |
| Codex `state_5.sqlite` | Opening read-write; hard-coding filename | `SQLITE_OPEN_READ_ONLY`, glob for `state_*.sqlite`, busy_timeout 250ms |
| Codex rollouts | Assuming `rate_limits` field is populated | Treat null as "unknown," fall back to token sums |
| Gemini web endpoint | Treating as a stable API | Treat as a fragile, ToS-edge scrape; aggressive backoff; visible warning in README |
| OS keyring | Different APIs per platform | Use `keyring-rs`, single trait works on macOS/Win/Linux |
| Terminal | Assuming all terminals render unicode the same | ASCII-default for bar chars; emoji only behind explicit opt-in |
| Tmux | Trusting `unicode-width` | Test under tmux; bias toward ASCII |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Re-reading every JSONL session file every tick | Tick taking >500ms, fan spinning during idle | Cache mtime; only re-parse files newer than last tick. Watch `~/.claude/projects/` with `notify` crate | Once user has 50+ projects accumulated |
| Holding the entire JSONL in memory | RAM grows over a long session | Stream and aggregate; keep only the rolling-window sums | Long Claude Code sessions (multi-MB JSONL) |
| Tokio runtime per tick | High CPU baseline | One long-lived runtime, tasks dispatched onto it | From day 1 — easy to accidentally `Runtime::new()` in a loop |
| Synchronous render call on slow network | TUI freezes during Gemini fetch | Channel between adapter task and render loop (Pitfall 9) | First time network is slow |
| SQLite open/close per tick | File handle churn; lock contention | Open once at startup, hold read-only connection | First time tick exceeds 1s |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Logging cookie/token via `Debug` derive on config struct | Token leaks to stderr, log files, error reports | `Secret<T>` newtype with redacting `Debug`; `#[serde(skip)]` on secret fields |
| Including secrets in `--json` output | Token exfiltrates via any script that captures AHB output | Audit JSON output type; CI test asserts no secret-shaped strings present |
| Storing Google cookie in plaintext config | Whole Google account compromise on dotfile-sync / backup leak | `keyring-rs` from day 1 |
| Hitting Google endpoint with default `reqwest` User-Agent | Trivial fingerprint → faster abuse detection | Use a browser-like UA *only if* spike confirms low-risk path exists; otherwise don't ship Gemini scraping at all (Pitfall 1) |
| Long-lived `panic` payload printed to log including request body | If body contained session cookie, leaks on crash | Strip request bodies from error reports; never `Debug`-print request objects containing secrets |
| Sending Gemini request before TLS verification | MITM can capture cookie | `reqwest::Client::builder().danger_accept_invalid_certs(false)` (the default) — never disable cert verification |
| Storing the cookie on disk encrypted with a key also on disk | Attacker who reads disk reads both → no protection | Use OS keyring (Pitfall 4) — not encrypted file with co-located key |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Showing "0%" when adapter is broken instead of "unknown" | User thinks they're rate-limited when they're not; or vice versa | Explicit three-state: healthy / degraded / unknown — never conflate "no data" with "no quota" |
| HP-bar with no time-to-reset | Visual is pretty but actionless | Always pair % with `reset in 2h12m`; the countdown is half the Core Value |
| Refresh interval misleading user into thinking it's real-time | User makes a request, expects bar to update immediately, doesn't for 15s | Show "last updated 8s ago" subtle indicator; manual refresh key (`r`) in TUI |
| Tool runs in TUI mode by default | New users overwhelmed; can't pipe | Default = single CLI invocation prints one-line status. `AHB tui` is opt-in |
| Color-only signal (red = empty) | Colorblind users / non-color terminals miss the signal | Always pair color with shape: `▰▰▰▰▱`, `[FULL]`, `[LOW]`, `[EMPTY]` labels |
| Acronyms in output (`tok/h`, `cwm`) | Confusing | Spell out in detailed mode; abbreviations only in compact |
| Refresh in TUI causes visible flicker | Distracting | Double-buffer (ratatui does this); only redraw on actual data change |

---

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Claude adapter:** Often missing the JSONL-summing step — verify the bar moves in sync with the in-Claude `/usage` numbers on a fresh 5h window
- [ ] **Codex adapter:** Often missing read-only open + busy_timeout — verify by running `codex` actively in another terminal and checking AHB doesn't crash
- [ ] **Gemini adapter:** Often missing the ToS-risk warning in README — verify README explicitly says "may violate Google ToS, opt-in only"
- [ ] **Panic handling:** Often missing — verify by inserting a `panic!()` into adapter code and confirming terminal restores
- [ ] **Token storage:** Often missing keyring integration — verify by grep'ing the codebase for `cookie =` / `token =` in config files
- [ ] **JSON output:** Often leaks ANSI codes — verify with `AHB --json | jq` (must not error)
- [ ] **Exit codes:** Often only 0/1 — verify documented exit codes match implementation
- [ ] **Cross-platform paths:** Often hardcodes `~/.config` — verify behaviour on macOS (should use `~/Library/Application Support`)
- [ ] **TTY detection:** Often missing — verify `AHB | cat` does not emit color codes
- [ ] **Adapter isolation:** Often missing — verify by intentionally breaking one adapter's data source; other adapters must still render
- [ ] **Refresh policy:** Often uniform 15s — verify Gemini doesn't fire every 15s
- [ ] **Schema drift detection:** Often silent — verify there's a test that fails when test fixtures are missing expected fields
- [ ] **Terminal restoration on SIGINT:** Often missing — verify Ctrl-C in TUI restores terminal
- [ ] **No secrets in logs:** Often missing — verify by `RUST_LOG=trace AHB` and grep'ing output for credential-shaped strings
- [ ] **Distribution paths:** Often only `cargo install` — verify README has at minimum: cargo install + binary download + Gatekeeper workaround

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Gemini account flagged | HIGH (irrecoverable in worst case) | Stop AHB Gemini adapter immediately; appeal via Google; rotate cookie; consider permanent disable of Gemini support |
| Claude schema changed → parser broken | LOW | Update parser, ship patch release; users degrade gracefully if schema-drift sentinel was implemented (Pitfall 2) |
| Codex DB corruption (already documented issue #21750) | MEDIUM | Not AHB's fault — but the docs/README should call out the risk and link the upstream issue; recovery is "let codex rebuild" |
| Terminal stuck after panic | LOW (per-user) | User runs `reset` or `stty sane`; document in TROUBLESHOOTING.md |
| Token leak to dotfile sync | HIGH | Rotate the cookie / re-auth Gemini; communicate to user via release notes; introduce keyring if not yet present |
| One adapter takes down TUI | LOW | Hotfix to add per-adapter `catch_unwind`; ship patch |
| Gatekeeper blocks binary | LOW | Document workaround; longer-term invest in notarization |
| Schema-drift sentinel false-positives | LOW | Tune thresholds; add to test corpus |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1: Gemini ToS / account ban | **Phase 0 (spike)** | Written go/no-go memo before any Gemini code is written |
| 2: Claude schema drift | Phase 1 (Claude adapter) | Snapshot fixtures in CI; drift sentinel in TUI |
| 3: Codex SQLite locking | Phase 1 (Codex adapter) | Integration test: run codex in parallel, AHB tick succeeds N times |
| 4: Plaintext token storage | Phase 1 (config + first adapter needing credentials) | `Secret<T>` newtype landed; CI grep test for plaintext credentials |
| 5: Terminal not restored on panic | Phase 1 (TUI scaffolding) | Test that injects panic verifies restore |
| 6: One adapter crashing all | Phase 1 (orchestration) | Test that kills one adapter mid-run; others still render |
| 7: ANSI leak to pipes | Phase 2 (CLI output) | `AHB \| cat` test; `--json` round-trips through `jq` |
| 8: Unicode width breakage | Phase 1 (rendering) | Manual test under tmux + screen + Windows Terminal |
| 9: Blocking render loop | Phase 1 (TUI scaffolding) | Render loop has zero blocking I/O calls; lint enforced |
| 10: macOS Gatekeeper | Phase 4 (distribution) | README has install instructions for unsigned binary; `cargo binstall` works |
| 11: Slow / failing `cargo install` | Phase 4 (distribution) | Pre-built binaries published; `cargo binstall` works |
| 12: Excessive Gemini polling | Phase 1 (Gemini, if shipped) + Phase 3 (refresh policy) | Per-adapter refresh interval; default for network adapter ≥5min |
| 13: Inadequate exit codes | Phase 2 (CLI ergonomics) | Documented exit codes; integration test for each code path |
| 14: "AHB" undiscoverable | Phase 4 (release prep) | Crate description filled; README headline includes "AI HP Bar" |
| 15: TUI in non-TTY context | Phase 2 (TUI hardening) | TTY check at TUI entry; clear error message on non-TTY |
| 16: Config path cross-platform | Phase 1 (config) | `directories` crate; manual smoke test on each OS |

---

## Sources

**Critical (HIGH confidence):**
- [Generative AI Prohibited Use Policy - Google](https://support.google.com/gemini/answer/16625148?hl=en) — ToS basis for Pitfall 1
- [Gemini API Abuse Monitoring](https://ai.google.dev/gemini-api/docs/usage-policies) — account-level enforcement
- [Antigravity bans / Gemini CLI](https://github.com/google-gemini/gemini-cli/discussions/20632) — real ban incidents
- [Ratatui Panic Hooks Recipe](https://ratatui.rs/recipes/apps/panic-hooks/) — Pitfall 5 official guidance
- [Ratatui `init` / `restore` docs](https://docs.rs/ratatui/latest/ratatui/fn.init.html)
- [Ratatui Async Event Stream tutorial](https://ratatui.rs/tutorials/counter-async-app/async-event-stream/) — Pitfall 9
- [keyring-rs](https://github.com/open-source-cooperative/keyring-rs) — Pitfall 4 mitigation
- [SQLite WAL mode docs](https://sqlite.org/wal.html) — Pitfall 3 background
- [Codex state SQLite corruption issue #21750](https://github.com/openai/codex/issues/21750) — Pitfall 3 evidence
- [Codex rate_limits null issue #14880](https://github.com/openai/codex/issues/14880) — adapter null handling
- [Codex GUI SQLite init failure #23848](https://github.com/openai/codex/issues/23848) — DB fragility
- [Claude Code Monitoring docs](https://code.claude.com/docs/en/monitoring-usage)
- [ccusage tool](https://github.com/ryoppippi/ccusage) — reference parser for Claude JSONL schema
- [Claude Code 5-hour + weekly limits](https://usagebar.com/blog/claude-code-weekly-limit-vs-5-hour-lockout)
- [BSWEN: Monitor Claude cache stats](https://docs.bswen.com/blog/2026-04-01-monitor-cache-stats/) — stats-cache.json details
- [coding_agent_usage_tracker](https://github.com/Dicklesworthstone/coding_agent_usage_tracker) — prior art for multi-provider tracking
- [rust std::io::IsTerminal](https://alexwlchan.net/notes/2024/detect-tty-in-rust/) — Pitfall 7
- [Rust CLI color recommendations](https://rust-cli-recommendations.sunshowers.io/colors.html)
- [cargo-dist](https://github.com/axodotdev/cargo-dist) — Pitfall 10, 11
- [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) — Pitfall 11

**Supporting (MEDIUM confidence):**
- [Gatekeeper unsigned Rust binaries](https://users.rust-lang.org/t/distributing-cli-apps-on-macos/70223)
- [Ratatui tmux/emoji issue #1271](https://github.com/ratatui/ratatui/issues/1271)
- [Ratatui emoji discussion #1438](https://github.com/ratatui/ratatui/discussions/1438)
- [tmux emoji width #647](https://github.com/tmux/tmux/issues/647)
- [YAGNI / premature abstraction patterns](https://yagnipedia.com/wiki/yagni)
- [Codex inconsistent quota #17537](https://github.com/openai/codex/issues/17537)
- [Linux sysexits / exit code conventions](https://www.ditig.com/linux-exit-status-codes)

**Domain experience (LOW confidence — represents author knowledge synthesised from above sources):**
- Refresh-rate / abuse-detection asymmetry (Pitfall 1, 12)
- `Secret<T>` newtype pattern (Pitfall 4) — common convention but no canonical citation
- Adapter isolation contract (Pitfall 6) — derived from health-state literature, not a single canonical source

---
*Pitfalls research for: AI HP Bar (multi-provider LLM-CLI usage tracker, Rust)*
*Researched: 2026-05-22*
