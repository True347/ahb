---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: executing
stopped_at: Plan 02-02 complete (CORE-03 — --detailed + Claude weekly bar)
last_updated: "2026-05-25T03:30:15.095Z"
last_activity: 2026-05-25
progress:
  total_phases: 5
  completed_phases: 2
  total_plans: 12
  completed_plans: 11
  percent: 40
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-05-22)

**Core value:** 任何時刻、一個指令，立即看到所有訂閱的 AI CLI「現在還剩多少 session 額度、什麼時候 reset」。
**Current focus:** Phase 02 — codex-output-formats

## Current Position

Phase: 02 (codex-output-formats) — EXECUTING
Plan: 3 of 3
Status: Ready to execute
Last activity: 2026-05-25

Progress: [█████████░] 92%

## Performance Metrics

**Velocity:**

- Total plans completed: 4
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
| 01 | 4 | - | - |

**Recent Trend:**

- Last 5 plans: —
- Trend: — (no data yet)

*Updated after each plan completion*
| Phase 00-spike-spine P01 | 3m | 3 tasks | 9 files |
| Phase 00-spike-spine P02 | 5m | 2 tasks | 4 files |
| Phase 00-spike-spine P03 | 6m | 3 tasks | 6 files |
| Phase 01-engine-claude-tui-scaffold P01 | 17min | 3 tasks | 17 files |
| Phase 1 P2 | 12min | 2 tasks | 14 files |
| Phase 01-engine-claude-tui-scaffold P03 | 12min | 2 tasks | 12 files |
| Phase 01-engine-claude-tui-scaffold P04 P01-04 | 22min | 4 tasks | 10 files |
| Phase 02-codex-output-formats P01 | 25min | 2 tasks | 14 files |
| Phase 02-codex-output-formats P02 | 12min | 2 tasks | 11 files |

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
- [Phase ?]: [Phase 01-01]: Engine fan-out uses JoinSet + HashMap<task::Id, ProviderId> for Pitfall L4 panic recovery; DEFAULT_PER_PROVIDER_TIMEOUT = 2s for Phase 1 (local IO).
- [Phase ?]: [Phase 01-01]: ClaudeProvider sums cache_creation_input_tokens ONLY (D-33 amended per L1; input_tokens+output_tokens are upstream-broken streaming placeholders per ccusage #866). Window label is 'claude' (provider id), not 'claude-5h' — UI-SPEC binding.
- [Phase ?]: [Phase 01-01]: config::load_or_init returns LoadOutcome::{Initialized, Loaded} (caller decides exit); D-37 first-run path writes embedded template via include_str! and prints 'initialized {} — enable providers and rerun'.
- [Phase ?]: [Phase 01-01]: filled_cells / format_countdown / id_label promoted to pub(crate) so Plan 03 TUI widget re-uses without duplication or scoped-clippy drift (WARNING #3 + #5 resolutions).
- [Phase ?]: Plan 02: Secret<T> newtype (D-42) with Drop→zeroize, Debug→***, Serialize→[REDACTED], NO Deserialize, single .expose() unwrap
- [Phase ?]: Plan 02: AHB_SECRETS_MOCK=1 debug-only test affordance lets backend-less CI runners exercise Plan 01 happy path while production D-41 hard-error remains binding
- [Phase ?]: Plan 02: drift detector uses raw serde_json::Value re-parse (NOT typed schema widening) — preserves Plan 01 u64 Usage schema (WARNING #2 path-a)
- [Phase ?]: Plan 02: SchemaDrift renderer uses id_label(id) (NOT hard-coded 'claude') so non-Claude adapters triggering drift render cleanly (WARNING #5)
- [Phase ?]: Plan 02 Task 1: package legitimacy gate self-verified — all 5 crates' repository fields point to github.com/open-source-cooperative/* (keyring-core + dbus/apple/windows stores) or github.com/RustCrypto/utils (zeroize)
- [Phase ?]: Plan 03: crossterm 0.29 listed as direct dep with event-stream feature (Rule 3 deviation) — ratatui-crossterm does not propagate the feature; Cargo feature unification keeps single crossterm version (Pitfall L2 invariant verified via cargo tree -i crossterm)
- [Phase ?]: Plan 03: clippy.toml disallowed-types relaxed to empty (Rule 3 deviation) — type-level bans fight legitimate ratatui::crossterm re-exports; PITFALLS L2 invariant moved to dep-tree level via cargo tree
- [Phase ?]: Plan 03: ratatui::run sync signature LOCKED (Context7-verified) — async loops bridge via tokio::task::spawn_blocking + Handle::current().block_on. ratatui::init+restore manual pair forbidden (Pitfall L2 grep gate enforces)
- [Phase ?]: Plan 03 Task 2 checkpoint auto-approved under auto-mode — TUI-04 panic-safe restore + TUI-05 non-TTY refusal verified by automated portable-pty + assert_cmd tests
- [Phase ?]: Plan 01-04 BL-01: clock injection extended to src/tui/widgets/; AppState.now is the single data path; tui_loop render-tick arm is the SINGLE authorized TUI wall-clock site
- [Phase ?]: Plan 01-04 BL-02: canonical ProviderId row order Claude=0/Codex=1/Gemini=2/Mock=3 locked at the engine boundary via Engine::sort_key; Mock last (debug/fault-injection only)
- [Phase ?]: Plan 01-04 BL-03: 5h cluster gap uses jiff::Span::total(Unit::Second) > FIVE_HOURS_SECS strict-greater; three boundary tests lock the contract (4h59m30s, exactly-5h, 5h0m30s)
- [Phase ?]: Plan 01-04 WR-06: D-41 error path uses config::default_path().ok().map_or_else(...) for cross-OS path display; TODO(future-phase) preserves [secrets].storage = 'file' escape-hatch contract for a future plan
- [Phase ?]: Plan 01-04 WR-08: run_tui_stub deleted from src/cli/mod.rs; grep gate at 0 hits across src/ and tests/
- [Phase ?]: Plan 02-01 ADP-04: CodexProvider end-to-end — read-only sqlite (zero SELECT, busy_timeout 250ms) + JSONL rate_limits parse + spawn_blocking narrow wrap
- [Phase ?]: Plan 02-01 Rule 2 deviation: compact_line row label now sourced from id_label(state.id), not windows[0].label — UI-SPEC line 141 binding; Mock compact flips from 'mock-session  …' to 'mock  …'
- [Phase ?]: Plan 02-01: SchemaDrift sentinel generalized via id_label_titlecase in BOTH cli/render_text.rs and tui/widgets/hp_row.rs; Claude byte-identical to Phase 1; pre-existing TUI bug (Codex drift falsely claiming 'Claude adapter…') fixed
- [Phase 02-codex-output-formats]: Plan 02-02: HpWindow.detailed_label additive field (D-52) — preserves Phase 0 mock-session + Phase 1 claude compact literals; JSON-additive, serde-skipped when None
- [Phase 02-codex-output-formats]: Plan 02-02: CLAUDE_WEEKLY_TOKEN_LIMIT locked at None for Phase 2 (no reliable estimate); NaN sentinel + (limit unknown) footer distinct from SchemaDrift
- [Phase 02-codex-output-formats]: Plan 02-02: Claude weekly anchor = ISO-week Monday 00:00 LOCAL via jiff to_monday_one_offset; WeekAnchor enum locks type shape for future FirstPrompt variant
- [Phase 02-codex-output-formats]: Plan 02-02 Rule 1 bug: jiff::Timestamp rejects calendar Span units at runtime; use Span::hours(N*24) on Timestamp OR Date.checked_add(Span::days) on civil dates

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

Last session: 2026-05-25T03:29:48.886Z
Stopped at: Plan 02-02 complete (CORE-03 — --detailed + Claude weekly bar)
Resume file: None
