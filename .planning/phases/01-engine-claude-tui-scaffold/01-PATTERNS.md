# Phase 1: Engine + Claude + TUI Scaffold — Pattern Map

**Mapped:** 2026-05-23
**Files analyzed:** 18 (new + modified)
**Analogs found:** 12 strong / 18 (6 files have no analog yet — flagged with structural recommendations)

> 用途：planner 在寫 PLAN action 時，引用本檔具體 file:line 範圍 + 既有 style，避免新 module 漂走 Phase 0 已立的 lint floor / serde shape / 模組慣例。

---

## File Classification

| 新 / 改的檔 | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `src/engine/mod.rs` | service (engine entry) | fan-out → batch | `src/provider/mod.rs` | role-match（同樣 module 邊界 + dyn-safety 顧慮） |
| `src/engine/fanout.rs` | service (concurrency) | request-response × N | (none) | **no analog** — 結構建議見下 |
| `src/engine/events.rs` | model (enum + channel types) | pub-sub | `src/model.rs` (closed enum + serde) | role-match |
| `src/provider/claude/mod.rs` | adapter (Provider impl) | file-IO → CRUD-once | `src/provider/mock.rs` | **exact**（同 trait、同 ctx 用法） |
| `src/provider/claude/jsonl.rs` | utility (line-stream parser) | streaming file-IO | (none) | **no analog** — RESEARCH Example 2 即 reference |
| `src/provider/claude/window.rs` | utility (domain math) | transform | `src/cli/render_text.rs::format_countdown` | role-match（同樣是純 jiff math + cast lint scope） |
| `src/provider/mock.rs` *(modified)* | adapter | gated mock | `src/provider/mock.rs` (self) | exact（只加 panic-injection 路徑） |
| `src/config.rs` | service (config loader) | file-IO read-once | `src/secrets.rs` (Phase 0 stub shape) | role-match（module 結構 + Phase 0→1 演進慣例） |
| `src/secrets.rs` *(modified)* | service (keyring wire) | request-response | `src/secrets.rs` (self) | self-expand |
| `src/tui/mod.rs` | controller (TUI entry) | event-driven | `src/main.rs` (async entry + early-exit) | role-match |
| `src/tui/app.rs` | model (app state cache) | event-driven | `src/model.rs::ProviderState` | role-match |
| `src/tui/ui.rs` | renderer (ratatui Frame) | transform | `src/cli/render_text.rs::compact_line` | role-match（同樣 `&ProviderState` → 視覺輸出） |
| `src/tui/widgets/hp_row.rs` | renderer (one row widget) | transform | `src/cli/render_text.rs` (整檔) | role-match |
| `src/cli/mod.rs` *(modified)* | controller (clap dispatch) | request-response | `src/main.rs` Cli struct | role-match |
| `src/cli/tty.rs` | utility (TTY decision) | transform | `src/cli/render_text.rs::filled_cells`（scope-限定 cast） | role-match |
| `src/main.rs` *(modified)* | controller (dispatch) | request-response | `src/main.rs` (self) | self-expand |
| `src/templates/default-config.toml` | config asset | static | (none) | **no analog** — embedded via `include_str!` |
| `tests/*.rs`（integration） | test | request-response | `src/provider/mock.rs::tests` + `src/cli/render_text.rs::tests` | role-match |

---

## Pattern Assignments

### `src/provider/claude/mod.rs` — Provider impl (adapter, file-IO)

**Analog:** `src/provider/mock.rs` (EXACT match — 同 trait、同 ctx 用法、同 clock-injection 契約)

**Imports pattern**（`mock.rs` lines 1-6）：
```rust
use std::borrow::Cow;

use async_trait::async_trait;

use crate::model::{HpWindow, ProviderError, ProviderId, ProviderState, ResetInfo};
use crate::provider::{FetchCtx, Provider};
```
照抄這個 import 順序：std → external (async_trait) → crate::（model 先、provider 後）。

**Provider impl skeleton**（`mock.rs` lines 13-38）：
```rust
pub struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Mock
    }

    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        // CRITICAL: use ctx.now (the injected clock), never a wall-clock read,
        // per RESEARCH Anti-Patterns. Clock-injection contract for testability.
        let resets_at = ctx.now + jiff::Span::new().hours(2);

        Ok(ProviderState {
            id: ProviderId::Mock,
            windows: vec![HpWindow {
                label: Cow::Borrowed("mock-session"),
                percent_remaining: 60.0,
                reset: ResetInfo { resets_at },
                bar_color: None,
            }],
            fetched_at: ctx.now,
            source: Cow::Borrowed("mock"),
        })
    }
}
```
Claude adapter 複製這結構，差異點：
- `ProviderId::Claude` 取代 `Mock`
- `Cow::Borrowed("claude")` source、`Cow::Borrowed("claude-5h")` label
- `ctx.now` 用法不變（NEVER `jiff::Timestamp::now()`，acceptance grep 將要擴張到 `claude.rs` — RESEARCH Anti-Patterns）
- 一律不用 `unwrap()`/`expect()`/`panic!()`（lib.rs lint floor）
- `_ = ctx.secrets;`（SEC-04：拿到 secrets ref 但不讀，contract preserved — RESEARCH Example 4）

**Test pattern**（`mock.rs` lines 40-105）：
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_impl_all;

    assert_impl_all!(MockProvider: Send, Sync);
    assert_impl_all!(Box<dyn Provider>: Send, Sync);

    #[tokio::test]
    #[allow(clippy::default_constructed_unit_structs)]
    async fn mock_returns_expected_shape() {
        let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
        let secrets = crate::secrets::Secrets::default();
        let ctx = FetchCtx { now, secrets: &secrets };
        ...
    }
}
```
Claude adapter test 照抄這 pattern：
- `assert_impl_all!(ClaudeProvider: Send, Sync);` 開頭
- `#[tokio::test]` + 固定 timestamp 字面 + `Secrets::default()` + 在 test 內 `.unwrap()` OK（clippy.toml 已 allow-unwrap-in-tests）
- 「frozen-clock 證明 fetched_at == ctx.now」test（lines 69-81）必複製，作 Claude adapter 的 clock-injection 守護

