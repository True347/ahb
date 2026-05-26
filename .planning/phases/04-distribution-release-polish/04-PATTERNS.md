# Phase 4: Distribution & Release Polish — Pattern Map

**Mapped:** 2026-05-26
**Files analyzed:** 7 (5 in-tree changes + 2 external repos)
**Analogs found:** 5 / 7（2 個無 code analog：screenshot.png 是 binary asset、外部 GH repos 是 out-of-tree state）

> Phase 4 是 release-engineering phase — 所有「analog」都是同一個 repo 既有的 `Cargo.toml` / `README.md` / `ci.yml`（self-reference 模式）。沒有 src code 變更，所以 pattern map 的內容是 **TOML key 排列順序、README section 結構、GitHub Actions matrix shape**，不是 Rust 程式碼片段。

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `Cargo.toml` (modify) | config (manifest) | declarative metadata | `Cargo.toml` (self, current state) | exact (self-amend) |
| `Cargo.lock` (regenerate) | build artifact | tool-generated | n/a — cargo 自動 regen，無 analog | n/a |
| `README.md` (full rewrite) | config (documentation) | rendered markdown | `README.md` (self, current state) | exact (self-rewrite + 保留 D-65 Gemini status verbatim) |
| `.github/workflows/release.yml` (new) | config (CI) | event-driven (tag push) | `.github/workflows/ci.yml` | role-match (both GH Actions, both 3-OS matrix) |
| `.github/assets/screenshot.png` (new) | asset (binary) | static file served by raw.githubusercontent.com | none (binary; document capture command instead) | n/a |
| `True347/ahb` (new external GH repo) | infrastructure (out-of-tree) | `gh repo create` side-effect | none — bootstrap via Wave 3 commands | n/a |
| `True347/homebrew-tap` (new external GH repo) | infrastructure (out-of-tree) | `gh repo create` + cargo-dist auto-push | none — bootstrap via Wave 3 commands | n/a |

---

## Pattern Assignments

### `Cargo.toml` (config, declarative metadata)

**Analog:** `Cargo.toml` (self, lines 1-12 既有 `[package]` + line 130-131 `[profile.release]` placeholder)

**既有 `[package]` block 字面（Cargo.toml:1-12）— D-75/76/82 需 amend 此區，但結構模式照抄：**
```toml
# Cargo.toml — Phase 0 + Phase 1
[package]
name = "ahb"
version = "0.0.1"
edition = "2024"
rust-version = "1.88"
license = "MIT OR Apache-2.0"
description = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance"
repository = "https://github.com/True347/ahb"
readme = "README.md"
keywords = ["claude", "codex", "gemini", "cli", "tui"]
categories = ["command-line-utilities"]
```

**模式重點（planner 抄此排列順序）：**
- 5 個必要 metadata 欄位順序 `description → repository → readme → keywords → categories` 已 Phase 0 鎖定 — Phase 4 在 `categories` 之後**插入** `exclude = [...]`、再在 `[package]` block 之後**新增** `[[bin]]` block，**不改既有 5 欄順序**
- Phase 4 預期改動點（per RESEARCH Example 1）：
  - `name = "ahb"` → `name = "ai-hp-bar"` (D-75)
  - `version = "0.0.1"` → `version = "0.1.0"` (D-76)
  - **insert** `exclude = [".planning/", ".github/", "tests/data/", ".claude/", ".omg/", "CLAUDE.md"]` (D-82)
  - **append after `[package]`**：`[[bin]] name = "ahb" path = "src/main.rs"` (D-75 + Finding 2)

**既有 `[profile.release]` placeholder（Cargo.toml:130-131）— D-81 需填內容，結構照抄：**
```toml
[profile.release]
# Phase 4 will tune more. Phase 0 default is fine.
```

**模式重點：** Phase 0 已留 `[profile.release]` 區塊但無設定；Phase 4 替換成三行 LOCKED 字面（D-81）：
```toml
[profile.release]
lto = true
strip = "symbols"
opt-level = 3
```

**Per-OS cfg-gated dependency 模式（Cargo.toml:101-108）— `[workspace.metadata.dist].targets` 必須與此 cfg matrix 對齊：**
```toml
[target.'cfg(target_os = "linux")'.dependencies]
dbus-secret-service-keyring-store = { version = "1", default-features = false, features = ["crypto-rust"] }

[target.'cfg(target_os = "macos")'.dependencies]
apple-native-keyring-store = "1"

[target.'cfg(target_os = "windows")'.dependencies]
windows-native-keyring-store = "1"
```

