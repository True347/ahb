---
phase: 03-gemini-conditional-cache-refresh-policy
plan: "01"
subsystem: config
tags: [config, toml, serde, refresh_interval, CFG-03, D-72]

requires:
  - phase: 01-engine-claude-tui-scaffold
    provides: ProviderConfig + KNOWN_PROVIDER_FIELD_KEYS + D-38 forward-compat warn-walker
  - phase: 02-codex-output-formats
    provides: Stable ProviderConfig consumers (engine + cli dispatch) ready for additive field
provides:
  - ProviderConfig.refresh_interval Option<u64> field (CFG-03)
  - KNOWN_PROVIDER_FIELD_KEYS includes "refresh_interval" so D-38 warn-walker is silent on the new key
  - Per-provider DEFAULT_REFRESH_INTERVAL_SECS = 15 const on claude / codex / gemini / mock
  - Integration test surface (refresh_interval_config_parse.rs, 5 tests)
affects: [03-02-engine-cache, 03-03-tui-stale-row, 03-04-gemini-stub-finalization, 03-05-integration-tests]

tech-stack:
  added: []
  patterns:
    - "Per-provider DEFAULT_REFRESH_INTERVAL_SECS const lives in owning module (D-72) — no shared constants module; Engine::new will import each independently in Plan 02"
    - "ProviderConfig stays a pure DTO — clamping / validation logic deferred to Engine boundary (Plan 02). Forward-compat schema growth via #[serde(default)] + KNOWN_PROVIDER_FIELD_KEYS allow-list extension"

key-files:
  created:
    - tests/refresh_interval_config_parse.rs
  modified:
    - src/config.rs
    - src/provider/claude/mod.rs
    - src/provider/codex/mod.rs
    - src/provider/gemini.rs
    - src/provider/mock.rs
    - src/engine/mod.rs (Rule 3: struct-literal fix)
    - src/cli/mod.rs (Rule 3: struct-literal fix)
    - tests/engine_row_order.rs (Rule 3: struct-literal fix)

key-decisions:
  - "ProviderConfig.refresh_interval is Option<u64> with #[serde(default)] — Option captures absent-vs-explicit-zero; u64 matches Duration::from_secs and serde rejects negatives at parse time"
  - "Clamping ≥5s safety floor (D-72) deliberately NOT enforced at parse layer — pushed into Engine::new (Plan 02) to keep config.rs a pure DTO; this is the layered-validation pattern that lets the parser stay decoupled from the runtime cap"
  - "DEFAULT_REFRESH_INTERVAL_SECS = 15 lives in each provider module separately per D-72 (\"建議放在各 provider module 內\") instead of a shared constants module — Engine::new imports each via crate::provider::<id>::DEFAULT_REFRESH_INTERVAL_SECS in Plan 02"
  - "Phase 3 Plan 01 Rule 3 deviation: pre-existing struct-literal ProviderConfig { enabled: true } sites in src/engine/mod.rs, src/cli/mod.rs, and tests/engine_row_order.rs all gained ..Default::default() — ProviderConfig already derived Default so this is the minimal sound fix and avoids forcing every consumer to learn the new field"

patterns-established:
  - "Layered config validation: parser = pure DTO (accept any valid u64); Engine = runtime cap (≥5s clamp + tracing::warn!). Future config fields should follow the same split"
  - "Per-provider runtime constants live in their owning module, not a shared crate-level module — keeps the per-adapter ownership boundary clean and lets each module document why its value is what it is (gemini stub's DEFAULT is cosmetic, mock matches claude/codex for parity)"

requirements-completed: [CFG-03]

duration: 3m
completed: 2026-05-25
---

# Phase 3 Plan 01: refresh_interval config schema + per-provider defaults Summary

**Added `refresh_interval: Option<u64>` to `ProviderConfig` and published `DEFAULT_REFRESH_INTERVAL_SECS = 15` from each provider module, unblocking Plan 02's engine-side cache + TTL gating without touching engine behavior.**

## Performance

- **Duration:** 3m 3s
- **Started:** 2026-05-25T11:11:25Z
- **Completed:** 2026-05-25T11:14:28Z
- **Tasks:** 3
- **Files modified:** 8 (5 plan-declared + 3 Rule 3 auto-fix sites)

## Accomplishments

