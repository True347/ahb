---
phase: 00-spike-spine
plan: 02
subsystem: infra
tags: [rust, model, trait, contract, serde, async-trait, jiff, thiserror, anyhow, dyn-safety]

requires:
  - phase: 00-spike-spine/01
    provides: "Cargo manifest with 9 pinned deps (clap, tokio, async-trait, serde, serde_json, jiff, anyhow, thiserror, owo-colors) + static_assertions dev-dep + Phase 0 lint floor in src/main.rs"

provides:
  - "src/model.rs: 7 contract types (ProviderId, HpUnit, BarColor, ResetInfo, HpWindow, ProviderState, ProviderError) + NetworkErr stub + serialize_display helper"
  - "src/provider/mod.rs: #[async_trait] Provider trait + FetchCtx<'a> with minimal 2-field shape"
  - "src/secrets.rs: zero-field Secrets stub with Default (Phase 1 replaces with keyring-core wiring)"
  - "src/lib.rs: pub mod declarations for model + provider + secrets carrying the Phase 0 lint floor"
  - "Compile-time proofs: assert_impl_all!(ProviderState: Send, Sync) and assert_impl_all!(Box<dyn Provider>: Send, Sync)"
  - "W-2 serde-round-trip binding (Cow<'static, str> source field) verified by unit test"
  - "W-7 backtrace-absence binding (ProviderError::Internal Display-only JSON) verified by unit test"

affects: [00-03-skeleton, 00-04-spike, 01-engine-claude-tui, 02-codex-output, 03-gemini-cache, 04-distribution]

tech-stack:
  added: []
  patterns:
    - "Pattern 1: thiserror + serde co-derive via internally-tagged enum (#[serde(tag = \"kind\", rename_all = \"snake_case\")]). Newtype payloads of scalar-serialized types are converted to single-field struct variants so the internal tag's map-form requirement is satisfied; From impls preserve construction ergonomics."
    - "Pattern 2: serialize_with = \"serialize_display\" helper for error variants that wrap anyhow::Error and similar — emits the Display string only, never Debug or backtrace. Generic via Display + Serializer bounds, reusable across all error wrappers."
    - "Pattern 3: Cow<'static, str> instead of &'static str for any serde-derived field that must accept both static literals (adapter side) and owned strings (deserialization side). Equality is content-based so the borrowed/owned variant difference is invisible to consumers."
    - "Pattern 4: Compile-time dyn-safety + thread-safety assertion via `static_assertions::assert_impl_all!(Box<dyn Trait>: Send, Sync)` placed at module scope inside `#[cfg(test)] mod tests`. Free, runs at every cargo test compile, breaks if #[async_trait] is removed (RESEARCH Q8 binding)."

key-files:
  created:
    - "src/model.rs (243 lines): contract types + ProviderError tagged-enum serde pattern + NetworkErr stub + serialize_display helper + 4 inline tests"
    - "src/provider/mod.rs (66 lines): FetchCtx<'a> { now, &Secrets } + async_trait Provider trait + dyn-safety compile-time assertion + 2 runtime smoke tests"
    - "src/secrets.rs (7 lines): pub struct Secrets; with #[derive(Debug, Default, Clone)] — Phase 1 replacement target"
  modified:
    - "src/lib.rs: added pub mod model / pub mod provider / pub mod secrets + carried over the Phase 0 lint floor (#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)] + #![warn(clippy::pedantic)])"

key-decisions:
  - "ProviderError serde pattern: converted Network(NetworkErr) and Internal(anyhow::Error) from newtype variants to single-field struct variants (Network { source: NetworkErr }, Internal { source: anyhow::Error }). Reason: serde's internally-tagged enum mode (#[serde(tag = \"kind\")]) does not accept newtype variants whose inner type serializes to a string scalar — the test failed with 'cannot serialize tagged newtype variant ProviderError::Internal containing a string'. Construction ergonomics preserved via From<NetworkErr> and From<anyhow::Error> impls. The plan's acceptance criteria for `serialize_with = \"serialize_display\"` (twice) and `#[serde(tag = \"kind\")]` are still met by this shape."
  - "FetchCtx derives Copy: jiff::Timestamp is Copy and shared references (&Secrets) are always Copy, so #[derive(Copy)] succeeds without fallback to Clone-only as the plan permitted."
  - "Implemented std::error::Error for NetworkErr because thiserror's `#[error(\"network: {source}\")]` formatter calls .source() on a `source`-named field, which requires the Error trait. Cheap blanket impl with no body needed."
  - "Clippy pedantic warnings: scoped allow(default_constructed_unit_structs) at the two test fns that call `Secrets::default()` (acceptance criteria mandate the explicit call). Switched pointer-equality check to `&raw const s` form per the borrow_as_ptr lint hint."