**約束 planner 要保留：**
- `pub struct ClaudeProvider { base_path: PathBuf, token_limit: u64 }` ←  RESEARCH Example 4。`base_path` 可在 test 注入 `tempfile::tempdir()`。
- 模組布局：`provider/claude/{mod.rs, jsonl.rs, window.rs}`（RESEARCH ARCHITECTURE 提到 3 個 cleanly separable concerns）。`mod.rs` 只放 Provider impl + struct，把 io 與 math 切到 jsonl.rs / window.rs。

---

### `src/provider/claude/jsonl.rs` — JSONL line-stream parser

**Analog:** none in src/ (Phase 0 沒做 file IO). Reference: RESEARCH Example 2（D-35 verbatim sketch）+ `src/model.rs` serde 慣例 + `src/cli/render_text.rs::format_countdown` 的「私有 fn + scoped cast lint」慣例。

**Imports pattern**（mirror `mock.rs` ordering）：
```rust
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Deserialize;
```

**Serde shape pattern**（複製 `model.rs::ProviderError` lines 93-125 的 `#[serde(...)]` 慣例）：
- 用 `#[serde(tag = "type")]` 區分 envelope variant（assistant / user / file-history-snapshot）
- 每個 deserialize-only 欄用 `#[serde(default)]` 容忍缺欄（D-38 forward-compat 精神）
- `#[serde(rename = "assistant")]` snake_case 一致
- NO `#[serde(deny_unknown_fields)]`（D-38 binding + 與 model.rs 慣例對齊）

**Streaming + tolerance pattern**（RESEARCH Example 2 verbatim — D-35 lock）：
```rust
let reader = BufReader::new(f);
let mut lines = reader.lines().peekable();
while let Some(line_res) = lines.next() {
    let line = match line_res { Ok(l) => l, Err(e) => { tracing::warn!(...); continue; } };
    let is_last = lines.peek().is_none();
    match serde_json::from_str::<JsonlEntry>(&line) {
        Ok(JsonlEntry::Assistant(a)) => out.push(a),
        Ok(JsonlEntry::Other) => {}
        Err(_) if is_last => { /* truncated trailing line — silent skip */ }
        Err(e) => { tracing::warn!("malformed jsonl line: {e}"); }
    }
}
```

**約束 planner 要保留：**
- `tracing::warn!` 進 dep 後當 logging primitive（不要 `eprintln!`）
- `File::open(path)?` 失敗 → return `Vec::new()` + `tracing::warn!`，NOT `?` propagate（adapter level 才回 `ProviderError`，io level 容忍）
- **絕對不用** `read_to_string` 然後 `split('\n')` — 大 jsonl 會吃 RAM；RESEARCH "Don't Hand-Roll" 明列禁止
- Token 欄位只反序列化「reliable 4 個」：`input_tokens` / `output_tokens` / `cache_creation_input_tokens` / `cache_read_input_tokens`（後 2 個才真正用作 sum；D-33 amended + L1）

---

### `src/provider/claude/window.rs` — 5h cluster anchor math

**Analog:** `src/cli/render_text.rs::format_countdown` (lines 80-90) — 同樣是「純 jiff Span math + scoped cast lint + Span::new() fallback」 pattern.

**Pattern to copy**（`render_text.rs` lines 80-90）：
```rust
fn format_countdown(now: &jiff::Timestamp, target: &jiff::Timestamp) -> String {
    let span = target
        .since((jiff::Unit::Hour, *now))             // ← Phase 0-03 deviation 1: 顯式 Unit::Hour
        .unwrap_or_else(|_| jiff::Span::new());      // ← B-1: Span::new() fallback, NOT unwrap_or_default
    let h = i64::from(span.get_hours());
    let m = span.get_minutes();
    let h = h.max(0);
    let m = m.max(0);
    format!("{h}h{m:02}m")
}
```

**Pct/u32 cast pattern**（`render_text.rs` lines 59-71，scoped clippy allow）：
```rust
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn filled_cells(pct: f32) -> usize { ... }
```
複製這個「私 fn + lint scope 縮到最小」慣例 — `percent_remaining(used: u64, limit: u64) -> f32` 內部的 `as f32` cast 用同樣 `#[allow]` 包，不要在整 module 上開。

**約束 planner 要保留：**
- 任何 `Timestamp::since` 呼叫 **必須** 帶 `(Unit::Hour, *other)` — Phase 0-03 deviation 1 教訓
- 用 `Span::new()` fallback 而非 `unwrap_or_default()`（B-1 binding）
- `pub const CLAUDE_5H_TOKEN_LIMIT: u64 = 44_000;` 與旁邊 doc comment 註明 "best-effort estimate; revisit quarterly"（D-44）
- 純 fn `find_active_cluster(...)` / `percent_remaining(...)` 留 `pub(crate)` 級別 + 對應 unit test（模 `render_text.rs::tests` 的 byte-exact 風格）

---

### `src/provider/mock.rs` *(modified)* — add panic-injection variant

**Analog:** self (existing file).

**Existing struct**（line 13）：
```rust
pub struct MockProvider;
```