- `ProviderConfig` gains `refresh_interval: Option<u64>` parseable from TOML; absent = `None`, never `0` sentinel (CFG-03 / D-72)
- `KNOWN_PROVIDER_FIELD_KEYS` extended with `"refresh_interval"` so the D-38 forward-compat warn-walker does NOT warn on the new key, while typos (`refresh_intervall`) still warn
- Four provider modules (`claude`, `codex`, `gemini`, `mock`) each export `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15` (D-72) — ready for Plan 02's `Engine::new` to consume
- 5 new integration tests pass; full test suite (lib + 16 integration test binaries) stays green — Engine behavior is byte-identical to Phase 2 (no cache logic yet, exactly as the plan demanded)

## Task Commits

Each task was committed atomically. Task 1 followed TDD (RED → GREEN):

1. **Task 1 RED: Failing tests for refresh_interval** — `5dbaaa2` (test)
2. **Task 1 GREEN: Add refresh_interval to ProviderConfig + update KNOWN_PROVIDER_FIELD_KEYS** — `b69d4ac` (feat)
3. **Task 2: Publish DEFAULT_REFRESH_INTERVAL_SECS per provider module** — `1456d0d` (feat)
4. **Task 3: Integration test for refresh_interval config parsing** — `fe9e48d` (test)

## Files Created/Modified

### Created

- `tests/refresh_interval_config_parse.rs` — 5 integration tests through the public `ahb::config::load_or_init` API: parse roundtrip, absent → None, zero accepted, large value accepted (24h), typo-key forward-compat tolerance

### Modified

- `src/config.rs` — Added `refresh_interval: Option<u64>` to `ProviderConfig` with rich doc comment explaining the layered-validation contract; extended `KNOWN_PROVIDER_FIELD_KEYS` with `"refresh_interval"`; added 4 unit tests inside the existing `#[cfg(test)] mod tests` block
- `src/provider/claude/mod.rs` — Added `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15` next to the existing `pub use window::CLAUDE_5H_TOKEN_LIMIT`
- `src/provider/codex/mod.rs` — Added `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15` next to the existing `pub mod` declarations
- `src/provider/gemini.rs` — Added `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15` with explicit doc-comment noting the value has no practical effect for the stub (cache never populated, so the interval never triggers a real skip — value exists for parity)
- `src/provider/mock.rs` — Added `pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 15`
- `src/engine/mod.rs` — Rule 3 fix: three `ProviderConfig { enabled: true }` literals now `ProviderConfig { enabled: true, ..Default::default() }`
- `src/cli/mod.rs` — Rule 3 fix: two `ProviderConfig { enabled: true }` literals now `ProviderConfig { enabled: true, ..Default::default() }`
- `tests/engine_row_order.rs` — Rule 3 fix: two `ProviderConfig { enabled: true }` literals now `ProviderConfig { enabled: true, ..Default::default() }`

## Decisions Made

