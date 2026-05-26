# Phase 4: Distribution & Release Polish - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-26
**Phase:** 4-distribution-release-polish
**Areas discussed:** Crate identity & GH remote, Extra distribution channels, Release profile + tarball hygiene, README scope for Phase 4

---

## Area selection

| Option | Description | Selected |
|--------|-------------|----------|
| Crate identity & GH remote | crates.io crate name / version bump / GitHub repo bootstrap | ✓ |
| Extra distribution channels | brew / Scoop / AUR — in or out for v1 | ✓ |
| Release profile + tarball hygiene | `[profile.release]` 設定 + `[package].exclude` | ✓ |
| README scope for Phase 4 | minimal vs Standard OSS vs full marketing rewrite | ✓ |

**User's choice:** All four selected.
**Notes:** Phase 4 is the final phase of v1; user wanted to cover all open implementation decisions before planning.

---

## Crate identity & GH remote

### Sub-question 1 — Crate name on crates.io

| Option | Description | Selected |
|--------|-------------|----------|
| `ahb` (3-letter) | Matches binary name; high risk of squatting | |
| `ai-hp-bar` | Full name; clear; hard to squat | ✓ |
| `aihpbar` | Single word; SEO-friendly | |
| Decide at publish (multi-fallback) | Try `ahb` first, fall back to `ai-hp-bar` | |

**User's choice:** `ai-hp-bar`
**Notes:** Binary name stays `ahb` via `[[bin]]` block — install command differs (`cargo install ai-hp-bar`) but run command (`ahb`) unchanged.

### Sub-question 2 — First public release version

| Option | Description | Selected |
|--------|-------------|----------|
| `0.1.0` (semver pre-1.0) | Standard Rust first-public-release; allows breaking changes | ✓ |
| `1.0.0` (stability promise) | Lock CLI / schema; force major bump for any break | |
| `0.1.0-rc.1` (dry-run first) | Test cargo-dist pipeline before official release | |

**User's choice:** `0.1.0`
**Notes:** Schema_version: 1 already locked in Phase 2, but CLI flags / behaviour may still polish. Pre-1.0 retains flexibility.

### Sub-question 3 — GitHub remote state

| Option | Description | Selected |
|--------|-------------|----------|
| `True347/ahb` does not exist; Phase 4 creates + pushes | Plan includes `gh repo create` + push | ✓ |
| Repo exists but local not connected | Just wire up `git remote add` | |
| Repo + remote connected, tags not pushed | Confirm and only push tags | |
| Other | User clarifies manually | |

**User's choice:** Phase 4 creates the repo.
**Notes:** `gh repo create True347/ahb --public --source=. --remote=origin --push` is now a required Plan step. Homebrew tap repo (`True347/homebrew-tap`) also created in same phase.

---

## Extra distribution channels

| Option | Description | Selected |
|--------|-------------|----------|
| Homebrew tap (recommended) | cargo-dist auto-generates formula; macOS sidesteps Gatekeeper | ✓ |
| Scoop bucket (Windows) | Windows native dev preference; manual bucket maintenance | |
| AUR (Arch Linux) | Hand-rolled PKGBUILD; higher maintenance burden | |
| None — v1 = strict 3 paths only | cargo install / binstall / GH release | |

**User's choice:** Homebrew tap only.
**Notes:** Scoop / AUR / Apple Developer ID signing all deferred to v2. Tap repo named `True347/homebrew-tap` (Homebrew convention), not `homebrew-ahb`.

---

## Release profile + tarball hygiene

### Sub-question 1 — `[profile.release]` settings

| Option | Description | Selected |
|--------|-------------|----------|
| `lto=true + strip=symbols + opt-level=3` (recommended) | STACK 'minimum-binary' variant; CI link +1-2 min; smaller binary | ✓ |
| `lto=true + strip=symbols + opt-level='z'` (size-optimized) | Smallest binary; potential TUI perf cost | |
| Defaults (no tuning) | Largest binary; fastest CI; lowest-risk | |

