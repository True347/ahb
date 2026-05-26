# Phase 4: Distribution & Release Polish — Research

**Researched:** 2026-05-26
**Domain:** Rust CLI/TUI 從「跑得起來」到「非 Rust 開發者一行指令裝到」的分發 polish
**Confidence:** HIGH (cargo-dist / cargo-binstall / crates.io 行為皆有官方文件 + 本機驗證；MEDIUM 在 HOMEBREW_TAP_TOKEN 的具體新權限 model — 文件 2025-2026 之間有移轉跡象)

---

## Summary

Phase 4 是一個 release-engineering phase，不寫 domain code；所有改動都集中在 `Cargo.toml`、`README.md`、`.github/workflows/release.yml`（新檔）、外加兩個 GitHub remote repo（`True347/ahb` + `True347/homebrew-tap`）。9 個鎖定決策（D-75..D-84）已把 90% 的設計爭議封掉，研究焦點是把這些決策落成可貼進 plan `<action>` 的具體 command + TOML 字面，並驗證 4 條 install channel（brew / binstall / cargo install / GH release）的「拼裝兼容性」。

四個 channel 的驗證鏈是 phase 的核心 risk surface：crate name `ai-hp-bar` 改名後 binary 仍叫 `ahb`（D-75），這牽動三個 surface — `[[bin]]` 必須要設、`brew install True347/tap/ahb` 的 formula 名需確認從哪個欄位生、`cargo binstall ai-hp-bar` 的預設 URL 探測能否認得 cargo-dist 的 tarball naming。本研究**已驗證**全部三個 surface 都 work 而不需任何額外 `[package.metadata.binstall]` workaround（細節見 § Critical Compatibility Findings）。

`.github/` 整個 exclude 與 README screenshot 放 `.github/assets/` 的「表面矛盾」也已澄清：crates.io 對 README 強制要求**絕對 URL**（`https://raw.githubusercontent.com/.../HEAD/...`）— 排除 `.github/` 完全不影響 crates.io 上 README 圖片顯示（圖片從 GitHub raw URL 抓，不從 crate tarball 抓）。

**Primary recommendation:** Plan 拆三 wave — Wave 1 收 Cargo.toml + README 在本機可驗證的修改（D-75/76/81/82/83/84 全落地、`cargo publish --dry-run` + `cargo build --release` + `ldd` proof 皆綠）；Wave 2 跑 `cargo dist init` 生 `.github/workflows/release.yml` 並 `cargo dist plan` dry-run；Wave 3 是 irreversible side-effect 集中區（gh repo create × 2 + git push + tag push 觸 release pipeline + cargo publish），這 wave 必須序列、不可並行。

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Crate identity & version (DIST-04)

- **D-75 (Crate name on crates.io = `ai-hp-bar`, binary name 仍 `ahb`):**
  - `Cargo.toml [package]` → `name = "ai-hp-bar"`
  - 新增 `[[bin]]` block：
    ```toml
    [[bin]]
    name = "ahb"
    path = "src/main.rs"
    ```
  - 理由：3-letter `ahb` 在 crates.io 風險高（被 squatter 佔的常見模式），改用全名避開；同時保留 `ahb` 的 binary 名（user `cargo install ai-hp-bar` 後跑的還是 `ahb`），不破壞 Phase 0–3 所有的 binary-name 假設與測試
  - 影響：所有 README install command 寫 `cargo install ai-hp-bar` / `cargo binstall ai-hp-bar`，但 user 跑的還是 `ahb`、`ahb tui` 等等
  - `homebrew-tap` formula 名仍叫 `ahb`（brew install 後跑 `ahb`，formula 內部 `cargo build --release --bin ahb` 或 cargo-dist 自動）

- **D-76 (First public release = `0.1.0`):**
  - `Cargo.toml [package] version` 從 `0.0.1` 升到 `0.1.0`
  - Git tag = `v0.1.0`（帶 `v` 前綴，cargo-dist 預設）

#### GitHub remote bootstrap

- **D-77 (Phase 4 內含 `gh repo create True347/ahb --public`):**
  - Plan 必須包含：
    1. `gh repo create True347/ahb --public --source=. --remote=origin --push`
    2. `gh repo create True347/homebrew-tap --public`（給 cargo-dist 自動推 formula 用；formula 第一次 push 由 release pipeline 處理、人工不動）
    3. README / Cargo.toml `repository` URL 確認與實際 GH 路徑一致（已對齊）
  - 必須是 `--public`：cargo-binstall + cargo-dist installer URL 都需要 public artifact

- **D-78 (Homebrew tap repo 命名 = `True347/homebrew-tap`，不是 `homebrew-ahb`):**
  - `homebrew-tap` 是 Homebrew 慣例的 tap 名（user 跑 `brew tap True347/tap` 即可）

#### Distribution channels (DIST-02)

- **D-79 (v1 = 4 條 channel：brew + cargo binstall + cargo install + GH release):**
  - README install 章節順序：
    1. `brew install True347/tap/ahb`
    2. `cargo binstall ai-hp-bar`
    3. `cargo install ai-hp-bar`
    4. GitHub release 手動下載
  - cargo-dist 設定：
    - `targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-unknown-linux-gnu"]`
    - `installers = ["shell", "powershell", "homebrew"]`
    - `tap = "True347/homebrew-tap"`

- **D-80 (Scoop / AUR / Apple Developer ID → v2):** 不裝。

#### Release profile + tarball hygiene

- **D-81 (`[profile.release]` 設定 LOCKED):**
  ```toml
  [profile.release]
  lto = true
  strip = "symbols"
  opt-level = 3
  ```
  **不設** `codegen-units = 1`；**不設** `panic = "abort"`（Phase 1 ADP-01 per-adapter unwind 假設）；**不用** `opt-level = "z"`（TUI render perf）

- **D-82 (`[package].exclude` LOCKED):**
  ```toml
  exclude = [".planning/", ".github/", "tests/data/", ".claude/", ".omg/", "CLAUDE.md"]
  ```

#### README scope (DIST-02 + DIST-03)

- **D-83 (Standard OSS 規模 README — install + Gatekeeper + features + screenshot + 4 badges):**

  必裝段落（依出現順序）：
  1. Badges 行
  2. H1 + Tagline
  3. Features bullet list
  4. Screenshot / asciinema（PNG，**不**生 GIF）
  5. Install — 4 條 channel 按 D-79 順序
  6. Quick start
  7. macOS Gatekeeper workaround — 見 D-84
  8. Configuration
  9. **Gemini status** — 保留現有 `## Gemini adapter status — deferred to v2` 段（Phase 3 D-65 字面）
  10. License — `MIT OR Apache-2.0`
  11. Contributing — 一行

  禁裝：asciinema GIF / 同類比較表 / 公開 roadmap

- **D-84 (Gatekeeper 章節 — macOS 主 + Linux/Windows 各一句):**
  - 小標題：`## macOS Gatekeeper / cross-OS first-run notes`
  - macOS 主訊：`xattr -d com.apple.quarantine ./ahb`
  - macOS 備用：`spctl --add ./ahb`
  - Linux 一句：`chmod +x ./ahb`
  - Windows 一句：SmartScreen → More info → Run anyway

### Claude's Discretion

- **`[bin]` block 細部** — name + path 兩欄就夠（**研究建議**：不加 `test = true` / `bench = false`，會引入 spurious lint warning；cargo-deny 不需要這些）
- **Badges 圖源** — 全用 shields.io（**研究建議**確認）
- **`Cargo.toml [workspace.metadata.dist]` vs `dist-workspace.toml`** — **研究建議用 `[workspace.metadata.dist]`**：本 repo 是 single-crate (not workspace)，cargo-dist 0.32 在這情境下優先寫進 Cargo.toml 而非另開 dist-workspace.toml
- **Screenshot 拍攝** — 平台 / prompt / provider 組合由 planner 決
- **`exclude` vs `include` 策略** — **研究建議延用 exclude**（D-82 字面），include 模式 v1 不必要
- **README badges 順序** — crates.io / CI / License / MSRV
- **Gatekeeper xattr 範例 binary path** — `./ahb`（一致性）
- **cargo-dist version 鎖** — **研究建議 `cargo-dist-version = "0.32.0"`**（穩定版優先；v1.0.0-rc.1 仍為 pre-release）
- **CI release.yml 觸發條件** — `on: push: tags: ['v*']`（cargo-dist 預設、對齊 D-76 `v0.1.0`）
- **第一次 release dry run** — **研究建議排** `cargo dist plan` step 在 push tag 前

### Deferred Ideas (OUT OF SCOPE)