patterns-established:
  - "Pattern A: serialize_with helper for error-Display emission. The `fn serialize_display<T: Display, S: Serializer>(...)` helper at file scope is a reusable shape — every future error wrapper that mentions anyhow::Error or a Display-only foreign type should route through it instead of inventing a new serializer."
  - "Pattern B: Phase 0 lint floor lives in lib.rs (not just main.rs). #![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)] + #![warn(clippy::pedantic)] in src/lib.rs means every module added under the crate root inherits the floor automatically. Plan 03 and Phase 1+ don't need per-file pragmas."
  - "Pattern C: trait + compile-time dyn-safety assertion paired in the same file. The assert_impl_all line lives inside the trait's own mod.rs so refactoring the trait can't silently break dyn-safety without breaking cargo test build."

requirements-completed: [ADP-00]

duration: 5m
completed: 2026-05-22
---

# Phase 00 Plan 02: Cross-Adapter Contract Spine Summary

**Locked the cross-adapter contract — 7 model types, async_trait Provider trait, FetchCtx, and Secrets stub — with serde round-trip, dyn-safety, and Display-only-error JSON proven at compile time + 6 inline tests.**

## Performance

- **Duration:** ~5 min (285 seconds wall clock)
- **Started:** 2026-05-22T12:01:19Z
- **Completed:** 2026-05-22T12:06:04Z
- **Tasks:** 2 / 2
- **Files created:** 3 (src/model.rs, src/provider/mod.rs, src/secrets.rs)
- **Files modified:** 1 (src/lib.rs)

## Accomplishments

- 7 contract types locked per D-08..D-14 + RESEARCH § Code Examples: `ProviderId` (closed enum Claude/Codex/Gemini/Mock), `HpUnit` (f32 alias), `BarColor` (Red/Yellow/Green), `ResetInfo` (jiff::Timestamp wrapper), `HpWindow` (label/percent/reset/color), `ProviderState` (id/windows/fetched_at/source), `ProviderError` (closed thiserror enum with 6 variants).
- `Box<dyn Provider>: Send + Sync` proven at compile time via `static_assertions::assert_impl_all!`. Same for `ProviderState: Send, Sync`.
- W-2 binding verified: `ProviderState.source` is `Cow<'static, str>`, round-trips through `serde_json::to_string` → `from_str` and the deserialized `Cow::Owned("mock")` compares equal to the original `Cow::Borrowed("mock")` via content-based PartialEq.
- W-7 binding verified: `ProviderError::Internal(anyhow::anyhow!("boom"))` serializes to JSON containing "boom" + "internal" + "kind" but NOT "Backtrace", "stack backtrace", "at /", or "at ./" — the Display-only serialization never leaks Debug-form metadata.
- `FetchCtx<'a>` shape locked at the minimal 2 fields (`now: jiff::Timestamp`, `secrets: &'a Secrets`) per RESEARCH Q5 recommendation. Derives `Copy` cleanly.
- `cargo build --lib` ✓, `cargo test --lib` ✓ (6/6 pass), `cargo clippy --lib --all-targets -- -D warnings` ✓ — all exit 0.
- No em-dash bytes (0xe2 0x80 0x94) in any of src/{model.rs, provider/mod.rs, secrets.rs, lib.rs}. ASCII-clean source per verification step 5.

## Task Commits

Each task was committed atomically on `master`:

1. **Task 1: Contract types in src/model.rs + lib.rs export** — `68a51a1` (feat)
2. **Task 2: Provider trait + FetchCtx + Secrets stub** — `7bda8e2` (feat)