**User's choice:** `lto=true / strip="symbols" / opt-level=3`
**Notes:** `opt-level=3` (not `"z"`) preserves TUI render perf. `panic = "abort"` NOT set — would break Phase 1 ADP-01 per-adapter isolation. `codegen-units = 1` NOT set — diminishing returns with `lto = true`.

### Sub-question 2 — `[package].exclude` rules

| Option | Description | Selected |
|--------|-------------|----------|
| Exclude `.planning/`, `.github/`, `tests/data/` (recommended) | Trims crates.io tarball significantly | ✓ |
| Exclude `.planning/` only | Keep CI workflows + test fixtures | |
| Don't exclude anything | Crate tarball stays full | |

**User's choice:** Exclude `.planning/` + `.github/` + `tests/data/` (+ `.claude/`, `.omg/`, `CLAUDE.md` added by Claude for completeness).
**Notes:** Source + integration tests + Cargo.toml + LICENSE + README still included — `cargo install --git` reproducibility preserved on the git side; only crates.io tarball slimmed.

---

## README scope for Phase 4

### Sub-question 1 — README scale

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal install-focused (smallest subset) | Just SC-2/3 minimum — install + Gatekeeper | |
| Standard OSS (recommended) | install + Gatekeeper + features + screenshot + 4 badges | ✓ |
| Full marketing rewrite | asciinema GIF + comparison table + public roadmap | |

**User's choice:** Standard OSS.
**Notes:** 4 badges = crates.io / CI / license / MSRV. Single PNG screenshot, NOT animated GIF (maintenance cost). No competitor comparison; no public roadmap. Existing Gemini status section (D-65) preserved verbatim.

### Sub-question 2 — Gatekeeper section coverage

| Option | Description | Selected |
|--------|-------------|----------|
| macOS main + Linux/Windows one-liner each (recommended) | xattr workaround + `chmod +x` + SmartScreen note | ✓ |
| macOS Gatekeeper only (minimum) | Only what SC-3 mandates | |
| Full three-OS troubleshooting | SELinux / SmartScreen / AntiVirus all covered | |

**User's choice:** macOS main + Linux/Windows one-liner each.
**Notes:** No Apple Developer ID plan documented (would set wrong user expectation). SELinux / AppArmor / SmartScreen deep-dive deferred to v2.

---

## Claude's Discretion

- `[bin]` block details (test / bench fields) — planner decides based on cargo-deny / cargo-dist warnings
- Badge image source (shields.io vs crates.io native) — recommend shields.io for consistency
- `[workspace.metadata.dist]` vs separate `dist-workspace.toml` — follow `cargo-dist init` default
- Screenshot capture details — platform / prompt / providers — recommend macOS + alacritty + claude+codex+mock
- `exclude` vs `include` strategy — planner can flip if cargo-dist verify-publish complains
- README badge ordering — version / CI / license / MSRV (planner adjusts to fit visual)
- Gatekeeper xattr binary path example (`./ahb` vs `~/Downloads/ahb`) — planner picks for consistency
- cargo-dist version pin: 0.32.x stable vs v1.0.0-rc.1 — recommend 0.32.x
- CI release.yml trigger condition — default `on: push: tags: ['v*']` (aligns with D-76 tag format)
- Dry-run before first real release — recommend `cargo dist plan` + `cargo publish --dry-run` step

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section. Highlights:

- Scoop bucket / AUR PKGBUILD → v2
- Apple Developer ID signing + notarization → v2 trigger condition
- `opt-level = "z"` minimum-binary variant → v2 if binary size becomes friction
- asciinema GIF / competitor comparison / public roadmap README → v2 marketing pass
- Linux SELinux / AppArmor / Windows SmartScreen full troubleshooting → v2
- `aarch64-pc-windows-msvc` target → v2 (very small user base)
- Linux musl variants → v2 (dbus + musl static linking awkward)
- `ahb-doctor` / `ahb-daemon` extra binaries → v2 OPS-01
- `cargo-deny` + `cargo-nextest` in CI → v2
- Auto-changelog (`git-cliff` / `cargo-release`) → v2
- Codecov / docs.rs / dependency-status badges → v2
- Feature gating (`--features extra-foo`) → v2
- Reproducible build / SLSA provenance → v2