**模式重點：** Phase 1 已建好「Linux / macOS / Windows」三 OS cfg-gated keyring-store backend；Phase 4 的 `[workspace.metadata.dist].targets` 列出的 5 個 triple **必須**全部對應到此三 cfg 之一，不可漏 OS（否則 cross-build 會缺 keyring backend → link error）。Verification: `x86_64-{linux,apple-darwin,pc-windows-msvc}` + `aarch64-{linux,apple-darwin}` 五個 triple 對應 cfg = linux/macos/windows × {x86_64, aarch64}，完全覆蓋。

**新增 `[workspace.metadata.dist]` block 預期字面（RESEARCH Example 3 — `cargo dist init` 寫 + Finding 3 manual `formula = "ahb"` override）：**
```toml
[workspace.metadata.dist]
cargo-dist-version = "0.32.0"
ci = ["github"]
installers = ["shell", "powershell", "homebrew"]
tap = "True347/homebrew-tap"
formula = "ahb"            # Finding 3: 鎖死 brew install True347/tap/ahb (與 [[bin]] name 一致, 不用 crate name)
publish-jobs = ["homebrew"]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-unknown-linux-gnu",
]
pr-run-mode = "plan"

[profile.dist]              # cargo-dist init 自動寫，不要 override
inherits = "release"
lto = "thin"
```

**模式重點：**
- `[workspace.metadata.dist]` 與 `[profile.dist]` 兩個 block 由 `cargo dist init` 自動 append 到 Cargo.toml **檔尾**（per Pitfall 4 — Wave 1 先手動編輯 `[package]` / `[[bin]]` / `[profile.release]` / `exclude`，Wave 2 才跑 init，這樣 init 只 append 不擾亂既有排列）
- `formula = "ahb"` **手動加**（不在 `cargo dist init` 預設輸出內）— Finding 3 鎖死 brew formula 名為 `ahb`、與 `[[bin]] name` 一致
- `[profile.dist]` 是 cargo-dist 偵測「project initialized」的標記 — **不可 override `lto = "thin"`**（Pitfall 8）

---

### `README.md` (config, rendered markdown)

**Analog:** `README.md` (self, lines 1-30 — Phase 1-3 已建好的 30 行)

**既有 H1 + Quick start (README.md:1-9) — D-83 sections 2 + 6 必須保留結構：**
```markdown
# AHB — AI HP Bar

A Rust CLI + TUI that shows your LLM subscription (Claude Code, Codex CLI, …)
session quota and reset countdowns as a game-style HP bar.

- `AHB` — compact one-line status (default).
- `AHB --detailed` — multi-line per-provider breakdown.
- `AHB --json` — machine-readable output (`schema_version: 1`).
- `AHB tui` — fixed-screen view that auto-refreshes every 15 seconds (default).
```

**模式重點：**
- 第 1 行 H1 `# AHB — AI HP Bar` 保留字面
- 第 3-4 行 tagline 保留字面（D-83 step 2「H1 + Tagline」要求）
- 第 6-9 行的 4 條 `AHB` command 範例字面 verbatim 進新「Quick start」section（D-83 step 6 + RESEARCH Open Question 5 確認「既有的四條」= 此 4 行）

**既有 `## Gemini adapter status — deferred to v2` block (README.md:11-30) — D-65 字面鎖定、Phase 4 必須 verbatim 保留：**
```markdown
## Gemini adapter status — deferred to v2

The Gemini adapter is deferred to v2. Phase 0's go/no-go spike (see
`.planning/research/GEMINI_SPIKE.md`) determined that `gemini-cli 0.41.2` does
not expose a non-interactive, stable, parseable stats endpoint:

- The local `/stats` slash command is REPL-only; `gemini -p "/stats"` forwards
  the literal string to the LLM as a chat prompt.
- `--output-format json` activates the full `LocalAgentExecutor` agent loop
  instead of emitting a thin metadata envelope.
- No probe produced quota or session-reset fields in a parseable format.

The v2 adapter will be re-spiked when one of the conditions in
`GEMINI_SPIKE.md § Kill criteria` is met (e.g., gemini-cli exposes a
non-interactive `/stats` entry point, or a stable stats endpoint with
documented schema becomes available).

**ToS warning.** Web-scraping `gemini.google.com/usage` carries account-ban
risk and is permanently out of scope for AHB; see `PITFALLS.md § Pitfall 1`
for details.
```

