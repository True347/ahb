---
phase: 04-distribution-release-polish
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - Cargo.lock
  - README.md
  - .github/assets/screenshot.png
autonomous: true
requirements: [DIST-03, DIST-04]
user_setup: []

must_haves:
  truths:
    - "Cargo.toml `[package].name = \"ai-hp-bar\"` and `version = \"0.1.0\"` (D-75 + D-76)"
    - "Cargo.toml `[[bin]] name = \"ahb\" path = \"src/main.rs\"` so `cargo install ai-hp-bar` still produces `ahb` binary (D-75 + Finding 2)"
    - "Cargo.toml `[package].exclude` lists `.planning/`, `.github/`, `tests/data/`, `.claude/`, `.omg/`, `CLAUDE.md` (D-82)"
    - "Cargo.toml `[profile.release]` sets `lto = true / strip = \"symbols\" / opt-level = 3` and DOES NOT set `panic = \"abort\"` or `codegen-units = 1` (D-81)"
    - "Cargo.lock regenerated with `name = \"ai-hp-bar\"` entry (no stale `name = \"ahb\"` package row)"
    - "README.md contains 11 sections in D-83 order: 4 shields.io badges, H1+tagline (verbatim from old line 1-4), 5 features bullets, screenshot (absolute raw.githubusercontent.com URL), Install (4 channels in D-79 order), Quick start (verbatim old line 6-9), `## macOS Gatekeeper / cross-OS first-run notes` (D-84), Configuration, `## Gemini adapter status — deferred to v2` (verbatim old line 11-30 — D-65 binding), License, Contributing"
    - ".github/assets/screenshot.png exists and is referenced by absolute https://raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png URL (Pattern C, Finding 4)"
    - "`cargo build --release` succeeds against the new manifest"
    - "`cargo publish --dry-run` packaging line does NOT include `.planning/`, `.github/`, `.claude/`, `.omg/`, or `CLAUDE.md` paths (D-82 verification)"
  artifacts:
    - path: "Cargo.toml"
      provides: "v0.1.0 package metadata + bin name pin + release profile + exclude rules"
      contains: "name = \"ai-hp-bar\""
    - path: "Cargo.lock"
      provides: "regenerated lockfile reflecting crate rename"
      contains: "name = \"ai-hp-bar\""
    - path: "README.md"
      provides: "Standard OSS README with install + Gatekeeper + features + screenshot + 4 badges"
      contains: "## macOS Gatekeeper / cross-OS first-run notes"
    - path: ".github/assets/screenshot.png"
      provides: "Static screenshot referenced from README by absolute URL"
  key_links:
    - from: "README.md"
      to: ".github/assets/screenshot.png"
      via: "absolute raw.githubusercontent.com URL"
      pattern: "https://raw\\.githubusercontent\\.com/True347/ahb/HEAD/\\.github/assets/screenshot\\.png"
    - from: "README.md"
      to: "old README.md:11-30 D-65 Gemini status block"
      via: "verbatim copy of `## Gemini adapter status — deferred to v2` section"
      pattern: "Gemini adapter status — deferred to v2"
    - from: "Cargo.toml [[bin]]"
      to: "tests/*.rs `assert_cmd::cargo_bin(\"ahb\")` call sites"
      via: "binary name pinned to `ahb` so all Phase 1-3 integration tests keep working (Finding 2)"
      pattern: "name = \"ahb\""
---

<objective>
Vertical slice toward the user-observable artifact `brew install True347/tap/ahb`: do all the reversible local-only edits that establish the crate identity + tarball hygiene + user-facing documentation before any irreversible publish step.

Purpose: 90% of "looks done but isn't" distribution risk is fixed here — get `Cargo.toml` and `README.md` to the exact shape that `cargo dist init` (Plan 02) and `cargo publish` (Plan 03) need, while keeping the Phase 0-3 binary contract (`ahb`) intact. Nothing in this plan touches a remote — everything is `git diff` reviewable and revertable.
Output: Cargo.toml + Cargo.lock + README.md + .github/assets/screenshot.png committed, `cargo publish --dry-run` clean, `cargo build --release` green.
</objective>

