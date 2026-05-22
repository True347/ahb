# Feature Research

**Domain:** Multi-provider LLM-CLI subscription session usage / reset-countdown tool (CLI + TUI)
**Researched:** 2026-05-22
**Confidence:** HIGH (Claude Code ecosystem — many real tools verified); MEDIUM (Codex/Gemini coverage — fewer tools, but provider docs verified); LOW (no existing multi-provider HP-bar tool found — competitive landscape is *single-provider*-heavy)

## Competitive Landscape Summary

The space is heterogeneous and **strongly fragmented per provider**:

- **Claude Code single-provider:** ccusage, Claude-Code-Usage-Monitor (Maciek-roboblog), ccburn, ccflare, claude-code-usage-bar (leeguooooo), claude-statusbar, ccstatusline, ClaudeGod, usage-monitor-for-claude (jens-duttke), claude-code-statusline (ohugonnot). Dense ecosystem; most read `~/.claude/projects/*.jsonl` and reconstruct usage.
- **Codex CLI:** codex-cli-usage (PyPI); native `/status` slash command and opt-in `tui.status_line` items (`five-hour-limit`, `weekly-limit`); `/backend-api/codex/usage` endpoint is the upstream source.
- **Gemini CLI:** native `/stats` command (current-session only, **not** subscription session/reset); b3nw/gemini-cli-usage extension monitors Cloud API quotas; no widely-used personal-subscription tracker exists.
- **Multi-provider:** code-notify (mylee04) — desktop notifications for Claude/Codex/Gemini, but **notifications-only, not usage display**. ai-cli-complete-notify (ZekerTop) — same shape, broader channels. Operator (untra) — multi-agent orchestration TUI, not usage-focused. **No tool found that unifies subscription session HP-bar across all three.**
- **Generic LLM observability** (Langfuse, Helicone, LLM Gateway, WhaTap) is **API-key/proxy-based**, not subscription-session-based, and lives in a web dashboard — wrong shape for AHB.

**Key insight for AHB:** The *single-provider* feature bar is set high by Claude Code tools (themes, daemons, JSON, statusline hooks). The *multi-provider* feature bar is essentially empty for subscription-session-with-reset use case. **AHB's white space is the cross-provider unified HP-bar**, not feature richness within any one provider.

## Feature Landscape

### Table Stakes (Users Expect These)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Live progress bar (% used) per tracked window | Every Claude Code monitor has one; HP-bar concept is the product | LOW | Maps to v1 req: "緊湊一行 HP bar 含 %". ratatui `Gauge` widget. |
| Reset countdown ("Xh Ym left") per window | Every tool shows it; users plan work around it | LOW | Maps to v1 req: "reset 倒數". Render once, refresh on tick. |
| One-line compact CLI output (pipeable) | ccusage `statusline`, ccburn `--compact`, leeguooooo single-line — universal pattern | LOW | Maps to v1 req: default `AHB` no-arg behavior. |
| `--json` output flag | ccusage, ccburn, claude-statusbar all expose JSON; users pipe into tmux/Starship | LOW | Maps to v1 req: `--json` flag. Stable schema is the real work. |
| `--detailed` / verbose mode | Maciek-roboblog Realtime View, ccusage daily — users want details on demand | LOW | Maps to v1 req: `--detailed` flag. |
| Configurable refresh interval | Maciek-roboblog `--refresh-rate`, ccusage refresh, claude-code-usage-bar daemon tick | LOW | Maps to v1 req: TUI 15s default, configurable via config. |
| TUI fixed screen with auto-refresh | Maciek-roboblog, ccburn, Claude-Code-Usage-Monitor — TUI is the dashboard pattern | MEDIUM | Maps to v1 req: `AHB tui`. ratatui main loop + tick. |
| Config file for tracked providers + auth | Every multi-source tool needs this; ccusage `~/.config/ccusage/ccusage.json`, claude-monitor flags | MEDIUM | Maps to v1 req: config file enumerating providers. TOML expected for Rust. |
| Auto-detection of available CLIs | ccusage auto-detects 10+ agents from local files; expected behavior | MEDIUM | Probe `~/.claude/`, `~/.codex/`, `gemini` auth state. Degrade gracefully if missing. |
| Color output with auto-dark-detection | All TUI tools do this; terminal background detection is standard | LOW | crate: `terminal_light`. |
| `--no-color` / `NO_COLOR` env respect | Standard CLI hygiene; users on CI/log capture expect it | LOW | Trivial; honor `NO_COLOR` env. |
| Single static binary distribution | `cargo install` is acceptable; release artifacts for download expected | LOW | Maps to v1 req. Rust gives this for free; `cross` for multi-arch. |
| Graceful degradation when a provider unreachable | Tools show "?" or grey-out when data missing; never crash entire dashboard | MEDIUM | Per-adapter `Result<Snapshot, AdapterError>`. UI renders error state per row. |
| Privacy: 100% local, no telemetry | Maciek-roboblog "fully local", jens-duttke "fully auditable" — community baseline | LOW | Hard-rule: no network except provider-direct (Gemini HTTP). |

