# Phase 4: Distribution & Release Polish - Context

**Gathered:** 2026-05-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 4 把已經跑得起來的 `ahb` binary 變成「非 Rust 開發者也能一行指令裝到」的 OSS artifact，並把所有「看起來做完了但其實沒」的 distribution pitfalls 收掉。具體交付：

1. **`cargo-dist` release pipeline** — 在 GitHub Actions 上 cross-build 5 個 target triple 的 pre-built static binary（`x86_64-{linux,apple-darwin,pc-windows-msvc}` + `aarch64-{linux,apple-darwin}`），輸出 tarball/zip + shell installer + PowerShell installer + Homebrew tap formula；無 OpenSSL / native-tls / dynamic runtime 依賴（rustls-only，Phase 1 已奠基）。
2. **Crate identity 落定 + crates.io 首發** — crate name 改 `ai-hp-bar`（binary 仍叫 `ahb`），版本由 `0.0.1` 升到 `0.1.0`，published 到 crates.io，metadata (description / keywords / repository / categories) 齊備且可搜到。
3. **4 條 install 路徑都實際可用** — `brew install True347/tap/ahb`、`cargo binstall ai-hp-bar`、`cargo install ai-hp-bar`、GitHub release artifact 手動下載；每條都在 README 有 copy-paste-able 指令並驗證過。
4. **GitHub repo bootstrap** — 用 `gh repo create True347/ahb --public` 建主 repo + push 既有 commits + tag `v0.1.0` 觸 cargo-dist 第一次正式 release；同時建第二個 repo `True347/homebrew-tap` 給 cargo-dist 自動推 formula。
5. **README 重寫 (Standard OSS 規模)** — install 章節（順序：brew → cargo binstall → cargo install → manual download）、Gatekeeper xattr workaround 章節（macOS 主、Linux/Windows 各一句備註）、features list（4-5 bullet）、一張 screenshot / asciinema、4 個 badges（crates.io / CI / license / MSRV），保留現有 Gemini status 段。
6. **Release profile tuning + tarball hygiene** — `[profile.release]` 設 `lto = true / strip = "symbols" / opt-level = 3`；`[package].exclude` 排除 `.planning/`、`.github/`、`tests/data/`。

**不做：**
- Apple Developer ID code signing + notarization（$99/yr，PITFALLS 10 已決：只走 `xattr` workaround）
- Scoop bucket / AUR PKGBUILD（v2 — bonus channel，需要持續維護）
- `opt-level = "z"` 極小化 binary（STACK alternative，但會犧牲 TUI runtime perf）
- 全 marketing README rewrite（asciinema GIF + 同類工具比較表 + 公開 roadmap）
- Linux/Windows 完整 troubleshooting 章節（SELinux / SmartScreen 細節 → v2）
- `aarch64-pc-windows-msvc` target（cargo-dist 預設 5 個 triple 已覆蓋 ROADMAP SC-1 字面，Windows ARM 用戶極少）
- Linux `musl` 變體（v1 直接用 `gnu` 工具鏈；keyring-store Linux 依 dbus-secret-service，本來就不是 100% static）
- `[[bin]]` 之外多個 binary（沒有 `ahb-doctor` / `ahb-daemon` — v2 OPS-01）
- 自動 changelog 生成（cargo-dist 的 release notes 直接用 git tag message 就好，v1 不裝 git-cliff）

**User-observable artifact：** 一個乾淨的 macOS / Linux / Windows 機器，跑下面任一條都能在 1-2 分鐘內裝起 `ahb`：

```
brew install True347/tap/ahb           # macOS / Linux
cargo binstall ai-hp-bar               # 任何已有 cargo 的機器
cargo install ai-hp-bar                # 同上、source-build
curl -fsSL https://github.com/True347/ahb/releases/latest/download/ahb-installer.sh | sh
```

裝完跑 `ahb` 出 Phase 1-3 既定的 compact HP bar。crates.io 搜「ai hp bar」/「claude codex usage」/「claude session quota」都找得到。macOS GUI 下載 release artifact 撞 Gatekeeper 時，README 有現成的 `xattr -d com.apple.quarantine ./ahb` 一條指令救得回來。

