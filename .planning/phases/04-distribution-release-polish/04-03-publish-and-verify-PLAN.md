---
phase: 04-distribution-release-polish
plan: 03
type: execute
wave: 3
depends_on: [04-01-local-prep, 04-02-cargo-dist-init]
files_modified: []
autonomous: false
requirements: [DIST-02]
user_setup:
  - service: github
    why: "Plan 03 creates two new public GitHub repos under the True347 account and pushes the v0.1.0 tag; the developer must be authenticated to `gh` CLI as True347 (or an identity with create-repo permission on that owner)."
    env_vars:
      - name: "GH_TOKEN (transparent to user — `gh auth status` must show authenticated)"
        source: "`gh auth login` — must be already done before plan starts; plan does not handle interactive login"
    dashboard_config:
      - task: "Create a fine-grained Personal Access Token for HOMEBREW_TAP_TOKEN"
        location: "https://github.com/settings/personal-access-tokens (Settings → Developer settings → Personal access tokens → Fine-grained tokens → Generate new token)"
        scope: "Repository access: Only select repositories → True347/homebrew-tap. Permissions: Contents = Read and write. Other permissions: keep defaults. Expiration: ≤90 days (renew before expiry)."
  - service: crates.io
    why: "Plan 03 final step runs `cargo publish` to push the `ai-hp-bar` crate to crates.io."
    env_vars:
      - name: "CARGO_REGISTRY_TOKEN (or `cargo login` already done)"
        source: "https://crates.io/me — Account Settings → API Tokens → New Token. Scope: publish-new + publish-update."

must_haves:
  truths:
    - "Public GitHub repository `True347/ahb` exists, has the entire current AHB source pushed to its default branch, and is the `origin` remote of the local clone (D-77 binding)"
    - "Public GitHub repository `True347/homebrew-tap` exists with an initial README commit on the default branch (Pitfall 9 — `--add-readme` flag is mandatory)"
    - "GitHub Secret `HOMEBREW_TAP_TOKEN` is set on `True347/ahb` (NOT on the tap repo — cargo-dist Homebrew installer docs binding per Finding 6 + Pitfall 6), sourced from a fine-grained PAT scoped to `True347/homebrew-tap` Contents:Read-and-Write only"
    - "Git tag `v0.1.0` exists and was pushed to `origin` via `git push origin v0.1.0` (Pitfall 7 — separate from branch push)"
    - "`.github/workflows/release.yml` ran to green on the tag push: GH release `v0.1.0` was published with 5 target tarballs + shell installer + PowerShell installer + checksums (D-79 deliverable)"
    - "`True347/homebrew-tap` now contains `Formula/ahb.rb` pushed by the cargo-dist `publish-homebrew` job (auto-push, no manual intervention)"
    - "`cargo publish` succeeded AFTER release.yml went green, publishing `ai-hp-bar v0.1.0` to crates.io (Pitfall 6 — order is mandatory: tag → release pipeline → cargo publish)"
    - "All four install channels in D-79 verified to work from a clean machine / container by the user: `brew install True347/tap/ahb`, `cargo binstall ai-hp-bar`, `cargo install ai-hp-bar`, `curl ... ahb-installer.sh | sh` (DIST-02 SC binding)"
  artifacts:
    - path: "GitHub repo True347/ahb (public)"
      provides: "Origin for source + tag triggering release pipeline"
      contains: "main/master branch with all current commits + tag v0.1.0"
    - path: "GitHub repo True347/homebrew-tap (public)"
      provides: "Homebrew tap that brew CLI resolves for `True347/tap/ahb`"
      contains: "Formula/ahb.rb auto-pushed by release.yml publish-homebrew job"
    - path: "GitHub release True347/ahb @ v0.1.0"
      provides: "5 target tarballs + shell + powershell installer + sha256 checksums"
      contains: "ai-hp-bar-0.1.0-x86_64-unknown-linux-gnu.tar.xz (and 4 others) + ahb-installer.sh + ahb-installer.ps1"
    - path: "crates.io entry ai-hp-bar v0.1.0"
      provides: "`cargo install ai-hp-bar` source path + `cargo binstall ai-hp-bar` discovery path"
      contains: "Crate metadata from D-75/D-76/D-82 manifest"
  key_links:
    - from: "git tag v0.1.0 on True347/ahb"
      to: ".github/workflows/release.yml"
      via: "GitHub Actions push:tags:['v*'] trigger fires; cargo-dist pipeline cross-builds 5 targets + publishes GH release + pushes brew formula"
      pattern: "v0\\.1\\.0"
    - from: "release.yml publish-homebrew job"
      to: "True347/homebrew-tap repo (Formula/ahb.rb)"
      via: "HOMEBREW_TAP_TOKEN fine-grained PAT — auth to push across repo boundary"
      pattern: "HOMEBREW_TAP_TOKEN"
    - from: "crates.io ai-hp-bar Cargo.toml `repository` field"
      to: "github.com/True347/ahb releases"
      via: "cargo-binstall walks `repository` → looks for cargo-dist tarballs at /releases/download/v{version}/ — Finding 1 binstall pattern #3 matches `ai-hp-bar-0.1.0-{target}.tar.xz`"
      pattern: "https://github.com/True347/ahb"
    - from: "brew formula at True347/homebrew-tap/Formula/ahb.rb"
      to: "GitHub release artifacts"
      via: "Formula Ruby DSL contains url-by-arch templates pointing at GH release tarballs; brew install resolves arch + downloads + symlinks binary to /opt/homebrew/bin/ahb (or /home/linuxbrew/.linuxbrew/bin/ahb)"
      pattern: "url \"https://github.com/True347/ahb"