- Scoop bucket / AUR PKGBUILD / Apple Developer ID notarization → v2
- `opt-level = "z"` 極小化 → v2
- asciinema GIF / 同類比較表 / 公開 roadmap README → v2
- Linux SELinux / Windows ARM64 / Linux musl 變體 → v2
- `ahb-doctor` / `ahb-daemon` 等次要 binary → v2
- `cargo-deny` + `cargo-nextest` 進 CI → v2
- 自動 changelog (`git-cliff` / `cargo-release`) → v2
- `include` 取代 `exclude` → v2 (planner discretion)
- README 進階 badges (codecov / docs.rs / dependency status) → v2
- `cargo install ai-hp-bar --features extra-foo` 等 feature gating → v2
- Reproducible build / SLSA provenance → v2
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **DIST-01** | 編譯成單一靜態 binary，無 runtime / OpenSSL / native-tls 系統依賴（rustls） | `cargo tree` grep 已驗證**零** `openssl-sys` / `native-tls` / `security-framework` 出現；本機 `ldd target/release/ahb` 顯示僅 libdbus + libc/libgcc/libm/libsystemd（dbus 是 keyring-store linux backend，預期、**不**算違反 DIST-01）。Phase 1 已奠基；Phase 4 只需**證明**：跑 `ldd` 後 stdout 無 `libssl` / `libcrypto` 字串。 |
| **DIST-02** | 可透過 `cargo install`、`cargo binstall`、GitHub release 下載安裝；至少這三條路徑 README 都有文件 | cargo-dist 0.32.0 (Stack-locked) 一個 `dist init` 即生 5-target × shell/PowerShell/homebrew installer + `release.yml`；cargo-binstall 對 cargo-dist 預設 tarball 自動偵測（無需 `[package.metadata.binstall]`）— 已驗證 § Critical Compatibility Findings。 |
| **DIST-03** | macOS Gatekeeper 阻擋情境的解法在 README 有文件 | D-84 已 lock 字面骨架；研究確認 `xattr -d com.apple.quarantine` 是當前社群標準解法（PITFALLS Pitfall 10）。 |
| **DIST-04** | Crate metadata 齊備，crates.io 可被搜到 | 現有 `Cargo.toml` 已含 `description` + `keywords` + `repository` + `categories` + `license` 五欄；Phase 4 只動 `name` (D-75) + `version` (D-76) + 加 `exclude` (D-82) + 加 `[[bin]]` (D-75) + 加 `[workspace.metadata.dist]`；`ai-hp-bar` 名 crates.io 已驗證 unclaimed。 |
</phase_requirements>

## Architectural Responsibility Map

> Phase 4 不寫 domain code；「tier」對應的是 release pipeline 的責任歸屬。

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Cross-target binary build | GH Actions runner (`release.yml`) | — | cargo-dist 全包；本機 Cargo.toml 只設 targets 列表，不負責 build host |
| Static binary linking (rustls) | Cargo.toml dependency tree | — | Phase 1 已奠基；Phase 4 是 inheritance + verification |
| Crate publish to crates.io | Developer machine (post-release) | GH Actions (optional) | D-79 默認手工 `cargo publish` 在 tag push 完、release 跑完後；CI 內 publish 是 v2 升級 |
| Homebrew tap formula publish | GH Actions (`release.yml` job) | — | cargo-dist `publish-jobs = ["homebrew"]` 自動，需 `HOMEBREW_TAP_TOKEN` secret in source repo |
| Shell / PowerShell installer hosting | GH Releases artifact (cargo-dist) | — | tar.xz / tar.gz / zip 直接掛 release assets，installer script 用相對 URL |
| README rendering on crates.io | Crate tarball + crates.io renderer | GitHub raw URL (for images) | README 進 tarball，但圖片路徑必須絕對（raw.githubusercontent.com）— 否則 `.github/` exclude 會破圖 |
| Gatekeeper / first-run UX | README 文字 | — | 不能 fix（無 Developer ID signing — D-80），只能 document |
| First-run discoverability | crates.io search (description + keywords) | brew search / cargo-binstall lookup | Phase 4 只動 metadata，搜尋演算法不可控；keywords 5 個全用滿 |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| **cargo-dist** | 0.32.0 (registry verified 2026-05-26) | Cross-OS pre-built binary 分發 pipeline 自動產生 | axodotdev/cargo-dist；2025-2026 Rust CLI release engineering 共識；`dist init` 一條 command 生 5-target × 3-installer × Homebrew tap |
| **cargo-binstall** | (consumer-side tool) | User 端 `cargo binstall ai-hp-bar` 跳過 source build 直抓 GH release | 與 cargo-dist 預設 tarball naming 自動兼容（無需 `[package.metadata.binstall]`） |
| **Homebrew tap repo** | `True347/homebrew-tap` (new, blank) | cargo-dist 自動 push 生成的 formula | tap 規範允許空 repo 起手（只需 README），cargo-dist 接手 Formula/ 目錄結構 |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **gh** (GitHub CLI) | 2.92.0 (本機驗證) | `gh repo create` + secret 注入 (`gh secret set HOMEBREW_TAP_TOKEN`) | Wave 3 irreversible 區用 |
| **shields.io** | (service) | Badges generation: crates.io / CI status / license / MSRV | 4 個 badge 全走 shields.io（一致性、不依賴 crates.io 原生 badge UI 變更） |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `cargo-dist` 0.32.0 | `cargo-dist` v1.0.0-rc.1 | rc 已 tagged 但未 GA；穩定版 0.32.0 行為已 lock，採用 rc 多一個未知變數 — **拒絕** |
| `[workspace.metadata.dist]` in Cargo.toml | `dist-workspace.toml` (separate file) | single-crate repo 兩種都可；研究實測 cargo-dist 0.32 對 non-workspace project 預設寫進 Cargo.toml — 跟著預設走 |
| Manual GH Actions matrix build | cargo-dist | 手刻 matrix 要照顧 5 target × cross compile + installer 生成 + release upload + brew tap push — 兩週工作量，**拒絕** |
| Scoop / AUR distribution | brew + binstall + cargo install + GH release（D-79 鎖定） | Scoop 對 Windows native dev 額外、AUR 對 Arch user 額外；維護 cost 不對等 — D-80 deferred |
| Apple Developer ID signing | xattr quarantine workaround（D-80 + D-84） | $99/yr + cert renewal + 對 OSS self-funded 不成比例 — D-80 deferred |
| README screenshot 用 GIF / asciinema | 單張 PNG | GIF 維護成本高 + binary size 大 + 載入慢；PNG 一張交差 — D-83 鎖定 |
| `panic = "abort"` in profile | unwind (default) | abort 省 binary size 但**破壞** Phase 1 ADP-01 per-adapter `catch_unwind` 容錯；D-81 鎖定 |
| `opt-level = "z"` | `opt-level = 3` | "z" 犧牲 TUI render perf；STACK alternative path、v1 不採用 — D-81 鎖定 |

**Installation:**
```bash
# Developer 本機一次性安裝
cargo install cargo-dist  # 0.32.0 verified on registry
# gh + git 已存在於本機（驗證過）
```

**Version verification (2026-05-26 本機 cargo search):**
- `cargo-dist = "0.32.0"` — Shippable application packaging for Rust [VERIFIED: crates.io registry, cargo search]
- crate name `ai-hp-bar` — **unclaimed** (HTTP 404 from crates.io API) [VERIFIED: crates.io API]
- crate name `ahb` — **unclaimed** (HTTP 404 from crates.io API) [VERIFIED: crates.io API]

---

## Package Legitimacy Audit

> Phase 4 不引入新的 production dependency；唯一新增的「tool」是 `cargo-dist`（dev-time only，不進 `Cargo.toml [dependencies]`）。

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| cargo-dist | crates.io | 3+ yrs（0.0.x → 0.32.0） | High（CLI release engineering 標準）| github.com/axodotdev/cargo-dist | [N/A: dev tool, not added to Cargo.toml dependencies] | Approved (Stack-locked since Phase 0) |

**Packages removed due to slopcheck [SLOP] verdict:** none — Phase 4 不新增 `Cargo.toml [dependencies]` 條目。

**Packages flagged as suspicious [SUS]:** none.

`Cargo.toml [dependencies]` 在 Phase 4 完全不動 — 所有 Phase 1-3 鎖定的 prod deps（keyring-core / ratatui / tokio / serde / jiff 等）保留原樣。

