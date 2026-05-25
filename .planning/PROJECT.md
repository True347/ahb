# AI HP Bar (AHB)

## What This Is

AHB 是一個用 Rust 寫的 CLI + TUI 工具，把多家 LLM 訂閱（Claude Code、Codex CLI、Gemini CLI 等）目前的 session 剩餘額度與 reset 倒數，用一條像遊戲血條的 HP bar 顯示出來。CLI mode `AHB` 印出緊湊狀態列、可用 flag 切換 detailed / json 輸出；TUI mode 提供固定畫面、可設定 refresh 頻率（預設 15s）。受眾是同時用多個 AI CLI 的開發者本人，最多走到 open source 分發；不是商業產品。

## Core Value

**任何時刻、一個指令，立即看到所有訂閱的 AI CLI「現在還剩多少 session 額度、什麼時候 reset」。** 重置式血條的視覺隱喻是必要的；只顯示累積活動量（only-goes-up dashboard）不算達成 core value。

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

- [x] CLI `AHB` 不帶 argument，預設輸出所有設定 provider 的緊湊一行 HP bar（含 % 與 reset 倒數）— Validated in Phase 2 (CORE-02, explicit `--compact` flag + default compact route)
- [x] CLI 支援 `--compact` / `--detailed` / `--json` flag 切換輸出格式 — Validated in Phase 2 (CORE-02 / CORE-03 / CORE-04, `--json` 鎖 `schema_version: 1`，clap `ArgGroup` 三選一互斥)
- [x] 支援 Codex CLI 訂閱的 session 額度 + reset 時間取得 — Validated in Phase 2 (ADP-04, rusqlite read-only + JSONL rollouts + `SchemaDrift` 邊界)

### Active

<!-- v1 hypotheses. Building toward these. -->

- [ ] TUI mode（`AHB tui` 或同等）顯示固定畫面、定時自動更新
- [ ] TUI refresh 頻率可由 config 設定，預設 15s
- [ ] Config 檔指定要追蹤的 provider 清單與其認證 / 來源設定，支援多 provider 同時啟用
- [ ] 支援 Claude Code 訂閱的 session 額度 + reset 時間取得
- [ ] 支援 Gemini CLI 訂閱的 session 額度 + reset 時間取得（資料來源候選：`gemini.google.com/usage`，需 spike 驗證 auth / 回應格式）
- [ ] 單一靜態 binary 分發（cargo install 或 release artifact），無 runtime 依賴

### Out of Scope

<!-- Explicit boundaries with reasoning. -->

- 累積活動量 dashboard（only-goes-up token counter） — Core Value 是重置式血條，不是儀表板
- 遊戲 overlay / 串流 OBS overlay — 命名雖像遊戲，產品定位是開發者 CLI / TUI 工具
- GUI 桌面視窗 / mobile app — TUI 已涵蓋常駐顯示需求，桌面 GUI 開發成本不成比例
- 多使用者 / 多帳戶管理 — 本工具是個人本機工具，不做 SaaS / team 帳號
- 為 provider 提供 API key 用量查詢（API quota） — 與訂閱式 session countdown 是不同 use case，混進來會稀釋 Core Value
- 對 LLM provider 本身計費 / 收錢 — 不是商業產品

## Context

- 使用者本人同時訂閱 Claude Code、Codex、Gemini CLI（未來可能更多），常因 rate-limit 撞牆而需要在多個 terminal 切換查看 `/usage` `/status`，痛點明確。
- 先驗研究顯示三家 provider 的訂閱用量資料路徑不一致：
  - **Claude Code**：`~/.claude/stats-cache.json` 有日活動，但 session 5h / weekly limit countdown 須另解析；`/usage` slash command 互動模式可用。
  - **Codex CLI**：`/status` 在 `codex exec` 模式可呼叫，回傳純文字含 limit 警告；`~/.codex/` 下有 SQLite + JSONL session log。
  - **Gemini CLI**：本地無用量資料，但使用者驗證過 `gemini.google.com/usage` 可 curl 取得，實際 auth / 回應 schema 待 spike。
- 視覺隱喻「HP Bar」是強訊號，不只是命名 — 重置式血條是 v1 核心 UX。

## Constraints

- **Tech stack**: Rust — 單 binary 分發、ratatui 是 TUI SOTA、未來 daemon 模式好擴。理由：對「給其他 multi-CLI 使用者用」的分發體驗最低摩擦。
- **Data source per provider**: 三家異質，預期需要 per-provider adapter，部分得靠 local state 解析 + 部分得靠 HTTP（Gemini）。
- **Distribution**: 自用為主，最多 open source；不寫 license / billing / multi-tenant 基礎建設。
- **Refresh budget**: TUI 預設 15s，避免對需要 HTTP 的 provider（Gemini）打太多；rate-limit 自保護由 adapter 內部處理。
- **Privacy**: 本機工具，不上傳任何 usage 資料到第三方。Provider 認證 token / cookie 必須只留在本機 config / OS keyring。

## Key Decisions

<!-- Decisions that constrain future work. -->

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| 顯示 rate-limit countdown（Path A），不是累積活動量（Path B） | Core Value 是「HP Bar 視覺 + reset 倒數」，only-goes-up dashboard 不符合 | — Pending |
| 用 Rust 寫 | 單 binary 分發體驗最佳、ratatui 適合固定 UI 定時更新、長期可演化 daemon | — Pending |
| TUI refresh 預設 15s（可調） | 訂閱 session 是小時級限制，15s 已足夠即時又不會 HTTP-flood Gemini endpoint | — Pending |
| CLI 預設輸出緊湊一行 + flag 切換 detailed/json | 開發者習慣管線化，json 利於整合其他 status line / tmux | — Pending |
| v1 鎖定 Claude Code、Codex、Gemini CLI 三家 | 是使用者本人實際在用的 CLI，能 dogfood；其他 provider 留 v2 | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-05-25 after Phase 2 (Codex + Output Formats) completion — 4/4 must-haves verified, 3 Active requirements promoted to Validated (CORE-02 default compact + ArgGroup, CORE-03/04 detailed/json formats, ADP-04 Codex adapter)*