</domain>

<decisions>
## Implementation Decisions

### Crate identity & version (DIST-04)

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
  - 理由：pre-1.0 ✓ user-base = 自己 + early adopters；schema_version: 1 已 locked 但 CLI flag / behaviour 仍可能 polish；不過早承諾 stability
  - `1.0.0` 留給「所有 v1 ROADMAP requirement 都 validated + 至少跑 1 個月沒重大 schema drift」時再 bump

### GitHub remote bootstrap

- **D-77 (Phase 4 內含 `gh repo create True347/ahb --public`):**
  - 本機目前無 git remote；Cargo.toml `repository = "https://github.com/True347/ahb"` 是預期值、repo 尚未建立
  - Plan 必須包含：
    1. `gh repo create True347/ahb --public --source=. --remote=origin --push`
    2. `gh repo create True347/homebrew-tap --public`（給 cargo-dist 自動推 formula 用；formula 第一次 push 由 release pipeline 處理、人工不動）
    3. README / Cargo.toml `repository` URL 確認與實際 GH 路徑一致（已對齊）
  - 必須是 `--public`：cargo-binstall + cargo-dist installer URL 都需要 public artifact

- **D-78 (Homebrew tap repo 命名 = `True347/homebrew-tap`，不是 `homebrew-ahb`):**
  - `homebrew-tap` 是 Homebrew 慣例的 tap 名（user 跑 `brew tap True347/tap` 即可）
  - 若叫 `homebrew-ahb` user 要打 `brew tap True347/ahb`，反而 less idiomatic
  - 同一個 tap repo 將來若有第二個 formula 也可重用

### Distribution channels (DIST-02 + 加分)

- **D-79 (v1 = 4 條 channel：brew + cargo binstall + cargo install + GH release):**
  - README install 章節順序（依 PITFALLS 10 的「最快 → 最慢」順序）：
    1. `brew install True347/tap/ahb`（macOS / Linux + Homebrew，**側面繞過 Gatekeeper**）
    2. `cargo binstall ai-hp-bar`（任何已裝 cargo + cargo-binstall 的機器，秒級）
    3. `cargo install ai-hp-bar`（fallback、source-build、慢但無 toolchain 假設）
    4. GitHub release 手動下載（最後手段，會撞 Gatekeeper → 第 4 點接 §Gatekeeper workaround）
  - cargo-dist `dist-workspace.toml` 或 `Cargo.toml [workspace.metadata.dist]` 設定：
    - `targets = ["x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "aarch64-apple-darwin", "x86_64-pc-windows-msvc", "aarch64-unknown-linux-gnu"]`
    - `installers = ["shell", "powershell", "homebrew"]`
    - `tap = "True347/homebrew-tap"`
    - GH Actions workflow 由 `dist init` 生成、commit 進 `.github/workflows/release.yml`

- **D-80 (Scoop / AUR / Apple Developer ID → v2):**
  - Scoop bucket：Windows native dev 大半已有 cargo，cargo binstall 已 cover；建 bucket repo 增加維護成本
  - AUR：cargo-dist 不直接支援，要手刻 PKGBUILD，v1 不裝
  - Apple Developer ID + notarization：$99/yr + Apple ID + cert renewal，與 v1 「自用、最多 OSS」尺寸不成比例；Gatekeeper workaround 文件已 cover SC-3

### Release profile + tarball hygiene

- **D-81 (`[profile.release]` 設定 LOCKED):**
  ```toml
  [profile.release]
  lto = true
  strip = "symbols"
  opt-level = 3
  ```
  - `lto = true`（fat LTO，**不**用 `"thin"`）：cross-crate inline、binary 體積與 cold-path perf 明顯改善；CI cross-build 多 1-2min/target 可接受
  - `strip = "symbols"`：拿掉 debug symbols（Rust 1.59+ 內建，**不**用外部 `strip` binary）；不影響 panic backtrace 在 release 中本來就不會出現 file:line
  - `opt-level = 3`：標準 release optimize；**不**用 `"z"`，避免 TUI 高頻 render 與 ratatui Buffer diff 受 perf 衝擊
  - **不設** `codegen-units = 1`：與 `lto = true` 配對效益遞減、CI link time 翻倍
  - **不設** `panic = "abort"`：Phase 1 ADP-01 per-adapter error isolation 假設 unwind 能 catch；改 abort 會破壞 fan-out 容錯設計

