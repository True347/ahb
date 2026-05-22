---
title: Gemini Local-Capture Spike — Go/No-Go Memo
date: 2026-05-22
decision: no-go
blocking: ADP-05 (Phase 3 scope)
---

## Go/No-Go decision: NO-GO

All three D-21 strict criteria fail. (1) **Non-interactive trigger** fails: `gemini -p "/stats"` and `printf '/stats\n' | gemini` both send the literal string `/stats` to the LLM as a chat prompt — the gemini-cli slash-command handler is REPL-only and is not reachable from non-interactive `-p` or stdin-pipe invocations. Both probes exit 0 with chat-response output (the LLM RAG'd over the local `.planning/` tree and synthesised an ad-hoc stats report), not a structured `/stats` payload. (2) **Quota + reset-window encoding** fails: no probe produced any JSON or text containing remaining-quota or session-reset fields. Probe 3 (`--output-format json`, the most-promising path per RESEARCH Assumption A1) entered a full `LocalAgentExecutor` agent loop for a trivial `"ok"` prompt — the `--output-format json` flag does NOT yield a thin response envelope; it activates the agentic runtime, which then failed with `[Routing] NumericalClassifierStrategy failed: Failed to generate content: Retry attempts exhausted` and `API returned invalid content after all retries` over ~6 minutes before being cancelled. The terminal JSON written to stdout in the cancel path was `{"session_id": "<SESSION-UUID>", "error": {"type": "FatalCancellationError", "message": "Operation cancelled.", "code": 130}}` — zero stats object. (3) **Stable parseable format** fails by transitivity: the only stable output we got is an unstructured LLM chat reply (Probes 1+2), which is exactly the "freeform" case D-21 #3 rejects. Per D-22 (any one of (1)-(3) fails → no-go), Gemini is deferred to v2 stub. Phase 3 routes to the no-go path in § "Phase 3 hand-off (no-go path)" below.

## Method

- Date: 2026-05-22
- OS: `Linux archServer 7.0.3-arch1-2 #1 SMP PREEMPT_DYNAMIC Fri, 01 May 2026 15:49:22 +0000 x86_64 GNU/Linux`
- gemini-cli version: `0.41.2` (installed via npm-global; binary at `~/.npm-global/lib/node_modules/@google/gemini-cli`)
- Auth mode: OAuth Google account (developer-local; the gemini-cli was already authenticated for the developer prior to this spike)
- Terminal: `xterm-256color` (Alacritty on Arch Linux, no tmux installed on this box)
- Three probes attempted on the dev box, verbatim captures below (personal session UUIDs anonymized as `<SESSION-UUID>`; no quota numbers were present in any probe output)

## Local capture result (3 probes)

### Probe 1: `gemini -p "/stats"`

Exit code: `0`

Output:
```
Warning: True color (24-bit) support not detected. Using a terminal with true color enabled will result in a better visual
Ripgrep is not available. Falling back to GrepTool.
Error executing tool activate_skill: Tool "activate_skill" not found. Did you mean one of: "update_topic", "read_file", "in
Error executing tool run_shell_command: Tool "run_shell_command" not found. Did you mean one of: "update_topic", "grep_sear
[LocalAgentExecutor] Blocked call: Unauthorized tool call: 'activate_skill' is not available to this agent.
[LocalAgentExecutor] Blocked call: Unauthorized tool call: 'run_shell_command' is not available to this agent.
# 📊 Project Statistics — v1.0 milestone

## Progress
[░░░░░░░░░░] 0/5 phases (0%)

## Plans
2/5 plans complete (40%)

## Phases
| Phase | Name | Plans | Completed | Status |
|-------|------|-------|-----------|--------|
| 0 | Spike & Spine | 5 | 2 | In Progress |
| 1 | Engine + Claude + TUI Scaffold | TBD | 0 | Not started |
| 2 | Codex + Output Formats | TBD | 0 | Not started |
| 3 | Gemini (conditional) + Cache & Refresh Policy | TBD | 0 | Not started |
| 4 | Distribution & Release Polish | TBD | 0 | Not started |

[...truncated: an LLM-generated ad-hoc statistics report synthesised from this repo's
.planning/ tree, ~40 more lines of markdown including a fake "Roadmap Analysis"
section. No `gemini /stats` slash-command payload, no quota field, no reset window.]
```

Verdict: **slash command treated as LLM prompt — chat-response nonsense**. The `/stats` literal was forwarded to the LLM as a user message; the LLM RAG'd over the repo's `.planning/` tree (visible because the gemini-cli's allowed workspace is the cwd) and produced a markdown report that mimics project-stats formatting but is fully LLM-fabricated. The repeated `Blocked call: Unauthorized tool call` lines are additional evidence that the slash-command handler is not on the non-interactive code path — even the agent's tool-use machinery is mostly blocked here.

### Probe 2: `printf '/stats\n' | gemini`

Exit code: `0`

Output:
```
Warning: True color (24-bit) support not detected. Using a terminal with true color enabled will result in a better visual
Ripgrep is not available. Falling back to GrepTool.
Error executing tool activate_skill: Tool "activate_skill" not found. Did you mean one of: "update_topic", "read_file", "in
(node:42320) [DEP0190] DeprecationWarning: Passing args to a child process with shell option true can lead to security vuln
(Use `node --trace-deprecation ...` to show where the warning was created)
Error executing tool run_shell_command: Tool "run_shell_command" not found. Did you mean one of: "update_topic", "grep_sear
[LocalAgentExecutor] Blocked call: Unauthorized tool call: 'run_shell_command' is not available to this agent.
# 📊 Project Statistics — v1.0 milestone

The project **AI HP Bar (AHB)** is currently in **Phase 0: Spike & Spine**, focusing on resolving technical risks and estab

## Progress
[████░░░░░░] 0/5 phases (0%) — *Phase 0 is 40% complete*

[...truncated: same shape as Probe 1 — an LLM-fabricated stats report citing
this repo's .planning/ files, no slash-command output. Did NOT hang
(issue #16567 not reproduced on this build).]
```

Verdict: **slash command treated as LLM prompt — chat-response nonsense**. Identical failure mode to Probe 1. Stdin pipe does not reach the REPL slash-command handler; the line is forwarded to the LLM verbatim. No hang (process exited normally), so gemini-cli issue #16567 did not reproduce on `0.41.2`.

### Probe 3: `gemini -p "ok" --output-format json`

Exit code: `130` (SIGINT — process was cancelled after ~6 minutes of retry-loop with the user's Ctrl-C, per D-21 the long latency itself disqualifies this path for a 15s-refresh TUI even before considering whether stats would ever appear)

Output:
```
Warning: True color (24-bit) support not detected. Using a terminal with true color enabled will result in a better visual experience.
Ripgrep is not available. Falling back to GrepTool.
Error executing tool activate_skill: Tool "activate_skill" not found. Did you mean one of: "update_topic", "read_file", "invoke_agent"?
Error executing tool run_shell_command: Tool "run_shell_command" not found. Did you mean one of: "update_topic", "grep_search", "invoke_agent"?
[LocalAgentExecutor] Blocked call: Unauthorized tool call: 'run_shell_command' is not available to this agent.
[LocalAgentExecutor] Blocked call: Unauthorized tool call: 'run_shell_command' is not available to this agent.
Error executing tool read_file: File not found.
API returned invalid content after all retries. Full report available at: /tmp/gemini-client-error-generateJson-invalid-content-2026-05-22T13-11-00-442Z.json Error: Retry attempts exhausted
    at retryWithBackoff (file:///.../gemini-cli/bundle/chunk-6DSAZLFF.js:270653:9)
    at async BaseLlmClient._generateWithRetry (file:///.../bundle/chunk-6DSAZLFF.js:270796:14)
    at async BaseLlmClient.generateJson (file:///.../bundle/chunk-6DSAZLFF.js:270703:21)
    at async NumericalClassifierStrategy.route (file:///.../bundle/chunk-6DSAZLFF.js:318078:28)
    at async CompositeStrategy.route (file:///.../bundle/chunk-6DSAZLFF.js:318143:26)
    at async ModelRouterService.route (file:///.../bundle/chunk-6DSAZLFF.js:318304:18)
    at async _LocalAgentExecutor.callModel (file:///.../bundle/chunk-6DSAZLFF.js:308645:26)
    [...stack trace truncated]
[Routing] NumericalClassifierStrategy failed: Error: Failed to generate content: Retry attempts exhausted
    [...stack trace truncated, same retry-exhaustion shape]
(node:43437) [DEP0190] DeprecationWarning: Passing args to a child process with shell option true can lead to security vulnerabilities
Error executing tool activate_skill: Tool "activate_skill" not found.
Error executing tool read_file: Path not in workspace: Attempted path "/home/chasel/.gemini/extensions/superpowers/skills/brainstorming/SKILL.md" resolves outside the allowed workspace directories: /home/chasel/REPO/AIHPBar or the project temp directory: /home/chasel/.gemini/tmp/aihpbar
Error executing tool write_file: Tool execution denied by policy. You are in Plan Mode and cannot modify source code. You may ONLY use write_file or replace to save plans to the designated plans directory as .md files.
Error executing tool invoke_agent: Tool execution denied by policy. You are in Plan Mode with access to read-only tools. Execution of scripts (including those from skills) is blocked.
{
  "session_id": "<SESSION-UUID>",
  "error": {
    "type": "FatalCancellationError",
    "message": "Operation cancelled.",
    "code": 130
  }
}
[ERROR] {
  "session_id": "<SESSION-UUID>",
  "error": {
    "type": "FatalCancellationError",
    "message": "Operation cancelled.",
    "code": 130
  }
}
```

Verdict: **stats absent — `--output-format json` does NOT yield a thin envelope; it activates `LocalAgentExecutor` (full agent loop with routing, classification, tool-use, retries). Even on a trivial `"ok"` prompt, the call ran ~6 minutes before the API gave up with `Retry attempts exhausted` and the user cancelled. The terminal JSON contained only `session_id` + `error` — no `stats`, no `totalTokens`, no `inputTokens`, no quota field, no reset window.** RESEARCH § "Pitfall 1 / Assumption A1" (lines 338-352, 790-803) hypothesised that `--output-format json` would expose a `stats` block per gemini-cli PR #15021; in `0.41.2` with OAuth auth, that hypothesis did not hold — at minimum, not on a path that completes reliably for a periodic-poll TUI.

## Parse feasibility

- **Format type:** freeform LLM chat output (Probes 1+2) / JSON error envelope only (Probe 3 — no successful structured response observed)
- **Stability assessment:** **unparseable for our purpose.** Probes 1+2 produced LLM-fabricated markdown — varies by model state, prompt, and what files happen to be in the cwd; not a stable schema. Probe 3 never produced a successful response in any of the attempted runs, so we have no stable JSON shape to parse — only the cancellation envelope (which is itself stable but useless to us).
- **Reset-window encoding:** **no** — neither cumulative tokens nor reset/window appears in any captured output. The LLM-fabricated stats reports in P1/P2 cite `.planning/` file counts (which are repo facts the LLM saw), not Gemini quota state.
- **If reset-window is absent:** **not doable in v1 — defer to v2.** Per D-22, AHB does NOT attempt to compute reset windows from local session logs or per-tier limit tables (no parseable input to work from, and the web-fallback route is forbidden by D-22 + Pitfall 1). Gemini support is deferred to v2 stub.

## Kill criteria

This spike's conclusion should be re-evaluated if any of these hold:

1. gemini-cli releases a change that exposes a non-interactive slash-command entry point (`gemini exec "/stats"` or `gemini --slash stats` analogous to `codex exec "/status"`).
2. A future gemini-cli version adds a thin `--output-format json` envelope path that emits a `stats` block on every invocation without entering the `LocalAgentExecutor` agent loop (i.e., a lightweight metadata-only mode). Per gemini-cli PR #15021 reversal: track upstream issue tracker.
3. OAuth-authenticated users gain explicit access to a stats endpoint (e.g., a new CLI subcommand) that returns quota + reset fields with a documented schema.
4. The `[LocalAgentExecutor]` agent-tool policy changes so that non-interactive `--output-format json` calls return without consulting the API (deterministic local-only output mode).
5. Google publishes ToS language that either explicitly forbids OR explicitly permits programmatic stats harvesting from the local CLI. (Forbid → re-confirm no-go. Permit → no-go-permission is not the blocker here, capability is — but the ToS update would trigger re-spike.)

## Phase 3 hand-off (no-go path)

Gemini deferred to v2 per D-22. Phase 3 ships an opt-in stub:

- **Config flag:** `--experimental-gemini` (opt-in CLI flag) + `[providers.gemini] enabled = false` default in `config.toml`. Without the flag/config, Gemini is not in the provider list — the user sees no Gemini row at all (not "Gemini: unconfigured").
- **Stub adapter** (registered only when the flag/config opts in): returns `Err(ProviderError::Unavailable { reason: "Gemini adapter deferred to v2 — see README §Gemini status".into() })` on every `fetch()` call. No HTTP, no spawn, no side effects. The error renders in the HP bar as a yellow "Gemini: unavailable" row so the user knows the flag took effect but no data is forthcoming.
- **README section** (Phase 4 doc plan, surfaced now so Phase 3 lands the placeholder):
  > **Gemini adapter status — deferred to v2.** Phase 0's go/no-go memo (see `.planning/research/GEMINI_SPIKE.md`) determined that gemini-cli 0.41.2 does not expose a non-interactive, stable, parseable stats endpoint that AHB can poll. The local `/stats` slash command is REPL-only; `--output-format json` activates the full agent runtime instead of a thin metadata envelope. Web-scraping `gemini.google.com/usage` carries account-ban risk (see PITFALLS #1) and is permanently out of scope. v2 will re-spike when one of the conditions in `GEMINI_SPIKE.md § Kill criteria` is met.
- **v2 work:** re-run all three probes against the then-current gemini-cli version, update this memo, decide GO/NO-GO. If GO, scope a v2 plan for adapter implementation. If NO-GO, refresh the README section with the new gemini-cli version cited.
- **What Phase 3 SHOULD still ship:** cache + refresh-policy work for the two provider adapters that DO ship (Claude + Codex). The Gemini absence does not reduce Phase 3's other deliverables — the cache layer, moka integration, refresh interval policy, and `--refresh` CLI flag all proceed. The opt-in stub is a 30-line addition, not a redirection of Phase 3 scope.

## Charset verification (per D-26)

Performed on dev box terminal: **kitty** (`TERM=xterm-256color`, Linux Arch x86_64, Phase 0 binary built via `cargo build --release` at `/home/chasel/REPO/AIHPBar/target/release/ahb`).
Date: 2026-05-22

### Byte-level proof (binding control per Pitfall 4)

Unicode default mode:

```
$ /home/chasel/REPO/AIHPBar/target/release/ahb > /tmp/ahb-out.txt
$ xxd /tmp/ahb-out.txt
00000000: 6d6f 636b 2d73 6573 7369 6f6e 2020 e296  mock-session  ..
00000010: 88e2 9688 e296 88e2 9688 e296 88e2 9688  ................
00000020: e296 91e2 9691 e296 91e2 9691 2036 3025  ............ 60%
00000030: 20e2 80a2 2072 6573 6574 7320 696e 2032   ... resets in 2
00000040: 6830 306d 0a                             h00m.
```

Bytes present (counted by streaming the full hex through `awk gsub`):
- U+2588 (`e2 96 88`) ×**6** ✓ (offsets 0x0e–0x1f, 18 contiguous bytes = 6 codepoints)
- U+2591 (`e2 96 91`) ×**4** ✓ (offsets 0x20–0x2b, 12 contiguous bytes = 4 codepoints)
- U+2022 (`e2 80 a2`) ×**1** ✓ (offsets 0x31–0x33)

Total stdout length: 69 bytes including trailing `0x0a` newline.

ASCII fallback mode:

```
$ /home/chasel/REPO/AIHPBar/target/release/ahb --ascii > /tmp/ahb-ascii.txt
$ xxd /tmp/ahb-ascii.txt
00000000: 6d6f 636b 2d73 6573 7369 6f6e 2020 2323  mock-session  ##
00000010: 2323 2323 2d2d 2d2d 2036 3025 207c 2072  ####---- 60% | r
00000020: 6573 6574 7320 696e 2032 6830 306d 0a    esets in 2h00m.
```

Bytes confirmed: `0x23` (`#`) ×**6** in the bar fill, `0x2d` (`-`) ×**4** in the bar empty + ×**1** in the `mock-session` label = 5 total, `0x7c` (`|`) ×**1** as the separator — all pure ASCII (no codepoint above 0x7F anywhere in the line).

### Regex assertions (binding shape per Plan 03 W-3)

```
$ /home/chasel/REPO/AIHPBar/target/release/ahb | grep -qP '^mock-session  ██████░░░░ 60% • resets in [0-9]+h[0-9]{2}m$'
$ echo $?  # → 0 (MATCH)

$ /home/chasel/REPO/AIHPBar/target/release/ahb --ascii | grep -qP '^mock-session  ######---- 60% \| resets in [0-9]+h[0-9]{2}m$'
$ echo $?  # → 0 (MATCH)
```

### Visual eyeball (per Pitfall 4 definition of "renders correctly")

| Environment | Result | Notes |
|-------------|--------|-------|
| Native: kitty (xterm-256color) | ✓ renders correctly | Chasel eyeballed on dev box 2026-05-22; all 5 criteria (a)-(e) confirmed |
| tmux | ⊘ not installed on dev box | `which tmux` → "tmux not found"; Pitfall 4 best-effort carve-out invoked |
| screen | ⊘ not installed on dev box | Same as tmux; Pitfall 4 carve-out |
| Windows Terminal | ⊘ deferred — no Windows machine on dev box | CI matrix on `windows-latest` (Plan 01 `.github/workflows/ci.yml`) provides byte-level proof per first push; awaiting CI green |

Definition met (Pitfall 4 lines 408-413):
- (a) U+2588 single column-wide solid block — **YES**
- (b) U+2591 single column-wide light shade — **YES**
- (c) U+2022 does not consume two columns — **YES**
- (d) no U+FFFD replacement character — **YES**
- (e) no "tofu" rectangle — **YES**

Overall verdict: **charset verified** on kitty (binding eyeball + binding byte-proof). tmux/screen/Windows-Terminal rows are best-effort per Pitfall 4; CI matrix `windows-latest` is the binding cross-platform proxy once Phase 0 is pushed.

## Sample fixtures status (D-24)

Per D-24, anonymized `gemini /stats` fixtures are NOT committed in Phase 0. Section 3 above captures the three probe outputs inline as prose. There are no Gemini fixtures to commit because the spike returned no parseable stats output — even the v2 re-spike will need to capture fresh fixtures from whatever future gemini-cli version triggers the kill-criteria re-evaluation. No `tests/fixtures/gemini/` directory is created in Phase 3 (no adapter implementation → no snapshot tests to back).

## Appendix: rationale for NOT spiking web fallback

Per D-22, this spike does NOT investigate `gemini.google.com/usage` or any web-scraping route in Phase 0, regardless of the local-capture path's outcome.

Reason: account-ban risk asymmetry. The local path reads the user's own CLI output — zero risk. The web path scrapes Google's servers from a non-browser client; if Google's anti-bot heuristics flag the access pattern, the consequence is the user's entire Google identity (Gmail, Drive, YouTube, Workspace) gets locked, not just AHB's Gemini support. AHB's v1 value proposition without Gemini still ships unified Claude + Codex HP bar visibility — the white-space differentiator survives. The expected value of v1 Gemini support is not worth a worst-case full-Google-identity lockout for AHB users.

If the user later wants Gemini in v1 anyway despite this memo's NO-GO, that is a separate scope decision (and a v2 conversation) — not a Phase 0 reopening. See `.planning/research/PITFALLS.md` § "Pitfall 1: Scraping `gemini.google.com/usage` with the user's real Google session cookie → account ban" for the full risk write-up.