**模式重點：** **整個 block 字面不動**（Phase 3 D-65 鎖定）— planner 用 `Read` 拿 README.md:11-30 後 verbatim paste 到新 README 的 section 9「Gemini status」。`.planning/research/GEMINI_SPIKE.md` 與 `PITFALLS.md § Pitfall 1` 兩條相對路徑保留（即使 `.planning/` 被 D-82 exclude 出 crate tarball — 此 block 出現在 GitHub README 而非 crates.io README rendering 時仍有效；crates.io 上這些路徑會 404 但不影響核心 install 流程）。

**新 README 11 個 section 結構（per D-83 + RESEARCH Examples 4-7）：**
1. **Badges 行** — 4 個 shields.io URL（RESEARCH Example 4 字面）：
   ```markdown
   [![crates.io](https://img.shields.io/crates/v/ai-hp-bar)](https://crates.io/crates/ai-hp-bar)
   [![CI](https://img.shields.io/github/actions/workflow/status/True347/ahb/ci.yml?branch=master&label=CI)](https://github.com/True347/ahb/actions/workflows/ci.yml)
   [![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](https://github.com/True347/ahb/blob/master/LICENSE-MIT)
   [![MSRV 1.88](https://img.shields.io/badge/MSRV-1.88-orange)](https://github.com/True347/ahb/blob/master/Cargo.toml)
   ```
2. **H1 + Tagline** — 沿用 README.md:1-4 字面
3. **Features bullet list** — 5 條（D-83 step 3）：
   ```markdown
   - Compact / detailed / JSON output modes
   - Static TUI mode with 15s auto-refresh
   - Multi-provider with per-adapter error isolation
   - Stale-on-error indicator for transient network failures (Phase 3)
   - OS keyring-backed credentials (no plaintext on disk)
   ```
4. **Screenshot** — 絕對 raw URL（Finding 4 強制）：
   ```markdown
   ![AHB demo — compact, detailed, and TUI modes side by side](https://raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png)
   ```
5. **Install** — 4 條 channel（RESEARCH Example 6 字面，順序 brew → binstall → cargo install → curl shell installer）
6. **Quick start** — 沿用 README.md:6-9 字面（4 條 `AHB` command）
7. **macOS Gatekeeper / cross-OS first-run notes** — D-84 + RESEARCH Example 7 字面骨架
8. **Configuration** — pointer 到 `~/.config/ahb/config.toml`（D-83 step 8）
9. **Gemini adapter status — deferred to v2** — 沿用 README.md:11-30 verbatim
10. **License** — `MIT OR Apache-2.0` dual-licensed 一段（D-83 step 10）
11. **Contributing** — 一行「PRs welcome, file issues for missing provider / unexpected output」（D-83 step 11）

**禁裝段落（D-83）：** asciinema GIF、同類比較表、公開 roadmap — planner 不可加。

---

### `.github/workflows/release.yml` (config, CI)

**Analog:** `.github/workflows/ci.yml` (Phase 0 baseline，30 行)

**既有 ci.yml 完整字面（30 行）：**
```yaml
# .github/workflows/ci.yml
name: ci
on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    name: ${{ matrix.os }} / ${{ matrix.rust }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable]
    steps:
      - uses: actions/checkout@v6
      - name: Install toolchain
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: ${{ matrix.rust }}
          components: clippy, rustfmt
      - name: Build
        run: cargo build --all-targets
      - name: Test
        run: cargo test --all-targets
      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
```

**模式重點（release.yml 與 ci.yml 共用慣例 — planner 抄這些 conventions）：**
- **3-OS matrix**：`ubuntu-latest / macos-latest / windows-latest` — release.yml 預期擴成 5-target（含 cross-build to aarch64），但 host runner 仍是這三個（aarch64 從 macos-latest [Apple Silicon] + ubuntu-latest [cross compile via cargo-dist] 出）
- **`fail-fast: false`** — Phase 0 既有設定；release.yml 預期沿用（一個 target build fail 不應 cancel 其他 target）
- **Action versions（pin major）**：`actions/checkout@v6`、`actions-rust-lang/setup-rust-toolchain@v1` — release.yml 預期同樣 pin major
- **觸發條件 differ**：ci.yml = `push: branches: [main] + pull_request`；release.yml = `push: tags: ['v*'] + pull_request`（後者 `pr-run-mode = "plan"` dry-run，per RESEARCH Pattern 3）
- **`release.yml` 由 `cargo dist init` 生成、不手寫** — Pitfall 4 + RESEARCH Pattern 3 — planner 跑 `cargo dist init` 後 review 生成的 YAML，**不要**手刻