**Pattern: env-var-gated panic injection**（RESEARCH Pitfall L6 + A9 recommendation）：
```rust
impl Provider for MockProvider {
    async fn fetch(&self, ctx: &FetchCtx<'_>) -> Result<ProviderState, ProviderError> {
        if std::env::var_os("AHB_DEBUG_PANIC").as_deref() == Some(std::ffi::OsStr::new("adapter:mock")) {
            #[allow(clippy::panic)]  // intentional fault injection for ADP-01 integration test
            { panic!("AHB_DEBUG_PANIC injected"); }
        }
        // ... existing body ...
    }
}
```
**約束：**
- `#[allow(clippy::panic)]` 必 scope 到那一行 block — lib.rs lint floor 否則會擋
- 環境變數讀取一次即可（不快取）；test integration 在 `assert_cmd::Command::env(...)` 注入
- 不引新 dep；`std::env::var_os` 已在 std

---

### `src/engine/mod.rs` — Engine struct + dispatch

**Analog:** `src/provider/mod.rs` (lines 1-42) — 同樣是「module entry + trait/struct + pub mod 子分」的 shape.

**Module layout pattern**（`provider/mod.rs` line 19）：
```rust
pub mod mock;
```
Engine mod.rs 對等：
```rust
pub mod events;
pub mod fanout;

pub struct Engine { ... }
impl Engine { pub async fn refresh_all(&self) -> Vec<(ProviderId, Result<...>)> { ... } }
```

**FetchCtx 構造 pattern**（`provider/mod.rs` lines 28-32）：
```rust
#[derive(Debug, Clone, Copy)]
pub struct FetchCtx<'a> {
    pub now: jiff::Timestamp,
    pub secrets: &'a Secrets,
}
```
Engine `refresh_all` 內部要把 `&self.providers` + `now` + `&self.secrets` 組成 `FetchCtx`，照 `main.rs` line 69-72 的構造方式。

**約束 planner 要保留：**
- `Vec<(ProviderId, Result<ProviderState, ProviderError>)>` 簽名（Phase 0 已 lock — CONTEXT「Established Patterns」第 1 條）
- Engine 不持有 `tokio::Handle` / `Runtime` — Phase 0 `main.rs` 用 `#[tokio::main(flavor = "current_thread")]`；Phase 1 在 main.rs 升級 features，Engine 本身保持 runtime-agnostic（只 spawn）

---

### `src/engine/fanout.rs` — JoinSet fan-out

**Analog:** none in src/. Reference: RESEARCH Pattern 1 (lines 428-473) — direct verbatim sketch.

**Structural recommendation:**
- 模仿 `provider/mod.rs` 的「外掛 trait + 內掛 impl」分層：把 fan-out 純函數 `pub(crate) async fn refresh_all(providers, ctx, timeout) -> Vec<...>` 放在 fanout.rs，Engine 在 mod.rs 只是 thin wrapper。
- L4 解法：用 `HashMap<tokio::task::Id, ProviderId>` bookkeeping（RESEARCH Recommendation #5 in Open Questions）— `JoinSet::spawn` 回傳 `AbortHandle`，但 task id 從 `JoinError::id()` 取。

**Pattern to use (verbatim from RESEARCH Pattern 1 + L4 fix):**
```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio::time::{timeout, Duration};

use crate::model::{ProviderError, ProviderId, ProviderState};
use crate::provider::{FetchCtx, Provider};

pub async fn refresh_all(
    providers: &[Arc<dyn Provider>],
    ctx: &FetchCtx<'_>,
    per_provider_timeout: Duration,
) -> Vec<(ProviderId, Result<ProviderState, ProviderError>)> {
    let mut set = JoinSet::new();
    let mut id_map: HashMap<tokio::task::Id, ProviderId> = HashMap::new();
    for p in providers {
        let p = p.clone();
        let pid = p.id();
        // Caveat: FetchCtx<'_> is not 'static; planner must either (a) own-the-clock
        // via an OwnedFetchCtx helper, or (b) restructure refresh_all to JoinSet over
        // futures-spawned-tasks rather than spawn-blocking. RESEARCH leaves this open.
        let handle = set.spawn(async move {
            let result = match timeout(per_provider_timeout, p.fetch(/* ctx */)).await {
                Ok(Ok(state)) => Ok(state),
                Ok(Err(e)) => Err(e),
                Err(_elapsed) => Err(ProviderError::Unavailable {
                    reason: format!("timed out after {per_provider_timeout:?}"),
                }),
            };
            (pid, result)
        });
        id_map.insert(handle.id(), pid);
    }
    // drain ... convert JoinError::is_panic() → ProviderError::Internal via id_map
    ...
}
```
**約束 planner 要保留：**
- `JoinError::is_panic()` → `ProviderError::Internal { source: anyhow::anyhow!("adapter panicked: {pid:?}") }`（ADP-01 binding；用 model.rs 既有 `From<anyhow::Error> for ProviderError`，lines 133-137）
- Per-adapter timeout 預設值在 engine config struct（D-29），建議常數 `pub const DEFAULT_PER_PROVIDER_TIMEOUT: Duration = Duration::from_secs(2);`，doc comment 註明 "Phase 1: Claude 本機 IO，2s 足夠；Phase 3 HTTP adapter 會 raise"
- 引 `tokio` 新 features 在 Cargo.toml 加註 phase comment（CONTEXT「Phase 0 dep 最小化」慣例）

---

### `src/engine/events.rs` — `EngineEvent` enum + mpsc plumbing

**Analog:** `src/model.rs::ProviderError` (lines 93-125) — 同樣是「closed enum + serde tag + variant per state」的 shape.

