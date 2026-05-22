---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: completed
stopped_at: Completed 00-spike-spine 00-03-PLAN.md (runtime spine wiring; cargo run prints the D-25 bar via the locked trait)
last_updated: "2026-05-22T14:28:39.655Z"
last_activity: 2026-05-22 -- Phase 00 marked complete
progress:
  total_phases: 5
  completed_phases: 1
  total_plans: 5
  completed_plans: 5
  percent: 20
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-05-22)

**Core value:** 任何時刻、一個指令，立即看到所有訂閱的 AI CLI「現在還剩多少 session 額度、什麼時候 reset」。
**Current focus:** Phase 00 — spike-spine

## Current Position

Phase: 00 — COMPLETE
Plan: 4 of 5 (Wave 3 done; only Plan 05 smoke + charset verification remains)
Status: Phase 00 complete
Last activity: 2026-05-22 -- Phase 00 marked complete

Progress: [██████░░░░] 60%

## Performance Metrics

**Velocity:**

- Total plans completed: 0
- Average duration: —
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 0. Spike & Spine | 0 | — | — |
| 1. Engine + Claude + TUI | 0 | — | — |
| 2. Codex + Output | 0 | — | — |
| 3. Gemini + Cache | 0 | — | — |
| 4. Distribution | 0 | — | — |

**Recent Trend:**

- Last 5 plans: —
- Trend: — (no data yet)

*Updated after each plan completion*
| Phase 00-spike-spine P01 | 3m | 3 tasks | 9 files |
| Phase 00-spike-spine P02 | 5m | 2 tasks | 4 files |
| Phase 00-spike-spine P03 | 6m | 3 tasks | 6 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Init]: Granularity=coarse → 5 phases (0 through 4); MVP mode → every phase ships a runnable binary, not a horizontal layer.
- [Init]: Phase 0 numbering preserved (not collapsed into Phase 1) because the Gemini go/no-go memo is gating — Phase 3 scope depends on its outcome.
- [Init]: ADP-05 (Gemini) is conditional — success criteria phrased to honor either spike outcome (ship full / stub with opt-in flag).
- [Init]: Foundational items (keyring-core + Secret<T> + ratatui panic hook + per-adapter Vec<Result> isolation) wired in Phase 1 BEFORE feature code, per research recommendation.
- [Phase ?]: Phase 0 dep minimalism: Cargo.toml pins exactly 9 production deps + 1 dev dep; tokio uses lean features [rt, macros] only — rationale comment in manifest documents Phase 1 upgrade path.
- [Phase ?]: Clippy disallowed-types uses concrete crossterm paths (event::Event, style::Color) — Clippy's lint does not accept glob patterns; explicit list grows in Plans 02/03 as needed.
- [Phase ?]: CI uses actions-rust-lang/setup-rust-toolchain@v1 (bundles rust-cache + problem matchers; documented successor to dtolnay/rust-toolchain) on 3-OS matrix with fail-fast=false.
- [Phase ?]: [Phase 0-02]: ProviderError serde shape — newtype variants Network(NetworkErr) and Internal(anyhow::Error) converted to single-field struct variants (Network { source }, Internal { source }) to satisfy serde internally-tagged enum constraints. From impls preserve construction ergonomics; #[serde(serialize_with = serialize_display)] still emits Display-only JSON (W-7 binding).
- [Phase ?]: [Phase 0-02]: ProviderState.source uses Cow<'static, str> (W-2) so serde round-trip yields Cow::Owned without lifetime errors; adapter ergonomics preserved via Cow::Borrowed for static labels.
- [Phase ?]: [Phase 0-02]: Phase 0 lint floor relocated to src/lib.rs (crate root) so all modules inherit deny(unwrap/expect/panic) + warn(pedantic) automatically.
- [Phase ?]: [Phase 0-02]: FetchCtx<'a> locked at minimal 2 fields (now, &Secrets); derives Copy because jiff::Timestamp is Copy. Additive fields deferred to Phase 1.
- [Phase ?]: [Phase 0-03]: jiff::Timestamp::since defaults to Unit::Second as the largest unit; for hours+minutes balanced spans, call since((Unit::Hour, *now)). RESEARCH's verbatim example used the default since() which silently broke the 2h00m countdown. Documented in Plan 03 Deviation 1.
- [Phase ?]: [Phase 0-03]: Wall-clock reads centralized at the binary entry boundary — src/main.rs is the only Phase 0 caller of jiff::Timestamp::now(); MockProvider uses ctx.now (clock-injection contract). Future adapters must follow the same rule (acceptance grep guards mock.rs).
- [Phase ?]: [Phase 0-03]: Phase 0 panic-hook (install_phase0_panic_hook) composes via take_hook+set_hook, called as the FIRST line of main(). Phase 1's ratatui::init() will wrap it cleanly per D-27 + Pitfall 5.

### Pending Todos

[From .planning/todos/pending/ — ideas captured during sessions]

None yet.

### Blockers/Concerns

[Issues that affect future work]

- Phase 3 scope is gated by Phase 0 outcome (Gemini spike). Do not plan Phase 3 in detail until Phase 0 memo lands.

## Deferred Items

Items acknowledged and carried forward from previous milestone close:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| *(none)* | | | |

## Session Continuity

Last session: 2026-05-22T13:31:01Z
Stopped at: Completed 00-spike-spine 00-03-PLAN.md (runtime spine wiring; cargo run prints the D-25 bar via the locked trait)
Resume file: None