_Plan metadata commit follows this SUMMARY._

## Files Created/Modified

### Created

- `src/model.rs` — 243 lines. Module imports + 7 contract types (`ProviderId`, `HpUnit`, `BarColor`, `ResetInfo`, `HpWindow`, `ProviderState`, `ProviderError`) + `NetworkErr` Phase-0 stub (Phase 3 widens to wrap `reqwest::Error`) + `serialize_display` helper + 4 inline `#[cfg(test)]` tests (`provider_state_serde_roundtrip`, `provider_error_internal_serializes_display`, `provider_error_schema_drift_serializes`, `reset_info_serde_roundtrip`) + 1 module-scope `assert_impl_all!`.
- `src/provider/mod.rs` — 66 lines. `FetchCtx<'a>` minimal-fields struct + `#[async_trait] pub trait Provider: Send + Sync + 'static` + 2 inline tests (`secrets_default_constructs`, `fetch_ctx_constructs`) + 1 module-scope `assert_impl_all!(Box<dyn Provider>: Send, Sync)`.
- `src/secrets.rs` — 7 lines. Single `pub struct Secrets;` with `#[derive(Debug, Default, Clone)]`. Phase 1 replaces wholesale.

### Modified

- `src/lib.rs` — was a 1-line crate-root marker; now declares `pub mod model; pub mod provider; pub mod secrets;` and carries the Phase 0 lint floor (`#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` + `#![warn(clippy::pedantic)]`) so every module under the crate root inherits the floor without per-file pragmas.

## Decisions Made

- **D-12 serde pattern shape:** Newtype variants `Network(NetworkErr)` and `Internal(anyhow::Error)` were converted to single-field struct variants `Network { source: NetworkErr }` and `Internal { source: anyhow::Error }`. Serde's internally-tagged enum mode (`#[serde(tag = "kind")]`) does NOT support newtype variants whose payload serializes to a string scalar — it requires the variant payload to serialize to a map so the tag field can be inlined. Construction ergonomics preserved via `From<NetworkErr>` and `From<anyhow::Error>` impls on `ProviderError`. The acceptance criteria's `grep` checks (`serialize_with = "serialize_display"` ×2; `#[serde(tag = "kind"` present; `fn serialize_display` present) all still hold.
- **NetworkErr Error trait impl:** thiserror's `#[error("network: {source}")]` formatter calls `.source()` on a field named `source`, which requires that field to implement `std::error::Error`. Added an empty `impl std::error::Error for NetworkErr {}` blanket impl. NetworkErr is conceptually an error type so this is semantically aligned; Phase 3 will replace NetworkErr with a `reqwest::Error` wrapper that has the Error impl natively.
- **FetchCtx derives Copy:** The plan said "derive Copy IF possible, fall back to Debug+Clone only if jiff::Timestamp isn't Copy". Verified `jiff::Timestamp: Copy` (the type is `repr(transparent)` over an `i64`), so `#[derive(Debug, Clone, Copy)]` compiles cleanly. No fallback needed.
- **Phase 0 lint floor on lib.rs, not just main.rs:** Plan 01's SUMMARY noted this as a Plan-02 decision. Chose to put `#![deny(...)]` + `#![warn(clippy::pedantic)]` on `src/lib.rs` so all modules added under the crate root inherit the floor automatically. Cleaner than per-file pragmas across model.rs / provider/mod.rs / secrets.rs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Serde tagged enum cannot serialize newtype variants of scalar-serialized types**