---

<objective>
The Wave 3 vertical slice that DELIVERS the user-observable artifact: `brew install True347/tap/ahb` actually works on a clean Mac. Plus the three sibling channels — `cargo binstall ai-hp-bar`, `cargo install ai-hp-bar`, and `curl ... ahb-installer.sh | sh`.

Purpose: every action in this plan is IRREVERSIBLE (gh repo create cannot be cleanly undone; git tag push to public repo creates a permanent ref; HOMEBREW_TAP_TOKEN PAT cannot be retroactively scoped; `cargo publish` to crates.io is final — yanking is possible but doesn't free the name). Pattern D Wave 3 ordering is HARD-LOCKED — every command's prerequisite is the previous command's successful side-effect on a public surface. The plan is structured to fail loudly and early on every gate so we never get stuck halfway through publishing.
Output: 4 working install channels + crate live on crates.io + 2 public GitHub repos with green release pipeline + DIST-02 fully satisfied.
</objective>

<execution_context>
@/home/chasel/REPO/AIHPBar/.claude/get-shit-done/workflows/execute-plan.md
@/home/chasel/REPO/AIHPBar/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@/home/chasel/REPO/AIHPBar/.planning/PROJECT.md
@/home/chasel/REPO/AIHPBar/.planning/ROADMAP.md
@/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-CONTEXT.md
@/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-RESEARCH.md
@/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-PATTERNS.md
@/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-01-SUMMARY.md
@/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-02-SUMMARY.md
@/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-02-DRY-RUN.md
@/home/chasel/REPO/AIHPBar/Cargo.toml
@/home/chasel/REPO/AIHPBar/README.md
@/home/chasel/REPO/AIHPBar/.github/workflows/release.yml
</context>

<tasks>

<task type="auto">
  <name>Task 1: gh repo create source + tap, push commits (Pattern D Wave 3 Steps 1-2)</name>
  <read_first>
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-RESEARCH.md (Example 8 Wave 3 command sequence, Pitfall 7 explicit-tag-push, Pitfall 9 `--add-readme` mandatory)
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-PATTERNS.md (Pattern D ordering)
  </read_first>
  <action>
    Pre-flight: verify `gh auth status` shows authenticated against an identity that can create repos under the True347 owner. If not, STOP and surface — the user must run `gh auth login` first (this is a pre-condition documented in `user_setup`, not a plan task).

    Pre-flight: verify `git status` shows a clean working tree. All Plan 01 + Plan 02 commits must be IN already. If not, STOP — committed state is what gh repo create + git push will publish to the world.

    (a) **Create the source repo** (D-77 Step 1):
        - Run `gh repo create True347/ahb --public --source=. --remote=origin --push --description "AHB — multi-CLI subscription session usage at a glance"`.
        - This single command does four things: creates the public repo on github.com, sets the local clone's `origin` to point at it, pushes the current branch, and registers the description for repo discoverability.
        - DO NOT use `--private` — D-77 + Pitfall 6 + cargo-dist installer URL all require public artifact hosting.
        - After this command, `git remote -v` MUST show `origin https://github.com/True347/ahb.git`.

    (b) **Create the homebrew tap repo with initial commit** (D-77 Step 2 + Pitfall 9):
        - Run `gh repo create True347/homebrew-tap --public --add-readme --description "Homebrew tap for True347 tools"`.
        - The `--add-readme` flag is MANDATORY (Pitfall 9). Without it, the tap repo's default branch has no commit, and cargo-dist's first `publish-homebrew` job will fail trying to push Formula/ahb.rb onto a phantom HEAD.
        - Verify with `gh api repos/True347/homebrew-tap | jq -r '.default_branch'` — must return a non-null branch name (typically `main`).

    (c) Do NOT push tags yet. Do NOT set HOMEBREW_TAP_TOKEN yet — that's Task 2 (which has the human-verify checkpoint for PAT creation). Do NOT `cargo publish` yet — that's Task 4 after the release pipeline has gone green (Pitfall 6 order).

    (d) Capture proof for SUMMARY: `gh repo view True347/ahb --json url,visibility,defaultBranchRef` and `gh repo view True347/homebrew-tap --json url,visibility,defaultBranchRef` — both must show `"visibility": "PUBLIC"` and a non-null defaultBranchRef.
  </action>
  <verify>
    <automated>
gh auth status 2>&1 | grep -q 'Logged in' && \
  gh repo view True347/ahb --json url,visibility,defaultBranchRef 2>/dev/null | \
    grep -q '"visibility":"PUBLIC"' && \
  gh repo view True347/ahb --json defaultBranchRef 2>/dev/null | \
    grep -qE '"name":"(main|master)"' && \
  gh repo view True347/homebrew-tap --json url,visibility,defaultBranchRef 2>/dev/null | \
    grep -q '"visibility":"PUBLIC"' && \
  gh repo view True347/homebrew-tap --json defaultBranchRef 2>/dev/null | \
    grep -qE '"name":"(main|master)"' && \
  cd /home/chasel/REPO/AIHPBar && \
  git remote get-url origin | grep -q 'github.com[:/]True347/ahb' && \
  echo "OK: both repos public + tap has initial README + origin set"
    </automated>
  </verify>
  <acceptance_criteria>
    - **gh-state-assert (source repo):** `True347/ahb` exists, is PUBLIC, has a default branch with at least one commit (the push from `--source=. --push`).
    - **gh-state-assert (tap repo):** `True347/homebrew-tap` exists, is PUBLIC, has a default branch with the README initial commit (Pitfall 9 binding — `defaultBranchRef.name` is non-null, not the empty string).
    - **local-state-assert:** `git remote -v` shows `origin` pointing at `github.com:True347/ahb` or `github.com/True347/ahb.git`.
    - **non-action gate:** No tag has been pushed yet (`git ls-remote --tags origin` is empty); no secret set yet (Task 2); no `cargo publish` yet (Task 4). This explicit "we have NOT done X yet" is part of the Pattern D ordering proof.
  </acceptance_criteria>
  <done>
    Both repos public; origin set; pre-flight for HOMEBREW_TAP_TOKEN provisioning (Task 2) is clean.
  </done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <name>Task 2: HUMAN-VERIFY checkpoint — create fine-grained PAT for HOMEBREW_TAP_TOKEN and set it on True347/ahb</name>
  <what-built>
    Tasks 1 (this plan) created two empty public repos. The next task (Task 3) will push the `v0.1.0` tag, which triggers `release.yml`, which calls a `publish-homebrew` job that needs `HOMEBREW_TAP_TOKEN` to push the formula across the True347/ahb → True347/homebrew-tap repo boundary. The default `GITHUB_TOKEN` provided by GH Actions CANNOT do cross-repo writes (Finding 6 + Pitfall 6). The user must therefore create a fine-grained Personal Access Token in the GitHub web UI (the `gh` CLI cannot atomically create-and-return a fine-grained PAT — this is a documented GitHub API limitation as of 2026-05).
  </what-built>
  <how-to-verify>
    USER ACTIONS (must be done in browser + terminal):

    1. Open https://github.com/settings/personal-access-tokens in a browser.
    2. Click "Generate new token" → "Generate new token (Fine-grained, repo-scoped)".
    3. Fill in:
       - **Token name**: `ahb-homebrew-tap-publish` (or similar — name is for user reference only).
       - **Expiration**: ≤90 days. Pick a date that you'll remember to renew before — if the PAT expires, the next release will fail at `publish-homebrew` step. (Renewal procedure: regenerate, re-run `gh secret set ...`. Document in SUMMARY for posterity.)
       - **Repository access**: Select "Only select repositories" → choose ONLY `True347/homebrew-tap` (NOT `True347/ahb` — the token is for writing INTO the tap repo, not the source repo).
       - **Repository permissions**: Set `Contents = Read and write`. Leave all others at "No access".
       - **Account permissions**: Leave all at "No access".
    4. Click "Generate token". Copy the token (starts with `github_pat_...`).
    5. In your terminal at `/home/chasel/REPO/AIHPBar`, run:
       ```sh
       gh secret set HOMEBREW_TAP_TOKEN --repo True347/ahb
       # When prompted, paste the PAT and press Enter. (gh secret set reads from stdin; do NOT pass via --body on the command line — it ends up in shell history.)
       ```
    6. Verify the secret was set:
       ```sh
       gh secret list --repo True347/ahb
       # Output should include a row "HOMEBREW_TAP_TOKEN" with a recent "Updated" timestamp.
       ```

    Confirm in this checkpoint reply:
    (a) PAT is created with scope = ONLY `True347/homebrew-tap`, Contents = Read+Write, no other permissions.
    (b) PAT is set as `HOMEBREW_TAP_TOKEN` secret on `True347/ahb` (NOT on the tap repo).
    (c) `gh secret list --repo True347/ahb` shows the secret row.
    (d) The PAT plaintext has NOT been pasted into shell history (`gh secret set` read it from stdin) and has NOT been committed anywhere.
  </how-to-verify>
  <resume-signal>
    Type `approved` once all four confirmations above are true. If anything looks off (PAT scope too broad, secret on wrong repo, etc.), type the issue and the plan halts so it can be fixed before any irreversible tag push.
  </resume-signal>
</task>

<task type="auto">
  <name>Task 3: git tag v0.1.0 + push + wait for release.yml green (Pattern D Wave 3 Steps 4-5)</name>
  <read_first>
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-RESEARCH.md (Example 8 Steps 4-5, Pitfall 7 explicit-tag-push, Open Question 3 tag annotation)
    @/home/chasel/REPO/AIHPBar/Cargo.toml (version line — must be exactly "0.1.0" per D-76)
  </read_first>
  <action>
    (a) Pre-flight: verify Task 2 checkpoint was approved (Wave 3 ordering — Pitfall 6 + 7 binding). If executor cannot see explicit user "approved" reply, STOP.

    (b) Pre-flight: re-run `cargo dist plan` one more time (cheap, ~1s). Output MUST match Plan 02 Task 2 baseline. If it has drifted (e.g., someone edited Cargo.toml between plans), STOP and re-verify Plan 02.

    (c) Pre-flight: confirm version pin. `grep -q '^version = "0.1.0"$' Cargo.toml` MUST succeed (D-76 binding). If the version was bumped elsewhere, STOP.

    (d) Create the annotated tag with release notes inline (Open Question 3 recommendation — `git tag -a` message becomes the GH release body when cargo-dist runs):
        ```
        git tag -a v0.1.0 -m "AHB 0.1.0 — first public release.

Highlights:
- Multi-provider HP bar: Claude Code + Codex CLI + (Gemini deferred to v2, see README).
- Compact / detailed / JSON output modes (schema_version: 1).
- Static TUI with 15s auto-refresh + panic-safe terminal restore.
- Stale-on-error indicator for transient network failures.
- OS keyring-backed credentials via keyring-core 1.0.
- Pre-built static binaries for 5 target triples — no OpenSSL / native-tls runtime deps.

Install:
  brew install True347/tap/ahb
  cargo binstall ai-hp-bar
  cargo install ai-hp-bar"
        ```
        Adjust the body slightly if Plan 01/02 SUMMARYs surfaced anything material; but keep the user-visible value statement.

    (e) Push the tag EXPLICITLY (Pitfall 7 — do NOT rely on `git push` default which doesn't push tags):
        `git push origin v0.1.0`

    (f) Wait for the release pipeline. Run `gh run watch --repo True347/ahb` from another terminal OR poll `gh run list --repo True347/ahb --workflow=release.yml --limit 1 --json conclusion,status,databaseId,headBranch,event` until status = `completed` and conclusion = `success`. Typical wall-time: 15-25 minutes for the 5-target cross-build.

    (g) On pipeline green, verify side-effects on the public surface:
        - `gh release view v0.1.0 --repo True347/ahb --json tagName,assets`: must list at least 5 tarball assets (one per target triple) + `ahb-installer.sh` + `ahb-installer.ps1` + sha256 checksum files.
        - `gh api repos/True347/homebrew-tap/contents/Formula/ahb.rb | jq -r '.path'` must return `Formula/ahb.rb` (the publish-homebrew job ran and pushed the formula).

    (h) If the pipeline fails:
        - Most likely cause is `HOMEBREW_TAP_TOKEN` misconfiguration (Pitfall 6 — wrong scope, wrong repo, expired). Re-do Task 2 with corrected PAT scope, then re-trigger via `gh workflow run release.yml --ref v0.1.0 --repo True347/ahb`.
        - Second most likely cause: a target build fails. Inspect logs via `gh run view --repo True347/ahb --log <run-id>`. Common: macOS keychain framework link issue (Phase 1 cfg-gated dep should handle, but verify); Windows MSVC missing toolchain (cargo-dist runner image should have it).
        - Do NOT delete the tag and retry from scratch unless absolutely necessary — tag re-use across releases is messy on GitHub. Prefer fixing forward and re-triggering the workflow.

    (i) Once pipeline is green AND tap has Formula/ahb.rb, proceed to Task 4. The plan does NOT `cargo publish` yet — Pitfall 6 mandates pipeline-green-before-crates-publish ordering.
  </action>
  <verify>
    <automated>
cd /home/chasel/REPO/AIHPBar && \
  git tag -l v0.1.0 | grep -q '^v0\.1\.0$' && \
  git ls-remote --tags origin v0.1.0 | grep -q 'v0\.1\.0' && \
  RUN_STATE=$(gh run list --repo True347/ahb --workflow=release.yml --limit 1 --json conclusion,status 2>/dev/null) && \
  echo "$RUN_STATE" | grep -q '"status":"completed"' && \
  echo "$RUN_STATE" | grep -q '"conclusion":"success"' && \
  ASSETS=$(gh release view v0.1.0 --repo True347/ahb --json assets 2>/dev/null) && \
  echo "$ASSETS" | grep -q 'x86_64-unknown-linux-gnu' && \
  echo "$ASSETS" | grep -q 'x86_64-apple-darwin' && \
  echo "$ASSETS" | grep -q 'aarch64-apple-darwin' && \
  echo "$ASSETS" | grep -q 'x86_64-pc-windows-msvc' && \
  echo "$ASSETS" | grep -q 'aarch64-unknown-linux-gnu' && \
  echo "$ASSETS" | grep -q 'ahb-installer.sh' && \
  echo "$ASSETS" | grep -q 'ahb-installer.ps1' && \
  gh api repos/True347/homebrew-tap/contents/Formula/ahb.rb 2>/dev/null | grep -q '"path":"Formula/ahb.rb"' && \
  echo "OK: tag pushed, pipeline green, 5 targets + 2 installers in release, formula in tap"
    </automated>
  </verify>
  <acceptance_criteria>
    - **git-state-assert (tag pushed):** Both `git tag -l v0.1.0` (local) and `git ls-remote --tags origin v0.1.0` (remote) confirm tag exists. Pitfall 7 binding — explicit remote tag presence.
    - **release.yml pipeline:** Latest run on workflow `release.yml` shows status `completed` AND conclusion `success`.
    - **GH release artifacts:** All 5 D-79 target triples appear as asset names AND both shell + PowerShell installers appear. D-79 SC-1 binding.
    - **Homebrew tap formula:** `Formula/ahb.rb` exists in `True347/homebrew-tap` repo — the publish-homebrew job ran successfully (Finding 6 confirms HOMEBREW_TAP_TOKEN flow worked).
    - **Pattern D ordering preserved:** `cargo publish` has NOT been called yet — `cargo info ai-hp-bar` (or crates.io API probe) returns 404 / not-found. Task 4 is the only place we may publish.
  </acceptance_criteria>
  <done>
    Tag v0.1.0 live on True347/ahb; release.yml green; 5 tarballs + 2 installers on the GH release; Formula/ahb.rb pushed into True347/homebrew-tap.
  </done>
</task>

<task type="auto">
  <name>Task 4: cargo publish ai-hp-bar v0.1.0 to crates.io (Pattern D Wave 3 Step 6)</name>
  <read_first>
    @/home/chasel/REPO/AIHPBar/.planning/phases/04-distribution-release-polish/04-RESEARCH.md (Pitfall 6 — order is non-negotiable, Open Question 2 — manual publish is the v1 path)
    @/home/chasel/REPO/AIHPBar/Cargo.toml (post-Plan-02 final state)
  </read_first>
  <action>
    (a) Pre-flight (Pitfall 6 binding):
        - Verify Task 3 acceptance gate passed (release.yml green, tap has formula). If `gh run list --workflow=release.yml --limit 1` does NOT show completed+success, STOP — publishing crate before GH release is the explicit Pitfall 6 failure mode (cargo-binstall would discover crates.io entry but find no binaries at the repository URL).
        - Verify `cargo login` already done OR `CARGO_REGISTRY_TOKEN` env var is set. Run `cargo publish --dry-run` once more to confirm credentials work and metadata is still clean (should match Plan 01 Task 3 baseline).

    (b) **Publish the crate** (irreversible):
        - Run `cargo publish` from repo root (no `--dry-run` this time).
        - cargo will: read manifest → bundle into a `.crate` tarball respecting `[package].exclude` (D-82) → upload to crates.io → register `ai-hp-bar v0.1.0` under the current user's namespace.
        - Expected output: "Uploading ai-hp-bar v0.1.0 (..." then "Published" message (cargo 1.94 wording may vary; check exit code).

    (c) Wait ~30-60 seconds for crates.io's indexer to populate the new version (the API may return 404 briefly after publish). Then verify:
        - `curl -s https://crates.io/api/v1/crates/ai-hp-bar | jq -r '.crate.max_version'` returns `0.1.0`.
        - `curl -s https://crates.io/api/v1/crates/ai-hp-bar | jq -r '.crate.repository'` returns `https://github.com/True347/ahb` (Pitfall 6 cross-link — this is what cargo-binstall walks).

    (d) If `cargo publish` fails:
        - Most likely cause: the crate name `ai-hp-bar` was claimed between Plan 01 Task 3 dry-run and now (extremely unlikely per RESEARCH but possible). Mitigation: STOP, surface to user, fallback to `aihpbar` per CONTEXT D-75 alternate. This would require a CONTEXT amendment + Cargo.toml re-edit + dry-run + republish — meaningfully painful, hence the rapid-cadence sequencing of Plans 01 → 02 → 03.
        - Second cause: credentials missing — re-run `cargo login`.
        - Third cause: metadata warning escalated to error (e.g., a categories slug typo slipped in between Plan 01 and now). Run `cargo publish --dry-run` to diagnose.
  </action>
  <verify>
    <automated>
cd /home/chasel/REPO/AIHPBar && \
  cargo publish 2>&1 | tee /tmp/cargo-publish.log | tail -3 && \
  grep -qE 'Uploading|Uploaded' /tmp/cargo-publish.log && \
  sleep 30 && \
  curl -fsSL https://crates.io/api/v1/crates/ai-hp-bar 2>/dev/null | \
    grep -qE '"max_version":"0\.1\.0"' && \
  curl -fsSL https://crates.io/api/v1/crates/ai-hp-bar 2>/dev/null | \
    grep -q '"repository":"https://github.com/True347/ahb"' && \
  echo "OK: ai-hp-bar v0.1.0 live on crates.io with correct repository link"
    </automated>
  </verify>
  <acceptance_criteria>
    - **publish-state-assert:** `cargo publish` exits 0; stdout shows "Uploading ai-hp-bar v0.1.0" and "Uploaded" (or "Published") signal.
    - **crates.io API:** `max_version` is `0.1.0` AND `repository` field is `https://github.com/True347/ahb` (Pitfall 6 cross-link — cargo-binstall uses this).
    - **Pattern D ordering proof:** This is the FINAL Wave 3 publishing step; no further public side-effects in this plan (only verification of the 4 install channels follows in Task 5).
  </acceptance_criteria>
  <done>
    `ai-hp-bar v0.1.0` live on crates.io with `repository = github.com/True347/ahb` indexed; `cargo binstall ai-hp-bar` will now be discoverable.
  </done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <name>Task 5: HUMAN-VERIFY checkpoint — verify all 4 install channels from clean environments (DIST-02 SC binding)</name>
  <what-built>
    Tasks 1-4 published: True347/ahb (public source repo), True347/homebrew-tap with Formula/ahb.rb, GH release v0.1.0 with 5 target tarballs + 2 installers, and ai-hp-bar v0.1.0 on crates.io.

    The DIST-02 success criterion is "all four install paths work from a clean machine" — this requires actual execution on machines that have NOT been used during development. The executor (Claude) cannot reliably perform this from inside the dev environment because the dev box has cached toolchains, dev-time secrets, and the in-progress `target/` directory. The user must run the 4 channels on clean environments (containers / VMs / fresh accounts) and report back.
  </what-built>
  <how-to-verify>
    USER ACTIONS (run on a CLEAN environment for each channel — e.g., fresh Docker container, second machine, or freshly reset VM. Document which environment was used for each.):

    **Channel 1 — Homebrew (macOS recommended, Linux brew also valid):**
    ```sh
    brew install True347/tap/ahb
    ahb --help          # expect Phase 1-3 --help output
    ahb                 # expect compact HP bar with the user's configured providers
    ```
    Expected: brew downloads from True347/homebrew-tap/Formula/ahb.rb, fetches the platform-matched tarball from True347/ahb release v0.1.0, installs binary at `/opt/homebrew/bin/ahb` (Apple Silicon) or `/usr/local/bin/ahb` (Intel) or `/home/linuxbrew/.linuxbrew/bin/ahb` (Linux brew). The Gatekeeper xattr is auto-stripped by brew — this is why D-79 puts brew first (Pitfall 1 mitigation).

    **Channel 2 — cargo binstall:**
    ```sh
    cargo install cargo-binstall            # if not present
    cargo binstall ai-hp-bar                # accept the prompt (or pass --no-confirm)
    ahb --help
    ```
    Expected: cargo-binstall reads crates.io metadata → walks `repository` → finds the GH release v0.1.0 → downloads the right tarball (Finding 1 pattern #3 `ai-hp-bar-0.1.0-{target}.tar.xz`) → extracts → installs `ahb` to `~/.cargo/bin/ahb`. NO source compile; should complete in seconds.

    **Channel 3 — cargo install (source build):**
    ```sh
    cargo install ai-hp-bar
    ahb --help
    ```
    Expected: cargo downloads `ai-hp-bar-0.1.0.crate` from crates.io → compiles from source → produces `~/.cargo/bin/ahb` (Finding 2 — bin name from `[[bin]]`). First-time build typically 2-5 minutes due to dependency tree compile. This is the slowest path; D-79 puts it third deliberately.

    **Channel 4 — Raw GitHub release artifact (manual download path, will exercise Gatekeeper on macOS):**
    ```sh
    curl -fsSL https://github.com/True347/ahb/releases/latest/download/ahb-installer.sh | sh
    ahb --help
    ```
    Expected on Linux: installer downloads tarball, extracts, places `ahb` on PATH (typically `~/.local/bin/ahb` or similar).
    Expected on macOS: installer downloads tarball, extracts, places `ahb` — first invocation will be BLOCKED by Gatekeeper ("cannot be verified"). User runs `xattr -d com.apple.quarantine $(which ahb)` per D-84 README. After xattr strip, `ahb --help` works. This is the deliberate Gatekeeper workaround validation; DIST-03 binding (README documents this exact recovery).

    Confirm in this checkpoint reply:
    (a) Channel 1 (brew) worked end-to-end on a clean macOS (or Linux brew) machine; no Gatekeeper hurdle.
    (b) Channel 2 (cargo binstall) worked on a clean machine with only Rust + cargo-binstall installed; download was pre-built (not source build).
    (c) Channel 3 (cargo install) worked end-to-end via source build; final binary is `ahb` not `ai-hp-bar` (Finding 2 confirmation).
    (d) Channel 4 (raw installer) worked; on macOS specifically, the README's xattr workaround successfully recovered after Gatekeeper block (DIST-03 binding).
    (e) crates.io search for any of "ai hp bar", "claude codex usage", or "claude session quota" finds the crate (DIST-04 SC-4 user-visible binding — keywords + description discoverability).
  </how-to-verify>
  <resume-signal>
    Type `approved` when all five confirmations above are true with environment details (which OS, which clean container/VM was used for each channel). If any channel fails, type the specific failure mode so the plan can produce a gap-closure recommendation (typically a follow-up Plan 04 with `--gaps` flag if anything systemic broke).
  </resume-signal>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| local dev → public GitHub | `gh repo create` + `git push` make repo state public + permanent. Any secret accidentally committed before Plan 01-02 would now be public; mitigated by Plan 01 Task 3 dry-run grep gates and `.gitignore`. |
| dev → fine-grained PAT scope | The HOMEBREW_TAP_TOKEN PAT is intentionally narrow (Contents:RW on `True347/homebrew-tap` only). A leaked token can only push to that one tap repo — blast radius limited to one Ruby formula file. |
| crates.io publish → world | `cargo publish` permanently registers `ai-hp-bar` in the public Rust ecosystem. Yanking is possible but the name is reserved. |
| user clean environment → install channels | Channels 1-3 traverse trusted publishers (Homebrew, crates.io, cargo-binstall). Channel 4 (raw curl-pipe-sh) is the classic supply-chain risk surface — `ahb-installer.sh` from True347/ahb release is auto-generated by cargo-dist with sha256 checksum verification baked in. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-04-10 | Spoofing | crates.io name `ai-hp-bar` | mitigate | Wave 3 ordering ensures publish is the LAST step (Pitfall 6) — minimizes the window where someone could squat. RESEARCH verified name unclaimed 2026-05-26; Plan 01 Task 3 dry-run probed availability immediately before publish. |
| T-04-11 | Tampering / Supply-chain | HOMEBREW_TAP_TOKEN PAT | mitigate | Fine-grained PAT scoped to ONLY `True347/homebrew-tap` Contents:RW (Task 2 checkpoint instructions are explicit on this). Token is set via stdin `gh secret set` (no shell history leakage). Token expiration ≤90 days documented in user_setup. Renewal procedure captured in SUMMARY. |
| T-04-12 | Information disclosure | Public GH repos created from local clone | mitigate | Plan 01 Task 3 `cargo publish --dry-run` proves D-82 exclude rules; Plan 02 only modifies Cargo.toml + .github/workflows/release.yml (no source code). Task 1 pre-flight asserts `git status` clean — anything inadvertently staged would be caught at the `git push` step. |
| T-04-13 | Tampering | Channel 4 raw installer (`curl ... | sh`) | accept | cargo-dist auto-generates the installer with embedded sha256 checksums per platform. We accept the classic `curl | sh` supply-chain risk as standard for OSS CLI distribution (Homebrew, rustup, etc. all do this). README D-79 ordering puts brew first to nudge users toward signature-verified channels. |
| T-04-14 | Elevation of privilege | macOS Gatekeeper bypass via `xattr -d com.apple.quarantine` | accept | Documented in D-84 / DIST-03. Trade-off was explicit (D-80 — Apple Developer ID $99/yr deferred to v2). User running this command on a malicious binary they got from a phishing site would defeat their own Gatekeeper; we accept this as a "user must verify the source" model and recommend brew (signature-verified) as the primary channel. |
| T-04-15 | Tampering / Supply-chain | release.yml runs on tag push | mitigate | release.yml is generated by cargo-dist 0.32.0 (pinned in Plan 02), runs only on tag push from authorized identities. Pipeline is in our own repo — we can audit/diff future cargo-dist updates. |
| T-04-16 | Information disclosure | `git tag -a v0.1.0 -m "..."` body becomes public GH release notes | accept | Release notes are intentionally user-facing marketing; Task 3 (d) template avoids exposing dev internals (no `.planning/` paths, no test commands, no internal phase numbers). |
</threat_model>

<verification>
After Plan 03 completes (all 5 tasks):

1. **GH repos:** Both `True347/ahb` and `True347/homebrew-tap` exist, public, with content. `gh repo view` confirms.
2. **Secret:** `gh secret list --repo True347/ahb` shows `HOMEBREW_TAP_TOKEN`.
3. **Tag + release:** `git ls-remote --tags origin v0.1.0` returns the tag; `gh release view v0.1.0 --repo True347/ahb` lists 5 tarball assets + 2 installer scripts.
4. **Tap formula:** `Formula/ahb.rb` exists in `True347/homebrew-tap` via `gh api`.
5. **crates.io:** `ai-hp-bar` v0.1.0 visible; `repository` field correctly points to True347/ahb.
6. **DIST-02 SC-2 channels:** All 4 install paths verified by user on clean environments (Task 5 checkpoint).
7. **DIST-04 SC-4 discoverability:** crates.io search for the locked keywords ("ai hp bar", "claude codex usage", or "claude session quota") finds the crate (Task 5 confirmation (e)).

If any item fails: gap-closure plan via `/gsd:plan-phase --gaps 4`.
</verification>

<success_criteria>
- **DIST-02 fully satisfied:** All 4 install channels work from clean environments — brew, cargo binstall, cargo install, raw GH release artifact.
- **Phase 4 user-observable artifact delivered:** `brew install True347/tap/ahb` on a clean Mac produces a working `ahb` binary in under 2 minutes with no Gatekeeper hurdle.
- **DIST-04 SC-4 discoverability:** crates.io search returns ai-hp-bar for at least one of the discoverable phrases.
- **DIST-01 cross-OS proof (deferred via release.yml):** All 5 target tarballs built green in CI without OpenSSL link errors — cargo-dist's `host` job aggregates failures from cross-builds, so a green pipeline implicitly proves DIST-01 across the matrix.
- **Pattern D ordering preserved end-to-end:** Each Wave 3 step's prerequisites were the prior step's verified side-effect; no out-of-order publishing occurred.
- **All locked decisions implemented:** D-75 (crate rename, bin pin), D-76 (v0.1.0), D-77 (gh repo create both), D-78 (tap repo name), D-79 (4 channels live), D-80 (no Scoop/AUR/Apple sign — confirmed by absence), D-83 (README structure), D-84 (Gatekeeper docs reachable from README) — all traceable via Plan 01/02 must_haves + this Plan's Task 1-5 acceptance gates.
</success_criteria>

<output>
Create `.planning/phases/04-distribution-release-polish/04-03-SUMMARY.md` recording:
- Final state of both GH repos (URL, default branch, visibility).
- The `HOMEBREW_TAP_TOKEN` PAT expiration date and renewal procedure (user must renew before this date — losing this PAT silently breaks future `release.yml` publish-homebrew jobs).
- The exact `cargo publish` output (Uploading line + Uploaded confirmation).
- crates.io API response showing `max_version: 0.1.0` and `repository` set.
- The clean-environment matrix from Task 5 checkpoint: which channel was verified on which OS / container / VM, and any quirks encountered (especially: which macOS version exhibited Gatekeeper, and confirmation that the xattr workaround in the README actually recovered).
- A flag for the upcoming roadmap RETROSPECTIVE: this is the first phase to verify on truly clean environments — document any gaps surfaced (e.g., a missing dependency in the Linux dbus path, a Windows SmartScreen behavior different from what D-84 describes) for v2 follow-ups.
</output>
</content>
</invoke>