# Phase 3: Gemini (conditional) + Cache & Refresh Policy - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-25
**Phase:** 3-gemini-conditional-cache-refresh-policy
**Areas discussed:** Gemini stub 開啟機制, Cache 儲存範圍, Stale indicator UI 字面, refresh_interval 語意 & 預設值

---

## Gemini stub 開啟機制

| Option | Description | Selected |
|--------|-------------|----------|
| 只靠 `enabled=true`、不加 CLI flag | v1 自用、stub 本身 harmless、`enabled=true` 已是重認動作 | ✓ |
| AND：`enabled=true` AND `--experimental-gemini` | 二重 opt-in、強調實驗性 placeholder | |
| OR：任一生效 | CLI flag 補上 config 不能改的場景；語意重疊 | |

**User's choice:** 只靠 `enabled=true`，不加 CLI flag
**Notes:** 對應 D-63 / D-64 / D-65。Phase 0 hand-off 提到的 `--experimental-gemini` 不採；改以「config 註解字面更新 + README §Gemini status」表達 deferred 性質。`GeminiUnimplementedProvider::fetch` 的 error reason 字面也對齊 README section。

---

## Cache 儲存範圍

| Option | Description | Selected |
|--------|-------------|----------|
| 純 in-memory、不 disk-persist | moka in-memory；TUI 長駐生效、CLI 短命無 cache | ✓ |
| Disk-persist last-good (`~/.cache/ahb/last-good.json`) | CLI 短命重啟仍能看 stale；代價：disk format / multi-process / 權限 | |
| In-memory + 留 trait 介面給 v2 | YAGNI 風險高 | |

**User's choice:** 純 in-memory、不 disk-persist
**Notes:** 對應 D-66 / D-67。CLI 短命模式遇 transient 不走 stale fallback，直接走 Phase 1/2 既有 `format_error_row_colored` ERROR row 路徑。Cache key = `ProviderId`，不揉 FetchCtx / config。

---

## Stale indicator UI 字面

| Option | Description | Selected |
|--------|-------------|----------|
| 接在 percent 後面、全行變黃色 | 單行 + 後綴 `(stale Ns ago)` + Yellow 整行；不增 row 高度 | ✓ |
| 在 row 下多一行 indent `stale 32s ago` 註解 | row 高度變 2 倍，多 provider 排版易爆 | |
| Status 區域可見、row 本身不變 | TUI footer 顯示 count；訊息量低、看不出是哪個 provider | |

**User's choice:** 接在 percent 後面、全行變黃色
**Notes:** 對應 D-68 / D-69 / D-70。Stale row 渲染路徑唯一在 TUI；compact / detailed / JSON 三條 CLI 路徑都不參與 stale。JSON `schema_version: 1` 不 bump、不加欄位。新增 `RowState::StaleOk { state, stale_age_secs }` variant 而非擴 `ProviderState`，保 JSON DTO 純淨。

---

## refresh_interval 語意 & 預設值

| Option | Description | Selected |
|--------|-------------|----------|
| Rate-limit cap + cache TTL = refresh_interval，預設 15s | TTL 同值；stale fallback 不受 TTL 限制；clamp ≥5s | ✓ |
| 只是 rate-limit cap；cache TTL 獨立 config field | 兩個欄位、進階但 v1 過重 | |
| Cache TTL = refresh_interval、CLI 也 honor | in-memory cache 跨進程不存在，CLI honor 等於不生效 | |

**User's choice:** Rate-limit cap + cache TTL = refresh_interval，預設 15s
**Notes:** 對應 D-71 / D-72 / D-73 / D-74。三態時間軸（fresh-cache hit / 重新 fetch 成功 / fetch 失敗 fallback 到 stale）寫進 CONTEXT.md 表格。SC-4「cache TTL 與 refresh 解耦」讀作「stale-on-error fallback 不受 TTL 限制」，不是「兩個 config 欄位」。Clamp ≥5s 防 local FS 被迫接近無限迴圈。CLI 不 honor refresh_interval（cache 短暫且每次重啟）。TUI 全域 tick 15s 不動，per-provider gate 由 engine 內部依 `refresh_interval` 過濾。

---

## Claude's Discretion

以下細節在 CONTEXT.md `### Claude's Discretion` 段保留給 phase-researcher / planner 落實：

- Stale-on-error 對應的 `ProviderError` variant 分類（建議 Network + RateLimited 走 stale；Internal / SchemaDrift / Unavailable 不走）
- `moka::Cache` builder 具體配置（`max_capacity` / 是否設 `time_to_live`）
- Cache 寫入時機放在 `Engine::refresh_all` 外層 wrapper（建議）vs `fanout::refresh_all_inner` 內部
- `RowState::StaleOk` 字段命名（`state` vs `cached_state`；`stale_age_secs` vs `cached_at: Timestamp`）
- TUI stale row 顏色實作細節（單純 Yellow vs Yellow + DIM modifier）
- Integration test 形態（`IntermittentFailureProvider` + TUI snapshot 序列；wiremock infra v1 不裝）
- `Engine::new` signature / cache 注入點是否曝光供測試

## Deferred Ideas

- Disk-persist last-good cache → v2
- `--experimental-gemini` CLI flag → v2（GEMINI_SPIKE.md § Kill criteria 滿足時重新評估）
- Cache trait abstraction（`MemoryCache` / `DiskCache`）→ v2
- HTTP wiremock 200/304/401/429+Retry-After/500/slow 測試 infra → v2 Gemini GO 時實裝
- Separate `cache_ttl` config field → v2
- `--refresh` CLI flag（強制立即 fetch）→ v2
- `AHB doctor` 子命令（檢查 cache state / per-provider last_fetch_at）→ v2
- Daemon mode（背景 fetcher + 持久 IPC）→ v2 OPS-01
- Conditional GET / ETag / If-Modified-Since 機制 → v2（HTTP adapter 工作）
- Per-account daily request ceiling → v2（同上）
