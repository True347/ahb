# Requirements: AI HP Bar (AHB)

**Defined:** 2026-05-22
**Core Value:** 任何時刻、一個指令，立即看到所有訂閱的 AI CLI「現在還剩多少 session 額度、什麼時候 reset」。

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Core (CLI Entry)

- [x] **CORE-01**: 不帶 argument 跑 `AHB` 預設輸出所有設定 provider 的緊湊一行 HP bar（含 % + reset 倒數）
- [x] **CORE-02**: `AHB --compact` 強制緊湊輸出
- [x] **CORE-03**: `AHB --detailed` 多行 per provider，含 session 與 weekly 兩條 bar
- [x] **CORE-04**: `AHB --json` 輸出帶 `schema_version: 1` 的穩定 JSON 結構，安全可供 tmux / Starship / shell pipeline 消費
- [x] **CORE-05**: CLI 在非 TTY（被 pipe）情境下自動關閉色彩、ANSI escape；尊重 `NO_COLOR` 環境變數
- [x] **CORE-06**: 適當 exit code — 0 表示至少一個 provider 正常；1 表示全部 provider 失敗；2 表示 config / secrets 不可用

### TUI

- [x] **TUI-01**: `AHB tui` 進入固定畫面，每 provider 一條 HP bar + reset 倒數
- [x] **TUI-02**: TUI 定時自動 refresh，預設 15s
- [x] **TUI-03**: TUI refresh 頻率可由 config 設定，per-provider 可覆寫（必要時 network adapter 可拉長到 ≥5min）
- [x] **TUI-04**: TUI panic / Ctrl-C 結束時保證 terminal 還原（無 altscreen / hidden cursor 殘留）
- [x] **TUI-05**: TUI 非 TTY 環境下不啟動（給出 clear error），不會把 escape sequence 噴給 pipe

### Config

- [x] **CFG-01**: TOML config 檔列出要追蹤的 provider 清單，每個 provider 可獨立啟用 / 停用
- [x] **CFG-02**: Config 檔位於 cross-platform 標準位置（`directories` crate 處理 Linux/macOS/Windows 差異）
- [x] **CFG-03**: Config 允許 per-provider 指定 refresh interval、limit 覆寫、auth 來源
- [x] **CFG-04**: 未配置的 provider 自動 skip 不顯示，不算 failure

### Secrets

- [x] **SEC-01**: 所有 provider 認證（cookie、token、session id）一律存進 OS keyring（`keyring-core` 1.0；macOS Keychain / Win Cred Manager / Linux Secret Service）
- [x] **SEC-02**: 內部以 `Secret<T>` newtype 包裝，`Debug` 自動 redact、`#[serde(skip)]` 防意外序列化
- [x] **SEC-03**: `--json` / log / error message 中絕不出現原始 secret 值（CI grep 測試守住）
- [x] **SEC-04**: 純無 secret 的 provider（如 Claude Code 純讀本地）仍走相同介面（不破壞統一 contract）

### Provider Adapters

- [x] **ADP-00**: `Provider` trait + `FetchCtx` + `ProviderState` / `ResetInfo` / `HpUnit` / `ProviderError` 統一介面，三家共用
- [x] **ADP-01**: Adapter 失敗只影響該 provider — 不會讓整個 AHB crash 或 blank（per-adapter timeout + `Vec<Result<...>>` + cache stale fallback）
- [x] **ADP-02**: Claude Code adapter — 從 `~/.claude/projects/**/*.jsonl` 計算 5h rolling window 用量 + reset 時間（不依賴 stats-cache.json 為 source of truth）
- [x] **ADP-03**: Claude adapter schema drift sentinel — 當期望欄位大量缺失時顯示「adapter may be out-of-date」警告
- [x] **ADP-04**: Codex CLI adapter — read-only 開啟 `~/.codex/state_*.sqlite`（動態 version glob）+ `busy_timeout` + 偏好 append-only JSONL rollouts；`rate_limits: null` 視為 unknown
- [x] **ADP-05**: Gemini CLI adapter — **conditional on Phase 0 spike**。若 spike pass：HTTP 走 `gemini.google.com/usage`（或更安全的 local `gemini /stats` capture），refresh 最少 5min、ETag、daily ceiling、README ToS warning。若 spike fail：stub 在 v2 opt-in flag 後，README 寫明 deferred 原因。

### Quality & Distribution

- [ ] **DIST-01**: 編譯成單一靜態 binary，無 runtime / OpenSSL / native-tls 系統依賴（rustls）
- [ ] **DIST-02**: 可透過 `cargo install`、`cargo binstall`、GitHub release 下載安裝；至少這三條路徑 README 都有文件
- [ ] **DIST-03**: macOS Gatekeeper 阻擋情境的解法在 README 有文件
- [ ] **DIST-04**: Crate metadata（description / keywords / repository）齊備，crates.io 可被搜到

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Differentiators (defer if v1 already large)

