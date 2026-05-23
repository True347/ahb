---
status: partial
phase: 01-engine-claude-tui-scaffold
source: [01-VERIFICATION.md]
started: 2026-05-23T10:30:00Z
updated: 2026-05-23T10:30:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. TUI Visual Behavior — bordered frame, HP-bar row, quit feel
expected: Bordered full-screen frame with title ` AHB ` (leading + trailing space). One `claude` row with 10 bar cells + percent + bullet + `resets in Xh{MM}m`. Quit hint `q quit  ·  ctrl-c quit` in DarkGray at bottom. q/Ctrl-C returns terminal cleanly (no raw/altscreen leak; verify with `stty -a`).
result: [pending]

How to test:
- With `[providers.claude] enabled = true` in `~/.config/ahb/config.toml`, run `./target/release/ahb tui` (prefix `AHB_SECRETS_MOCK=1` on Linux hosts without dbus).
- Visually confirm border, row layout, and quit-hint placement against `01-UI-SPEC.md`.
- Press `q`, then `Ctrl-C` in a second session — both should restore the shell cleanly.

### 2. Color Threshold Verification — Green/Yellow/Red at 30%/10% boundaries
expected: Bar fill renders Green at percent ≥ 30%, Yellow at 10–30%, Red at < 10%. Empty cells always DarkGray.
result: [pending]

How to test:
- Craft three synthetic JSONL fixtures with `cache_creation_input_tokens` totals corresponding to ~60%, ~20%, and ~5% of the Claude session cap.
- Point `~/.claude/projects/...` at each fixture in turn, run `./target/release/ahb tui`, and confirm the color shifts visually.

### 3. Keyring Backend on macOS and Windows — InitOutcome::Ready
expected: `cargo test --test keyring_init_sanity` exits 0 on macOS and Windows. Macos may surface a Keychain access prompt on first run.
result: [pending]

How to test:
- macOS: `cargo test --test keyring_init_sanity` — approve the Keychain prompt if it appears.
- Windows: `cargo test --test keyring_init_sanity` — verify Credential Manager backend returns `InitOutcome::Ready(_)`.
- Linux is intentionally excluded from this UAT row because dev/CI runs with `AHB_SECRETS_MOCK=1`.

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