<execution_context>
@/home/chasel/REPO/AIHPBar/.claude/get-shit-done/workflows/execute-plan.md
@/home/chasel/REPO/AIHPBar/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@/home/chasel/REPO/AIHPBar/.planning/PROJECT.md
@/home/chasel/REPO/AIHPBar/.planning/ROADMAP.md
@/home/chasel/REPO/AIHPBar/.planning/STATE.md
@/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-CONTEXT.md
@/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-RESEARCH.md
@/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-PATTERNS.md
@/home/chasel/REPO/AIHPBar/Cargo.toml
@/home/chasel/REPO/AIHPBar/README.md
@/home/chasel/REPO/AIHPBar/.github/workflows/ci.yml

<interfaces>
<!-- Phase 4 has no Rust code surface; "interfaces" here are the literal TOML keys + Markdown section anchors that downstream tooling reads. -->

Cargo.toml — the five EXISTING metadata fields whose ORDER must be preserved (Phase 0 lock, Pattern A):
- name (modified)
- version (modified)
- edition (unchanged: "2024")
- rust-version (unchanged: "1.88")
- license (unchanged: "MIT OR Apache-2.0")
- description (unchanged)
- repository (unchanged: "https://github.com/True347/ahb")
- readme (unchanged: "README.md")
- keywords (unchanged: ["claude", "codex", "gemini", "cli", "tui"])
- categories (unchanged: ["command-line-utilities"])

Cargo.toml — NEW keys inserted by this plan:
- [package].exclude — array of paths (D-82 verbatim)
- [[bin]] name + path — single bin block right after [package] (D-75 + Finding 2)
- [profile.release].lto / .strip / .opt-level — three lines REPLACING the existing placeholder comment at Cargo.toml:130-131 (D-81 verbatim)

