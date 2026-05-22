# Phase 0: Spike & Spine - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `00-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-05-22
**Phase:** 0-Spike & Spine
**Areas discussed:** Repo scaffolding choices, `model.rs` contract shape, Charset & HP bar visual, Gemini spike scope & kill criteria

---

## Repo Scaffolding Choices

### Crate name

| Option | Description | Selected |
|--------|-------------|----------|
| `ahb` | Short, easy to type, already used in PROJECT.md. CLI bin name = `ahb`. Crates.io risk: name squatting | ✓ |
| `ai-hp-bar` | Long but SEO-rich; `cargo install ai-hp-bar` is descriptive | |
| `aihpbar` | One word, matches `~/REPO/AIHPBar` directory | |

**User's choice:** `ahb`
**Notes:** Risk of crates.io name squat accepted; can fall back to `ai-hp-bar` later if needed.

### Rust edition

| Option | Description | Selected |
|--------|-------------|----------|
| 2024 | Latest, default in Rust 1.85+; let-else, lifetime improvements | ✓ |
| 2021 | Conservative; full ecosystem compatibility | |

**User's choice:** 2024

### License

| Option | Description | Selected |
|--------|-------------|----------|
| `MIT OR Apache-2.0` | Rust de-facto standard; consumer choice; SPDX tools recognize | ✓ |
| MIT only | Most permissive; no Apache patent grant | |
| Apache-2.0 only | Patent grant; less GPL-compatible | |

**User's choice:** `MIT OR Apache-2.0`

### MSRV

| Option | Description | Selected |
|--------|-------------|----------|
| 1.88 | ratatui 0.30 MSRV floor; gains let-chains, async fn in trait | ✓ |
| stable (dynamic) | Track latest stable; risk for intermediate users | |
| 1.85 | Rust 2024 edition minimum; conflicts with ratatui 0.30 | |

**User's choice:** 1.88

### CI scope

| Option | Description | Selected |
|--------|-------------|----------|
| Minimum (build + test + clippy × 3 OS) | GHA on push/PR; ubuntu/macos/windows | ✓ |
| Minimum + fmt + audit + deny | Adds fmt-check, cargo audit, cargo deny | |
| Defer all CI | Phase 4 polishes CI | |

**User's choice:** Minimum
**Notes:** fmt/audit/deny deferred to Phase 4 alongside cargo-dist.

---

## `model.rs` Contract Shape

### `ProviderState` shape — how many windows per fetch?

| Option | Description | Selected |
|--------|-------------|----------|
| Multiple windows (`Vec<HpWindow>`) | One provider returns N reset windows; e.g., Claude has 5h session + weekly. Matches `--compact` worst-case + `--detailed` all-windows. | ✓ |
| Single bar + optional weekly | Primary HP bar = session; secondary = `Option<HpBar>`. Simpler, but blocks future 3rd/4th window. | |
| Always exactly one bar | Worst-case window only. Conflicts with REQ CORE-03 `--detailed`. | |

**User's choice:** Multiple windows
**Notes:** Avoids N+1 fetches; aligns with research-recommended `HpWindow` slice in `ProviderState`.

### `HpUnit` shape — what raw data does the bar need?

| Option | Description | Selected |
|--------|-------------|----------|
| Percentage only (`percent_remaining: f32 + label`) | Transparent, normalized. Providers absorb their own units. Prevents "am I token-throttled?" confusion. | ✓ |
| Percent + raw fields | `raw_used`, `raw_limit`, `unit`. `--detailed` could show "12k / 50k tokens" — but Claude 5h window has no fixed limit number anyway. | |
| Just `u8` 0..=100 | Most compact; loses JSON metadata for downstream tooling. | |

**User's choice:** Percentage only

### `ResetInfo` shape — duration vs instant?

| Option | Description | Selected |
|--------|-------------|----------|
| Absolute `jiff::Timestamp` + computed countdown | Source of truth = `resets_at`; UI computes countdown. TUI 1s tick doesn't re-fetch; snapshot tests freeze time. | ✓ |
| Duration (`jiff::Span`) only | Direct but UI must re-fetch (or compute from absolute — defeats point). | |
| Both | Redundant; violates single source of truth. | |

**User's choice:** Absolute instant + computed countdown

### `ProviderError` variants

| Option | Description | Selected |
|--------|-------------|----------|
| Closed enum: Unconfigured / Unavailable / SchemaDrift / Network / RateLimited / Internal | thiserror enum; UI can render each variant uniquely. Adding variant = deliberate API change. | ✓ |
| Open: `thiserror + anyhow::Error` wrapping | Lossy; UI just sees "something failed". | |
| Single String `{ code, message }` | Most flexible; zero type safety. | |

**User's choice:** Closed enum

### Continue or next area

