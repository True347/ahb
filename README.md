# AHB — AI HP Bar

A Rust CLI + TUI that shows your LLM subscription (Claude Code, Codex CLI, …)
session quota and reset countdowns as a game-style HP bar.

- `AHB` — compact one-line status (default).
- `AHB --detailed` — multi-line per-provider breakdown.
- `AHB --json` — machine-readable output (`schema_version: 1`).
- `AHB tui` — fixed-screen view that auto-refreshes every 15 seconds (default).

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