**release.yml 預期結構（不需手寫，但 planner verification 時要確認 cargo-dist 生成有這些 job — per RESEARCH Pattern 3）：**
```yaml
# 期望由 cargo-dist 0.32 init 生成
name: Release
on:
  push:
    tags: ['v*']
  pull_request:           # pr-run-mode = "plan" dry-run
permissions:
  contents: write         # 創 GH release
  id-token: write         # OIDC (AHB v1 不用 signing)
jobs:
  plan:                   # always runs
  build-local-artifacts:  # per-target matrix
  build-global-artifacts: # installers + checksums
  host:                   # 創 GH release + 上傳 assets
  publish-homebrew:       # 用 HOMEBREW_TAP_TOKEN
```

---

### `.github/assets/screenshot.png` (asset, binary)

**Analog:** 無 — 此為 PNG binary asset，無 code analog。

**Capture command 模式（planner 執行此步驟時抄）：**

| OS | 推薦 capture 流程 | 推薦終端 / 字型 |
|----|------------------|----------------|
| macOS（CONTEXT 建議首選） | `⌘⇧4` 然後 space + click terminal window，或 `screencapture -i -w screenshot.png` | iTerm2 + JetBrains Mono / SF Mono；prompt 簡化 |
| Linux | `import -window $(xdotool getactivewindow) screenshot.png`（ImageMagick）或 `flameshot gui` | alacritty + JetBrains Mono / Fira Code |
| Windows | `PrtScn` + Snipping Tool | Windows Terminal + Cascadia Code |

**內容構圖（D-83 step 4）：** 一張 PNG（不 GIF），三條輸出並排：
```
$ AHB
[claude] ▰▰▰▰▰▱▱▱▱▱  47% • resets 02:14
[codex]  ▰▰▰▰▰▰▰▰▱▱  82% • resets 18:42 (weekly)
[mock]   ▰▰▰▰▱▱▱▱▱▱  40% • resets 04:31

$ AHB --detailed
…

$ AHB tui
[full TUI screenshot]
```

**檔案放置：** `.github/assets/screenshot.png`
- 雖然 `.github/` 被 D-82 exclude 出 crate tarball，但 README 圖片 URL 用絕對 `raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png` — Finding 4 驗證此模式對 crates.io rendering 完全相容

---

### `True347/ahb` + `True347/homebrew-tap` (外部 GH repos)

**Analog:** 無 — out-of-tree GitHub state，由 Wave 3 命令建立。

**Bootstrap command 模式（RESEARCH Example 8 verbatim，Wave 3 序列）：**
```bash
# Step 1 — 創 source repo（auto-push current branch）
gh repo create True347/ahb --public --source=. --remote=origin --push

# Step 2 — 創 tap repo with initial README（Pitfall 9：避免 empty-branch failure）
gh repo create True347/homebrew-tap --public --add-readme

# Step 3 — provision HOMEBREW_TAP_TOKEN（manual PAT creation in browser）
# 在 github.com/settings/personal-access-tokens 建 fine-grained PAT：
#   Repository access: Only select repositories → True347/homebrew-tap
#   Permissions: Contents = Read and write
gh secret set HOMEBREW_TAP_TOKEN --repo True347/ahb --body "<paste-pat-here>"

# Step 4 — tag and push（分兩步，Pitfall 7）
git tag v0.1.0
git push origin v0.1.0
# triggers release.yml → 5-target cross-build → GH release → publish-homebrew

# Step 5 — wait for pipeline
gh run watch --repo True347/ahb

# Step 6 — only AFTER release.yml 成功，才 publish 到 crates.io（Pitfall 6 順序硬要求）
cargo publish

# Step 7 — clean-machine verification of 4 channels（acceptance）
# brew install True347/tap/ahb
# cargo binstall ai-hp-bar
# cargo install ai-hp-bar
# curl -fsSL https://github.com/True347/ahb/releases/latest/download/ahb-installer.sh | sh
```