**Enum pattern**（`model.rs` lines 93-125 verbatim shape）：
```rust
#[derive(Debug)]  // NOT Serialize — Phase 1 events 不上 wire
pub enum EngineEvent {
    Refresh(Vec<(ProviderId, Result<ProviderState, ProviderError>)>),
    TickError { source: anyhow::Error },
    Shutdown,
}
```
**約束 planner 要保留：**
- Closed enum（Phase 0 D-08 慣例延續到 EngineEvent）
- mpsc buffer：建議 `mpsc::channel(64)`（CONTEXT discretion）— 一個 const `EVENT_BUFFER: usize = 64;` 放這裡
- channel-not-mutex（ARCHITECTURE.md Anti-Pattern 3）— 不要塞 `Arc<Mutex<...>>`

---

### `src/config.rs` — TOML loader + first-run init

**Analog:** `src/secrets.rs`（self-shape — Phase 0 stub 演進範本）+ `src/main.rs` Cli derive 結構.

**Phase 0-stub → Phase 1-real evolution pattern**（`secrets.rs` 全檔）：
```rust
//! Phase 0 stub. Phase 1 (per CONTEXT `canonical_refs`) wires `keyring-core` 1.0 +
//! `Secret<T>` newtype + `#[serde(skip)]` redaction. Keep the type empty so
//! `FetchCtx<'_>`'s `&Secrets` reference can be constructed without ABI breakage
//! when Phase 1 widens it.

#[derive(Debug, Default, Clone)]
pub struct Secrets;
```
複製「module 開頭 doc comment 寫清 Phase 演進邊界 + Phase 0/1/2/3 哪一階段 lock 什麼」的慣例。Config 對等：
```rust
//! Config loader. Phase 1: TOML schema per CONTEXT D-36, ProjectDirs path per D-39,
//! first-run auto-create per D-37, unknown-key warn-and-ignore per D-38. Phase 3
//! adds CFG-03 (per-provider refresh_interval / auth_source); the struct here
//! holds the surface those phases extend.
```

**Struct + derive pattern**（mirror `main.rs` Cli, lines 22-35）：
```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: Providers,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Providers {
    #[serde(default)]
    pub claude: ProviderConfig,
    #[serde(default)]
    pub codex: ProviderConfig,
    #[serde(default)]
    pub gemini: ProviderConfig,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub enabled: bool,
}
```
**重要**：**不要** `#[serde(deny_unknown_fields)]`（D-38）— forward-compat 與 model.rs 慣例對齊。

**First-run pattern (D-37):**
```rust
const DEFAULT_CONFIG: &str = include_str!("../templates/default-config.toml");

pub fn load_or_init(path: &Path) -> anyhow::Result<Config> {
    if !path.exists() {
        if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
        std::fs::write(path, DEFAULT_CONFIG)?;
        println!("initialized {} — enable providers and rerun", path.display());
        std::process::exit(0);
    }
    let text = std::fs::read_to_string(path)?;
    // D-38: unknown-key warn — toml's Deserializer doesn't surface unknown keys by default
    // when deny_unknown_fields is off; planner uses toml::Value pre-pass + diff to detect.
    let cfg: Config = toml::from_str(&text)?;
    Ok(cfg)
}
```
**約束 planner 要保留：**
- `std::process::exit(0)` 在 first-run 路徑可接受（D-37 字面 + pipe-safe — 不互動 wizard）— 但 main.rs 內部要走 anyhow::Result 慣例，所以 first-run 路徑用 `exit()` 早 return；非 first-run 用 `?` propagate
- `directories::ProjectDirs::from("", "", "ahb")`（D-39 三 arg 字面 + RESEARCH Pitfall L5：6.0 latest，三 arg 簽名穩）
- `include_str!("../templates/default-config.toml")` 字面（D-37）— `src/templates/` 必須在 `src/` 下才能被 `include_str!` 從 `src/config.rs` 看見
- Unknown-key warn 機制（D-38）：建議用 `toml::Value` 解析一遍取頂層 keys、與已知 keys diff，差集走 `tracing::warn!("unrecognized config key '{key}' — see README")`

---

### `src/secrets.rs` *(modified)* — keyring-core wire + Secret<T>

**Analog:** self (Phase 0 stub).

**Phase 0 → Phase 1 演進範本** — 模 model.rs 既有 doc-comment 寫法 + secrets.rs 開頭已預告的「Phase 1 widens it」承諾。

**Secret<T> pattern (D-42 verbatim — RESEARCH Pattern 6 lines 626-657):**
```rust
use std::fmt;
use serde::{Serialize, Serializer};
use zeroize::Zeroize;

pub struct Secret<T: Zeroize + Clone>(T);

impl<T: Zeroize + Clone> Drop for Secret<T> {
    fn drop(&mut self) { self.0.zeroize(); }
}

impl<T: Zeroize + Clone> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

impl<T: Zeroize + Clone> Serialize for Secret<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("[REDACTED]")
    }
}
// 不實作 Deserialize（D-42 binding：secret 從 keyring，不從 toml/json）

impl<T: Zeroize + Clone> Secret<T> {
    pub fn new(inner: T) -> Self { Self(inner) }
    pub fn expose(&self) -> &T { &self.0 }
}
```

**Secrets struct evolution**（保留 `Secrets::default()` API surface — `main.rs` line 67 + `provider/mod.rs` tests 都依賴）：
```rust
#[derive(Debug, Default, Clone)]
pub struct Secrets;  // Phase 1: API surface 不變，內部 store 改由 keyring-core 默默接管

impl Secrets {
    pub fn new() -> Self { Self }
    // future: pub fn get(&self, provider: ProviderId) -> Option<Secret<String>>
}
```