| Option | Description | Selected |
|--------|-------------|----------|
| More questions (ProviderState top-level, ttl_hint, bar_color) | Deeper API surface dive | |
| Next area | Central API set; remaining details go to Claude's discretion | ✓ |

**User's choice:** Next area

---

## Charset & HP Bar Visual

### Default bar charset

| Option | Description | Selected |
|--------|-------------|----------|
| Unicode full block `█ / ░` | SOTA default. Renders in modern terminals. | ✓ |
| ASCII `# / -` | Maximum compatibility; less HP-bar-like visually | |
| Sliced blocks `█ ▌ ▐ ░` | Sub-cell precision; needed only if bar width is tiny | |

**User's choice:** Unicode full block `█ / ░`

### Bar width

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed 10 cells | Compact; stable snapshot tests; multi-provider alignment | ✓ |
| Fixed 20 cells | More visually obvious "bar"; 3 providers per line may need wrap | |
| Terminal-relative | Auto-fit; TUI-friendly but breaks CLI snapshot tests | |

**User's choice:** Fixed 10 cells

### Color default

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-on TTY, auto-off pipe | Modern CLI standard; respects `NO_COLOR` + `--color` | ✓ |
| Off by default | User must opt in; loses HP-bar visual signal | |

**User's choice:** Auto-on TTY, auto-off pipe

### Fallback strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Unicode default + `--ascii` fallback | Single fallback flag; emoji excluded to avoid tmux width bugs | ✓ |
| Unicode default + `--ascii` + `--emoji` | Adds emoji mode (🟩🟧🟥); risks tmux/screen width issues | |

**User's choice:** Unicode default + `--ascii` fallback
**Notes:** Emoji deferred to v2 (DIFF-01 pace icon).

---

## Gemini Spike Scope & Kill Criteria

### Spike path

| Option | Description | Selected |
|--------|-------------|----------|
| Local `gemini /stats` capture first | Zero account-ban risk — reads our own CLI's output | ✓ |
| Web `gemini.google.com/usage` first | Verified curl-able by user, but Google account-ban risk | |
| Both in parallel | Time budget × 2; accelerates decision | |

**User's choice:** Local `gemini /stats` capture first

### Go criteria for local route

| Option | Description | Selected |
|--------|-------------|----------|
| Strict: parseable + remaining quota + reset time (all three) | Highest bar; Phase 3 inherits clean adapter contract | ✓ |
| Loose: any visible bar info | Risks Phase 3 finding gaps later | |

**User's choice:** Strict (all three)

### Fallback if local fails

| Option | Description | Selected |
|--------|-------------|----------|
| Fall back to web spike (same Phase 0) | Keeps Gemini in v1; absorbs account-ban risk | |
| Defer Gemini to v2 stub | Phase 3 ships stub behind `--experimental-gemini`; no account-ban risk | ✓ |
| Try both regardless | Doesn't gain over deferring; ban risk persists | |

**User's choice:** Defer Gemini to v2 stub

### Spike output format

| Option | Description | Selected |
|--------|-------------|----------|
| Markdown memo at `.planning/research/GEMINI_SPIKE.md` | Per ROADMAP.md Phase 0 success #1; structured sections | ✓ |
| Markdown + sample fixtures (txt files) | Memo + anonymised captures for Phase 3 reuse | |
| Memo only with Go/No-Go conclusion | Most minimal; risks rework | |

**User's choice:** Markdown memo only
**Notes:** Sample fixtures deferred to Phase 3 (when Gemini adapter is actually built). Phase 0 memo includes captures inline as prose.

---

## Claude's Discretion

The user explicitly deferred to Claude on:

- Choice of color crate (`nu-ansi-term` / `anstyle` / `owo-colors`) — pick based on TTY-skip ergonomics
- `ProviderId` shape (enum vs `Cow<'static, str>` newtype)
- Exact `FetchCtx` API beyond `now` + `secrets`
- `tokio::main` vs `pollster::block_on` for Phase 0 CLI entry
- Whether to include Phase 1+ deps eagerly or keep Phase 0 `Cargo.toml` minimal
- Whether to split `lib.rs` from `main.rs` for testability
- `bar_color` rendering thresholds (red/yellow/green cutoffs)

---

## Deferred Ideas

(Captured in CONTEXT.md `<deferred>` section — not repeated here.)

Highlights:

- `ttl_hint` on `ProviderState` → Phase 3 (cache)
- `stale: bool` flag on `ProviderState` → Phase 3 (cache fallback)
- Web `gemini.google.com/usage` route → explicit no-spike in Phase 0; revisit if v1 needs to expand
- Gemini fixture files → Phase 3
- Emoji / pace icon → v2 DIFF-01
- `cargo fmt --check` / `cargo audit` / `cargo deny` → Phase 4
- Cargo workspace split → only if v2+ adds daemon mode