- **D-82 (`[package].exclude` LOCKED):**
  ```toml
  [package]
  exclude = [
      ".planning/",
      ".github/",
      "tests/data/",
      ".claude/",
      ".omg/",
      "CLAUDE.md",
  ]
  ```
  - `.planning/`：50+ 治理檔（ROADMAP / PROJECT / REQUIREMENTS / phase artifacts / research notes），對 crates.io 使用者無價值，**直接最大宗瘦身來源**
  - `.github/`：CI + release workflows，不需要進 crate tarball
  - `tests/data/`：fixture（如 JSONL sample）若體積大，保留也行；先列著
  - `.claude/` + `.omg/` + `CLAUDE.md`：Claude Code 工作流檔，與最終使用者無關
  - **保留** `tests/*.rs`：integration tests 是 source 結構的一部分、helpful for `cargo install --git`
  - **保留** `LICENSE-MIT` + `LICENSE-APACHE`：法律上必須

### README scope (DIST-02 + DIST-03)

- **D-83 (Standard OSS 規模 README — install + Gatekeeper + features + screenshot + 4 badges):**

  必裝段落（依出現順序）：
  1. **Badges 行** — `[![crates.io](...)](...)` `[![CI](...)](...)` `[![License](...)](...)` `[![MSRV 1.88](...)](...)`，貼在 H1 標題下、tagline 之上
  2. **H1 + Tagline** — `AHB — AI HP Bar` + 一句 "Multi-CLI subscription session quota at a glance"（保留現有）
  3. **Features bullet list** — 4-5 行，例：
     - Compact / detailed / JSON output modes
     - Static TUI mode with 15s auto-refresh
     - Multi-provider with per-adapter error isolation
     - Stale-on-error indicator for transient network failures (Phase 3)
     - OS keyring-backed credentials (no plaintext on disk)
  4. **Screenshot / asciinema** — 一張 PNG（terminal 視窗截 `AHB` + `AHB --detailed` + `AHB tui` 三圖合一即可），檔放 `.github/assets/` 或 repo 根；**不**生 GIF（GIF 維護成本高）
  5. **Install** — 4 條 channel 按 D-79 順序
  6. **Quick start** — 既有的四條 `AHB` command 範例（沿用 Phase 1-3 既有的 README 第 6-9 行）
  7. **macOS Gatekeeper workaround** — 見 D-84
  8. **Configuration** — pointer 到 `~/.config/ahb/config.toml`，列現有 provider 字段（claude / codex / gemini / mock + refresh_interval）
  9. **Gemini status** — 保留現有 `## Gemini adapter status — deferred to v2` 段（D-65 字面）
  10. **License** — `MIT OR Apache-2.0`，dual-licensed 說明
  11. **Contributing** — 一行：「PRs welcome, file issues for missing provider / unexpected output」

  禁裝：
  - asciinema GIF / WebM 動畫
  - 與 hpup / llmstat 等同類比較表
  - 公開 roadmap / Phase 5+ 規劃

- **D-84 (Gatekeeper 章節 — macOS 主 + Linux/Windows 各一句):**

  小標題：`## macOS Gatekeeper / cross-OS first-run notes`

  內容（建議 8-10 行）：
  - macOS 主訊：
    ```
    xattr -d com.apple.quarantine ./ahb
    ```
    + 一句解釋（Apple quarantine 在 web download 後標記、unsigned binary 受阻）
  - macOS 備用：`spctl --add ./ahb`（user 已 disabled Gatekeeper 或想加 explicit allow）
  - Linux 一句：`chmod +x ./ahb` 即可（無 quarantine 機制；少數發行版有 AppArmor / SELinux 限制 → 指向 v2 doc）
  - Windows 一句：SmartScreen 跳「Windows protected your PC」→ More info → Run anyway（無等價 CLI 解法、user 必走 GUI 點兩下）
  - **不**寫 Apple Developer ID signing 計畫（Phase 4 確認不裝、寫了反而給 user 錯誤期待）