**keyring-core 1.0 bootstrap pattern**（RESEARCH Pattern 5 lines 583-620）：
```rust
#[cfg(target_os = "linux")]
fn make_default_store() -> Result<Box<dyn keyring_core::api::CredentialStore>, anyhow::Error> {
    Ok(Box::new(dbus_secret_service_keyring_store::Store::new("ahb")?))
}
// + macos / windows cfg-gated variants
```
**約束 planner 要保留：**
- `Secret<T>`、`Secrets` 二者 export；`Secrets::default()` 簽名不變（不破 mock.rs/tests:54、provider/mod.rs/tests:58/69、main.rs:67 三個既有依賴點）
- 新 dep：`zeroize` + `keyring-core` + 一個 OS-specific `*-keyring-store`（用 `cfg(target_os = ...)` 在 Cargo.toml `[target."cfg(...)".dependencies]` 分區）
- D-41 hard-error 字面字串 verbatim：`no secret store available on this system; set [secrets].storage = "file" in ~/.config/ahb/config.toml to opt into 0600 file storage`
- A2（slopsquat 風險）：planner 必須在加 `*-keyring-store` 前 verify crates.io repository 欄指向 `github.com/open-source-cooperative/*` — `checkpoint:human-verify` gate

---

### `src/tui/mod.rs` — `pub async fn run(engine) -> Result<()>`

**Analog:** `src/main.rs` (lines 56-83) — 同樣是「async entry + early-exit + anyhow::Result」慣例.

**Entry pattern**（`main.rs` line 58-83）：
```rust
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    install_phase0_panic_hook();
    // ... early-exit branches ...
    // ... main body ...
    Ok(())
}
```

**TUI entry pattern (RESEARCH Pattern 2 + UI-SPEC TUI-05):**
```rust
use std::io::IsTerminal;
use ratatui::DefaultTerminal;
use tokio::time::{interval, Duration};

pub async fn run(engine: crate::engine::Engine) -> anyhow::Result<()> {
    // TUI-05 verbatim copy (UI-SPEC LOCKED):
    if !std::io::stdout().is_terminal() {
        eprintln!(
            "AHB tui requires a terminal (stdout is not a TTY). \
             Run AHB without 'tui' for piped / non-interactive output."
        );
        std::process::exit(2);
    }

    // ratatui::run installs its panic hook ON TOP OF Phase 0's existing hook.
    // Chain order on panic: ratatui (restore terminal) → Phase 0 (eprintln) → default.
    ratatui::run(|terminal: &mut DefaultTerminal| async move {
        let mut app = crate::tui::app::AppState::new(&engine);
        let mut events = ratatui::crossterm::event::EventStream::new();
        let mut fetch_tick = interval(Duration::from_secs(15));
        let mut render_tick = interval(Duration::from_secs(1));
        app.apply_results(engine.refresh_all().await);
        loop {
            tokio::select! {
                Some(Ok(ev)) = futures_util::StreamExt::next(&mut events) => {
                    if app.handle_event(ev) { break; }
                }
                _ = fetch_tick.tick() => {
                    app.apply_results(engine.refresh_all().await);
                }
                _ = render_tick.tick() => {
                    terminal.draw(|f| crate::tui::ui::draw(f, &app))?;
                }
            }
        }
        Ok::<_, anyhow::Error>(())
    }).await
}
```
**約束 planner 要保留：**
- 用 `ratatui::run()` **NOT** `init()` + `restore()`（RESEARCH Pitfall L2 — 此 Pitfall 直接 contradict 既有 PITFALLS.md 與 STACK.md 的 init/restore 寫法，planner 必須走 run）
- `ratatui::crossterm::event::EventStream`（**不引 crossterm direct dep**；用 ratatui re-export — clippy.toml lines 8-11 已 disallowed `crossterm::*` types）
- `std::io::IsTerminal` 是 std 1.70+（MSRV 1.88 — 安全）
- 退出字面（UI-SPEC LOCKED）verbatim 不改字
- 在 `Cargo.toml` 新加 `futures-util`（StreamExt）— 若 planner 寧可不引、可改用 `events.next().await`（ratatui 0.30 直接提供 async EventStream，verify at impl time）

---

### `src/tui/app.rs` — `AppState` cache

**Analog:** `src/model.rs::ProviderState` (lines 61-70) — 同樣是「pub struct + 容器 + serde-free 但 Clone-able」.

**Pattern:**
```rust
#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub rows: Vec<RowState>,  // 每 provider 一 row 的最新快照
}

#[derive(Debug, Clone)]
pub enum RowState {
    Ok(ProviderState),
    Err { id: ProviderId, message: String },          // per-row error 字面在 ui.rs 組
    SchemaDrift { id: ProviderId },                   // 觸發 sentinel 字面
}
```
**約束：**
- 不持有 `Engine` ref（channel-not-mutex）— 由 fetch_tick 從 mpsc/直接呼叫 engine 後 push 進 `AppState`
- `handle_event(&mut self, ev: ratatui::crossterm::event::Event) -> bool` 回 `true` 代表 quit（`q` / `Ctrl-C` 兩條 — UI-SPEC interaction table）

---

### `src/tui/ui.rs` — ratatui Frame draw

**Analog:** `src/cli/render_text.rs::compact_line` (lines 25-57) — 同樣是「`&ProviderState` → 視覺輸出 + scoped cast + 固定 width grid」.

**Pattern to copy from `render_text.rs` lines 33-47** — bar fill cell count + ascii toggle:
```rust
let bar = if ascii {
    format!("{}{}", "#".repeat(filled), "-".repeat(BAR_WIDTH - filled))
} else {
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(BAR_WIDTH - filled))
};
```
TUI 在 ui.rs 內把這個改成 ratatui Spans：filled 段套 `Style::default().fg(Color::Green/Yellow/Red)`、empty 段套 `Style::default().fg(Color::DarkGray)`（UI-SPEC Color section 60/30/10 對應）。

