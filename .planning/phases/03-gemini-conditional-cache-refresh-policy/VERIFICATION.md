---
phase: 03-gemini-conditional-cache-refresh-policy
verified: 2026-05-25T12:02:33Z
status: passed
score: 7/7
overrides_applied: 0
re_verification: false
---

# Phase 3: Gemini (conditional) + Cache & Refresh Policy — Verification Report

**Phase Goal:** Wire the third provider per Phase 0's outcome — full Gemini HTTP adapter if the spike cleared, opt-in stub otherwise — and introduce the per-provider refresh-interval mechanism + moka stale-on-error cache that smooths transient network failures without blanking the bar.
**Verified:** 2026-05-25T12:02:33Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Verdict: PASS

Phase 0 spike concluded NO-GO. Phase 3 delivered all three components of the conditional path:

1. **Gemini error stub** — `GeminiUnimplementedProvider` returns `Err(Unavailable)` with the D-65 locked reason string; README §Gemini status section present; default-config.toml comment updated.
2. **Per-provider `refresh_interval`** — `ProviderConfig.refresh_interval: Option<u64>` parsed from TOML; `Engine::new` consumes per-provider constants + config values with ≥5s clamp.
3. **moka stale-on-error cache** — `Engine` owns `Cache<ProviderId, CacheEntry>`; `refresh_all` returns `Vec<(ProviderId, RowOutcome)>`; `RowState::StaleOk` variant in TUI; `build_stale_ok_line` renders yellow row with `(stale Ns ago)` suffix.

---

## Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | CFG-03: `refresh_interval = 30` in TOML → `ProviderConfig { refresh_interval: Some(30) }` | VERIFIED | `src/config.rs:48` — `pub refresh_interval: Option<u64>`; 5 integration tests in `tests/refresh_interval_config_parse.rs` all green |
| 2 | Typo key `refresh_intervall` warns but does not crash (D-38 forward-compat) | VERIFIED | `KNOWN_PROVIDER_FIELD_KEYS` at `src/config.rs:30` includes `"refresh_interval"`; `refresh_interval_typo_key_does_not_panic` test green |
| 3 | Values < 5s clamped to 5s with `tracing::warn!` (D-72 safety floor) | VERIFIED | `src/engine/mod.rs:175-182` — `resolve_interval` fn clamps; `refresh_interval_clamps_to_five_seconds_minimum` unit test green |
| 4 | Absent `refresh_interval` → 15s default per provider (D-72) | VERIFIED | `DEFAULT_REFRESH_INTERVAL_SECS = 15` in all 4 provider modules; `refresh_interval_absent_deserializes_to_none` test green |
| 5 | TUI shows `(stale Ns ago)` yellow row on transient adapter failure (TUI-03 + D-69) | VERIFIED | `RowState::StaleOk` in `src/tui/app.rs:36-39`; `build_stale_ok_line` in `src/tui/widgets/hp_row.rs:120-154`; 5 hp_row unit tests green (`build_stale_ok_line_uses_yellow_for_all_spans`, `_includes_stale_suffix`, `_two_spaces_before_suffix`, `_zero_secs`, `build_line_dispatches_stale_ok_to_stale_line`) |
| 6 | Gemini stub returns exact reason `"Gemini adapter deferred to v2 — see README §Gemini status"` (ADP-05 / D-65) | VERIFIED | `src/provider/gemini.rs:54`; `gemini_placeholder_returns_unavailable` + `gemini_error_reason_does_not_contain_old_literals` tests green |
| 7 | D-71 three-state timeline + SC-3 multi-provider cadence verified by tests | VERIFIED | 5 tests in `tests/cache_stale_on_error.rs` all green: `engine_fresh_stale_fresh_three_tick_sequence`, `engine_non_transient_error_does_not_stale_despite_cache`, `engine_no_cache_with_transient_error_produces_failed`, `engine_cache_hit_within_ttl_returns_fresh_without_fetch`, `engine_multi_provider_different_intervals_only_stale_provider_fetched` |