**模式重點：**
- 必須 `--public`（D-77 — cargo-binstall + cargo-dist installer URL 需 public artifact）
- `--add-readme` 在 tap repo（Pitfall 9 — 避免空 default branch 導致 cargo-dist 第一次 push formula 失敗）
- `HOMEBREW_TAP_TOKEN` 設在 **source repo (`True347/ahb`)**，不是 tap repo（Pitfall 6 + cargo-dist Homebrew installer 文件）
- `cargo publish` **在 release.yml 跑完 GH release 後才 publish**（Pitfall 6 順序硬要求）— 否則 cargo-binstall 會抓到 crates.io 有但 GH release 缺的 broken state
- PAT creation 是 **human-in-the-loop**（gh CLI 無法 atomically 創建 fine-grained PAT 並回傳 token）— plan 必須標 `checkpoint:human-verify`

---

## Shared Patterns

### Pattern A: 「self-amend」模式（Phase 4 大半改動）

**Source:** Phase 0 既有 `Cargo.toml` + `README.md` 結構
**Apply to:** `Cargo.toml` modify + `README.md` rewrite

**操作模式：**
1. 用 `Read` 拿既有檔案完整字面
2. 識別「Phase 4 改動點」vs「Phase 0-3 鎖定保留點」
3. 改動點按 D-75..D-84 字面替換
4. 保留點 verbatim 不動（特別是 README D-65 Gemini block）

**識別表：**

| 檔案 | Phase 0-3 保留點 | Phase 4 改動點 |
|------|------------------|----------------|
| Cargo.toml | 5 個 metadata 欄位 (description/repository/readme/keywords/categories)、所有 `[dependencies]` / `[target.cfg]` / `[dev-dependencies]` | `name`、`version`、加 `exclude`、加 `[[bin]]`、填 `[profile.release]`、加 `[workspace.metadata.dist]` + `[profile.dist]` |
| README.md | H1+tagline (line 1-4)、Quick start 4 條 command (line 6-9)、Gemini status block (line 11-30 D-65 verbatim) | 加 4 badges、加 features bullet list、加 screenshot、加 install 4 channels、加 Gatekeeper、加 Configuration、加 License、加 Contributing |

### Pattern B: cargo-dist init 是 single-shot generator，**不要**手刻

**Source:** RESEARCH Pattern 1 + Pattern 3 + Pitfall 4 + Pitfall 8 + Don't Hand-Roll
**Apply to:** `Cargo.toml` 新增 `[workspace.metadata.dist]` + `[profile.dist]` + `.github/workflows/release.yml`

**操作模式：**
1. **Wave 1**：手動編輯 Cargo.toml 完成 `[package]` / `[[bin]]` / `[profile.release]` / `exclude` 四大區塊
2. **Wave 2 第一步**：`cargo install cargo-dist`（dev-time only，不進 Cargo.toml deps）
3. **Wave 2 第二步**：`cargo dist init` — 由 cargo-dist 自動 append `[workspace.metadata.dist]` + `[profile.dist]` 到 Cargo.toml 檔尾，並生成 `.github/workflows/release.yml`
4. **Wave 2 第三步**：手動 patch — 在 `[workspace.metadata.dist]` 加 `formula = "ahb"`（Finding 3 — cargo-dist init 預設不加此 key）
5. **Wave 2 第四步**：`cargo dist plan` dry-run，verify output 含 5 targets × 3 installers + Homebrew tap publish job

**禁忌：**
- 不要手寫 `release.yml`（cargo-dist 自動生成 200+ 行 YAML）
- 不要 override `[profile.dist].lto = "thin"`（Pitfall 8 — cargo-dist 用此偵測 initialized 狀態）
- 不要在跑 `cargo dist init` 之前先寫 `[workspace.metadata.dist]`（Pitfall 4 順序敏感）
- 不要加 `[package.metadata.binstall]`（Finding 1 — cargo-dist 預設 tarball 名與 cargo-binstall pattern #3 直接相容）

### Pattern C: 絕對 URL 強制（for crates.io README rendering）

**Source:** Finding 4 + rust-lang/crates.io issue #13376 PSA
**Apply to:** README.md 所有 image references + badge links

**規則：**
- 圖片：`https://raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png`（`HEAD` 永遠指 default branch 最新，不用 `master`/`main`/commit SHA）
- Badges：`https://img.shields.io/...`（shields.io 服務）
- Badge link target：`https://crates.io/crates/ai-hp-bar` / `https://github.com/True347/ahb/...`（絕對 URL）

**禁忌：**
- 不寫相對路徑 `![](.github/assets/screenshot.png)`（在 GitHub repo 上 work、在 crates.io 上破圖，因為 `.github/` 被 exclude 出 tarball）
- 不寫相對 link `[CI](../actions)`（同樣 crates.io 上 broken）