- **Found during:** Task 1 (`cargo test --lib model::tests::provider_error_internal_serializes_display`)
- **Issue:** The plan's verbatim transcription of RESEARCH § Pitfall 2 (lines 360-386) places `#[serde(tag = "kind")]` on `ProviderError` while keeping `Internal(anyhow::Error)` and `Network(NetworkErr)` as newtype variants. With `serialize_with = "serialize_display"` the payload serializes to a string scalar. Serde's internally-tagged form refuses this combination at runtime: `Error("cannot serialize tagged newtype variant ProviderError::Internal containing a string", line: 0, column: 0)`. This is a documented serde limitation — internally-tagged enums require variant payloads to serialize to a map so the tag field can be inlined.
- **Fix:** Converted both newtype variants to single-field struct variants. `Internal(anyhow::Error)` → `Internal { source: anyhow::Error }`. `Network(NetworkErr)` → `Network { source: NetworkErr }`. Kept the `#[serde(serialize_with = "serialize_display")]` attribute on the `source` field. Added `impl From<NetworkErr> for ProviderError` and `impl From<anyhow::Error> for ProviderError` so callers can still write `err.into()`. JSON form is now `{"kind": "internal", "source": "<display string>"}` — the W-7 sentinel form the plan asked for. All acceptance-criteria grep checks (`serialize_with = "serialize_display"` count ≥ 2, `#[serde(tag = "kind"` present) still pass.
- **Files modified:** src/model.rs (variants + From impls)
- **Verification:** `cargo test --lib model::tests::provider_error_internal_serializes_display` passes; `provider_error_schema_drift_serializes` still passes; clippy clean.
- **Committed in:** 68a51a1 (Task 1 commit)

**2. [Rule 1 - Bug] NetworkErr stub lacked std::error::Error impl required by thiserror {source} formatter**