**Layout pattern (UI-SPEC TUI fixed-view + Layout::vertical):**
```rust
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::{Block, Borders};

pub fn draw(f: &mut ratatui::Frame, app: &AppState) {
    let chunks = Layout::vertical([
        Constraint::Length(1),                          // top pad
        Constraint::Min(app.rows.len() as u16),         // provider rows
        Constraint::Length(1),                          // footer pad
        Constraint::Length(1),                          // quit hint
    ]).split(f.area());

    let outer = Block::default().title(" AHB ").borders(Borders::ALL);
    f.render_widget(outer, f.area());
    // ... render each row via widgets::hp_row::render(...)
    // quit hint at chunks[3]: "q quit  ·  ctrl-c quit" in Color::DarkGray (UI-SPEC LOCKED)
}
```
**約束 planner 要保留：**
- Border 用 `Block::default().borders(Borders::ALL)` — UI-SPEC LOCKED；不要自己畫 box-drawing chars（typography section banned 直接 emit U+2500-U+257F）
- 顏色閾值 verbatim：`Green when percent ≥ 30.0`、`Yellow when 10.0 ≤ percent < 30.0`、`Red when percent < 10.0`（UI-SPEC Color section）
- `BarColor` 若 adapter 有提供 hint（`HpWindow::bar_color`），override percent-based default
- Schema-drift sentinel 字面 verbatim：`claude  ▒▒▒▒▒▒▒▒▒▒ ??% • Claude adapter may be out-of-date`（U+2592 not U+2591 — UI-SPEC distinguishes）
- 不引 `owo-colors`（既有 Phase 0 dep）— TUI 路徑只用 ratatui::Style；雙 coloring crate 是 double-escape 風險（UI-SPEC color source note）

---

### `src/tui/widgets/hp_row.rs` — 一 row 的 widget impl

**Analog:** `src/cli/render_text.rs` (整檔結構：pub fn render + 內掛 helper fn + 內掛 tests).

**Pattern:**
- 一個 `pub fn render(area: Rect, buf: &mut Buffer, row: &RowState, ascii: bool, color_on: bool)`，或實作 `Widget` trait
- 內部 helper fn `fn build_spans(...)` 模仿 `render_text.rs::filled_cells` 那種「小 fn + scoped lint」結構
- Tests 模 `render_text.rs::tests` 的 byte-exact 風格（ratatui `TestBackend` snapshot — insta crate）

**約束：**
- 為了 snapshot 測試穩定，build_spans 必須 deterministic（時間從 `ctx.now` 傳進來，不查 `Timestamp::now()`）
- 暫不引 `insta`（test dep）若 planner 評估 byte-exact assert 已夠用；CONTEXT discretion

---

### `src/cli/mod.rs` *(modified)* — add `tui` subcommand

**Analog:** `src/main.rs` Cli derive (lines 22-35).

**Pattern to extend**（`main.rs` 既有 Cli struct）：
```rust
#[derive(clap::Parser)]
#[command(version, about = "AHB — AI HP Bar — multi-CLI subscription session usage at a glance")]
pub struct Cli {
    #[arg(long)] pub ascii: bool,
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)] pub color: ColorMode,
    #[command(subcommand)] pub command: Option<Command>,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Launch the fixed-frame TUI.
    Tui,
}
```
**約束 planner 要保留：**
- `Option<Command>` — 不帶 subcommand 走 default compact path（CORE-01）；帶 `tui` 走 TUI（TUI-01）
- ColorMode enum 從 main.rs 移到 cli/mod.rs（pub re-export 給 main.rs）— 模組化邊界整齊
- `--ascii` flag forward 到 render_text + tui::ui 兩條路徑
- 此 file 從 lib.rs 已 export（`pub mod cli;` 既存）— 不破 import path

---

### `src/cli/tty.rs` — TTY + color decision

**Analog:** `src/cli/render_text.rs::filled_cells` (lines 61-65) — 同樣是「小 pure fn + scoped clippy allow + 完整 unit test」.

**Pattern (RESEARCH Pattern 4 verbatim — UI-SPEC color-off paths binding):**
```rust
use std::io::IsTerminal;

pub fn should_colorize(cli_flag: ColorMode, json_mode: bool) -> bool {
    if json_mode { return false; }
    match cli_flag {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            std::env::var_os("NO_COLOR").is_none()
                && std::io::stdout().is_terminal()
        }
    }
}
```
**約束：**
- 四條 color-off paths（UI-SPEC binding）：`--json` / `--color=never` / `NO_COLOR` / non-TTY — 每條都要對應 unit test（模 `render_text.rs::tests::bar_width_fixed_at_ten` 那種多 case 一 test）

---

### `src/main.rs` *(modified)* — dispatch + tracing init

**Analog:** self (Phase 0).

**Existing pattern to preserve**（lines 56-83）：
- `install_phase0_panic_hook()` 必是 **第一行** — D-27 contractual（RESEARCH Pitfall L7：然後才 init tracing，順序不可反）
- `#[tokio::main(flavor = "current_thread")]` 與 Phase 1 升級到 multi-thread features 並不衝突（runtime flavor 是另一回事；feature 只是讓 Engine 內部可選擇 spawn 到 multi-thread）
- `Cli::parse()` 既有
- `Secrets::default()` 與 `FetchCtx { now, secrets: &secrets }` 既有構造