### Pattern D: Wave 3 順序硬要求（irreversible operations）

**Source:** Pitfall 6 + Pitfall 7 + RESEARCH Example 8
**Apply to:** 整個 Wave 3

**順序鎖定（不可並行）：**
```
gh repo create source → gh repo create tap → set HOMEBREW_TAP_TOKEN
  → git tag → git push origin <tag> → wait release.yml pass → cargo publish
```

**禁忌：**
- 不要 `cargo publish` 在 GH repo 創建之前（`repository = "https://github.com/True347/ahb"` 會 404，binstall 探測直接破）
- 不要 `cargo publish` 在 release.yml 跑完之前（crates.io 已上、GH release 缺 → binstall fail）
- 不要省略 `--add-readme` 在 tap repo（Pitfall 9 — 空 default branch 會讓 cargo-dist 第一次 push 失敗）
- 不要 `git push` 不帶 explicit tag ref（Pitfall 7 — 預設 push 不含 tags）

### Pattern E: Per-OS cfg-gated dependencies × cargo-dist targets 對齊

**Source:** Cargo.toml:101-108 既有 cfg-gated keyring-store 模式 + RESEARCH targets list
**Apply to:** `[workspace.metadata.dist].targets`

**對齊表：**

| cargo-dist target triple | Cargo.toml cfg 對應 | Keyring backend |
|--------------------------|---------------------|-----------------|
| `x86_64-unknown-linux-gnu` | `cfg(target_os = "linux")` | dbus-secret-service-keyring-store (crypto-rust) |
| `aarch64-unknown-linux-gnu` | `cfg(target_os = "linux")` | dbus-secret-service-keyring-store (crypto-rust) |
| `x86_64-apple-darwin` | `cfg(target_os = "macos")` | apple-native-keyring-store |
| `aarch64-apple-darwin` | `cfg(target_os = "macos")` | apple-native-keyring-store |
| `x86_64-pc-windows-msvc` | `cfg(target_os = "windows")` | windows-native-keyring-store |

**驗證 gate（planner 加進 acceptance criteria）：** 每個 target build 完跑 DIST-01 check：
```bash
# Linux
ldd target/x86_64-unknown-linux-gnu/release/ahb | grep -E '(libssl|libcrypto|libnative-tls)' && echo FAIL || echo OK

# macOS（在 CI macos-latest runner 上）
otool -L target/release/ahb | grep -E 'OpenSSL|libssl|libcrypto' && echo FAIL || echo OK
```

---

## No Analog Found

| File | Role | Data Flow | Reason | Planner 替代 |
|------|------|-----------|--------|--------------|
| `.github/assets/screenshot.png` | binary asset | static file | PNG is binary, no code pattern | 抄 capture command 模式 + 構圖描述（per § screenshot.png） |
| `Cargo.lock` | tool-generated build artifact | auto-regenerated by cargo | Cargo 自動管理，無人寫 pattern | Wave 1 後跑 `cargo build` / `cargo update -p ahb`，commit 進 git；無 manual edit |
| `True347/ahb` repo | external GH state | `gh repo create` side-effect | out-of-tree infrastructure | 抄 Wave 3 命令序列（per § external repos） |
| `True347/homebrew-tap` repo | external GH state | `gh repo create` side-effect | out-of-tree infrastructure | 抄 Wave 3 命令序列（per § external repos） |

---

## Metadata

**Analog search scope:** repo root (`Cargo.toml`, `README.md`)、`.github/workflows/` (`ci.yml`)
**Files scanned:** 3
**External targets documented (no code analog):** 4
**Pattern extraction date:** 2026-05-26

**Phase 4 特殊性：** 此 phase 不產生 Rust 程式碼，所有「pattern」都是 declarative config（TOML / YAML / Markdown）+ shell command 序列。Pattern map 的價值在於：
1. 鎖定既有 Cargo.toml `[package]` 5 欄順序與 cfg-gated dep 模式（Pattern A + Pattern E）
2. 鎖定 README D-65 Gemini block verbatim 保留（Pattern A）
3. 鎖定「cargo-dist init 是 single-shot generator」的工作流順序（Pattern B + Pitfall 4）
4. 鎖定 crates.io README 絕對 URL 強制要求（Pattern C + Finding 4）
5. 鎖定 Wave 3 irreversible 順序（Pattern D + Pitfalls 6/7/9）
6. 鎖定 cargo-dist targets × Cargo.toml cfg 對齊規則（Pattern E）

## PATTERN MAPPING COMPLETE