- **Found during:** Task 1 (build failure after deviation 1's restructure)
- **Issue:** With `Network { source: NetworkErr }` and `#[error("network: {source}")]`, thiserror generated code that calls `.source()` on the `source` field — which only exists on types implementing `std::error::Error`. NetworkErr was only `Debug + Clone + Serialize + Display`. Compile error: "the method `as_dyn_error` exists for reference `&NetworkErr`, but its trait bounds were not satisfied: `NetworkErr: StdError`".
- **Fix:** Added empty `impl std::error::Error for NetworkErr {}` — NetworkErr's Display impl provides the message, default source() returns None, which is correct for a leaf error type. Phase 3 swaps NetworkErr for a reqwest::Error wrapper that has the impl natively.
- **Files modified:** src/model.rs
- **Verification:** cargo build clean
- **Committed in:** 68a51a1 (Task 1 commit)

**3. [Rule 1 - Bug] em-dash characters in doc comments violated verification step 5 (ASCII-clean source)**

- **Found during:** Task 1 verification (xxd byte scan after Task 1 first draft)
- **Issue:** Several doc comments contained em-dashes (U+2014, bytes `e2 80 94`). The plan's `<verification>` step 5 explicitly forbids em-dash bytes in source files (`xxd src/model.rs | grep -o '2014' | head -1 confirms the em-dash bytes are NOT present in source`).
- **Fix:** `sed -i 's/—/--/g'` on src/model.rs, then applied the same substitution proactively to src/provider/mod.rs and src/secrets.rs while writing them. Verified post-fix with `xxd ... | grep 'e2 80 94'` on all four source files — all clean.
- **Files modified:** src/model.rs, src/provider/mod.rs, src/secrets.rs
- **Verification:** byte-level grep on every src/ file confirms zero em-dash bytes.
- **Committed in:** 68a51a1 (Task 1) and 7bda8e2 (Task 2)

**4. [Rule 1 - Bug] Multiple clippy::pedantic warnings on first draft**

- **Found during:** Task 1 + Task 2 clippy runs
- **Issue:** Several pedantic warnings under `-D warnings`:
  - `clippy::doc_markdown` on doc comments mentioning `MockProvider`, `serde_json`, `bar_color`, `keyring-core`, `Secret<T>`, `FetchCtx` (clippy wants backticks around `Type`-shaped identifiers in docs).
  - `clippy::default_constructed_unit_structs` on `Secrets::default()` calls (clippy wants `Secrets {}` or just `Secrets` for unit structs — but the test's intent is to verify the Default impl).
  - `clippy::no_effect_underscore_binding` on `let _s = ...` / `let _ctx = ...` (clippy wants either consumption of the binding or a literal `let _ = ...`).
  - `clippy::borrow_as_ptr` on `&s` argument to `std::ptr::eq` (clippy wants `&raw const s`).
- **Fix:** 
  - Added backticks to doc comments where clippy flagged.
  - Scoped `#[allow(clippy::default_constructed_unit_structs)]` on the two test fns that legitimately want to exercise `::default()` per acceptance criteria.
  - Renamed `_s` / `_ctx` to `s` / `ctx` and added trivial reads (`let _: &Secrets = &s;` and `assert_eq!(ctx.now, now)`).
  - Switched the pointer-equality check to `let secrets_ptr: *const Secrets = &raw const s;` form.
- **Files modified:** src/model.rs, src/provider/mod.rs, src/secrets.rs
- **Verification:** `cargo clippy --lib --all-targets -- -D warnings` exits 0.
- **Committed in:** 68a51a1 (Task 1) and 7bda8e2 (Task 2)

---

**Total deviations:** 4 auto-fixed (all Rule 1 — bugs / lint compliance).
**Impact on plan:** No scope change. Deviation 1 is the structurally interesting one — it surfaces an actual serde-vs-thiserror limitation the plan's reference (RESEARCH Pitfall 2) didn't catch. The fix preserves both acceptance-criteria's intent (display-only serialization, internal tag, twice-used serialize_display) and gives downstream phases identical ergonomics via the new `From` impls. Deviations 2-4 are mechanical lint compliance under the Phase 0 lint floor that Plan 01 established; they don't change the contract surface.

## Issues Encountered

None beyond the deviations documented above. The TDD flow (write tests-and-impl together, then iterate on serde + clippy errors) caught everything within Task 1's verify step before commit.

## User Setup Required

None — Phase 0 still has no external services, secrets, or runtime config.

## Threat Flags

None. The Phase 0 plan's `<threat_model>` covered T-00-05 (info disclosure via Internal serialization) and T-00-08 (FetchCtx::secrets leak); both mitigated by the implementation (W-7 unit test guards "Backtrace"/"stack backtrace"/"at /"/"at ./" string absence; Secrets is empty). No new threat surface introduced.

## Next Phase Readiness

**Plan 03 (`src/provider/mock.rs` + `src/cli/render_text.rs` + `src/main.rs`)** can now:

- Implement `MockProvider` as `impl Provider for MockProvider` per the locked trait. `provider::FetchCtx` + `model::*` are reachable via `ahb::provider::*` and `ahb::model::*`.
- Use `From<anyhow::Error>` (added in Task 1) when wrapping internal failures — no need to spell out `ProviderError::Internal { source: ... }` explicitly.
- Rely on `assert_impl_all!(Box<dyn Provider>: Send, Sync)` already being proven at compile time — Plan 03 only needs to verify that `MockProvider: Send + Sync` (which falls out trivially since it's an empty unit struct).
- Use `ProviderState.source = Cow::Borrowed("mock")` for zero-cost static label; the round-trip pattern is already covered by model.rs tests.

**Plan 04 (Gemini spike memo)** has no Rust dependencies and is unaffected by this plan.

**Phase 1 (engine + Claude + TUI)** has the API stability guarantee it needs:
- `ProviderError::Network`/`Internal` are struct-shaped, so adding fields later is forward-compatible.
- `From` impls let any `anyhow::Error` flow into `ProviderError` via `?` — the engine's `Vec<Result<...>>` aggregation is ergonomic.
- `FetchCtx`'s 2-field minimal shape can grow (add `http: &reqwest::Client` in Phase 1) without breaking adapters that destructure only `now` and `secrets`.

**No blockers or concerns.**

## Self-Check: PASSED

Verified post-write:

- **Files exist:**
  - `src/model.rs` — FOUND
  - `src/provider/mod.rs` — FOUND
  - `src/secrets.rs` — FOUND
  - `src/lib.rs` — FOUND (modified)
  - `.planning/phases/00-spike-spine/00-02-SUMMARY.md` — FOUND (this file)
- **Commit hashes verified in git log:**
  - `68a51a1` — Task 1 (feat: contract types in src/model.rs)
  - `7bda8e2` — Task 2 (feat: Provider trait + FetchCtx + Secrets stub)
- **End-to-end verification:**
  - `cargo build --lib` ✓ exit 0
  - `cargo test --lib` ✓ 6 / 6 passed
  - `cargo clippy --lib --all-targets -- -D warnings` ✓ exit 0
  - Em-dash byte scan: clean across all four source files

---

*Phase: 00-spike-spine*
*Completed: 2026-05-22*