**Extension pattern:**
```rust
fn main() -> anyhow::Result<()> {  // tokio::main wrap unchanged
    install_phase0_panic_hook();                                   // FIRST — contract
    tracing_subscriber::fmt()                                       // SECOND — RESEARCH Pitfall L7
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = ahb::cli::Cli::parse();
    let config = ahb::config::load_or_init(&config_path())?;       // D-37 may exit(0)
    let secrets = ahb::secrets::init()?;                            // D-41 may exit(2)
    let engine = ahb::engine::Engine::new(config, secrets);
    match cli.command {
        Some(ahb::cli::Command::Tui) => ahb::tui::run(engine).await,
        None => ahb::cli::run_compact(engine, cli.ascii, cli.color).await,
    }
}
```
**約束 planner 要保留：**
- Line 68 `jiff::Timestamp::now()` 是 wall-clock 唯一 caller — Phase 1 引入 Engine 後，Engine 內部 refresh tick 仍從 main.rs 注入 `now`（每次 fetch_tick 呼叫前 `let now = jiff::Timestamp::now(); let ctx = FetchCtx { now, secrets };`），保 acceptance grep 通過
- `tokio::main(flavor = "current_thread")` 升級為 `tokio::main` 預設 multi-thread + runtime feature `rt-multi-thread`（CONTEXT「Established Patterns」第 5 點）
- `_ = cli.color;` 那行（line 65）刪除 — Phase 1 真的 wire 進 render

---

### `src/templates/default-config.toml` — embedded fixture

**Analog:** none. Content verbatim from CONTEXT `<specifics>` section 第 6 個 bullet.

**File content (verbatim):**
```toml
# AHB — AI HP Bar config. Enable the providers you have subscriptions for.
#
# Run `AHB` after editing — uncovered providers are silently skipped (not flagged as failures).

[providers.claude]
enabled = false  # Claude Code subscription — reads ~/.claude/projects/**/*.jsonl

[providers.codex]
enabled = false  # Codex CLI subscription — not yet implemented (Phase 2)

[providers.gemini]
enabled = false  # Gemini CLI subscription — not yet implemented (Phase 3)
```
**約束：** 字面字字 lock。`include_str!` 從 `src/config.rs` reach `src/templates/default-config.toml`（相對路徑 `../templates/...`）— Cargo 預設 `src/` build root，include_str 用 src-relative path。

---

### Integration tests (`tests/*.rs`)

**Analog:** `src/provider/mock.rs::tests` (lines 40-105) + `src/cli/render_text.rs::tests` (lines 92-204).

**Test file 結構 pattern**（複製 mock.rs tests 那種「frozen-clock + Secrets::default + 字面 assert」）：
```rust
#[tokio::test]
async fn claude_adapter_real_fixture() {
    let tmp = tempfile::tempdir().unwrap();
    // ... write fake ~/.claude/projects/<slug>/<uuid>.jsonl using verbatim schema from RESEARCH Example 3 ...
    let now: jiff::Timestamp = "2026-05-23T12:00:00Z".parse().unwrap();
    let provider = ClaudeProvider::new(tmp.path(), CLAUDE_5H_TOKEN_LIMIT);
    let secrets = Secrets::default();
    let ctx = FetchCtx { now, secrets: &secrets };
    let state = provider.fetch(&ctx).await.unwrap();
    assert_eq!(state.id, ProviderId::Claude);
    // ... percent + reset assertions ...
}
```

**Integration tests planner 必須 ship（RESEARCH + CONTEXT 鎖定）：**
1. **Panic-injection test** (`tests/panic_injection.rs`) — `assert_cmd::Command::cargo_bin("ahb").env("AHB_DEBUG_PANIC", "adapter:mock")` 跑、assert 其他 providers 仍 render、退出 code 非 0、stderr 含 `ahb panicked:`（Phase 0 hook 字面）— ADP-01 binding
2. **Non-TTY refusal** (`tests/tui_non_tty.rs`) — `assert_cmd::Command::cargo_bin("ahb").arg("tui").write_stdin("").assert().code(2).stderr(predicates::str::contains("AHB tui requires a terminal"))` — TUI-05
3. **Secret-leak grep** (`tests/secret_leak.rs`) — D-43 verbatim：構造 `Secret::new("deadbeefcafe1234567890abcdef".to_string())`、雙路徑 `Debug` + `serde_json::to_string`、雙 assert（literal 缺席 + 20-char alphanumeric regex 不匹配）— SEC-02 + SEC-04
4. **Schema-drift sentinel** (`tests/schema_drift.rs`) — 喂老 JSONL（少 `cache_creation_input_tokens` 欄）給 ClaudeProvider、走 render，assert 字面 `claude  ▒▒▒▒▒▒▒▒▒▒ ??% • Claude adapter may be out-of-date` — ADP-03
5. **First-run init** (`tests/first_run.rs`) — empty config dir、跑 `ahb`、assert stdout 含 `initialized ... — enable providers and rerun`、exit code 0、檢 config dir 真的有檔 — D-37

**約束：**
- 全 integration test 用 `assert_cmd` + `predicates`（dev-dep）+ `tempfile`（dev-dep）
- 不在 `tests/` 內讀真 `~/.claude/projects/`（CI cross-OS — Windows / macOS runners 沒 fixture）；一律 `tempfile::tempdir()` 構造 fake
- regex test dep 只進 `[dev-dependencies]`（D-43 secret-leak test）

---

## Shared Patterns（跨多個新 file 適用）

### Crate-root lint floor — 自動繼承
**Source:** `src/lib.rs` lines 7-8 + `src/main.rs` lines 10-11
```rust
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]
```
**Apply to:** 每個 Phase 1 新 module（`engine/mod.rs`、`engine/fanout.rs`、`engine/events.rs`、`provider/claude/mod.rs`、`provider/claude/jsonl.rs`、`provider/claude/window.rs`、`config.rs`、`tui/mod.rs`、`tui/app.rs`、`tui/ui.rs`、`tui/widgets/hp_row.rs`、`cli/tty.rs`）。
**規矩：**
- 每檔開頭都要這 2 行 deny + warn（與既有 lib.rs / main.rs / 各既有 file 對齊）
- 任何 `unwrap`/`expect`/`panic` 要用 scoped `#[allow]` + comment 解釋為何（如 mock.rs 加 panic-injection 那條）
- Tests 內 `unwrap` OK（clippy.toml 已 `allow-unwrap-in-tests = true`）