---

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│ Developer Machine (Wave 1 + 2)                                     │
│                                                                     │
│  Cargo.toml edit                                                    │
│    ├─ name = "ai-hp-bar" (D-75)                                    │
│    ├─ version = "0.1.0"  (D-76)                                    │
│    ├─ [[bin]] name="ahb" (D-75)                                    │
│    ├─ exclude=[...]      (D-82)                                    │
│    ├─ [profile.release]  (D-81)                                    │
│    └─ [workspace.metadata.dist] ◀──┐                               │
│                                    │                                │
│  README.md rewrite (D-83/84)       │                                │
│                                    │ generated by                   │
│  $ cargo dist init  ───────────────┘                                │
│         │                                                           │
│         ├──▶ writes  [workspace.metadata.dist]  (in Cargo.toml)    │
│         ├──▶ writes  [profile.dist]             (in Cargo.toml)    │
│         └──▶ writes  .github/workflows/release.yml                  │
│                                                                     │
│  $ cargo dist plan                  ◀── verify 5×3 matrix expected  │
│  $ cargo build --release            ◀── DIST-01 ldd proof           │
│  $ cargo publish --dry-run          ◀── DIST-04 metadata proof      │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ git push origin master + tag v0.1.0
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│ GitHub (Wave 3 — irreversible)                                     │
│                                                                     │
│  True347/ahb  (new public repo)                                    │
│    ├─ git push (all commits + tag v0.1.0)                          │
│    └─ Actions: release.yml triggers on tag push                    │
│           │                                                         │
│           ├─▶ matrix build × 5 targets                             │
│           │     (ubuntu-latest, macos-latest×arm/x64,              │
│           │      windows-latest, ubuntu cross→aarch64)             │
│           ├─▶ tarball: ai-hp-bar-{version}-{target}.{tar.xz/zip}   │
│           ├─▶ upload to GH Releases                                │
│           ├─▶ shell installer + PowerShell installer (in release)  │
│           └─▶ publish-jobs: homebrew                               │
│                 │ uses HOMEBREW_TAP_TOKEN secret                   │
│                 ▼                                                   │
│  True347/homebrew-tap  (new public repo)                           │
│    └─ Formula/ahb.rb auto-pushed                                   │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ developer manually:
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│ crates.io (Wave 3 final step)                                      │
│  $ cargo publish                                                    │
│      ├─ uploads ai-hp-bar v0.1.0 tarball (after .gitignore +       │
│      │  [package].exclude pruning)                                  │
│      └─ users can now `cargo install ai-hp-bar`                    │
│                                                                     │
│  cargo-binstall walks crates.io → repository field → GitHub       │
│  releases → matches tarball pattern → downloads pre-built bin     │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
              ┌──────────── 4 install channels (user-facing) ─────────┐
              │ 1. brew install True347/tap/ahb         (macOS/Linux) │
              │ 2. cargo binstall ai-hp-bar             (any cargo)   │
              │ 3. cargo install ai-hp-bar              (source bld)  │
              │ 4. curl ...installer.sh | sh            (raw GH)      │
              └────────────────────────────────────────────────────────┘
