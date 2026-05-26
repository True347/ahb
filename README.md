[![crates.io](https://img.shields.io/crates/v/ai-hp-bar)](https://crates.io/crates/ai-hp-bar)
[![CI](https://img.shields.io/github/actions/workflow/status/True347/ahb/ci.yml?branch=master&label=CI)](https://github.com/True347/ahb/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](https://github.com/True347/ahb/blob/master/LICENSE-MIT)
[![MSRV 1.88](https://img.shields.io/badge/MSRV-1.88-orange)](https://github.com/True347/ahb/blob/master/Cargo.toml)

# AHB — AI HP Bar

A Rust CLI + TUI that shows your LLM subscription (Claude Code, Codex CLI, …)
session quota and reset countdowns as a game-style HP bar.

- Compact / detailed / JSON output modes
- Static TUI mode with 15s auto-refresh
- Multi-provider with per-adapter error isolation
- Stale-on-error indicator for transient network failures (Phase 3)
- OS keyring-backed credentials (no plaintext on disk)

![AHB demo — compact, detailed, and TUI modes side by side](https://raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png)

## Install

Pick whichever matches your machine. `brew` is fastest; the others fall
back to source-build.

```sh
# macOS / Linux with Homebrew (recommended — sidesteps Gatekeeper)
brew install True347/tap/ahb

# Any machine with cargo + cargo-binstall (seconds, no compile)
cargo binstall ai-hp-bar

# Any machine with cargo (source build — 2-5 minutes first time)
cargo install ai-hp-bar

# Raw GitHub release artifact
curl -fsSL https://github.com/True347/ahb/releases/latest/download/ahb-installer.sh | sh
```

## Quick start

- `AHB` — compact one-line status (default).
- `AHB --detailed` — multi-line per-provider breakdown.
- `AHB --json` — machine-readable output (`schema_version: 1`).
- `AHB tui` — fixed-screen view that auto-refreshes every 15 seconds (default).

## macOS Gatekeeper / cross-OS first-run notes

Pre-built binaries downloaded from GitHub releases are unsigned (AHB is a
self-funded OSS tool). First-run hurdles:

- **macOS**: `"ahb" cannot be opened because the developer cannot be verified.`
  Strip the quarantine flag:
  ```sh
  xattr -d com.apple.quarantine ./ahb
  ```
  Alternative: `spctl --add ./ahb` to allow once.
- **Linux**: `chmod +x ./ahb` — no quarantine concept; minor SELinux /
  AppArmor edge cases not yet documented.
- **Windows**: SmartScreen → "More info" → "Run anyway" (one-time).

Installing via `brew install True347/tap/ahb` sidesteps all three.

## Configuration

AHB reads its config from `~/.config/ahb/config.toml` (path resolved per OS via
the `directories` crate — macOS: `~/Library/Application Support/ahb/`,
Windows: `%APPDATA%\ahb\`). On first run, AHB writes a default template with
four provider blocks (`claude`, `codex`, `gemini`, `mock`) each gated by an
`enabled` flag plus an optional `refresh_interval` (seconds, ≥ 5; defaults to
15). Enable the providers you use, restart, and the HP bar populates.

## Gemini adapter status — deferred to v2

The Gemini adapter is deferred to v2. Phase 0's go/no-go spike (see
`.planning/research/GEMINI_SPIKE.md`) determined that `gemini-cli 0.41.2` does
not expose a non-interactive, stable, parseable stats endpoint:

- The local `/stats` slash command is REPL-only; `gemini -p "/stats"` forwards
  the literal string to the LLM as a chat prompt.
- `--output-format json` activates the full `LocalAgentExecutor` agent loop
  instead of emitting a thin metadata envelope.
- No probe produced quota or session-reset fields in a parseable format.

The v2 adapter will be re-spiked when one of the conditions in
`GEMINI_SPIKE.md § Kill criteria` is met (e.g., gemini-cli exposes a
non-interactive `/stats` entry point, or a stable stats endpoint with
documented schema becomes available).

**ToS warning.** Web-scraping `gemini.google.com/usage` carries account-ban
risk and is permanently out of scope for AHB; see `PITFALLS.md § Pitfall 1`
for details.

## License

AHB is dual-licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

## Contributing

PRs welcome, file issues for missing provider or unexpected output.