### Claude's Discretion

- **`[bin]` block 細部** — name + path 兩欄就夠，要不要再加 `test = true`/`bench = false` 由 planner 視 cargo-dist 與 cargo-deny 是否會出現 spurious warning 而定
- **Badges 圖源** — shields.io vs crates.io 原生 badge：建議全用 shields.io（一致性），planner 視 README 可讀性決定排版（單行 vs 兩行）
- **`Cargo.toml [workspace.metadata.dist]` vs `dist-workspace.toml`** — cargo-dist 0.32 兩種都支援；建議用 `[workspace.metadata.dist]` 避免多一個 top-level 檔；若 `dist init` 預設輸出 `dist-workspace.toml`，跟著預設即可
- **Screenshot 拍攝** — 平台（macOS Terminal / Linux alacritty / Windows Terminal）、prompt（zsh starship / bash plain）、provider 組合（real claude/codex/mock）由 planner 決；建議：macOS + alacritty + 真 claude+codex+mock 三 row
- **`exclude` vs `include` 策略** — 若 `include` 比較乾淨（cargo-dist verify-publish 更容易），planner 可改用 `include = ["src/**", "tests/**", "Cargo.toml", "README.md", "LICENSE-*"]`；只要結果相同
- **README badges 順序** — crates.io 版號 / CI status / License / MSRV — planner 依 README 美感 / shields.io quota 微調
- **Gatekeeper xattr 範例 binary path** — `./ahb` vs `~/Downloads/ahb` vs `/usr/local/bin/ahb`：planner 依 README 整體一致性決
- **cargo-dist version 鎖** — 0.32.x（STACK 寫的）vs v1.0.0-rc.1（已 tagged 但 pre-release）；建議 0.32.x 穩定版優先、release `dist-version` field 鎖 minor
- **CI release.yml 觸發條件** — 預設 `on: push: tags: ['v*']`；planner 確認與 D-76 tag 格式 (`v0.1.0`) 一致
- **第一次 release 是否要 `cargo-dist plan --output-format=json` dry run** — 建議 plan 內排 `--dry-run` step（確保 cargo-dist config 對齊）後再正式 push tag

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 4 直接 input
- `.planning/ROADMAP.md` § Phase 4 — goal + SC-1..SC-4 + requirement IDs (DIST-01..04)
- `.planning/REQUIREMENTS.md` — DIST-01 / DIST-02 / DIST-03 / DIST-04 原句
- `.planning/PROJECT.md` — Active requirement「單一靜態 binary 分發」、Constraints (Distribution / Privacy)

### Phase 4 critical priors（必讀，否則決策失準）
- `.planning/research/PITFALLS.md`:
  - **§ Pitfall 10 (macOS Gatekeeper)** — xattr workaround 字面、為何不裝 Developer ID signing
  - **§ Pitfall 11 (cargo install 慢 / fails)** — 為何 brew → cargo binstall → cargo install → manual download 的順序
  - **§ Pitfall 4 (per-adapter Vec<Result> isolation)** — D-81 `panic = "abort"` 不設的理由（會破壞 fan-out 容錯）
- `.planning/research/STACK.md`:
  - **§ Distribution / Recommended Stack / Development Tools** — cargo-dist 0.32 LOCKED、target triple list、release profile patterns
  - **§ What NOT to Use** — `tokio features = ["full"]`、native-tls、`chrono` default-features、`opt-level = "z"` 之類禁忌
  - **§ Stack Patterns by Variant** — minimum-binary variant 的 `lto = true / strip / opt-level` 字面（D-81 直接抄）
- `.planning/research/SUMMARY.md` — 整體研究摘要（若 planner 要 context overview）