- **Field type `Option<u64>`** — `u64` matches `Duration::from_secs` directly and serde rejects negative TOML integers automatically; `Option` distinguishes "absent → use per-provider default" from explicit user values without resorting to a `0` sentinel. Documented in the `refresh_interval` field's doc-comment in `src/config.rs`.
- **Clamp lives in Engine, not config** — the parse layer accepts any valid `u64` (including `0` and `86400+`). The ≥5s safety floor and `tracing::warn!` belong in `Engine::new` (Plan 02). This is the canonical layered-validation split: parser is a DTO, runtime layer applies caps. The integration test `refresh_interval_zero_accepted_by_parser` pins this contract so Plan 02 cannot regress it by adding parse-time validation.
- **Per-module constants** — `DEFAULT_REFRESH_INTERVAL_SECS` lives in each provider's own module rather than a shared `crate::provider::defaults` module. This matches D-72's hint (`建議放在各 provider module 內`) and lets each provider document why its value is what it is (gemini's value is cosmetic because the stub never caches; mock matches claude/codex for parity). Plan 02's `Engine::new` will import each via `crate::provider::<id>::DEFAULT_REFRESH_INTERVAL_SECS`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Struct-literal call sites of ProviderConfig**

- **Found during:** Task 1 GREEN (after adding `refresh_interval` field, `cargo test --lib config` failed to compile because seven existing `ProviderConfig { enabled: true }` struct literals were missing the new field)
- **Issue:** Rust's struct-literal syntax requires every field unless `..Default::default()` is used. Adding a new non-`pub` field to a public struct without updating call sites breaks the build. The plan declared `files_modified` only for the four provider modules + config.rs + the new test file — it did not anticipate this knock-on edit.
- **Fix:** Added `..Default::default()` to all seven call sites (`src/engine/mod.rs` ×3, `src/cli/mod.rs` ×2, `tests/engine_row_order.rs` ×2). `ProviderConfig` already derived `Default` (verified in `src/config.rs:32`), so this is the minimal sound fix; no need to change `ProviderConfig`'s visibility or introduce a `Default::default()` builder.
- **Files modified:** `src/engine/mod.rs`, `src/cli/mod.rs`, `tests/engine_row_order.rs`
- **Verification:** `cargo build --lib` clean; full `cargo test` suite green (160 lib tests + all 16 integration test binaries).
- **Committed in:** `b69d4ac` (Task 1 GREEN — bundled with the field addition since the build-break-and-fix pair logically belongs together)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary for the lib to compile against the widened `ProviderConfig`. No scope creep — purely a knock-on edit forced by Rust's struct-literal exhaustiveness rule. None of the affected call sites change semantically (they all already meant "enabled=true, everything else default").

## Issues Encountered

None — the plan was very precisely scoped (additive schema extension only) and the only friction was the predictable Rule 3 deviation above, which was resolved in <1 minute.

## TDD Gate Compliance

- **RED gate (commit `5dbaaa2`):** `test(03-01): add failing tests for refresh_interval field on ProviderConfig` — confirmed RED via `cargo test --lib config::tests::provider_config` (3 compile errors on `no field 'refresh_interval'`).
- **GREEN gate (commit `b69d4ac`):** `feat(03-01): add refresh_interval to ProviderConfig (CFG-03)` — confirmed GREEN via `cargo test --lib config` (10/10 tests pass).
- **REFACTOR gate:** Not needed; implementation was already minimal.

Task-level TDD applied only to Task 1 per the plan's `tdd="true"` attribute. Tasks 2 and 3 are additive (constant exports + integration test file) and follow standard `feat`/`test` commits.

## Plan-Level Verification Results

All seven verification steps from PLAN.md `<verification>` passed:

1. ✅ `cargo test --lib config` — 10/10 pass
2. ✅ `cargo build --lib` — clean, no warnings
3. ✅ `grep -rn "DEFAULT_REFRESH_INTERVAL_SECS" src/provider/ | grep "pub const"` — exactly 4 lines (claude, codex, gemini, mock)
4. ✅ `grep -n "refresh_interval" src/config.rs` — hits in `KNOWN_PROVIDER_FIELD_KEYS` (line 30) AND `ProviderConfig` (line 48)
5. ✅ `cargo test refresh_interval_config_parse` — 5/5 pass
6. ✅ `cargo test` (full suite) — all integration tests still green
7. ✅ `grep -rn "\-\-experimental-gemini" src/` — 0 matches (D-63 binding preserved)

## User Setup Required

None — this plan is pure code changes; no external service configuration, env vars, or manual steps.

## Next Phase Readiness

- **Plan 02 (moka cache + engine rewrite)** can now read `cfg.providers.<id>.refresh_interval` and fall back to `crate::provider::<id>::DEFAULT_REFRESH_INTERVAL_SECS` without circular dependency. The schema and constants are stable.
- Engine behavior is byte-identical to Phase 2 — Plan 02 starts from a known-green baseline.
- No blockers; no concerns.

## Self-Check: PASSED

- ✅ `src/config.rs` modified (verified: contains `refresh_interval: Option<u64>` and `"refresh_interval"` in `KNOWN_PROVIDER_FIELD_KEYS`)
- ✅ `src/provider/claude/mod.rs` modified (verified: contains `pub const DEFAULT_REFRESH_INTERVAL_SECS`)
- ✅ `src/provider/codex/mod.rs` modified (verified: contains `pub const DEFAULT_REFRESH_INTERVAL_SECS`)
- ✅ `src/provider/gemini.rs` modified (verified: contains `pub const DEFAULT_REFRESH_INTERVAL_SECS`)
- ✅ `src/provider/mock.rs` modified (verified: contains `pub const DEFAULT_REFRESH_INTERVAL_SECS`)
- ✅ `tests/refresh_interval_config_parse.rs` created (verified: file exists, 5 tests pass)
- ✅ Commit `5dbaaa2` (Task 1 RED) — verified in `git log`
- ✅ Commit `b69d4ac` (Task 1 GREEN) — verified in `git log`
- ✅ Commit `1456d0d` (Task 2) — verified in `git log`
- ✅ Commit `fe9e48d` (Task 3) — verified in `git log`

---
*Phase: 03-gemini-conditional-cache-refresh-policy*
*Completed: 2026-05-25*