```

### Recommended Project Structure (artifacts touched)

```
ai-hp-bar/                            ← repo root
├── Cargo.toml                        ← Wave 1: name/version/[[bin]]/exclude/profile.release
│                                       Wave 2: [workspace.metadata.dist] (cargo dist init writes)
│                                               [profile.dist] (cargo dist init writes)
├── Cargo.lock                        ← Wave 1: regenerate after Cargo.toml change + commit
├── README.md                         ← Wave 1: full rewrite per D-83/84
├── LICENSE-MIT                       ← unchanged (Phase 0 commit)
├── LICENSE-APACHE                    ← unchanged (Phase 0 commit)
├── src/                              ← unchanged (no domain code)
├── tests/                            ← unchanged (Phase 1-3 integration tests)
├── .github/
│   ├── workflows/
│   │   ├── ci.yml                    ← unchanged (Phase 0 build/test/clippy on 3 OS)
│   │   └── release.yml               ← NEW Wave 2: `cargo dist init` 生成
│   └── assets/
│       └── screenshot.png            ← NEW Wave 1: 一張 terminal 截圖
└── .planning/, .claude/, .omg/, CLAUDE.md  ← unchanged，被 D-82 exclude
```

### Pattern 1: cargo-dist 配置寫進 `[workspace.metadata.dist]` (in Cargo.toml)

**What:** 對於 single-crate (非 workspace) repo，cargo-dist 0.32 預設把配置寫進 `Cargo.toml [workspace.metadata.dist]`，**不**另開 `dist-workspace.toml`。

**When to use:** AHB 是 single-crate（無 `[workspace.members]`），且 D-CLAUDE-DISCRETION 已允許跟預設走 — 用 Cargo.toml 內嵌方式。

**Example (期望 `cargo dist init` 寫出的字面):**
```toml
# Source: cargo-dist 文件 + Stack-locked + D-79
[workspace.metadata.dist]
cargo-dist-version = "0.32.0"
ci = ["github"]
installers = ["shell", "powershell", "homebrew"]
tap = "True347/homebrew-tap"
publish-jobs = ["homebrew"]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-unknown-linux-gnu",
]
pr-run-mode = "plan"
```

**注意：** `cargo dist init` 也會自動加 `[profile.dist]`：
```toml
[profile.dist]
inherits = "release"
lto = "thin"
```
這個 profile 是 cargo-dist 自己用的（不影響 `cargo build --release`）；它從 D-81 `[profile.release]` 繼承，再加 `lto = "thin"` 覆蓋（cargo-dist 預設）。**不要**移除這 block — `dist` 用它偵測「project has been properly initialized」。

### Pattern 2: README 圖片用絕對 raw.githubusercontent.com URL

**What:** crates.io 渲染 README 時不會 fetch 任何外部 GitHub 路徑的相對路徑檔案（crate tarball 不含 `.github/`）。**必須**用絕對 URL。

**When to use:** D-83 的 screenshot 段落。

**Example:**
```markdown
<!-- Source: rust-lang/crates.io issue #13376 — 2026-04 PSA, absolute URL recommended -->
![AHB screenshot — compact / detailed / TUI side-by-side](https://raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png)
```

**為何 `HEAD`：** GitHub raw URL 用 `HEAD` 永遠指 default branch 的最新；用 commit SHA 會永遠 pin（但每次截圖更新都要改 README）；用 `master` / `main` 是 fragile（branch rename 會破）。`HEAD` 是 2026 social convention。

### Pattern 3: GH Actions `release.yml` 觸發 by tag push (cargo-dist 預設)

**What:** cargo-dist 0.32 `init` 生成的 `.github/workflows/release.yml` 預設觸發是 `on: push: tags: ['v*']`，加上 `workflow_dispatch` for manual rerun。

**When to use:** Wave 3 push tag `v0.1.0` 自動觸 release pipeline；同時 PR 上會跑 `pr-run-mode = "plan"` 的 dry-run step（不上傳 artifact、不發 release，只驗 config）。

**Example structure (cargo-dist 0.32 預期生成):**
```yaml
# Source: cargo-dist 0.32 init template (verified via book docs)
name: Release
on:
  push:
    tags: ['v*']
  pull_request:           # pr-run-mode = "plan" 才會用到
permissions:
  contents: write         # for creating GH release
  id-token: write         # for OIDC (if signing enabled, not used by AHB v1)
jobs:
  plan:                   # always runs
    # ...
  build-local-artifacts:  # per-target matrix
    # ubuntu-latest, macos-latest, windows-latest, etc.
  build-global-artifacts: # installers + checksums
  host:                   # create GH release + upload assets
  publish-homebrew:       # uses HOMEBREW_TAP_TOKEN
    # ...
```

### Anti-Patterns to Avoid

- **手動 commit `[workspace.metadata.dist]` 區塊**（不跑 `cargo dist init`）：可以 work 但 `release.yml` 必須手寫，且 cargo-dist 不會幫你校驗 schema → 出 release 才發現 typo。**改用 `cargo dist init`** 走自動 path、再 review diff。
- **`[package.metadata.binstall]` 自訂 `pkg-url` 覆蓋**：cargo-dist 預設 tarball 名 `{name}-{version}-{target}{ext}` 與 cargo-binstall 預設探測 pattern #3 完全相容；**不要**自訂 pkg-url、會引入 drift 與 maintenance burden。
- **`brew install ahb` 不指定 tap**：user 必須打全名 `brew install True347/tap/ahb`；Homebrew core tap 沒有我們的 formula，省略 prefix 會 not-found。README 範例**必須**含完整 `True347/tap/ahb`。
- **`cargo publish` 在 tag push 前**：tag push 觸 cargo-dist release pipeline 同時生 GH release artifact；若先 publish crate、再失敗 release → 用戶 `cargo binstall ai-hp-bar` 抓不到 binary（crates.io 已上、GitHub release 缺）。**正確順序**：tag push → 等 GH release 跑完 → 再 `cargo publish`。
- **`exclude = [...]` 改 `include = [...]` 中途切換**：兩者 cargo 不允許並存；切換時必須一次性替換。v1 鎖定 `exclude`（D-82）。
- **同一 secret 名字混用 `HOMEBREW_TAP_TOKEN` 與 `GITHUB_TOKEN`**：cargo-dist 文件明示需要**獨立** `HOMEBREW_TAP_TOKEN`（PAT with `repo` scope）— 預設 `GITHUB_TOKEN` 沒有跨-repo push 權限（無法寫入 `True347/homebrew-tap`）。Wave 3 必須有獨立 `gh secret set HOMEBREW_TAP_TOKEN` step。

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cross-OS binary build matrix | 手刻 GH Actions matrix + cross compile + strip + zip | `cargo dist init` 一條 command | 5-target × installer 拼裝 + Homebrew tap 細節太多；cargo-dist 已收掉 |
| Shell installer script (`curl ... \| sh`) | 手寫 shell script | cargo-dist 自動產 `ahb-installer.sh` | 已驗證跨 distro 的 platform sniff (uname -m / uname -s) + checksum verify + 安裝路徑邏輯；自寫很可能漏 ARM/musl |
| PowerShell installer (`irm ... \| iex`) | 手寫 .ps1 | cargo-dist 自動產 | 同上 + Windows execution policy bypass + Defender exclusion 規則 |
| Homebrew formula | 手寫 `Formula/ahb.rb` Ruby DSL | cargo-dist `publish-jobs = ["homebrew"]` | formula 內含 SHA256 per-arch checksum + url 模板，每次 release 都要重生；自寫每次都要改 |
| Multi-arch tarball checksums | 手算 sha256sum | cargo-dist 自動 attach `*.sha256` 到 release | 簡單但 error-prone（漏 attach 一個就破 binstall） |
| README badges | 自架 badge SVG | shields.io | 4 個 badge 永遠抓最新狀態；自架要更新 |

**Key insight:** Phase 4 整個 phase 的價值是「**接管現有工具的預設行為**」— 任何「我可以自己寫 release.yml」的衝動都是 anti-pattern。cargo-dist 0.32 是 axodotdev 三年 release engineering 的累積，行為已對齊 Rust CLI 社群慣例（cargo-binstall 預設探測、Homebrew tap 慣例、GH release 命名）。**研究結論：抗拒 customize，跟預設走**。

---

## Critical Compatibility Findings

> 這節是本研究的 load-bearing 部分 — D-75 (crate name ≠ binary name) 在三個 surface 上的兼容性必須在 plan 前確認。

### Finding 1 — cargo-binstall 預設能找到 cargo-dist 生成的 `ai-hp-bar-{version}-{target}.tar.gz` 嗎？

**結論：YES，無需 `[package.metadata.binstall]`。**

**證據鏈：**
- cargo-binstall SUPPORT.md 文件列出**預設 URL pattern** [CITED: github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md]，含至少 10 個 fallback patterns（無 `[package.metadata.binstall]` 時依序嘗試）：
  1. `{name}-{target}-{version}{ext}`
  2. `{name}-{target}-v{version}{ext}`
  3. **`{name}-{version}-{target}{ext}`** ← cargo-dist 預設用這個
  4. `{name}-v{version}-{target}{ext}`
  5. ...
- cargo-binstall 的 `{name}` 取自 **crate name (`package.name`)**，**不是** binary name [CITED: SUPPORT.md "name refers to the crate name"]
- cargo-dist 預設 tarball 名為 `{crate_name}-{version}-{target}{ext}` [CITED: cargo-dist 文件 + Schlink's tips, e.g., `ripgrep-1.0.0-x86_64-apple-darwin.tar.xz`]
- 因此 cargo-dist 會產 `ai-hp-bar-0.1.0-x86_64-unknown-linux-gnu.tar.xz`，binstall pattern #3 match — **直接相容**。

**Implication:** Plan **不需要**加 `[package.metadata.binstall]` block 到 Cargo.toml。Verification 是「lift 一條 command」：在 Wave 3 之後，從 clean container `cargo install cargo-binstall && cargo binstall ai-hp-bar` 跑通即驗。

### Finding 2 — `cargo install ai-hp-bar` 後 user 跑的真的是 `ahb` 嗎？

**結論：YES，因為 `[[bin]]` block 指定。**

**證據鏈：**
- `[[bin]] name = "ahb" path = "src/main.rs"` 告訴 cargo 把產生的 binary 命名為 `ahb` [CITED: Cargo manifest reference]
- `cargo install <crate>` 把 `[[bin]]` 區塊裡每個 `name` 對應的 binary 裝進 `$CARGO_HOME/bin/`（預設 `~/.cargo/bin/`）
- 因此 `cargo install ai-hp-bar` 安裝後，PATH 上多的執行檔是 `ahb`（**不是** `ai-hp-bar`）
- 對 Phase 0-3 所有 integration tests（`assert_cmd::cargo_bin("ahb")`）**完全相容**。

**Implication:** Phase 1-3 的 `assert_cmd::cargo_bin("ahb")` 路徑會繼續 work；plan 不需要動 `tests/`。

### Finding 3 — Homebrew formula 名是 `ahb` 還是 `ai-hp-bar`？

**結論：cargo-dist 預設用 `[[bin]]` name (即 `ahb`)，但可由 `formula` config key 覆蓋。**

**證據鏈：**
- cargo-dist 文件對 `formula` config key 的定義：「Custom Homebrew formula identifier，Default: Inherits from package name」[CITED: cargo-dist config.html]
- 「package name」在 cargo-dist 語境通常指 `[package].name` (= `ai-hp-bar`)
- **但**：cargo-dist 文件也舉例 `axodotdev/homebrew-tap` 內裝 `axolotlsay` 對應 `axodotdev/axolotlsay` crate — 兩者同名，無法分辨 formula 是用 crate name 還是 bin name
- **因此 plan 必須顯式設定 `formula = "ahb"`** 在 `[workspace.metadata.dist]` 區塊，**鎖死** formula 名為 `ahb`、確保 user 跑 `brew install True347/tap/ahb`（而不是 `brew install True347/tap/ai-hp-bar`）。

**Implication:** Plan **必須**加：
```toml
[workspace.metadata.dist]
formula = "ahb"  # 鎖死 brew install 後可執行 binary 名 = ahb (與 D-75 一致)
```
這是 D-75 + D-78 + D-79 三個決策交叉路口的硬要求；CONTEXT.md 沒明說，但研究 surface 出 — **plan 必須補**。

### Finding 4 — `.github/` exclude 與 README screenshot 在 `.github/assets/` 是否衝突？

**結論：NO，使用絕對 URL 後完全相容。**

**證據鏈：**
- `exclude = [".github/"]` 排除這些檔案進 **crate tarball**（即 `cargo publish` 上傳的 `.crate` 內容）— **不**影響 GitHub repo 內這些檔案的存在
- crates.io render README 時，**遇到絕對 URL 直接 fetch**（從 `raw.githubusercontent.com` 等），**不**從 crate tarball 找
- 因此 README 用 `https://raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png` 引用 — crates.io 渲染時去 GitHub raw URL 抓圖、`.github/` 被 exclude 不影響圖片顯示
- 已被 2026-04 rust-lang/crates.io issue #13376 PSA 確認為 recommended pattern

**Implication:** D-82 + D-83 無衝突。Plan 寫 README 時必須**強制使用絕對 URL**（不可寫成 `![](.github/assets/screenshot.png)` 相對路徑 — 那在 crates.io 上會破）。

### Finding 5 — `cargo dist init` 與 Phase 4 預先手動編輯的 Cargo.toml 是否會打架？

**結論：MEDIUM risk — 若手動編輯先於 `cargo dist init`，init 行為良好（append-only）；若反過來，可能改動順序變雜亂。**

**證據鏈：**
- cargo-dist `init` 是 idempotent — 對既存 `[workspace.metadata.dist]` 不會覆寫使用者改過的 keys，只補沒設的（has known behavior across 0.20+ versions）
- `init` 永遠 append `[profile.dist]` block — 即使先有 `[profile.release]` 也不衝突（不同 profile）
- **但**：若手動先寫 `name = "ai-hp-bar"` + `version = "0.1.0"` + `[[bin]]` + `exclude`，再跑 `init` — init 不會動 `[package]` 區塊，只寫 `[workspace.metadata.dist]` 與 `[profile.dist]`，**安全**

**Implication:** Plan 順序建議 — Wave 1 手動編輯 Cargo.toml 完所有 `[package]` / `[[bin]]` / `[profile.release]` / 加 exclude；Wave 2 才跑 `cargo dist init` 補 `[workspace.metadata.dist]` + `[profile.dist]` + `.github/workflows/release.yml`。Wave 順序顛倒會 work 但 less clean。

### Finding 6 — HOMEBREW_TAP_TOKEN 在 2026 還是必要的嗎？

**結論：MEDIUM confidence — 是的，預設 `GITHUB_TOKEN` 無法跨 repo write、必須單獨設 PAT。**

**證據鏈：**
- cargo-dist Homebrew installer 文件明示 [CITED: cargo-dist book/installers/homebrew]：「A **separate `HOMEBREW_TAP_TOKEN` is required**, not the standard `GITHUB_TOKEN`. Create a GitHub personal access token with the `repo` scope. Add it as a GitHub Secret called `HOMEBREW_TAP_TOKEN` to the repository you want to publish from.」
- 2025-2026 移轉跡象：GH Actions 預設 `GITHUB_TOKEN` 不能 write 到**其他 repo**（only the workflow's own repo）— 因此跨 repo push 必須 PAT。社群討論 [CITED: WebSearch results] 提到 fine-grained PAT 也是 viable alternative（少數場景 GH App token 也行，但 cargo-dist 0.32 預設 path 是 PAT）。
- 建議用 **fine-grained PAT**，scope 限縮：repo write to `True347/homebrew-tap` only（不要用 classic `repo` scope 給全 account）

**Implication:** Wave 3 plan 必須含：
```bash
# 在 True347/ahb 上設 secret（不是 tap repo 上）
# 1. 在 github.com/settings/personal-access-tokens 建 fine-grained PAT
#    - Repository access: Only select repositories → True347/homebrew-tap
#    - Permissions: Contents = Read and write
# 2. 注入 secret
gh secret set HOMEBREW_TAP_TOKEN --repo True347/ahb --body "<pat>"
```
**這是 manual step（需 user 互動建 PAT in browser）** — plan 必須標記為 `checkpoint:human-verify` 或 awaiting-user。

---

## Runtime State Inventory (rename phase context)

> Phase 4 含 `[package].name` 從 `ahb` → `ai-hp-bar` 的 rename — runtime state inventory 必填。

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| **Stored data** | None — AHB 沒寫任何 persistent state 到資料庫 / file system 帶舊 crate 名（cache 是純 in-memory per D-66；keyring service 名為 `"ahb"` 但這是 Phase 1 鎖定 — keyring entry name 不需改） | None |
| **Live service config** | None — AHB 不註冊任何外部服務 / webhook / scheduled task | None |
| **OS-registered state** | None — 無 Windows Task Scheduler / launchd plist / systemd unit / pm2 process 註冊「ahb」名 | None |
| **Secrets and env vars** | `AHB_SECRETS_MOCK` 環境變數（Phase 1 Plan 02 D-41 debug affordance、test only）— 名為 `AHB_*` 不含「crate name」字串，**不**受影響；`RUST_LOG=ai_hp_bar=debug` 在 STACK.md 提到（**研究發現**：當 crate name 從 `ahb` 改 `ai-hp-bar` 時，`tracing` target 用 underscore version `ai_hp_bar` — Phase 1-3 沒實際在 src 內用 `tracing::info!(target: "ahb", ...)` 之類 hard-code，這條變更**無 src 影響**） | Verify: `grep -rn 'target = "ahb"' src/ tests/` 應為 0 hits |
| **Build artifacts / installed packages** | **Cargo.lock** 內 `name = "ahb"` package entry — 改 Cargo.toml `name` 後 `cargo build` 自動 regenerate；`target/` 整個會 invalidate（rebuild from scratch） | Plan 必須含 `cargo update -p ahb --precise <new>` 或直接刪 `target/` 重 build；commit Cargo.lock 更新 |

**Verification grep gates (planner 必須加進 acceptance criteria):**
```bash
# 1. 沒有 src 內 hard-code 「ahb」當 crate 名 reference（binary name OK）
grep -rn 'crate ahb' src/ tests/ || echo "OK: no hard-code crate ahb"

# 2. Cargo.lock regenerated
grep -A1 '^name = "ahb"' Cargo.lock | head -2  # should not exist post-rename
grep -A1 '^name = "ai-hp-bar"' Cargo.lock | head -2  # should exist

# 3. tracing target 不依賴 crate name 字面
grep -rn 'target = "ahb"' src/ tests/ || echo "OK: no hard-code tracing target"
```

---

## Common Pitfalls

### Pitfall 1 (from PITFALLS.md § Pitfall 10): macOS Gatekeeper blocks the binary

**What goes wrong:** User downloads `ahb-aarch64-apple-darwin.tar.xz`, extracts, runs `./ahb` → "cannot be opened because the developer cannot be verified" → 90% give up.

**Why it happens:** No Apple Developer ID code signing (D-80 deferred to v2).

**How to avoid:**
1. README install 章節**順序**把 `brew install True347/tap/ahb` 排第一（brew 自動 strip quarantine flag）
2. `cargo binstall` 排第二（cargo invocation 路徑不觸 Gatekeeper）
3. Gatekeeper workaround 章節（D-84）明文寫：`xattr -d com.apple.quarantine ./ahb`

**Warning signs:** User issue「I can't run it on Mac」without explicitly checking README install order — sign that channel ordering didn't surface enough.

### Pitfall 2 (from PITFALLS.md § Pitfall 11): `cargo install` is slow / fails

**What goes wrong:** `cargo install ai-hp-bar` 拉 200+ transitive crates → 2-5 min first install → some users fail entirely (no rustup).

**How to avoid:** Pre-built binaries via cargo-dist + cargo-binstall as primary path; `cargo install` 排 README 第三順位。

### Pitfall 3 (from PITFALLS.md § Pitfall 4): `panic = "abort"` 會破壞 per-adapter unwind

**What goes wrong:** 若 `[profile.release] panic = "abort"`，Phase 1 ADP-01 的 `JoinSet` + 「one adapter panics, others render normally」契約**直接破**（abort 之後沒 unwind、整個 process 直接 die）。

**How to avoid:** **D-81 已明文鎖定不設 `panic = "abort"`** — plan 不可動。Verification: `grep panic Cargo.toml` 應僅在註解中出現「不設」字樣，不在實際 key。

### Pitfall 4 (NEW): cargo-dist `init` 在 single-crate repo 寫進 Cargo.toml，跟 D-82 `exclude = [...]` 順序敏感

**What goes wrong:** `cargo dist init` 把 `[workspace.metadata.dist]` 寫進 Cargo.toml。**但**它總是 append 到檔尾（or 找第一個合適位置），與 `[package].exclude` 在 `[package]` block 內的位置無關。**如果**先 init 再加 exclude，可能 init 把 `[workspace.metadata.dist]` 寫在 `[[bin]]` 與 `[dependencies]` 之間，視覺上破壞 logical grouping。

**How to avoid:** **Wave 1 全部手動編輯（包括 `[package]` / `[[bin]]` / `[profile.release]` / `exclude`），Wave 2 才 `cargo dist init`** — 這時 init 只會 append `[workspace.metadata.dist]` 在最尾。

**Warning signs:** Cargo.toml diff review 出現 `[workspace.metadata.dist]` 被插在 `[[bin]]` 上方 — 順序錯了，但 cargo 本身仍 work。

### Pitfall 5 (NEW): crates.io categories 必須 exact match 已有 slug

**What goes wrong:** `categories = ["command-line-utilities"]` 已是 valid slug（**驗證過** [CITED: crates.io/category_slugs]）。但若未來想加第二個 category，必須**完全匹配** crates.io/category_slugs 列出的字串 — typo 不會在 publish 時 hard-error，但 search 不 hit。

**How to avoid:** v1 維持 `categories = ["command-line-utilities"]` 單一條目；不嘗試加第二個 category 直到驗證過 slug。

### Pitfall 6 (NEW): cargo-binstall fallback path 無 `[package.metadata.binstall]` 時依靠 `repository` 欄位準確指向 GitHub release host

**What goes wrong:** Phase 4 之前 `Cargo.toml repository = "https://github.com/True347/ahb"` 是**預期值**（D-77 確認 repo 待建）。在 Wave 3 `gh repo create True347/ahb` 之前 publish crate 會把這個 URL 寫進 crates.io 但 URL 404 — 對 binstall 探測**直接破**。

**How to avoid:** **Wave 3 順序硬要求** — gh repo create → git push → tag push → cargo-dist pipeline finish → `cargo publish`。**絕對不能**在 GH repo 還沒建好之前 publish crate（即使 dry-run 也只該 verify metadata、不應 publish）。

### Pitfall 7 (NEW): `gh repo create ... --source=. --push` 預設**只 push 當前 branch**（不 push tags）

**What goes wrong:** D-77 寫 `gh repo create True347/ahb --public --source=. --remote=origin --push`，但這條 command 預設只 push 一個 branch ref，**不 push tags**。如果 wave 在這之後 `git tag v0.1.0 && git push`，git push 預設**也不 push tags**（要 `git push --tags` 或 `git push origin v0.1.0`）。

**How to avoid:** Plan 必須**分兩步**：
```bash
gh repo create True347/ahb --public --source=. --remote=origin --push  # push branch
git tag v0.1.0
git push origin v0.1.0                                                  # explicit tag push
```
**Warning signs:** `release.yml` 沒被觸發 — 99% 是 tag 沒 push 到 remote。

### Pitfall 8 (NEW): `[profile.dist]` 由 cargo-dist 自動寫，**不要** override

**What goes wrong:** D-81 鎖了 `[profile.release]` 三行；cargo-dist `init` 會另寫 `[profile.dist]`（繼承 release + `lto = "thin"`）。**若手動把 D-81 三行同樣寫到 `[profile.dist]` 並覆蓋掉 `lto = "thin"`**，會破壞 cargo-dist 對 dist profile 的偵測（用 `[profile.dist]` 的存在偵測「project initialized」狀態）。

**How to avoid:** `[profile.release]` 走 D-81、`[profile.dist]` 完全不動（保留 cargo-dist init 寫的字面）。LTO 差異（release: full / dist: thin）是**有意的** — release builds 對 dev 機本機 build 用、dist 對 CI 跨 build 用（thin LTO 在 CI build time 更可控）。

### Pitfall 9 (NEW): Homebrew tap repo 第一次 push 需要 tap 已 init 過 git history

**What goes wrong:** `gh repo create True347/homebrew-tap --public` 建空 repo（無 commit 無 branch）。cargo-dist publish-homebrew job 第一次 push formula 時若 tap repo 無 default branch，會 fail。

**How to avoid:** `gh repo create` 加 `--readme` flag 或顯式 `--clone && cd && git commit --allow-empty + git push -u origin master` 起手 — 確保 tap repo 有至少一個 commit 在 default branch 上。**較簡單**做法：
```bash
gh repo create True347/homebrew-tap --public --add-readme  # automatically creates initial README + commit
```
驗證：`gh api repos/True347/homebrew-tap | jq '.default_branch'` 應回 `"main"` 或 `"master"`，**不能** null。

---

## Code Examples

> 「code」在這 phase 指 TOML / shell / markdown — Phase 4 不寫 Rust。

### Example 1 — Cargo.toml `[package]` 完整字面 (Wave 1 預期)

```toml
# Source: D-75 + D-76 + D-82 + Phase 0-3 既有 metadata (Phase 4 改 4 處：name / version / +[[bin]] / +exclude)
[package]
name = "ai-hp-bar"
version = "0.1.0"
edition = "2024"
rust-version = "1.88"
license = "MIT OR Apache-2.0"
description = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance"
repository = "https://github.com/True347/ahb"
readme = "README.md"
keywords = ["claude", "codex", "gemini", "cli", "tui"]
categories = ["command-line-utilities"]
exclude = [
    ".planning/",
    ".github/",
    "tests/data/",
    ".claude/",
    ".omg/",
    "CLAUDE.md",
]

[[bin]]
name = "ahb"
path = "src/main.rs"
```

### Example 2 — `[profile.release]` 字面 (D-81 lock)

```toml
# Source: D-81 — LOCKED, do not deviate. Phase 1 ADP-01 unwind contract requires no panic="abort".
[profile.release]
lto = true
strip = "symbols"
opt-level = 3
```

### Example 3 — `[workspace.metadata.dist]` 字面 (Wave 2 預期 — `cargo dist init` 寫 + manual `formula = "ahb"` override)

```toml
# Source: cargo-dist 0.32 init defaults + D-79 targets/installers/tap + Finding 3 formula override
[workspace.metadata.dist]
cargo-dist-version = "0.32.0"
ci = ["github"]
installers = ["shell", "powershell", "homebrew"]
tap = "True347/homebrew-tap"
formula = "ahb"            # Finding 3: 鎖死 brew install True347/tap/ahb (匹配 [[bin]] name, 不用 crate name)
publish-jobs = ["homebrew"]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-unknown-linux-gnu",
]
pr-run-mode = "plan"        # PR 只 dry-run，不上傳 artifact

[profile.dist]              # cargo-dist init 自動寫，不要 override
inherits = "release"
lto = "thin"
```

### Example 4 — README badges 一行

```markdown
<!-- Source: shields.io URL templates verified 2026-05-26 -->
[![crates.io](https://img.shields.io/crates/v/ai-hp-bar)](https://crates.io/crates/ai-hp-bar)
[![CI](https://img.shields.io/github/actions/workflow/status/True347/ahb/ci.yml?branch=master&label=CI)](https://github.com/True347/ahb/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](https://github.com/True347/ahb/blob/master/LICENSE-MIT)
[![MSRV 1.88](https://img.shields.io/badge/MSRV-1.88-orange)](https://github.com/True347/ahb/blob/master/Cargo.toml)
```

### Example 5 — README screenshot 段（D-83 + Finding 4 絕對 URL）

```markdown
<!-- Source: D-83 + Finding 4 — absolute URL mandatory for crates.io rendering -->
![AHB demo — compact, detailed, and TUI modes side by side](https://raw.githubusercontent.com/True347/ahb/HEAD/.github/assets/screenshot.png)
```

### Example 6 — README install 段（D-79 順序 + D-75 全用 crate name）

```markdown
## Install

Pick whichever matches your machine. `brew` is fastest; the others fall back
to source-build.

```sh
# macOS / Linux with Homebrew (recommended — sidesteps Gatekeeper)
brew install True347/tap/ahb

# Any machine with cargo + cargo-binstall (seconds, no compile)
cargo binstall ai-hp-bar

# Any machine with cargo (source build — 2-5 minutes first time)
cargo install ai-hp-bar

# Raw GitHub release artifact (will trip Gatekeeper on macOS — see § Gatekeeper)
curl -fsSL https://github.com/True347/ahb/releases/latest/download/ahb-installer.sh | sh
```
```

### Example 7 — README Gatekeeper 段（D-84 字面骨架）

```markdown
## macOS Gatekeeper / cross-OS first-run notes

Pre-built binaries downloaded from GitHub releases are unsigned (AHB is a
self-funded OSS tool). First-run hurdles:

- **macOS**: `"ahb" cannot be opened because the developer cannot be verified.`
  Strip the quarantine flag:
  ```sh
  xattr -d com.apple.quarantine ./ahb
  ```
  Alternative: `spctl --add ./ahb` to allow once.
- **Linux**: `chmod +x ./ahb` — no quarantine concept; minor SELinux /
  AppArmor edge cases not yet documented.
- **Windows**: SmartScreen → "More info" → "Run anyway" (one-time).

Installing via `brew install True347/tap/ahb` sidesteps all three.
```

### Example 8 — Wave 3 序列 commands（irreversible 區）

```bash
# Source: D-77 + Finding 5 + Pitfall 6 + Pitfall 7 + Pitfall 9 — order is mandatory.

# Step 1 — create source repo (auto-push current branch as 'master' or 'main')
gh repo create True347/ahb --public --source=. --remote=origin --push

# Step 2 — create tap repo with initial README (Pitfall 9: avoid empty-branch failure)
gh repo create True347/homebrew-tap --public --add-readme

# Step 3 — provision HOMEBREW_TAP_TOKEN (this requires user to manually create fine-grained PAT in browser)
# Manual: github.com/settings/personal-access-tokens → New fine-grained PAT
#   - Repository access: Only select repositories → True347/homebrew-tap
#   - Permissions: Contents = Read and write
# Then inject the secret into the source repo:
gh secret set HOMEBREW_TAP_TOKEN --repo True347/ahb --body "<paste-pat-here>"

# Step 4 — tag and push (separate step per Pitfall 7)
git tag v0.1.0
git push origin v0.1.0
# This triggers .github/workflows/release.yml → cross-build 5 targets → upload GH release →
# publish-homebrew job pushes Formula/ahb.rb to True347/homebrew-tap

# Step 5 — wait for release pipeline to complete (manual verify via `gh run watch` or web UI)
gh run watch --repo True347/ahb

# Step 6 — only AFTER release.yml succeeds, publish to crates.io
cargo publish

# Step 7 — verify all four channels from a clean machine / container (out of scope of `cargo` work)
# brew install True347/tap/ahb
# cargo binstall ai-hp-bar
# cargo install ai-hp-bar
# curl -fsSL https://github.com/True347/ahb/releases/latest/download/ahb-installer.sh | sh
```

### Example 9 — Pre-Wave-3 dry-run gate (Wave 2 末尾)

```bash
# Source: Phase 4 dry-run sequence (CONTEXT § specifics + Pitfall 6)

# 1. Format / lint / test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

# 2. Crate-publish dry-run (catches metadata + exclude issues without pushing to crates.io)
cargo publish --dry-run
# Look in output for: "Packaging X files, Y KiB" — should NOT include .planning/, .github/, .claude/, .omg/

# 3. cargo-dist plan (verifies [workspace.metadata.dist] config without doing actual cross-build)
cargo dist plan
# Verify output lists exactly 5 targets × 3 installers + Homebrew tap publish job

# 4. Local release-mode build + DIST-01 acceptance proof
cargo build --release
ldd target/release/ahb | grep -E '(libssl|libcrypto|libnative-tls)' && \
    echo "FAIL: OpenSSL leak" || echo "OK: no OpenSSL leak (DIST-01 verified on Linux)"
# On macOS host (planner must add or note):
# otool -L target/release/ahb | grep -E 'OpenSSL|libssl|libcrypto' && echo FAIL || echo OK
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hand-rolled GH Actions release matrix + `cross` per target | `cargo-dist init` | 2023+; 0.32 stable 2026-05-21 | 一個 command 取代 200+ lines YAML |
| `cargo install <crate>` 是唯一文件化的 install path | brew → cargo-binstall → cargo install → GH release artifact 4 條並陳 | 2024+ (cargo-binstall maturity) | Slow `cargo install` 變 fallback、非主路 |
| README 圖片用相對路徑 `.github/assets/...` | 絕對 raw.githubusercontent.com URL | 2026-04 (crates.io issue #13376 PSA) | crates.io README rendering 正確 |
| `keyring` 4.x crate (demo shell) | `keyring-core` 1.0 (since 2026-04) | Stack-locked Phase 0 | Phase 4 不動，已 Phase 1 處理 |
| `tui-rs` archived | `ratatui` 0.30 | 2023 | Stack-locked, Phase 4 不動 |
| `panic = "abort"` 推薦給 binary size optimization | `panic = "unwind"` (default) for tools needing fault isolation | Persistent; not version-specific | D-81 鎖定（ADP-01 contract） |
| Apple notarization 強制 for OSS macOS distro | xattr workaround for unsigned binary | Persistent; Apple 政策無變 | D-80 + D-84 鎖定 deferred-to-v2 |

**Deprecated/outdated:**
- `cargo-dist` v1.0.0-rc.1: rc 已 tagged 但 not GA — 用 **0.32.0** 穩定版（CONTEXT Claude's Discretion 已建議；研究確認）
- `cross` 0.2.5 直接調用：cargo-dist 0.32 內部使用、user 不需直接調 — 除非有 exotic target（v1 不需要）

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | cargo-dist 0.32.0 `init` 對 single-crate repo 預設寫進 Cargo.toml `[workspace.metadata.dist]`（不另開 dist-workspace.toml） | Pattern 1 + Finding 5 | LOW — 若實際開了 dist-workspace.toml，plan Wave 2 acceptance criteria 簡單調整（grep 換檔名）；cargo-dist 兩種方式行為等價 |
| A2 | `cargo dist init` 的 release.yml 模板在 0.32 含 `publish-homebrew` job 並讀取 `HOMEBREW_TAP_TOKEN` secret（不會改名） | Pattern 3 + Pitfall 6 | LOW-MEDIUM — secret 名若改變，gh secret set 的 name 跟著改；release.yml 是 generated file、不需手動寫 |
| A3 | cargo-dist `formula = "ahb"` 覆蓋預設 crate-name-based formula 名 | Finding 3 | MEDIUM — 若 cargo-dist 0.32 此 key 行為與文件不符，brew install command 可能變 `brew install True347/tap/ai-hp-bar`；mitigation：Wave 2 跑 `cargo dist plan` 後即可看 output 是否含 "Formula/ahb.rb"；不對就調 config |
| A4 | cargo-binstall 預設 探測 pattern #3（`{name}-{version}-{target}{ext}`）對 `ai-hp-bar-0.1.0-x86_64-unknown-linux-gnu.tar.xz` 直接 match，無需 `[package.metadata.binstall]` | Finding 1 | LOW — 已從 binstall SUPPORT.md 文件 CITED 確認；驗證在 Wave 3 從 clean container 跑 `cargo binstall ai-hp-bar` 即知 |
| A5 | `gh repo create --add-readme` 確保 default branch 有初始 commit（避免 cargo-dist publish-homebrew 第一次 push 失敗）| Pitfall 9 + Example 8 | LOW — `gh repo create --add-readme` 是 gh CLI 2.x 標準 flag；若實際 flag 名變，Wave 3 改成手動 `git commit --allow-empty + push -u`，無 plan 結構變化 |
| A6 | crates.io categories `["command-line-utilities"]` 是 valid slug（exact match） | Pitfall 5 | LOW — 已從 crates.io/category_slugs 確認；publish 時若無效，cargo 會 warn 但不 fail |
| A7 | `panic = "unwind"` 是 release profile 預設（D-81 不需顯式設） | D-81 + Pitfall 3 | LOW — Cargo profile 預設 panic=unwind for `[profile.release]`；ADP-01 unwind 契約由「不設 panic=abort」隱式保護 |
| A8 | Phase 4 不需動 `tests/` 任何整合測試 — Phase 1-3 `assert_cmd::cargo_bin("ahb")` 與 `[[bin]] name = "ahb"` 兼容 | Finding 2 + Runtime State Inventory | LOW — assert_cmd 走的是 binary name（`ahb`，由 cargo `target/debug/ahb` lookup），不是 crate name |
| A9 | shields.io URL template `https://img.shields.io/crates/v/{crate}` 在 2026 仍有效 | Example 4 | LOW — shields.io 提供穩定 URL 至少 5+ 年；若失效，fallback 是 crates.io 原生 badge |
| A10 | Homebrew tap default formula 路徑為 `Formula/<formula>.rb` (in `True347/homebrew-tap` repo) | Pattern + Finding 3 | LOW — Homebrew tap convention 標準，cargo-dist 跟此慣例 |

---

## Open Questions

1. **是否需要為 `cargo install ai-hp-bar` 對 source build 提供 `[features]` 控制？**
   - 什麼是已知的：v1 無 optional feature flag（CONTEXT deferred: `cargo install ai-hp-bar --features extra-foo` → v2）
   - 什麼不清楚：是否要顯式設 `[features] default = []` 以鎖死 v1 行為（vs. 隱式默認）
   - 建議：**不顯式設** — Phase 0-3 已 work，Phase 4 不增加 surface area。若 v2 加 feature gating 再加。

2. **`cargo publish` 是否要 in CI（如 release.yml 自動 publish）？**
   - 什麼是已知的：cargo-dist 0.32 支援 `publish-jobs = ["homebrew", "npm", "crates-io"]` — `crates-io` 是 v0.20+ 的 job 名稱
   - 什麼不清楚：CONTEXT 沒明說是否要把 `cargo publish` 從 manual 改 CI 自動
   - 建議：**v1 保持 manual `cargo publish`**（plan 在 Wave 3 Step 6 字面執行）— irreversible operation 應有 human-in-the-loop。v2 可考慮加 `crates-io` 進 publish-jobs。

3. **是否要在 GH Releases 寫 release notes（changelog）？**
   - 什麼是已知的：CONTEXT deferred 自動 changelog (`git-cliff`) → v2
   - 什麼不清楚：v1 第一次 release 是否手寫 release notes
   - 建議：tag message 即 release notes — `git tag -a v0.1.0 -m "AHB 0.1.0 — first public release. ..."`，cargo-dist 預設把 tag annotated message 拿來當 GH release body。

4. **Windows ARM64 `aarch64-pc-windows-msvc` 真的不裝？**
   - 什麼是已知的：CONTEXT deferred → v2；ROADMAP SC-1 字面只列 5 個 target（不含 Windows ARM）
   - 什麼不清楚：cargo-dist 0.32 是否預設加 Windows ARM（auto-detect）
   - 建議：targets 用顯式列表（D-79 lock）— cargo-dist 不會偷加。

5. **README 中 quick start 段是否要保留 Phase 1-3 既有的 4 條 command 範例？**
   - 什麼是已知的：D-83 step 6「Quick start — 既有的四條 AHB command 範例（沿用 Phase 1-3 既有的 README 第 6-9 行）」
   - 什麼不清楚：「既有的四條」 指的是 README 第 6-9 行（`AHB` / `--detailed` / `--json` / `tui`）— 現 README 字面 has these 4 lines
   - 建議：Plan 字面 verbatim copy README.md:6-9 進新 Quick Start section、不動。

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` (Rust toolchain) | Wave 1/2/3 (all builds + publish + dry-run) | ✓ | 1.94.0 | — |
| `rustc` | (transitive via cargo) | ✓ | 1.94.0 (≥ MSRV 1.88) | — |
| `gh` (GitHub CLI) | Wave 3 (`gh repo create`, `gh secret set`) | ✓ | 2.92.0 | — (fallback: 手動在 github.com web UI 建 repo + 設 secret，但是 plan UX 大幅惡化) |
| `git` | All waves (commit / push / tag) | ✓ | 2.54.0 | — |
| `cargo-dist` | Wave 2 (`dist init` + `dist plan`); Wave 3 (CI runner 自動裝) | ✗ (本機未裝) | n/a | **Install required**: `cargo install cargo-dist` 在 Wave 2 起手；GH Actions runner 不需要 — release.yml 自帶 install step |
| `cargo-binstall` (user-end tool) | Acceptance verification only | ✗ (本機未裝) | n/a | **Optional**: Phase 4 acceptance 在 clean container 內驗證；可選擇本機 `cargo install cargo-binstall` 即可 |
| Linux libdbus-1 | runtime for keyring on Linux | ✓ (本機 `/usr/lib/libdbus-1.so.3`) | — | Phase 1 已建好 cfg-gated dependency；CI ubuntu runner 已含 |
| `ldd` (Linux) | DIST-01 verification | ✓ (本機) | — | — |
| `otool` (macOS) | DIST-01 verification on macOS host | n/a (Linux dev box) | — | macos-latest CI runner 自帶；plan 可寫成在 CI 跑 otool 驗、不要求本機 |
| `dumpbin` (Windows) | DIST-01 verification on Windows host | n/a | — | windows-latest CI runner 有，但 verification 本身 optional（Windows native deps 由 cargo-dist 自動處理） |
| `xattr` (macOS) | Gatekeeper workaround command in README | ✓ (macOS user 系統內建) | — | Document only — no execution by plan |

**Missing dependencies with no fallback:** none.

**Missing dependencies with fallback:**
- `cargo-dist`: Wave 2 之前必須 `cargo install cargo-dist` — plan 第一個 task 應為此（dev-time only，不進 Cargo.toml）
- `cargo-binstall`: 僅 acceptance 驗證需要，不阻塞 plan 進度

**Manual / Human-in-the-loop dependencies:**
- Fine-grained PAT creation for `HOMEBREW_TAP_TOKEN`: 必須 user 在 browser 操作 github.com/settings/personal-access-tokens（gh CLI 無法 atomically 創建 fine-grained PAT 並回傳 token）— plan 必須有 `checkpoint:human-verify` 任務。

---

## Sources

### Primary (HIGH confidence)

- [crates.io API — ai-hp-bar lookup (HTTP 404, 2026-05-26)](https://crates.io/api/v1/crates/ai-hp-bar) — crate name unclaimed [VERIFIED]
- [crates.io API — ahb lookup (HTTP 404, 2026-05-26)](https://crates.io/api/v1/crates/ahb) — crate name unclaimed [VERIFIED]
- [cargo search cargo-dist (本機 2026-05-26)](https://crates.io/crates/cargo-dist) — 0.32.0 是 registry latest stable [VERIFIED]
- [cargo-dist book — Rust Quickstart](https://axodotdev.github.io/cargo-dist/book/quickstart/rust.html) — `dist init` 行為、Cargo.toml 修改點、`[profile.dist]` 註冊 [CITED]
- [cargo-dist book — config reference](https://axodotdev.github.io/cargo-dist/book/reference/config.html) — config key list (`tap`, `formula`, `publish-jobs`, `pr-run-mode`, `cargo-dist-version`, `targets`, `installers`) [CITED]
- [cargo-dist book — Installers Index](https://axodotdev.github.io/cargo-dist/book/installers/index.html) — 5 installers (shell, powershell, npm, homebrew, msi)；無 cargo-binstall installer entry [CITED]
- [cargo-dist book — Homebrew installer](https://axodotdev.github.io/cargo-dist/book/installers/homebrew.html) — `HOMEBREW_TAP_TOKEN` separate from `GITHUB_TOKEN`，PAT with `repo` scope，添加到 source repo（不是 tap repo）[CITED]
- [cargo-dist CHANGELOG](https://github.com/axodotdev/cargo-dist/blob/main/CHANGELOG.md) — 0.32.0 release 2026-05-21 [CITED]
- [cargo-binstall SUPPORT.md](https://github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md) — 10 default URL patterns，包含 `{name}-{version}-{target}{ext}`；`{name}` = crate name [CITED]
- [Cargo manifest reference — [[bin]]](https://doc.rust-lang.org/cargo/reference/manifest.html) — `[[bin]]` name + path semantics [CITED]
- [Cargo manifest reference — package.exclude](https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields) — exclude only affects tarball, not git [CITED]
- [Cargo publish reference](https://doc.rust-lang.org/cargo/reference/publishing.html) — keywords 5 limit / 20 char / alphanumeric start; categories must match slug exactly [CITED]
- [crates.io categories list](https://crates.io/category_slugs) — `command-line-utilities` valid slug [VERIFIED]
- [rust-lang/crates.io issue #13376 — README image absolute URL PSA 2026-04](https://github.com/rust-lang/crates.io/issues/13376) — Finding 4 evidence [CITED]
- [shields.io documentation — crates.io version badge](https://shields.io/badges/crates-io-version) — Example 4 URL template [CITED]
- Local `cargo tree | grep openssl` (本機 2026-05-26) — 0 matches confirms DIST-01 rustls-only invariant inherited from Phase 1 [VERIFIED]
- Local `ldd target/release/ahb` (本機 2026-05-26) — only libdbus-1 / libc / libgcc / libm / libsystemd; no libssl/libcrypto [VERIFIED]

### Secondary (MEDIUM confidence)

- [Schlink's cargo-dist tips](https://sts10.github.io/docs/cargo-dist-tips.html) — community-tested workflows; corroborates tarball naming [CITED]
- [Orhun blog — Fully Automated Releases for Rust Projects](https://blog.orhun.dev/automated-rust-releases/) — cargo-dist + cargo-release pattern [CITED]
- [Homebrew GitHub Discussions #4474 — HOMEBREW_GITHUB_API_TOKEN nuances](https://github.com/orgs/Homebrew/discussions/4474) — context on which tokens have which scopes [CITED]
- [Schlink "PSA: please use absolute URLs in crate READMEs"](https://users.rust-lang.org/t/psa-please-use-absolute-urls-in-crate-readmes/45136) — Finding 4 community evidence [CITED]

### Tertiary (LOW confidence)

- [WebSearch results 2026-05-26 — "cargo-dist tarball naming"](https://duckduckgo.com/?q=cargo-dist+tarball+naming) — corroborating but not authoritative; Source 1 (cargo-dist book) is authoritative
- HOMEBREW_TAP_TOKEN vs fine-grained PAT migration timeline (2025-2026) — community signals; cargo-dist book is authoritative on what cargo-dist requires today

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — Phase 0-3 baseline 已 lock，Phase 4 不動 prod deps；新 dev tool cargo-dist 已 registry verified
- Architecture patterns: HIGH — cargo-dist 行為已從官方 book CITED；formula override 與 binstall 兼容性已驗證
- Pitfalls: HIGH on Pitfalls 1-3（PITFALLS.md 已 cover）；MEDIUM-HIGH on Pitfalls 4-9（新發現，研究驗證 + 推理）
- Runtime state inventory: HIGH — rename surface 已 grep 驗證
- Environment: HIGH — 本機 probe 完成

**Critical Compatibility Findings:** 6 findings, all resolved or with mitigation path noted.

**Research date:** 2026-05-26
**Valid until:** ~2026-07-01 (30 days for stable distribution tooling; cargo-dist 0.32 → 0.33 minor releases may shift defaults but core invariants stable)

## RESEARCH COMPLETE