### 前段 Phase 決策（避免決策衝突）
- `.planning/phases/03-gemini-conditional-cache-refresh-policy/03-CONTEXT.md`:
  - **§ D-65 README §Gemini status section** — Phase 4 README 重寫**必須**保留此段、字面不動
  - **§ D-66 cache 純 in-memory** — Phase 4 不引入 disk-persisted cache
- `.planning/phases/02-codex-output-formats/02-CONTEXT.md`:
  - **§ D-49..D-58 JSON DTO + schema_version: 1** — Phase 4 release 不 bump schema、CLI 行為與 Phase 2/3 等價
  - **§ D-59 exit code grid** — Phase 4 release notes 若提 exit code，須對齊此 grid
- `.planning/phases/01-engine-claude-tui-scaffold/01-CONTEXT.md`:
  - **§ ADP-01 per-adapter Vec<Result> isolation** — D-81 panic 設定不可動
  - **§ Secret<T> + keyring-core 1.0** — Phase 4 README features bullet 點到「OS keyring-backed credentials」時，描述要與 D-40 字面對齊
- `.planning/phases/00-spike-spine/00-CONTEXT.md`:
  - **§ License = MIT OR Apache-2.0** — Phase 4 dual-license LICENSE 檔已就緒（LICENSE-MIT + LICENSE-APACHE 已 commit）

### Code anchors (Phase 4 變更點)
- `Cargo.toml` — `[package]`：name 改 `ai-hp-bar`、version 升 `0.1.0`、加 `exclude`；新增 `[[bin]] name = "ahb" path = "src/main.rs"`；新增 `[profile.release]` 三行；新增 `[workspace.metadata.dist]` block（或 `dist-workspace.toml`）
- `Cargo.lock` — release tag 前必須 `cargo update -p ai-hp-bar` + commit（cargo-dist 嚴格檢查 lockfile）
- `README.md` — 全段重寫（D-83 結構）
- `.github/workflows/release.yml` — **新檔**，由 `cargo-dist init` 生成
- `.github/workflows/ci.yml` — **不動**（Phase 0 已建好 build + test + clippy on 3 OS）
- 外部 repo `True347/homebrew-tap` — **新建**，cargo-dist 自動 push formula
- 外部 repo `True347/ahb` — **新建**，即本 repo 的 origin
- `.github/assets/screenshot.png`（或同等）— **新檔**，README features 段下方

### 工具版本 lock（給 planner 確認）
- `cargo-dist` 0.32.x（STACK; v1.0.0-rc.1 已 tagged 但 pre-release，建議穩定版優先）
- `cargo-binstall` — 不需要 lock，user 端工具
- Homebrew — formula 由 cargo-dist 生、tap repo 由 cargo-dist push、無 brew 本體版本依賴

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`Cargo.toml [package]` 既有 metadata**（`Cargo.toml:7-12`）— description / keywords / repository / categories / readme / license 五個欄位 Phase 0 已建好，DIST-04 大半已就緒；Phase 4 主要動 name + version + 加 exclude + 加 `[[bin]]` + 加 `[workspace.metadata.dist]`
- **`.github/workflows/ci.yml`** — Phase 0 已建好 cross-OS build/test/clippy；Phase 4 release.yml 與此並存、不互動
- **`LICENSE-MIT` + `LICENSE-APACHE`** — Phase 0 commit 進 repo，crate publish + GH release 直接重用
- **既有 README.md § Gemini status**（`README.md:11-30`）— Phase 3 D-65 鎖定字面，Phase 4 重寫時保留不動
- **`[profile.release]` block placeholder**（`Cargo.toml:130-131`）— Phase 0 留註解 "Phase 4 will tune more"，Phase 4 直接填三行設定即可

