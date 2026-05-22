---
phase: 00-spike-spine
plan: 04
subsystem: research
tags: [gemini, spike, gating-memo, no-go, provider-deferred]

requires:
  - phase: 00-spike-spine
    provides: Cargo scaffold from 00-01 (binary build target referenced in Section 7 placeholder)
provides:
  - Written go/no-go memo at .planning/research/GEMINI_SPIKE.md
  - Binding decision for Phase 3 scope: NO-GO — Gemini deferred to v2 stub
  - Section 7 (Charset verification) placeholder for Plan 05 to amend with byte-level proof
  - Phase 3 hand-off contract: opt-in `--experimental-gemini` flag + Unavailable stub adapter
affects: [Phase 3, README, ROADMAP-Phase-3, ADP-05]

tech-stack:
  added: []
  patterns:
    - Spike-memo with frontmatter `decision: go|no-go` so downstream automation can grep
    - Kill-criteria list as the re-evaluation contract (avoid "this is decided forever" failure mode)
    - Placeholder section for follow-on plan to amend rather than separate file (D-26 charset note)

key-files:
  created:
    - .planning/research/GEMINI_SPIKE.md
    - .planning/phases/00-spike-spine/00-04-SUMMARY.md
  modified:
    - .planning/STATE.md
    - .planning/ROADMAP.md

key-decisions:
  - "NO-GO: gemini-cli 0.41.2 has no usable stats path for AHB — all three D-21 criteria fail."
  - "Phase 3 routes to no-go path: opt-in `--experimental-gemini` flag + Unavailable stub adapter, README §Gemini-status placeholder."
  - "Web fallback (gemini.google.com/usage) explicitly NOT spiked per D-22 — account-ban asymmetry."
  - "Charset Section 7 left as placeholder; Plan 05 fills with byte-level xxd + visual eyeball."

patterns-established:
  - "Spike memo: decision in frontmatter + Section 1 walks the criteria, body provides evidence + hand-off + kill-criteria."
  - "When a sub-task depends on a not-yet-built artifact (binary), create a placeholder section with explicit fill-in contract for the later plan."

requirements-completed: []  # ADP-00 was completed by Plan 02; this plan informs ADP-05 scope but does not satisfy it.

duration: 0h15m
completed: 2026-05-22
---

# Phase 00: Gemini local-capture spike — NO-GO, defer to v2 stub

**`gemini-cli 0.41.2` exposes no non-interactive stats path that satisfies D-21's three criteria; Phase 3 ships an opt-in stub adapter behind `--experimental-gemini`, no web-scraping fallback per D-22.**

## Performance

- **Duration:** ~15 min (orchestrator + human-in-the-loop spike + memo write)
- **Started:** 2026-05-22T13:00Z (Probe 1 capture)
- **Completed:** 2026-05-22T13:48Z (memo committed)
- **Tasks:** 2 (Task 1: 3 probe captures by human; Task 2: memo written)
- **Files modified:** 1 created + 2 tracking files

## Accomplishments

- Three probes captured verbatim from the developer's machine (`gemini -p "/stats"`, `printf '/stats\n' | gemini`, `gemini -p "ok" --output-format json`)
- D-21 strict criteria walked through with concrete evidence; all three FAIL → NO-GO decision binding for Phase 3
- Phase 3 hand-off contract drafted: opt-in `--experimental-gemini` flag, `ProviderError::Unavailable` stub, README §Gemini-status placeholder text
- Kill criteria recorded (5 conditions) so v2 re-spike has clear triggers
- D-22 web-fallback exclusion documented in Appendix with PITFALLS reference

## Task Commits

1. **Task 1: Human probe capture** — N/A (data only, captured into resume signal; no source commit)
2. **Task 2: Write GEMINI_SPIKE.md memo** — commit hash assigned at git_commit step below

## Files Created/Modified

- `.planning/research/GEMINI_SPIKE.md` (195 lines) — go/no-go memo with all 9 required sections per D-23
- `.planning/STATE.md` — plan progress 3/5, Phase 0 40% → 60%
- `.planning/ROADMAP.md` — Phase 0 Plan 04 marked complete

## Decisions Made

- **NO-GO is unambiguous.** Probes 1+2 confirm the slash-command handler is REPL-only; the `/stats` literal is forwarded to the LLM as a chat prompt. Probe 3 confirms `--output-format json` activates the full `LocalAgentExecutor` agent runtime (6-minute retry loop on a trivial `"ok"` prompt) rather than emitting a thin stats envelope. No probe surfaced a `stats` object, a quota field, or a reset-window field. The hypothesis from RESEARCH § Pitfall 1 / Assumption A1 (that PR #15021 made `--output-format json` a thin-envelope path) did not survive contact with gemini-cli 0.41.2.
- **Phase 3 scope reduced by exactly one adapter.** Phase 3's cache + refresh-policy + opt-in-stub work proceeds — only the Gemini adapter implementation is removed. Total Phase 3 scope shrinks but does not redirect.
- **Charset Section 7 left as placeholder for Plan 05** — `ahb` binary doesn't exist yet at Plan 04 time (Plan 03 builds it). Plan 05's E2E smoke is where xxd + visual eyeball happens. Plan 04 writes the contract Plan 05 must satisfy (expected bytes, criteria a-e, eyeball form).

## Deviations from Plan

None — plan executed exactly as written. The verify regex passes (`^## Go/No-Go decision`, `^## Method`, `^## Local capture result`, `^## Parse feasibility`, `^## Kill criteria`, `^## Phase 3 hand-off`, `^## Charset verification`, `^## Sample fixtures`, `^## Appendix`, `^decision: (go|no-go)` all present).

## Issues Encountered

- **Probe 3 was cancelled (Ctrl-C, exit 130) after ~6 min** because the gemini-cli `--output-format json` path entered an unrecoverable retry loop (`Retry attempts exhausted`, `API returned invalid content after all retries`). This was treated as decisive evidence of unusability rather than an inconclusive run — even if a subsequent run succeeded, a 6-minute happy-path latency is incompatible with AHB's 15s TUI refresh budget. The cancellation envelope (no `stats` block, only `session_id` + `FatalCancellationError`) is itself a stable failure mode that disqualifies the path.
- **Developer environment leaked Plan Mode policy into the non-interactive call** (`Tool execution denied by policy. You are in Plan Mode...`). This is an additional signal that gemini-cli `0.41.2`'s non-interactive code path inherits interactive runtime policies in a way that makes it unsuitable for unattended polling — even if quota stats existed, this layering would break under typical user gemini configs.

## User Setup Required

None — memo is informational. Phase 3 will register a config flag (`--experimental-gemini`) but that is a Phase 3 deliverable, not a Phase 0 user-setup item.

## Next Phase Readiness

- **Plan 05 ready to amend Section 7** with xxd + eyeball proof once Plan 03 builds the binary.
- **Phase 3 planning unblocked** — scope rewrite is one bullet: drop Gemini adapter, add opt-in stub. The `experimental-gemini` flag wiring and `ProviderError::Unavailable` stub implementation are small (~30 lines) and slot into existing Phase 3 cache/refresh-policy work.
- **ROADMAP § Phase 3 success criteria** can be tightened post-Phase-0: instead of "ADP-05 conditional on Phase 0 outcome", phrase as "ADP-05: opt-in stub returning Unavailable; full adapter deferred to v2 per GEMINI_SPIKE.md NO-GO".
- **No blockers for Phase 0 itself.** Plan 03 (MockProvider + render_text + main.rs) and Plan 05 (E2E smoke) proceed unaffected.

---
*Phase: 00-spike-spine*
*Completed: 2026-05-22*