### Disallowed-types — clippy gate
**Source:** `clippy.toml` lines 8-11
```toml
disallowed-types = [
    { path = "crossterm::event::Event", reason = "use owo-colors (Phase 0) or ratatui::crossterm (Phase 1+) — see PITFALLS.md double-version hazard" },
    { path = "crossterm::style::Color", reason = "..." },
]
```
**Apply to:** TUI module group（`tui/*.rs`）。
**規矩：**
- 直接 `use crossterm::...` 一律走 ratatui re-export：`use ratatui::crossterm::event::Event;`
- Phase 1 加入 ratatui 後可能要擴充 `disallowed-types` list（如 `crossterm::terminal::*`、`crossterm::event::EventStream`）— planner 評估是否要加
- `Cargo.toml` 絕對不直接 `cargo add crossterm`（CONTEXT「Established Patterns」第 6 點）

### Acceptance grep — Wall-clock injection
**Source:** Phase 0 contract + CONTEXT「Established Patterns」第 2 條
- 唯一 `jiff::Timestamp::now()` callsite 是 `src/main.rs` line 68（Phase 1 在 main.rs 內 fetch_tick 之前可能還會多呼叫，但仍只在 main.rs）
- 所有 adapter (`provider/claude/**/*.rs`) **不可** `Timestamp::now()` — 必須 `ctx.now`
- Phase 1 acceptance test 要加：`tests/no_walltime_in_adapter.rs` 用 `grep -r "Timestamp::now" src/provider/` 應只 match 註解，不 match code（或更嚴格：grep src/provider/ 整段為空字串）

### Cargo.toml dep comment 慣例
**Source:** `Cargo.toml` line 16-17
```toml
# Phase 0 dep minimalism — features kept to ["rt", "macros"]; Phase 1 may upgrade to "rt-multi-thread" when first network adapter ships.
tokio = { version = "1.52", default-features = false, features = ["rt", "macros"] }
```
**Apply to:** Phase 1 新加的每個 dep。
**規矩：** 每加一個 dep 在 `Cargo.toml` 上方註明 phase 來源（CONTEXT「Established Patterns」第 5 點），例：
```toml
# Phase 1 — TUI rendering (REQ TUI-01..05); pulls crossterm 0.29 transitively.
# DO NOT add `crossterm` directly — use ratatui::crossterm re-export (PITFALLS.md double-version).
ratatui = "0.30"

# Phase 1 — JSONL file discovery for Claude adapter (REQ ADP-02).
glob = "0.3"
```

### Serde shape — closed enum + snake_case
**Source:** `model.rs::ProviderId` (lines 17-24) + `model.rs::ProviderError` (lines 93-125)
- 所有 enum：`#[serde(rename_all = "snake_case")]`
- 不開 `#[serde(deny_unknown_fields)]`（D-38）
- Internally-tagged enum 用 `#[serde(tag = "kind")]` / `#[serde(tag = "type")]`
**Apply to:** `engine/events.rs` 若加 Serialize、`provider/claude/jsonl.rs` 的 `JsonlEntry` envelope enum、`config.rs` 的所有 Deserialize struct

### Test 風格 — frozen clock + Secrets::default
**Source:** `provider/mock.rs::tests::mock_returns_expected_shape` (lines 51-67)
```rust
let now: jiff::Timestamp = "2026-05-22T12:00:00Z".parse().unwrap();
let secrets = crate::secrets::Secrets::default();
let ctx = FetchCtx { now, secrets: &secrets };
```
**Apply to:** Claude adapter tests、engine tests、tui app/state tests（凡是要構造 FetchCtx 的 test）
**規矩：** 固定字面 timestamp、`Secrets::default()`、`.unwrap()` 在 test 內 OK（clippy.toml allow）

---

## No Analog Found

下列檔在 src/ 沒 close match — planner 用 RESEARCH 內 Code Example / Pattern 取代。

| File | Role | Data Flow | Reason | Substitute reference |
|---|---|---|---|---|
| `src/engine/fanout.rs` | service | JoinSet fan-out | Phase 0 沒做 concurrency | RESEARCH Pattern 1 (lines 428-473) + L4 fix |
| `src/provider/claude/jsonl.rs` | utility | streaming file-IO | Phase 0 沒做 file IO | RESEARCH Code Example 2 (lines 845-917) — D-35 verbatim |
| `src/tui/mod.rs` | controller | event-driven select! loop | Phase 0 沒做 TUI | RESEARCH Pattern 2 (lines 484-531) — ratatui::run |
| `src/tui/ui.rs` | renderer | ratatui Frame composition | Phase 0 沒做 ratatui | UI-SPEC Layout & Interaction section + ratatui 0.30 docs（Context7 lookup at impl） |
| `src/tui/widgets/hp_row.rs` | renderer | Widget impl | Phase 0 沒做 ratatui | ratatui 0.30 Widget trait docs |
| `src/templates/default-config.toml` | config asset | static | embedded fixture | CONTEXT `<specifics>` 第 6 bullet — verbatim |

---

## Metadata

**Analog search scope:** `/home/chasel/REPO/AIHPBar/src/` 全樹（5 dirs、8 source files、~26 KB code）
**Files scanned:** 8 src files + 1 Cargo.toml + 1 clippy.toml + 1 ci.yml = 11
**Pattern extraction date:** 2026-05-23
**Phase 0 lock-ins inherited:** D-08..D-19 (model.rs / charset / 視覺), D-25 (mock format), D-27 (panic hook composition contract)
**Phase 1 lock-ins this map preserves:** D-28..D-44 (decisions), UI-SPEC all 6 dimensions