- **DIFF-01**: Pace indicator（behind / on-pace / too-hot icon）
- **DIFF-02**: Plain-ASCII fallback mode for tmux / Windows Terminal / non-unicode
- **DIFF-03**: Claude Code statusline-hook compatibility（吃 stdin JSON，輸出單行）
- **DIFF-04**: Color-blind friendly palette toggle
- **DIFF-05**: Per-provider 同時顯示 5h session + weekly 兩條 bar（不只顯示 worst-case window）

### Provider extensions

- **EXT-01**: Cursor / Windsurf / Amp / Copilot CLI 等更多 adapter（trait 已備）
- **EXT-02**: Gemini adapter v2 opt-in（若 Phase 0 將其 defer）

### Operating modes

- **OPS-01**: Daemon mode — 只有 v1 inline refresh 出現實際痛點時才做
- **OPS-02**: Watch mode CLI (`AHB watch`) — long-running CLI 對應 TUI（給 GUI 環境 spawn 用）
- **OPS-03**: tmux / Starship 整合 recipes（文件，非程式碼）

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| 累積活動量 dashboard（only-goes-up token / cost counter） | 違反 Core Value 的重置式血條本質；ccusage 已做得很好，文件連結即可 |
| ML / P90 limit prediction | `% remaining + pace indicator` 已涵蓋價值，預測模型只加複雜度 |
| 歷史 trend 圖 | 需要 SQLite 持久化 — 是另一個產品（usage-history-viewer），不是 AHB |
| Desktop / 推播 notification | 與 `code-notify` 等工具 compose 即可，不重做 |
| Web dashboard / GUI 桌面視窗 / OBS overlay / mobile app | 錯誤的 form factor；CLI + TUI 已涵蓋常駐顯示需求 |
| API key / quota 追蹤（API usage tracking） | 與訂閱式 session countdown 是不同 use case，混進來會稀釋 Core Value |
| Provider 計費 / 收費 / multi-tenant SaaS | 不是商業產品 |
| Plan-tier 自動偵測 | 從 provider 宣告的 limit 讀，不要 infer |
| Telemetry / usage upload | 本機工具，privacy-first，絕不上傳任何資料 |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CORE-01 | Phase 1 | Complete |
| CORE-02 | Phase 2 | Complete |
| CORE-03 | Phase 2 | Complete |
| CORE-04 | Phase 2 | Complete |
| CORE-05 | Phase 1 | Complete |
| CORE-06 | Phase 2 | Complete |
| TUI-01 | Phase 1 | Complete |
| TUI-02 | Phase 1 | Complete |
| TUI-03 | Phase 3 | Complete |
| TUI-04 | Phase 1 | Complete |
| TUI-05 | Phase 1 | Complete |
| CFG-01 | Phase 1 | Complete |
| CFG-02 | Phase 1 | Complete |
| CFG-03 | Phase 3 | Complete |
| CFG-04 | Phase 1 | Complete |
| SEC-01 | Phase 1 | Complete |
| SEC-02 | Phase 1 | Complete |
| SEC-03 | Phase 2 | Complete |
| SEC-04 | Phase 1 | Complete |
| ADP-00 | Phase 0 | Complete |
| ADP-01 | Phase 1 | Complete |
| ADP-02 | Phase 1 | Complete |
| ADP-03 | Phase 1 | Complete |
| ADP-04 | Phase 2 | Complete |
| ADP-05 | Phase 3 | Pending (conditional on Phase 0 spike) |
| DIST-01 | Phase 4 | Pending |
| DIST-02 | Phase 4 | Pending |
| DIST-03 | Phase 4 | Pending |
| DIST-04 | Phase 4 | Pending |

**Coverage:**
- v1 requirements: 29 total (CORE×6 + TUI×5 + CFG×4 + SEC×4 + ADP×6 + DIST×4)
- Mapped to phases: 29 ✓
- Unmapped: 0

**Phase distribution:**
- Phase 0 (Spike & Spine): 1 requirement (ADP-00)
- Phase 1 (Engine + Claude + TUI): 15 requirements (CORE-01, CORE-05, TUI-01/02/04/05, CFG-01/02/04, SEC-01/02/04, ADP-01/02/03)
- Phase 2 (Codex + Output): 6 requirements (CORE-02/03/04/06, SEC-03, ADP-04)
- Phase 3 (Gemini + Cache): 3 requirements (TUI-03, CFG-03, ADP-05)
- Phase 4 (Distribution): 4 requirements (DIST-01/02/03/04)

---
*Requirements defined: 2026-05-22*
*Last updated: 2026-05-22 after roadmap creation — traceability filled, total corrected from 28 to 29 (ADP has 6 items: ADP-00 through ADP-05)*