### Differentiators (Competitive Advantage)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Unified multi-provider HP-bar in one view** | **No existing tool does Claude+Codex+Gemini together** — this *is* the product | MEDIUM | Per-adapter trait → unified `ProviderSnapshot` → single render. v1 core. |
| **Reset-aware visual metaphor** (HP bar drains, then refills at reset) | Mental model superiority over "only-goes-up" cost dashboards (which AHB explicitly rejects) | LOW | Pure rendering choice — color shifts as % rises, hits 100, snaps back at reset. |
| Per-provider window granularity (5h + 7d both shown) | ccburn shows session + weekly + weekly-Sonnet; users need both to plan | MEDIUM | UI: stacked bars per provider; data model: list of windows per snapshot. |
| Adapter trait abstraction (extensible to v2 providers) | Future-proofs against Cursor, Windsurf, Amp, etc. — community PRs likely | MEDIUM | Trait `Provider { snapshot() -> Result<Snapshot> }`. Document for contributors. |
| Pace indicator (🧊 behind / 🔥 on-pace / 🚨 too-hot) | ccburn invented this UX; users say it's the killer feature for behavioral nudging | LOW | Burn-rate calc: `% used / % time elapsed in window`. |
| Plain-ASCII fallback mode | jens-duttke, ccusage compact mode for narrow terminals (<100 cols); accessibility for non-unicode terminals | LOW | Branch on `terminal_size` + `--ascii` flag. |
| Stable JSON schema with `schema_version` field | Differentiator: enables 3rd-party Starship/tmux modules built on AHB output | LOW | Serde struct, semver the schema separately from the binary. |
| Statusline-hook compatibility (Claude Code / Codex `tui.status_line`) | Users already integrate ccusage/ccburn into Claude Code's own status line — AHB should drop in there too | MEDIUM | Accept stdin JSON (Claude Code hook contract); emit compact line on stdout. |
| Tmux/Starship integration recipes documented | Doc, not code — but every successful CLI tool ships these | LOW | README section with copy-paste snippets. Cost: docs only. |
| Color-blind friendly palette | Maciek-roboblog markets WCAG; differentiator vs ccburn-style emoji-only | LOW | Two palettes shipped; auto-detect via env or config. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Cumulative cost / token dashboard** (ccusage's whole shape) | Users love ccusage's daily/monthly tables; tempting to copy | Violates AHB Core Value (HP-bar is *reset-based*, not only-goes-up); doubles surface area; ccusage already does this excellently | **Explicit Out of Scope.** Link to ccusage in README for users who want that shape. |
| ML/P90 prediction of limit (Maciek-roboblog headline feature) | Sounds smart; "predicts when you'll hit limit" | Cross-provider prediction requires history schema we don't otherwise need; the actual `% remaining` already tells user what to do; adds a stats library + persistence layer | Display burn-rate as plain ratio (pace indicator covers the value); leave depletion ETA derivable from JSON output. |
| Historical trend graphs / daily/monthly aggregation | "I want to see my usage over the week" | Requires persistent storage (SQLite like ccburn); turns AHB into a different product; orthogonal to "current HP" | Stay snapshot-only. If trend needed, point at ccusage. |
| Desktop notifications when limit approaching | code-notify exists for this; users assume any monitor includes it | OS-specific code paths (Notify on Linux, NSUserNotification on macOS, Windows toasts), permission handling, daemon mode required to fire when AHB isn't open — large scope creep | Provide thresholds in JSON output → users wire into their own notifier. Compose, don't bundle. |
| Daemon / background service mode | claude-code-usage-bar v3.6 default-on; "1% CPU vs 3%" | v1 doesn't need it — refresh is 15s, not sub-second; daemon adds IPC, PID files, crash recovery, install/uninstall semantics | Inline refresh is fine for v1. Revisit if user reports CPU pain. PROJECT.md already notes "未來 daemon 模式好擴". |
| Multi-account / team / org management | "I have personal + work Claude accounts" | Hard "Out of Scope" in PROJECT.md; opens user-management can of worms | One config = one account set. Users with two accounts use two config files: `AHB --config work.toml`. |
| Web dashboard | Familiar; LLM observability tools do this | Wrong form factor — AHB is CLI/TUI by definition; HTTP server, browser UI, port management = different product | TUI is the persistent display. README says so. |
| API key / API-quota tracking | Confusion with subscription tracking; users will ask | Hard "Out of Scope" in PROJECT.md — different use case (per-request billing, not session windows) | Document the distinction in README FAQ. Point API-quota users at provider dashboards. |
| GUI desktop app / menu bar app | ClaudeGod, jens-duttke do this for Claude only | Out of Scope per PROJECT.md; multiplies dist surface (macOS bundle, Windows EXE, Linux .deb), kills Rust single-binary win | TUI mode is the always-on display. `AHB tui` running in tmux pane = same outcome, lower cost. |
| Streaming / OBS overlay | "HP bar" naming evokes gamer overlay | Out of Scope per PROJECT.md | n/a |
| Plan auto-detection (Pro/Max5/Max20 inference) | Maciek-roboblog markets this heavily | Provider limit data already includes plan info or % directly; inferring from token counts is fragile and only helps Claude | Read declared limits from each provider's own data. Don't infer; trust the source. |
| Cost calculation per provider | ccusage core feature | Cross-provider pricing is a moving target (rate cards change quarterly); user already pays a subscription, not per-token | Hard skip. JSON output exposes raw tokens — let downstream tools (ccusage) price them. |
| Slack / webhook / email integrations | "Notify my team when…" | Multi-user → Out of Scope; opens auth/secret-management complexity | n/a |
| Configurable custom "plan" with arbitrary limits | Maciek-roboblog `--plan custom` | Implies the tool guesses plan limits — we read them from the provider | If a provider doesn't expose limits, surface "unknown" rather than fake it. |

## Feature Dependencies

```
[Adapter trait abstraction]
    └──requires──> [Config file] (which providers, what auth)
                       └──requires──> [Single-binary dist] (config path conventions)

[Multi-provider HP-bar render]
    └──requires──> [Adapter trait abstraction]
    └──requires──> [Per-provider window granularity]
    └──enhances──> [Pace indicator]
    └──enhances──> [Reset-aware visual metaphor]

[TUI auto-refresh]
    └──requires──> [Configurable refresh interval]
    └──requires──> [Graceful degradation on provider failure]
                       └──otherwise──> single broken adapter freezes the screen

[Statusline-hook compatibility]
    └──requires──> [Stable JSON schema with schema_version]
    └──requires──> [Compact one-line mode]

[Tmux/Starship integration]
    └──requires──> [Compact one-line mode]
    └──requires──> [--no-color / NO_COLOR respect]
    └──requires──> [Stable JSON schema]

[Plain-ASCII fallback]
    └──conflicts──> ASCII can't express the emoji pace icons → need a parallel symbol set ([!] [=] [~])

[Color-blind palette] ──conflicts──> emoji-heavy pace indicators (color-blind users may also lack emoji font on minimal terminals)
    Resolution: pace icons must be readable in *both* color and shape, e.g. ASCII letters not just color.

[Auto-detection of available CLIs]
    └──enhances──> [Config file] — auto-populated default; user can override
```

### Dependency Notes

- **Adapter trait must land before any provider work** — getting it wrong forces rewrites when Codex/Gemini land. Design it with all three in mind from day one, even if Claude is implemented first.
- **JSON schema versioning matters** — once tmux/Starship users build on it, breaking changes hurt. Add `schema_version: 1` to v1 output even when it feels unnecessary.
- **Graceful degradation is load-bearing for multi-provider** — if Gemini's HTTP call times out and freezes the TUI, the whole product feels broken. Each adapter must run on its own timeout budget; UI shows "Gemini: unreachable" not blank.
- **Pace indicator depends on knowing window start** — for Claude's rolling 5h window this requires reading the session-start timestamp, not just current %. Codex `/status` doesn't always include start time → may need to derive from first activity in window.

## MVP Definition

### Launch With (v1)

Maps directly to PROJECT.md Active Requirements. Validated as MVP-essential.

- [ ] CLI `AHB` (no args) → compact one-line HP-bar per provider — Core Value entry point
- [ ] `--compact` / `--detailed` / `--json` flags — table stakes for any CLI in this space
- [ ] `AHB tui` mode with auto-refresh, default 15s, config-overridable — table stakes
- [ ] Config file (TOML) listing providers + auth/source — required to scope what we read
- [ ] Claude Code adapter (5h + weekly windows) — must dogfood
- [ ] Codex CLI adapter (5h + weekly windows) — must dogfood
- [ ] Gemini CLI adapter (subscription session if reachable; otherwise `unknown` with friendly message) — must dogfood
- [ ] Reset countdown rendered per window — Core Value
- [ ] Single static binary distribution (`cargo install` + release artifacts) — table stakes
- [ ] Graceful per-adapter failure (one provider down ≠ whole tool down) — required for credibility of "multi-provider" pitch
- [ ] Auto-detection / skip-missing for un-configured providers — usability baseline
- [ ] Stable JSON schema with `schema_version` field — locks in extensibility before users build on it
- [ ] Color + auto-dark-detect + `NO_COLOR` respect — table stakes
- [ ] No telemetry; no network except provider-direct — privacy baseline

### Add After Validation (v1.x)

Trigger: v1 ships, real usage confirms the HP-bar metaphor lands, users ask for these specifically.

- [ ] Pace indicator (behind / on-pace / too-hot icons) — strong differentiator if users like it, but cheap to defer until we know the resting UX feels right
- [ ] Plain-ASCII fallback mode (`--ascii`) — only matters if users complain about emoji rendering
- [ ] Statusline-hook stdin contract for Claude Code's `tui.status_line` — add once we see users wiring AHB into Claude Code itself
- [ ] Color-blind palette toggle — add when first request lands
- [ ] Tmux / Starship integration recipes (docs, not code) — write after we have a working JSON schema and one user using it
- [ ] Per-provider window selection in CLI (e.g. `AHB --window 5h`) — defer until users ask

### Future Consideration (v2+)

- [ ] Daemon mode for sub-second refresh / cheap statusline calls — only if v1 refresh causes pain
- [ ] Additional provider adapters (Cursor, Windsurf, Amp, Codebuff, Copilot CLI) — community PRs likely; trait must be ready, code itself can wait
- [ ] Threshold values in JSON output (e.g. `"warn_at_pct": 80`) to enable external notifiers — only after a user actually wires up code-notify-style integration
- [ ] Watch mode for CLI (`AHB --watch`) as alternative to full TUI — TUI already covers this; only if requested
- [ ] Internationalization (l10n strings) — defer until non-English contributors arrive

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Compact one-line HP-bar (default `AHB`) | HIGH | LOW | P1 |
| Multi-provider in one view | HIGH | MEDIUM | P1 |
| Reset countdown rendering | HIGH | LOW | P1 |
| Claude Code adapter | HIGH | MEDIUM | P1 |
| Codex CLI adapter | HIGH | MEDIUM | P1 |
| Gemini CLI adapter | HIGH | HIGH (HTTP + auth spike) | P1 |
| TUI fixed-screen auto-refresh | HIGH | MEDIUM | P1 |
| Config file + auto-detect | MEDIUM | MEDIUM | P1 |
| JSON output with stable schema | MEDIUM | LOW | P1 |
| `--detailed` / `--compact` flags | MEDIUM | LOW | P1 |
| Per-adapter graceful failure | HIGH | LOW | P1 |
| Color / auto-dark / NO_COLOR | MEDIUM | LOW | P1 |
| Static binary dist | MEDIUM | LOW | P1 |
| Adapter trait abstraction | HIGH (for extensibility) | LOW (design, day-one) | P1 |
| Pace indicator (🧊/🔥/🚨) | MEDIUM | LOW | P2 |
| Plain-ASCII fallback | LOW (small audience) | LOW | P2 |
| Statusline-hook compatibility | MEDIUM | MEDIUM | P2 |
| Color-blind palette | LOW (until requested) | LOW | P2 |
| Tmux / Starship doc recipes | MEDIUM | LOW (docs only) | P2 |
| Per-window CLI filtering | LOW | LOW | P3 |
| Daemon mode | LOW (v1) | HIGH | P3 |
| Additional provider adapters | MEDIUM | MEDIUM per adapter | P3 |
| Threshold fields in JSON | LOW | LOW | P3 |
| ML/P90 limit prediction | LOW (covered by % + pace) | HIGH | ANTI (skip) |
| Historical trend graphs | LOW (off-mission) | HIGH | ANTI (skip) |
| Cost / token cumulative dashboard | NEGATIVE (violates Core Value) | MEDIUM | ANTI (skip) |
| Desktop notifications | MEDIUM | HIGH (cross-OS) | ANTI (skip, compose with code-notify) |
| API-key quota tracking | NEGATIVE (dilutes Core Value) | MEDIUM | ANTI (skip) |
| Web dashboard / GUI | NEGATIVE (wrong form factor) | HIGH | ANTI (skip) |

**Priority key:** P1 = Must have for v1 launch. P2 = Add when v1 stable. P3 = Future / community. ANTI = Explicitly don't build.

## Competitor Feature Analysis

| Feature | ccusage | Claude-Code-Usage-Monitor | ccburn | claude-code-usage-bar | code-notify | AHB Plan |
|---------|---------|---------------------------|--------|-----------------------|-------------|----------|
| Multi-provider in *one view* | reads multiple agents but per-report, not unified bar | Claude-only | Claude-only | Claude-only | Claude+Codex+Gemini for notify only | **YES — unified HP-bar across all three** |
| Subscription session HP-bar | session block report (numeric) | progress bar (5h only) | burn-up chart | one-line 5h/7d gauges | n/a | **YES — explicit visual metaphor, all providers** |
| Reset countdown | yes (blocks report) | yes (5h) | yes (5h + weekly) | yes (5h + 7d) | n/a | **YES — per provider, per window** |
| Compact / statusline | `statusline` beta, JSON in | n/a (TUI only) | `--compact` | one-line is the whole product | n/a | **YES — default `AHB` output is compact** |
| TUI fixed-screen | no (CLI snapshots) | yes (Rich) | yes | no (statusline only) | n/a | **YES — `AHB tui`** |
| JSON output | yes | partial | yes | n/a | n/a | **YES — versioned schema** |
| Themes / colors | basic | light/dark/classic/auto + WCAG | emoji-driven | 3 styles × 9 themes | n/a | Color + auto-dark + NO_COLOR; color-blind palette in v1.x |
| Refresh strategy | invoked per call | configurable refresh + display Hz | invoked per call | daemon ~3-5ms per tick | event-driven hooks | Inline 15s default; daemon = future |
| ML prediction / P90 | no | yes (headline) | burn rate only | no | n/a | **Skip — burn-rate ratio enough** |
| History / trend | yes (daily/monthly tables) | yes (daily/monthly views) | SQLite-persisted | no | n/a | **Skip — snapshot-only, point at ccusage** |
| Desktop notifications | no | warnings in TUI only | no | no | yes (cross-provider) | **Skip — compose with code-notify** |
| Privacy / offline | local | local | local + SQLite | local | local hooks | **Local; explicit no-telemetry promise** |
| Plan auto-detection | n/a | yes (Pro/Max5/Max20) | n/a | n/a | n/a | Read declared limits from each provider; don't infer |
| Distribution | npm / bunx | pip (PyPI) | binary | pip | brew / npm / script | `cargo install` + GitHub release artifacts; single binary |

## Sources

- [ccusage GitHub](https://github.com/ryoppippi/ccusage)
- [ccusage docs](https://ccusage.com/)
- [ccusage Statusline guide](https://ccusage.com/guide/statusline)
- [ccusage Blocks Reports](https://ccusage.com/guide/blocks-reports)
- [ccusage JSON Output](https://ccusage.com/guide/json-output)
- [Claude-Code-Usage-Monitor (Maciek-roboblog)](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor)
- [ccburn (JuanjoFuchs)](https://github.com/JuanjoFuchs/ccburn)
- [Introducing ccburn (blog)](https://juanjofuchs.github.io/ai-development/2026/01/13/introducing-ccburn-visual-token-tracking.html)
- [ccflare](https://github.com/snipeship/ccflare)
- [claude-code-usage-bar (leeguooooo)](https://github.com/leeguooooo/claude-code-usage-bar)
- [claude-statusbar PyPI](https://pypi.org/project/claude-statusbar/)
- [ccstatusline](https://github.com/sirmalloc/ccstatusline)
- [claude-code-statusline (ohugonnot)](https://github.com/ohugonnot/claude-code-statusline)
- [usage-monitor-for-claude (jens-duttke)](https://github.com/jens-duttke/usage-monitor-for-claude)
- [code-notify (mylee04)](https://github.com/mylee04/code-notify)
- [ai-cli-complete-notify (ZekerTop)](https://github.com/ZekerTop/ai-cli-complete-notify)
- [Codex CLI `/status` issue #15281](https://github.com/openai/codex/issues/15281)
- [Codex CLI status line docs (jdhodges)](https://www.jdhodges.com/blog/codex-usage-cli-status-line/)
- [codex-cli-usage PyPI](https://pypi.org/project/codex-cli-usage/)
- [Codex slash commands](https://developers.openai.com/codex/cli/slash-commands)
- [Gemini CLI quota & pricing](https://geminicli.com/docs/resources/quota-and-pricing/)
- [b3nw/gemini-cli-usage](https://github.com/b3nw/gemini-cli-usage)
- [Gemini CLI usage monitoring (Google support)](https://support.google.com/gemini/thread/355460179/how-do-i-monitor-gemini-cli-usage)
- [Claude Code statusline docs](https://code.claude.com/docs/en/statusline)
- [Claude Code rate limits (SessionWatcher)](https://www.sessionwatcher.com/guides/claude-code-rate-limits-explained)
- [Claude Code Usage Monitor: ccusage, ccflare, and Hooks (claudefa.st)](https://claudefa.st/blog/tools/monitors/claude-code-usage-monitor)
- [Monitor and Optimize Claude Code Usage (apidog.com)](https://apidog.com/blog/open-source-tools-to-monitor-claude-code-usages/)
- [awesome-tuis](https://github.com/rothgar/awesome-tuis)

---
*Feature research for: Multi-provider LLM-CLI subscription session usage tracker (CLI + TUI)*
*Researched: 2026-05-22*