**Score:** 7/7 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/config.rs` | `ProviderConfig` with `refresh_interval: Option<u64>` | VERIFIED | Line 48: field present; `KNOWN_PROVIDER_FIELD_KEYS` includes `"refresh_interval"` at line 30 |
| `src/provider/claude/mod.rs` | `DEFAULT_REFRESH_INTERVAL_SECS = 15` | VERIFIED | `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15` at line 33 |
| `src/provider/codex/mod.rs` | `DEFAULT_REFRESH_INTERVAL_SECS = 15` | VERIFIED | `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15` at line 40 |
| `src/provider/gemini.rs` | `DEFAULT_REFRESH_INTERVAL_SECS = 15` + deferred error reason | VERIFIED | Line 36 (const), line 54 (exact reason string) |
| `src/provider/mock.rs` | `DEFAULT_REFRESH_INTERVAL_SECS = 15` | VERIFIED | `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15` at line 10 |
| `src/engine/cache.rs` | `CacheEntry`, `RowOutcome`, `is_transient` | VERIFIED | All 3 types present; 6 `is_transient` unit tests green |
| `src/engine/mod.rs` | `moka::sync::Cache` field + `refresh_intervals` + `refresh_all` returns `RowOutcome` | VERIFIED | `cache: Cache<ProviderId, CacheEntry>` at line 67; `refresh_intervals: HashMap<ProviderId, Duration>` at line 71; `refresh_all` signature changed |
| `Cargo.toml` | `moka = { version = "0.12", features = ["sync"] }` | VERIFIED | `moka` present in `Cargo.lock` (v0.12.15) |
| `src/tui/app.rs` | `RowState::StaleOk` variant + updated `apply_results` | VERIFIED | `StaleOk { state, stale_age_secs }` at line 36-39; `apply_results` accepts `Vec<(ProviderId, RowOutcome)>` at line 86 |
| `src/tui/widgets/hp_row.rs` | `build_stale_ok_line` + `build_line` StaleOk arm | VERIFIED | `build_stale_ok_line` at line 120; `build_line` StaleOk arm at line 71-74; 12× `Color::Yellow` in file |
| `src/tui/mod.rs` | SCAFFOLD removed; `apply_results` receives `RowOutcome` directly | VERIFIED | No `SCAFFOLD` string anywhere in `src/`; `app.apply_results(outcomes)` at line 115 and 154 |
| `src/templates/default-config.toml` | Gemini comment updated (D-64) | VERIFIED | `enabled = false  # Gemini CLI subscription — deferred to v2 stub (see README §Gemini status)` |
| `README.md` | `## Gemini adapter status — deferred to v2` section + ToS warning | VERIFIED | Section present; `gemini.google.com/usage` + `Kill criteria` references present |
| `tests/refresh_interval_config_parse.rs` | 5 config parse tests | VERIFIED | All 5 tests green |
| `tests/cache_stale_on_error.rs` | 5 engine-level stale tests | VERIFIED | All 5 tests green |
| `tests/no_walltime_in_adapter.rs` | Extended to scan `src/engine/` | VERIFIED | `scan_dirs: [&str; 3] = ["src/provider", "src/tui/widgets", "src/engine"]` at line 45 |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/config.rs` | `src/engine/mod.rs` | `Engine::new` reads `cfg.providers.<id>.refresh_interval` | VERIFIED | `resolve_interval` fn at line 171; called for all 4 providers in `Engine::new` |
| `src/provider/*.rs` | `src/engine/mod.rs` | `Engine::new` uses `DEFAULT_REFRESH_INTERVAL_SECS` fallback | VERIFIED | `claude::DEFAULT_REFRESH_INTERVAL_SECS` etc. imported at line 49; used in `resolve_interval` calls |
| `src/engine/cache.rs` | `src/engine/mod.rs` | `Engine` owns `Cache<ProviderId, CacheEntry>` | VERIFIED | `use cache::CacheEntry, RowOutcome, is_transient` at line 43-44; cache field at line 67 |
| `src/engine/mod.rs` | `src/cli/mod.rs` | `outcome_to_result` converts `RowOutcome` → `Result` | VERIFIED | `outcome_to_result` fn in `src/cli/mod.rs`; maps Fresh→Ok, Stale→unreachable!, Failed→Err |
| `src/engine/mod.rs` | `src/tui/mod.rs` | `tui_loop` calls `engine.refresh_all(now).await` → `app.apply_results(outcomes)` | VERIFIED | Lines 114-115 (priming) and 153-154 (fetch tick) in `src/tui/mod.rs` |
| `src/tui/app.rs` | `src/tui/widgets/hp_row.rs` | `build_line` dispatches `RowState::StaleOk` to `build_stale_ok_line` | VERIFIED | `build_line` match arm at line 71-74 in `hp_row.rs` |
| `src/provider/gemini.rs` | `README.md` | Error reason string aligns with README section title | VERIFIED | Both contain `"Gemini adapter deferred to v2"` |

---

## Invariant Audit (D-* + BL-01)

| Invariant | Status | Evidence |
|-----------|--------|----------|
| **D-63** No `--experimental-gemini` CLI flag | VERIFIED | `grep -rn "\-\-experimental-gemini" src/ README.md` → 0 matches |
| **D-64** `default-config.toml` Gemini comment updated | VERIFIED | Exact string `"deferred to v2 stub (see README §Gemini status)"` present |
| **D-65** `GeminiUnimplementedProvider::fetch` error reason exact | VERIFIED | `"Gemini adapter deferred to v2 — see README §Gemini status"` at `gemini.rs:54` |
| **D-66** moka cache in-memory only, no disk persist | VERIFIED | `Cache::builder().max_capacity(8).build()` with no TTL/TTI; CLI `outcome_to_result` maps Stale → `unreachable!()` |
| **D-68** CLI render_text.rs + render_json.rs not modified | VERIFIED | `grep -rn "stale" src/cli/render_text.rs` → 0 matches; same for render_json.rs |
| **D-70** `ProviderState` struct not modified (no stale fields) | VERIFIED | `grep -rn "stale_age_secs" src/model.rs` → 0 matches |
| **D-71** Cache TTL decoupled from stale fallback (SC-4) | VERIFIED | `is_transient` function in `cache.rs:68-73`; stale fallback uses cache regardless of TTL expiry state; `engine_returns_stale_on_transient_error_with_cache_hit` test confirms |
| **D-72** Per-provider defaults all 15s; clamp ≥5s | VERIFIED | All 4 `DEFAULT_REFRESH_INTERVAL_SECS = 15`; `REFRESH_INTERVAL_MIN_SECS = 5` at `engine/mod.rs:56`; clamp unit test green |
| **D-74** TUI global tick 15s unchanged | VERIFIED | `tokio::time::interval(Duration::from_secs(15))` present exactly once in `src/tui/mod.rs` (count=1) |
| **BL-01** No `Timestamp::now()` under `src/provider/`, `src/tui/widgets/`, `src/engine/` | VERIFIED | All hits are in doc-comments; `no_walltime_in_adapter.rs` test (now scanning 3 dirs) passes green |
| **SC-2** README ToS warning for Gemini | VERIFIED | `**ToS warning.** Web-scraping gemini.google.com/usage carries account-ban risk...` present |
| **SC-3** Multi-provider cadence test | VERIFIED | `engine_multi_provider_different_intervals_only_stale_provider_fetched`: Mock (TTL=5s) re-fetched, Codex (TTL=600s) served from cache |
| **SCAFFOLD removed** Plan 03-02 bridge fully gone | VERIFIED | `grep -rn "SCAFFOLD" src/` → 0 matches |

---

## Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| `src/tui/widgets/hp_row.rs::build_stale_ok_line` | `state: &ProviderState`, `stale_age_secs: u64` | `Engine::refresh_all` → `RowOutcome::Stale { state, stale_age_secs }` → `AppState::apply_results` → `RowState::StaleOk` → `build_line` → `build_stale_ok_line` | Data flows from moka cache entry (written on successful fetch from real provider adapter); `stale_age_secs` computed as `duration_since(now, entry.fetched_at)` | FLOWING |
| `src/engine/mod.rs::refresh_all` | `CacheEntry { state, fetched_at }` | `fanout::refresh_all_inner` returns `Ok(state)` → `cache.insert(id, CacheEntry { state: state.clone(), fetched_at: state.fetched_at })` | Real provider data, `fetched_at` from `FetchCtx::now` (BL-01) | FLOWING |

---

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Binary builds cleanly | `cargo build --release` | Finished in 7.18s, no errors | PASS |
| Full test suite passes | `cargo test` | 185 lib tests + 15 integration test suites, 0 failed | PASS |
| `AHB --help` works | `./target/release/ahb --help` | Displays usage with exit codes; no stale/gemini/experimental flags exposed | PASS |
| `AHB --json` runs (no secret store) | `./target/release/ahb --json` | Prints keyring warning, exits normally (no crash) | PASS |
| `cargo test cache_stale_on_error` | 5 specific tests | All 5 green | PASS |
| `cargo test refresh_interval_config_parse` | 5 specific tests | All 5 green | PASS |
| `cargo test no_walltime_in_adapter` | scan 3 dirs | 0 offenders found | PASS |

---

## Probe Execution

No `probe-*.sh` files declared or present for Phase 3. Behavioral spot-checks above serve as the equivalent validation.

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| TUI-03 | 03-02, 03-03, 03-05 | TUI refresh frequency configurable per-provider; network adapter can be ≥5min | SATISFIED | `ProviderConfig.refresh_interval` + `Engine::refresh_intervals`; stale-on-error path; multi-provider cadence test |
| CFG-03 | 03-01 | Config allows per-provider `refresh_interval`, limit override, auth source | SATISFIED | `refresh_interval: Option<u64>` field with 5-test integration coverage; forward-compat warn guard |
| ADP-05 | 03-04, 03-05 | Gemini adapter conditional on Phase 0; stub with README in NO-GO path | SATISFIED | `GeminiUnimplementedProvider` with exact D-65 reason; README section; default-config comment; regression tests |

---

## Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| None | — | — | No `TBD`, `FIXME`, `XXX`, or placeholder patterns found in Phase 3 modified files |

All files scanned: `src/config.rs`, `src/engine/cache.rs`, `src/engine/mod.rs`, `src/provider/gemini.rs`, `src/provider/claude/mod.rs`, `src/provider/codex/mod.rs`, `src/provider/mock.rs`, `src/tui/app.rs`, `src/tui/widgets/hp_row.rs`, `src/tui/mod.rs`, `src/templates/default-config.toml`, `README.md`, `tests/refresh_interval_config_parse.rs`, `tests/cache_stale_on_error.rs`, `tests/no_walltime_in_adapter.rs`.

---

## Human Verification Required

None — all acceptance criteria are verifiable programmatically. The TUI visual path (`AHB tui` showing a yellow row on transient failure) requires a running process with a deliberate network-cut scenario, but the rendering path is fully covered by unit tests (`build_stale_ok_line_uses_yellow_for_all_spans`, etc.) that freeze the input state deterministically.

---

## Gaps Summary

No gaps. All 7 observable truths are VERIFIED, all 15+ artifacts exist and are substantive, all key links are wired, all 3 requirements are satisfied, all invariants hold, and the full test suite (185 lib + 5 integration suites) passes with zero failures.

---

## Notable: Planned vs. Actual

### What shipped as planned
- All 5 plans executed exactly per spec.
- SCAFFOLD adapter (Plan 03-02) was introduced and removed (Plan 03-03) as designed — zero residue.
- `Engine::new_with_providers` is `pub` (not `#[cfg(test)]`) to allow integration tests to use it across crate boundaries — a pragmatic deviation documented in the code comment; does not affect production API surface (`#[doc(hidden)]`).

### What did NOT ship (intentional deferrals to v2)
- Full Gemini HTTP adapter (ETag, If-Modified-Since, daily ceiling, 5min floor) — NO-GO from Phase 0.
- `--experimental-gemini` CLI flag — v2 trigger.
- Disk-persisted cache — v2.
- Separate `cache_ttl` config field — v2.
- HTTP wiremock test infra (200/304/401/429/500) — no HTTP adapter in v1.

All deferrals are documented in `03-CONTEXT.md §Deferred`.

---

_Verified: 2026-05-25T12:02:33Z_
_Verifier: Claude (gsd-verifier)_