### Established Patterns
- **rustls-only HTTPS / 無 OpenSSL**（Phase 1 已 deny native-tls 路徑、Phase 3 `dbus-secret-service-keyring-store` 已用 `crypto-rust` feature）— Phase 4 release 即可 verify `ldd target/release/ahb` 在 Linux 上不出 OpenSSL 相依（key acceptance for DIST-01）
- **Per-OS cfg-gated dependencies**（`Cargo.toml:101-108`）— Phase 1 已建好 `cfg(target_os = "linux"/"macos"/"windows")` 模式；cargo-dist cross-build 時各 target triple 各自挑對應 keyring-store backend
- **Forward-compat unknown-key warning**（Phase 1 D-38）— Phase 4 release 後 user 在舊版 config 加新 field 不會 break（已就緒）
- **Binary 命名 = `ahb` 小寫**（Phase 0 D-25 / D-26）— Phase 4 用 `[[bin]] name = "ahb"` 保護此契約即使 crate name 改

### Integration Points
- **`Cargo.toml` 對外契約面** — `[package]` 是 crates.io / cargo install / cargo binstall 共同接觸點；`[workspace.metadata.dist]` 是 cargo-dist 接觸點；`[[bin]]` 是「user `cargo install` 後的執行檔名」契約點
- **Git tag → cargo-dist trigger** — `.github/workflows/release.yml` 由 cargo-dist `dist init` 生成、預設 `on: push: tags: ['v*']`；user 推 `v0.1.0` 即觸發 cross-build + GH release + brew tap formula push
- **`exclude` 規則的副作用** — `.planning/` 排除後，crate tarball ~80% 變小；但 `cargo install --git` 仍會抓全部（git tree 不過濾），所以 `.planning/` 是「crates.io 不要、git 仍保留」的雙態 — 這正是我們要的
- **Homebrew tap 觸發** — cargo-dist 推 formula 進 `True347/homebrew-tap` 的 `Formula/ahb.rb`；user 跑 `brew install True347/tap/ahb` 時 brew 從 tap pull formula → 自動下載 GH release binary → 自動 strip quarantine flag → `/opt/homebrew/bin/ahb`（macOS）或 `/home/linuxbrew/.linuxbrew/bin/ahb`（Linux brew）

</code_context>

<specifics>
## Specific Ideas

- **README install 章節字面範例**（D-79 落實）：
  ```markdown
  ## Install

  Pick whichever matches your machine. `brew` is fastest; the others fall
  back to source-build.

  ```sh
  # macOS / Linux with Homebrew (recommended — sidesteps Gatekeeper)
  brew install True347/tap/ahb

  # Any machine with cargo + cargo-binstall (seconds, no compile)
  cargo binstall ai-hp-bar

  # Any machine with cargo (source build — 2-5 minutes first time)
  cargo install ai-hp-bar

  # Raw GitHub release artifact
  curl -fsSL https://github.com/True347/ahb/releases/latest/download/ahb-installer.sh | sh
  ```
  ```

- **`Cargo.toml [package]` 改造後預期字面**（D-75 + D-76 + D-82 落實）：
  ```toml
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
  exclude = [".planning/", ".github/", "tests/data/", ".claude/", ".omg/", "CLAUDE.md"]

  [[bin]]
  name = "ahb"
  path = "src/main.rs"
  ```

- **`[profile.release]` 預期字面**（D-81 落實）：
  ```toml
  [profile.release]
  lto = true
  strip = "symbols"
  opt-level = 3
  ```

- **`[workspace.metadata.dist]` 期望字面骨架**（planner 依 `cargo-dist init` 輸出 polish）：
  ```toml
  [workspace.metadata.dist]
  cargo-dist-version = "0.32.x"  # planner 鎖定具體 patch
  ci = ["github"]
  installers = ["shell", "powershell", "homebrew"]
  tap = "True347/homebrew-tap"
  targets = [
      "x86_64-unknown-linux-gnu",
      "x86_64-apple-darwin",
      "aarch64-apple-darwin",
      "x86_64-pc-windows-msvc",
      "aarch64-unknown-linux-gnu",
  ]
  pr-run-mode = "plan"  # PR 上只 plan 不 build
  ```

- **Gatekeeper 章節字面骨架**（D-84 落實）：
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