README.md — NEW section anchors (matched by literal heading text):
- "# AHB — AI HP Bar" (unchanged H1)
- "## Install" (new)
- "## Quick start" (new)
- "## macOS Gatekeeper / cross-OS first-run notes" (new — D-84 binding)
- "## Configuration" (new)
- "## Gemini adapter status — deferred to v2" (verbatim D-65 block from old README:11-30)
- "## License" (new)
- "## Contributing" (new)
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Rewrite Cargo.toml [package] + [[bin]] + [profile.release] + regenerate Cargo.lock</name>
  <read_first>
    @/home/chasel/REPO/AIHPBar/Cargo.toml
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-CONTEXT.md (decisions D-75, D-76, D-81, D-82)
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-RESEARCH.md (Examples 1 + 2, Findings 2 + 5)
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-PATTERNS.md (Pattern A "self-amend" identification table)
  </read_first>
  <action>
    Edit Cargo.toml in place per Pattern A self-amend, preserving the existing 5-metadata-field ORDER (Phase 0 lock).

    (a) [package] block changes (in this order — per D-75 + D-76 + D-82):
        - Change `name = "ahb"` to `name = "ai-hp-bar"` (D-75).
        - Change `version = "0.0.1"` to `version = "0.1.0"` (D-76).
        - Leave `edition`, `rust-version`, `license`, `description`, `repository`, `readme`, `keywords`, `categories` UNCHANGED — Phase 0-3 already locked their values.
        - Insert a new `exclude` key AFTER `categories` and BEFORE the first `[dependencies]` table. The exclude array MUST contain exactly these six entries in this order (D-82): `".planning/"`, `".github/"`, `"tests/data/"`, `".claude/"`, `".omg/"`, `"CLAUDE.md"`.

    (b) Insert a new `[[bin]]` block IMMEDIATELY after the `[package]` block (D-75 + Finding 2). Exactly two keys: `name = "ahb"` and `path = "src/main.rs"`. Do not add `test = true` / `bench = false` (per CONTEXT Claude's Discretion + RESEARCH discretion note — they introduce spurious lint).

    (c) Replace the existing `[profile.release]` placeholder block at the END of Cargo.toml (currently the comment line `# Phase 4 will tune more. Phase 0 default is fine.`) with the three D-81 LOCKED keys: `lto = true`, `strip = "symbols"`, `opt-level = 3`. CRITICAL: do NOT add `codegen-units = 1` (defeats LTO), do NOT add `panic = "abort"` (Pitfall 3 — breaks ADP-01 per-adapter unwind isolation contract from Phase 1).

    (d) Do NOT add `[workspace.metadata.dist]` or `[profile.dist]` in this task — Plan 02 will run `cargo dist init` which appends them (Pitfall 4 — order-sensitive).

    (e) Do NOT touch `[dependencies]`, `[target.'cfg(target_os = ...)'.dependencies]`, or `[dev-dependencies]` — Phase 1-3 locked these.

    (f) Run `cargo build --release` from the repo root to regenerate Cargo.lock with the new crate name (per Runtime State Inventory). Commit the updated Cargo.lock alongside Cargo.toml.

    (g) Sanity: `grep -n '^name = "ai-hp-bar"' Cargo.toml` must hit `[package].name` line; `grep -c '^name = "ahb"' Cargo.toml` should equal 1 (only the new `[[bin]] name`, since the old `[package] name` was renamed away).
  </action>
  <verify>
    <automated>
cd /home/chasel/REPO/AIHPBar && \
  grep -q '^name = "ai-hp-bar"$' Cargo.toml && \
  grep -q '^version = "0.1.0"$' Cargo.toml && \
  grep -A1 '^\[\[bin\]\]$' Cargo.toml | grep -q 'name = "ahb"' && \
  grep -A2 '^\[\[bin\]\]$' Cargo.toml | grep -q 'path = "src/main.rs"' && \
  grep -A8 '^\[package\]' Cargo.toml | grep -q '^exclude = \[' && \
  grep -Pzo '(?s)exclude = \[.*?\.planning/.*?\.github/.*?tests/data/.*?\.claude/.*?\.omg/.*?CLAUDE\.md.*?\]' Cargo.toml >/dev/null && \
  grep -q '^lto = true$' Cargo.toml && \
  grep -q '^strip = "symbols"$' Cargo.toml && \
  grep -q '^opt-level = 3$' Cargo.toml && \
  ! grep -Eq '^(panic\s*=|codegen-units\s*=)' Cargo.toml && \
  grep -q '^name = "ai-hp-bar"' Cargo.lock && \
  ! grep -q '^name = "ahb"$' Cargo.lock && \
  cargo build --release 2>&1 | tail -3
    </automated>
  </verify>
  <acceptance_criteria>
    - **source-assert (Cargo.toml):** Line `name = "ai-hp-bar"` appears exactly once at top of `[package]`; line `version = "0.1.0"` immediately follows.
    - **source-assert (Cargo.toml [[bin]]):** Exactly one `[[bin]]` block exists; contains literal `name = "ahb"` and `path = "src/main.rs"`; no `test = `/`bench = ` keys.
    - **source-assert (Cargo.toml exclude):** `exclude` array under `[package]` contains all six D-82 paths in order — `.planning/`, `.github/`, `tests/data/`, `.claude/`, `.omg/`, `CLAUDE.md` — verified via the `grep -Pzo` multi-line gate above.
    - **source-assert (Cargo.toml [profile.release]):** Block contains exactly `lto = true`, `strip = "symbols"`, `opt-level = 3` (three lines) with NO `panic`/`codegen-units` keys — Pitfall 3 + D-81 binding.
    - **source-assert (Cargo.lock):** Top-level `name = "ai-hp-bar"` entry exists; old `name = "ahb"` package row absent.
    - **behavior-assert:** `cargo build --release` exits 0 and produces `target/release/ahb` (binary name preserved by [[bin]]).
    - **regression guard:** `cargo test --all-targets --no-run` builds without renaming-related errors (`assert_cmd::cargo_bin("ahb")` callers still resolve — Finding 2).
  </acceptance_criteria>
  <done>
    Cargo.toml + Cargo.lock committed together; `target/release/ahb` exists; `cargo test --all-targets --no-run` builds clean.
  </done>
</task>

<task type="auto">
  <name>Task 2: Capture screenshot.png and rewrite README.md to 11-section D-83 structure</name>
  <read_first>
    @/home/chasel/REPO/AIHPBar/README.md (lines 1-30 — H1+tagline + Quick start commands + D-65 Gemini block which must be preserved verbatim)
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-CONTEXT.md (D-83 section list, D-84 Gatekeeper text)
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-RESEARCH.md (Examples 4 + 5 + 6 + 7 — verbatim badge URLs, screenshot URL, install commands, Gatekeeper block; Finding 4 absolute-URL rule; Pattern C)
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-PATTERNS.md (README.md section ordering table)
  </read_first>
  <action>
    (a) Capture `.github/assets/screenshot.png`:
        - Create `.github/assets/` directory if missing.
        - On the macOS dev box: use `screencapture -i -w .github/assets/screenshot.png` (Pattern alternative: ImageMagick `import` on Linux box if user runs there). Acceptable composition: a terminal window showing two stacked invocations — `AHB` (compact line with claude+codex+mock rows) followed by `AHB --detailed` excerpt. PNG only — NOT a GIF (D-83 prohibits asciinema/GIF).
        - Place file at `.github/assets/screenshot.png` (path is literally inside the `.github/` tree that D-82 excludes from the crate tarball; this is intentional — Finding 4 confirms crates.io rendering uses the absolute raw URL not the tarball).
        - If running unattended without ability to capture, fall back to a placeholder PNG (e.g., one generated from `cargo run -- > /tmp/out.txt && convert -size 800x300 ...`) — but EXECUTOR MUST flag this in SUMMARY so a human can replace it before tagging v0.1.0.

    (b) Rewrite README.md from scratch into the 11-section structure per D-83 + PATTERNS § README. ORDER is binding. Section-by-section content rules:

        Section 1 — Badges row. Copy the four shields.io URLs verbatim from RESEARCH Example 4. Order: crates.io / CI / License / MSRV. Place IMMEDIATELY above the H1.

        Section 2 — H1 + tagline. Copy verbatim from current README.md lines 1-4: `# AHB — AI HP Bar` then blank line then the two-sentence tagline ("A Rust CLI + TUI that shows your LLM subscription ..."). Pattern A binding — do not rewrite.

        Section 3 — Features bullet list. Use exactly these five bullets in this order (CONTEXT D-83 step 3, RESEARCH PATTERNS § README step 3):
          - Compact / detailed / JSON output modes
          - Static TUI mode with 15s auto-refresh
          - Multi-provider with per-adapter error isolation
          - Stale-on-error indicator for transient network failures (Phase 3)
          - OS keyring-backed credentials (no plaintext on disk)

        Section 4 — Screenshot. Use literal Markdown image syntax with ABSOLUTE raw.githubusercontent.com URL — copy verbatim from RESEARCH Example 5: `![AHB demo — compact, detailed, and TUI modes side by side](https://raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png)`. Finding 4 binding — relative `.github/assets/...` URL would break crates.io rendering because `.github/` is excluded from tarball.

        Section 5 — Install. Heading `## Install`. Copy install block verbatim from RESEARCH Example 6 — exactly four `sh`-fenced commands in this order: `brew install True347/tap/ahb` (with comment "macOS / Linux with Homebrew (recommended — sidesteps Gatekeeper)"), `cargo binstall ai-hp-bar`, `cargo install ai-hp-bar`, `curl -fsSL https://github.com/True347/ahb/releases/latest/download/ahb-installer.sh | sh`. All `cargo install` / `cargo binstall` references use crate name `ai-hp-bar` (D-75); the brew formula reference uses `ahb` (Finding 3).

        Section 6 — Quick start. Heading `## Quick start`. Copy the four `AHB` example bullets VERBATIM from current README.md lines 6-9 — do not rewrite (RESEARCH Open Question 5).

        Section 7 — `## macOS Gatekeeper / cross-OS first-run notes`. Copy block verbatim from RESEARCH Example 7. macOS bullet uses `xattr -d com.apple.quarantine ./ahb` (D-84 binding); Linux bullet `chmod +x ./ahb`; Windows bullet "SmartScreen → 'More info' → 'Run anyway'". Closing line: "Installing via `brew install True347/tap/ahb` sidesteps all three."

        Section 8 — `## Configuration`. One short paragraph: point at `~/.config/ahb/config.toml` (cross-OS resolution via `directories` crate per CFG-02), list provider keys `claude`, `codex`, `gemini`, `mock` plus `refresh_interval` (D-83 step 8). Do NOT replicate the full TOML schema — just one paragraph + a `~/.config/ahb/config.toml` mention.

        Section 9 — `## Gemini adapter status — deferred to v2`. **VERBATIM COPY** of current README.md lines 11-30 (D-65 binding from Phase 3 — Phase 4 MUST NOT alter a single byte of this section). Use Read to fetch those lines, then paste them as-is into the new README at this position. The two relative path references in the block (`.planning/research/GEMINI_SPIKE.md` and `PITFALLS.md § Pitfall 1`) STAY relative — they render on GitHub but 404 on crates.io, which is acceptable per PATTERNS § README footnote.

        Section 10 — `## License`. Single short paragraph: dual-licensed under MIT OR Apache-2.0; link to LICENSE-MIT and LICENSE-APACHE files in the repo.

        Section 11 — `## Contributing`. One line literal: "PRs welcome, file issues for missing provider or unexpected output." (D-83 step 11).

    (c) DO NOT add any of these (D-83 prohibition): asciinema GIF, comparison table vs. hpup/llmstat/etc., public roadmap, advanced badges (codecov / docs.rs / dependency status), Apple Developer ID signing promises.

    (d) Pattern C absolute-URL gate applies: every `![](...)` image MUST use `https://` URL; every external link in badges + license MUST be absolute.
  </action>
  <verify>
    <automated>
cd /home/chasel/REPO/AIHPBar && \
  test -f .github/assets/screenshot.png && \
  grep -c '^## ' README.md | awk '$1>=9' && \
  grep -q '^# AHB — AI HP Bar$' README.md && \
  grep -q '^## Install$' README.md && \
  grep -q '^## Quick start$' README.md && \
  grep -q '^## macOS Gatekeeper / cross-OS first-run notes$' README.md && \
  grep -q '^## Configuration$' README.md && \
  grep -q '^## Gemini adapter status — deferred to v2$' README.md && \
  grep -q '^## License$' README.md && \
  grep -q '^## Contributing$' README.md && \
  grep -q 'img.shields.io/crates/v/ai-hp-bar' README.md && \
  grep -q 'img.shields.io/github/actions/workflow/status/True347/ahb' README.md && \
  grep -q 'img.shields.io/badge/license-MIT' README.md && \
  grep -q 'img.shields.io/badge/MSRV-1.88' README.md && \
  grep -q 'raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png' README.md && \
  ! grep -E '\!\[[^]]*\]\(\.github/' README.md && \
  grep -q 'brew install True347/tap/ahb' README.md && \
  grep -q 'cargo binstall ai-hp-bar' README.md && \
  grep -q 'cargo install ai-hp-bar' README.md && \
  grep -q 'releases/latest/download/ahb-installer.sh' README.md && \
  grep -q 'xattr -d com.apple.quarantine ./ahb' README.md && \
  grep -q '^The Gemini adapter is deferred to v2\.' README.md && \
  grep -q 'gemini-cli 0.41.2' README.md && \
  grep -q 'Web-scraping `gemini.google.com/usage`' README.md
    </automated>
  </verify>
  <acceptance_criteria>
    - **artifact-assert:** `.github/assets/screenshot.png` exists and is non-empty (size > 0 bytes).
    - **source-assert (README sections):** All nine `## ` headings from D-83 are present (Install / Quick start / macOS Gatekeeper / Configuration / Gemini adapter status / License / Contributing — plus the dynamic two that arise from the badge row + features list).
    - **source-assert (D-65 Gemini block preserved):** README contains the exact strings `The Gemini adapter is deferred to v2.`, `gemini-cli 0.41.2`, and `Web-scraping \`gemini.google.com/usage\`` — three byte-anchors that the regression gate above checks. If any of these is missing, executor accidentally rewrote the locked block.
    - **source-assert (badges):** All four shields.io URL fragments present.
    - **source-assert (install commands):** All four install commands present in their D-79 order — verified by the four `grep -q` lines above.
    - **source-assert (Gatekeeper):** Literal `xattr -d com.apple.quarantine ./ahb` appears.
    - **Pattern C gate (absolute URL):** Zero matches for `![...](.github/...)` relative image references — the `! grep -E` line above MUST pass.
    - **cross-check with Task 1 binary contract:** `grep -c "brew install True347/tap/ahb" README.md` ≥ 1 (formula name `ahb` from Finding 3, NOT `ai-hp-bar`).
  </acceptance_criteria>
  <done>
    README.md committed in 11-section D-83 form; `.github/assets/screenshot.png` committed; all D-65 Gemini section bytes preserved verbatim from old README.md:11-30.
  </done>
</task>

<task type="auto">
  <name>Task 3: cargo publish --dry-run gate proves D-82 tarball hygiene</name>
  <read_first>
    @/home/chasel/REPO/AIHPBar/Cargo.toml (after Tasks 1+2)
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-RESEARCH.md (Example 9 dry-run gate, Pitfall 5 categories slug)
  </read_first>
  <action>
    Execute `cargo publish --dry-run` from the repo root and inspect the output. This is the SC-4 metadata acceptance gate for Plan 01 — it proves that:

    (a) The crate metadata (`name = "ai-hp-bar"`, version `0.1.0`, all five locked fields description/repository/readme/keywords/categories) is publishable BEFORE we commit to creating GitHub repos in Plan 03.

    (b) The D-82 `exclude` rules actually fire — none of `.planning/`, `.github/`, `tests/data/`, `.claude/`, `.omg/`, or `CLAUDE.md` appear in the "Packaging X files" listing. The `cargo publish --dry-run` output enumerates every file going into the `.crate` tarball; grep that listing for each exclude entry — it MUST yield zero hits.

    (c) The Phase 1-3 `tests/*.rs` files (integration tests) ARE included in the tarball — the D-82 rationale calls them "保留 tests/*.rs" — so a downstream `cargo install --git` user can still run them. The dry-run output should show `tests/<name>.rs` files in the packaging.

    (d) No `cargo publish --dry-run` warning about `categories = ["command-line-utilities"]` (Pitfall 5 — slug verified valid; if cargo warns, executor must investigate before tagging).

    Do NOT actually publish (NEVER pass `cargo publish` without `--dry-run` in this plan — that's Plan 03 Wave 3 Step 6). The dry-run shells out to crates.io for a name-availability probe (HTTP request); RESEARCH already verified `ai-hp-bar` is unclaimed (2026-05-26), so this should pass.

    If dry-run fails with "name already taken" by the time of execution, STOP and surface to user — that's a hard block on D-75 and needs a CONTEXT amendment (fallback name was tentatively `aihpbar`).

    Commit any incidental fixes (e.g., if dry-run reveals a missing `description` byte that was lost during edit) and re-run dry-run until clean.
  </action>
  <verify>
    <automated>
cd /home/chasel/REPO/AIHPBar && \
  DRY=$(cargo publish --dry-run 2>&1) && \
  echo "$DRY" | grep -q 'Packaging .* ai-hp-bar v0.1.0' && \
  echo "$DRY" | grep -q 'Uploading ai-hp-bar v0.1.0' && \
  ! echo "$DRY" | grep -E '(^|/)(\.planning|\.github|\.claude|\.omg)/' && \
  ! echo "$DRY" | grep -E '(^|/)tests/data/' && \
  ! echo "$DRY" | grep -E '(^|/)CLAUDE\.md' && \
  echo "$DRY" | grep -qE 'tests/[a-zA-Z_]+\.rs' && \
  ! echo "$DRY" | grep -qiE 'warning.*(categories|keywords)' && \
  echo "DRY-RUN CLEAN"
    </automated>
  </verify>
  <acceptance_criteria>
    - **behavior-assert (cargo publish --dry-run):** Exits 0; stdout contains literal "Packaging" and "Uploading" lines for `ai-hp-bar v0.1.0`.
    - **D-82 exclude proof:** ZERO occurrences of any excluded path in the dry-run file listing — `.planning/`, `.github/`, `.claude/`, `.omg/`, `tests/data/`, `CLAUDE.md`.
    - **D-82 retention proof:** At least one `tests/*.rs` integration test file IS included (executor must list its name in the SUMMARY for the record).
    - **DIST-04 categories slug:** ZERO warnings about `categories` or `keywords` strings — Pitfall 5 binding.
    - **DIST-04 crate-name availability:** No "already exists" / 409 / "crate name unavailable" error from crates.io probe.
  </acceptance_criteria>
  <done>
    `cargo publish --dry-run` clean; SUMMARY.md records exact file count + total KiB from the Packaging line so Plan 02 can sanity-check that `cargo dist init` doesn't bloat the tarball.
  </done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| local-edits → crate tarball | Cargo.toml `exclude` controls what ships to crates.io; misconfigured exclude leaks `.planning/` governance prose, secrets-shaped fixtures, or CLAUDE.md prompt text to a public registry. |
| README on disk → crates.io renderer | crates.io fetches README from the tarball but evaluates image URLs in a separate sandbox; relative URLs break, absolute raw.githubusercontent.com URLs work (Finding 4). Bad URLs do not break security, only UX. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-04-01 | Information disclosure | Cargo.toml `[package].exclude` | mitigate | Task 3 `cargo publish --dry-run` greps the file listing for `.planning/`, `.github/`, `.claude/`, `.omg/`, `tests/data/`, `CLAUDE.md` — zero hits is the acceptance gate; misconfiguration becomes a build failure, not a silent leak. |
| T-04-02 | Tampering | Cargo.toml `[profile.release]` | mitigate | D-81 LOCKED keys are asserted by literal grep gates in Task 1 acceptance (`grep -q '^lto = true$'` etc.); accidental addition of `panic = "abort"` would silently break ADP-01 per-adapter unwind isolation (Pitfall 3) — guarded by `! grep -Eq '^(panic\s*=|codegen-units\s*=)'`. |
| T-04-03 | Spoofing | Crate name on crates.io (`ai-hp-bar`) | accept | Squat risk on `ahb` is the reason for D-75 rename to `ai-hp-bar`; full-name crates have low squat probability. Dry-run probes name availability before any irreversible step. Plan 03 publishes; this plan only verifies feasibility. |
| T-04-04 | Information disclosure | README.md D-65 Gemini block | accept | Block is intentionally public on GitHub README — it's the user-facing ToS warning. Phase 4 must NOT rewrite it (a single byte drift could weaken the ban-risk warning); Task 2 asserts three byte-anchors (`The Gemini adapter is deferred to v2.`, `gemini-cli 0.41.2`, `Web-scraping \`gemini.google.com/usage\``) survive the rewrite. |
| T-04-05 | Tampering | screenshot.png (binary asset) | accept | Static PNG with no executable content; served from `.github/assets/` via GitHub raw URL. If contents drift, only README aesthetic suffers, not security. v1 trust model: image is trusted because the source repo is single-maintainer. |
</threat_model>

<verification>
After all three tasks complete, run end-to-end gates from repo root:

1. **Build + test:** `cargo build --release && cargo test --all-targets` — both green; `target/release/ahb` exists and `target/release/ahb --help` exits 0.
2. **Tarball hygiene:** `cargo publish --dry-run` clean (Task 3 verification rerun).
3. **README rendering smoke test:** open README.md in a Markdown previewer or `gh markdown-toc` style tool — 11 sections render; D-65 Gemini block byte-identical to old README.md:11-30 (manual eyeball acceptable since the three byte-anchor grep gates already lock it).
4. **No mass file deletions:** `git diff --stat` shows touched files = {Cargo.toml, Cargo.lock, README.md, .github/assets/screenshot.png} only. Anything else means a task overreached.
</verification>

<success_criteria>
- Cargo.toml renamed to `ai-hp-bar` v0.1.0 with `[[bin]] name = "ahb"`, D-82 `exclude` list, and D-81 `[profile.release]` settings — verified by Task 1 grep gate.
- README.md fully rewritten into 11-section D-83 structure with verbatim D-65 Gemini block — verified by Task 2 grep gate.
- `.github/assets/screenshot.png` committed.
- `cargo publish --dry-run` succeeds with zero excluded-path leaks — verified by Task 3 grep gate.
- All Phase 1-3 integration tests still build (`cargo test --no-run` green) — proves `[[bin]] name = "ahb"` preserved the test contract (Finding 2 + A8).
- DIST-03 (Gatekeeper docs in README) and DIST-04 (crate metadata complete) requirements both satisfied — DIST-01 release profile partially satisfied (full ldd proof comes in Plan 02 + Plan 03 cross-OS CI).
</success_criteria>

<output>
Create `.planning/phases/04-distribution-release-polish/04-01-SUMMARY.md` recording:
- Final `cargo publish --dry-run` file count + tarball size (KiB).
- The list of `tests/*.rs` filenames that ARE in the tarball (D-82 retention proof).
- Whether `screenshot.png` is a real capture or a placeholder (and flag for replacement before v0.1.0 tag if placeholder).
- Any CONTEXT amendments needed (e.g., if `ai-hp-bar` turned out to be claimed by publish time — extremely unlikely per RESEARCH but flag-worthy).
</output>
</content>
</invoke>