- **Phase 4 release dry-run sequence**（planner 排成具體 plan/wave 用）：
  1. Local `cargo-dist init` 生 `[workspace.metadata.dist]` + `.github/workflows/release.yml`
  2. Local `cargo dist plan` → 確認 5 targets / 3 installers / homebrew tap 全列到
  3. Local `cargo build --release` → `ldd target/release/ahb`（Linux）/ `otool -L`（macOS）→ verify 無 OpenSSL / native-tls
  4. Local `cargo publish --dry-run` → confirm crate metadata / exclude OK
  5. `gh repo create True347/ahb --public --source=. --remote=origin --push`
  6. `gh repo create True347/homebrew-tap --public`
  7. `git tag v0.1.0` + `git push origin v0.1.0` → cargo-dist GH Actions 自動跑 cross-build + release + brew tap push
  8. release 完成後 `cargo publish` 推 crates.io（crate 名第一次有 squat 風險，dry-run 出錯即 fallback D-75 中提到的 `aihpbar`，但 plan 預設 `ai-hp-bar` 較不易撞名）
  9. 從 clean container / VM verify `brew install True347/tap/ahb`、`cargo binstall ai-hp-bar`、`cargo install ai-hp-bar` 三路徑都 work

</specifics>

<deferred>
## Deferred Ideas

- **Scoop bucket（Windows）** → v2 — Windows native dev 大半已有 cargo / cargo-binstall；要建 bucket repo + 維護 manifest，v1 收益有限
- **AUR PKGBUILD（Arch Linux）** → v2 — cargo-dist 不直接支援、要手刻 PKGBUILD + 維護 release 流程；v1 太炭
- **Apple Developer ID signing + notarization** → v2 trigger = 「真有非技術 user 在用 macOS 並反映 Gatekeeper hurt UX」 — $99/yr + Apple ID + cert renewal、與 v1 自用尺寸不成比例
- **`opt-level = "z"` 極小化 binary** → v2 — 等到 binary 體積真的成為 distribution 障礙時再考慮；現在 STACK 推薦的 minimum-binary 路徑 v1 暫不採用
- **asciinema GIF + 同類工具比較表 + 公開 roadmap README** → v2 marketing pass — 等 v1 有 traction / 有 external user 抱怨 README 看不懂時再升
- **Linux SELinux / AppArmor / 完整 troubleshooting 章節** → v2 — 等真有 user 報問題再寫；現在預設只跑 mainstream distro 不踩雷
- **Windows ARM64 (`aarch64-pc-windows-msvc`)** → v2 — Windows on ARM 用戶極少；cargo-dist 預設 5 target 已 cover ROADMAP SC-1 字面
- **Linux `musl` 變體 (`x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl`)** → v2 — keyring-store Linux backend 走 dbus，static-musl-linking 對 dbus 不直觀；v1 用 `gnu` 工具鏈即可
- **`ahb-doctor` / `ahb-daemon` 等次要 binary** → v2 — Phase 4 只 ship 一個 `ahb` binary；未來 OPS-01 daemon mode 才考慮多 `[[bin]]`
- **`cargo-deny` + `cargo-nextest` 進 CI** → v2 — STACK 列為 "Development Tools" 但 optional；Phase 4 release 不依賴
- **自動 changelog (`git-cliff` / `cargo-release` 自動 bump)** → v2 — v1 手寫 release notes 即可
- **`include` 取代 `exclude`（whitelist 模式）** → v2（planner discretion） — 若 cargo-dist verify-publish 對 exclude 有抱怨，改用 include；v1 預設 exclude
- **README badges 進階（codecov / docs.rs / dependency status）** → v2 — v1 鎖 crates.io / CI / License / MSRV 四個就好
- **`cargo install ai-hp-bar --features extra-foo`** 等 feature gating → v2 — v1 無 optional features，crate 是 binary-only
- **Reproducible build / SLSA provenance** → v2 — supply chain security 議題，OSS CLI v1 不裝

</deferred>

---

*Phase: 4-Distribution & Release Polish*
*Context gathered: 2026-05-26*
