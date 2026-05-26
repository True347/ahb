# Plan 04-02 Dry-Run Evidence

**Captured:** 2026-05-26
**Purpose:** Pre-Wave-3 gate per RESEARCH § Example 9 + Plan 04-02 Task 2 acceptance
**Source plan:** `.planning/phases/04-distribution-release-polish/04-02-cargo-dist-init-PLAN.md`

This file is the evidence trail for Wave 3 (Plan 04-03) to proceed. It is **out of crate tarball** (`.planning/` is in `[package].exclude` per D-82) but stays in git for posterity.

---

## 1. cargo-dist version proof

cargo-dist 0.32.0 was installed via `cargo install cargo-dist --version 0.32.0 --locked`. Note the binary name is `dist`, not `cargo-dist`, so the verification command is `dist --version` (not `cargo dist --version` — the plan's verification phrasing is inadapted to cargo-dist 0.32 binary naming; see Deviations in SUMMARY).

```
$ /home/chasel/.cargo/bin/dist --version
cargo-dist 0.32.0
```

PASS — exact version match (`0.32.0`).

---

## 2. `dist plan` dry-run output

Verifies `dist-workspace.toml [dist]` config is parseable and matrix expansion matches D-79 exactly.

```
$ /home/chasel/.cargo/bin/dist plan
announcing v0.1.0
  ai-hp-bar 0.1.0
    source.tar.gz
      [checksum] source.tar.gz.sha256
    ai-hp-bar-installer.sh
    ai-hp-bar-installer.ps1
    ahb.rb
    sha256.sum
    ai-hp-bar-aarch64-apple-darwin.tar.xz
      [bin] ahb
      [misc] LICENSE-APACHE, LICENSE-MIT, README.md
      [checksum] ai-hp-bar-aarch64-apple-darwin.tar.xz.sha256
    ai-hp-bar-aarch64-unknown-linux-gnu.tar.xz
      [bin] ahb
      [misc] LICENSE-APACHE, LICENSE-MIT, README.md
      [checksum] ai-hp-bar-aarch64-unknown-linux-gnu.tar.xz.sha256
    ai-hp-bar-x86_64-apple-darwin.tar.xz
      [bin] ahb
      [misc] LICENSE-APACHE, LICENSE-MIT, README.md
      [checksum] ai-hp-bar-x86_64-apple-darwin.tar.xz.sha256
    ai-hp-bar-x86_64-pc-windows-msvc.zip
      [bin] ahb.exe
      [misc] LICENSE-APACHE, LICENSE-MIT, README.md
      [checksum] ai-hp-bar-x86_64-pc-windows-msvc.zip.sha256
    ai-hp-bar-x86_64-unknown-linux-gnu.tar.xz
      [bin] ahb
      [misc] LICENSE-APACHE, LICENSE-MIT, README.md
      [checksum] ai-hp-bar-x86_64-unknown-linux-gnu.tar.xz.sha256
```

### Grep gate evidence

| Asserted substring | Status |
|--------------------|--------|
| `x86_64-unknown-linux-gnu` | PASS |
| `x86_64-apple-darwin` | PASS |
| `aarch64-apple-darwin` | PASS |
| `x86_64-pc-windows-msvc` | PASS |
| `aarch64-unknown-linux-gnu` | PASS |
| `installer.sh` (shell installer) | PASS |
| `installer.ps1` (PowerShell installer) | PASS |
| `ahb.rb` (Homebrew formula — Finding 3 locked) | PASS |

All 5 targets × 3 installer types + brew formula present. Binary name in tarball is `ahb` (Linux/macOS) / `ahb.exe` (Windows) — Finding 2 contract intact (`[[bin]] name = "ahb"` preserved through `cargo install ai-hp-bar`).

### Notes vs. plan acceptance text

- The plan's automated `<verify>` block expected `grep -qi 'shell'` and `grep -qi 'powershell'` against `dist plan` output. The actual output uses `installer.sh` (containing `sh`, not `shell`) and `installer.ps1` (containing `ps1`, not `powershell`). The fix is grep adaptation to match actual cargo-dist 0.32 plan output verbiage. The semantic check (shell installer + PowerShell installer + Homebrew formula present) is fully satisfied — see Deviations in SUMMARY.
- The plan also expected substrings like `Formula/ahb.rb` — `dist plan` instead prints the bare formula filename `ahb.rb`. Same semantic content; surface phrasing differs.

---

## 3. DIST-01 ldd proof (Linux release binary)

```
$ cargo build --release
    Finished `release` profile [optimized] target(s) in 0.08s

$ file target/release/ahb
target/release/ahb: ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV), dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2, for GNU/Linux 4.4.0, BuildID[sha1]=2ce76c521708e5f5610ff126120fb4f518d3b216, stripped

$ ldd target/release/ahb
	linux-vdso.so.1 (0x00007f33e8c23000)
	libdbus-1.so.3 => /usr/lib/libdbus-1.so.3 (0x00007f33e85ad000)
	libgcc_s.so.1 => /usr/lib/libgcc_s.so.1 (0x00007f33e8580000)
	libm.so.6 => /usr/lib/libm.so.6 (0x00007f33e844d000)
	libc.so.6 => /usr/lib/libc.so.6 (0x00007f33e8200000)
	libsystemd.so.0 => /usr/lib/libsystemd.so.0 (0x00007f33e80d6000)
	/lib64/ld-linux-x86-64.so.2 => /usr/lib64/ld-linux-x86-64.so.2 (0x00007f33e8c25000)
```

### DIST-01 gate

| TLS-impl substring | Match count | Disposition |
|--------------------|-------------|-------------|
| `libssl` | 0 | PASS (rustls-only) |
| `libcrypto` | 0 | PASS (rustls-only) |
| `libnative-tls` | 0 | PASS (rustls-only) |
| `security-framework` | 0 | PASS (no macOS native-tls; n/a on Linux but check still clean) |

### DIST-01 allow-list (informational)

| Library | Source | Reason |
|---------|--------|--------|
| `linux-vdso.so.1` | kernel | vDSO — always present, not a userspace dep |
| `libdbus-1.so.3` | system | Phase 1 D-40 — Linux keyring backend uses `dbus-secret-service-keyring-store` (`crypto-rust` feature, no OpenSSL); dbus is desktop service-bus IPC, not TLS |
| `libgcc_s.so.1` | toolchain | GCC unwinding support for `panic = "unwind"` (Phase 1 ADP-01 contract) |
| `libm.so.6` | system C runtime | math (used by serde_json float parsing, jiff calendar math) |
| `libc.so.6` | system C runtime | universal |
| `libsystemd.so.0` | system | transitively via libdbus — systemd-aware dbus integration on this distro |
| `/lib64/ld-linux-x86-64.so.2` | kernel | dynamic linker — always present |

PASS — DIST-01 verified on Linux (x86_64-unknown-linux-gnu) baseline. macOS / Windows DIST-01 verification deferred to CI (release.yml runs cargo-dist on all 5 targets — rustls invariant holds across cfg(target_os = …) per Pattern E of PATTERNS.md).

---

## 4. `cargo publish --dry-run` regression check (Pitfall 5)

Verifies the new `[profile.dist]` block in `Cargo.toml` + added `homepage` field + the `dist-workspace.toml` file did not introduce packaging warnings (categories/keywords, etc.).

```
$ cargo publish --dry-run --allow-dirty
    Updating crates.io index
   Packaging ai-hp-bar v0.1.0 (/home/chasel/REPO/AIHPBar)
    Updating crates.io index
    Packaged 56 files, 518.5KiB (137.1KiB compressed)
   Verifying ai-hp-bar v0.1.0 (/home/chasel/REPO/AIHPBar)
   Compiling ai-hp-bar v0.1.0 (/home/chasel/REPO/AIHPBar/target/package/ai-hp-bar-0.1.0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.83s
   Uploading ai-hp-bar v0.1.0 (/home/chasel/REPO/AIHPBar)
warning: aborting upload due to dry run
```

| Metric | Plan 04-01 | Plan 04-02 | Delta |
|--------|-----------|-----------|-------|
| File count | 55 | 56 | +1 (`dist-workspace.toml`) |
| Uncompressed | 517.2 KiB | 518.5 KiB | +1.3 KiB |
| Compressed | 136.6 KiB | 137.1 KiB | +0.5 KiB |

The single added file is `dist-workspace.toml`, which is intentional — cargo-dist users may `cargo install ai-hp-bar` and recover the dist config from the published tarball if they want to fork and re-release.

### Warning analysis

Filtered for non-benign warnings:

```
$ grep -iE '^warning' publish-dry-run.txt | grep -v 'aborting upload due to dry run'
(empty — no warnings)
```

PASS — only the expected `warning: aborting upload due to dry run` (which is informational, marking the dry-run boundary). Pitfall 5 categories/keywords slug check: `keywords = ["claude", "codex", "gemini", "cli", "tui"]` and `categories = ["command-line-utilities"]` produced no warning, confirming all are valid crates.io slugs.

### D-82 exclude re-verification

Files in tarball:

```
$ find target/package/ai-hp-bar-0.1.0/ -type f | wc -l
56

$ find target/package/ai-hp-bar-0.1.0/ -type f -path '*/.planning/*' -o -path '*/.github/*' -o -path '*/.claude/*' -o -path '*/.omg/*' -o -name 'CLAUDE.md' -o -path '*/tests/data/*'
(empty)
```

PASS — all 6 D-82 exclude entries verified absent from packaging.

---

## 5. `cargo test --all-targets` regression check

```
$ cargo test --all-targets 2>&1 | grep -E '^test result.*ok'
test result: ok. 185 passed; 0 failed; 0 ignored
... (multiple per-integration-binary lines, all 0 failed)
```

PASS — 185 unit + all integration tests green. Phase 0-3 binary-name contract (`assert_cmd::cargo_bin("ahb")`) intact through the dist scaffolding additions.

---

## 6. release.yml structural verification

`/home/chasel/REPO/AIHPBar/.github/workflows/release.yml` was generated by `dist init` + `dist generate` (idempotent re-run after `[dist]` config patch).

### Job topology (from `grep -nE '^  [a-z][a-z-]+:'`):

| Line | Job | Role |
|------|-----|------|
| 49 | `plan` | Always runs — config validation + matrix expansion |
| 91 | `build-local-artifacts` | Per-target matrix build (5 targets) |
| 169 | `build-global-artifacts` | Installers (.sh, .ps1) + checksums |
| 215 | `host` | Create GH release + upload assets |
| 281 | `publish-homebrew-formula` | Push Formula/ahb.rb to True347/homebrew-tap |
| 327 | `announce` | Final announcement step |

### Trigger

```yaml
on:
  pull_request:
  push:
    tags:
      - '**[0-9]+.[0-9]+.[0-9]+*'
```

Note the actual cargo-dist 0.32 pattern is a SemVer wildcard `**[0-9]+.[0-9]+.[0-9]+*`, not the literal `'v*'` the plan's acceptance text expected. The tag `v0.1.0` (D-76 contract) will match this pattern because the `*` at the end allows any prefix including `v`. PR trigger present because `pr-run-mode = "plan"` is set in `dist-workspace.toml`.

### HOMEBREW_TAP_TOKEN reference

```
$ grep -nE 'HOMEBREW_TAP_TOKEN' .github/workflows/release.yml
297:          token: ${{ secrets.HOMEBREW_TAP_TOKEN }}
```

PASS — Finding 6 + Pitfall 6 contract enforced. Wave 3 (Plan 04-03) must provision this secret on `True347/ahb` (not on the tap repo).

### ci.yml coexistence

```
$ git diff .github/workflows/ci.yml
(empty)
```

PASS — `.github/workflows/ci.yml` byte-identical, release.yml coexists without collision.

---

## 7. Wave 3 readiness signal

All Plan 04-02 acceptance criteria satisfied:

- [x] cargo-dist 0.32.0 installed and version-verified (binary name: `dist`)
- [x] dist config landed (in `dist-workspace.toml [dist]`, not `Cargo.toml [workspace.metadata.dist]` per cargo-dist 0.32 default for single-crate repos — Assumption A1 LOW-risk fallback)
- [x] All 5 D-79 targets present
- [x] All 3 installers configured (shell, powershell, homebrew)
- [x] `tap = "True347/homebrew-tap"` + `publish-jobs = ["homebrew"]` + `formula = "ahb"` (Finding 3) + `pr-run-mode = "plan"`
- [x] `Cargo.toml [profile.dist]` exists with `inherits = "release"` + `lto = "thin"` (Pitfall 8 — untouched)
- [x] `Cargo.toml [profile.release]` D-81 keys preserved (lto=true, strip="symbols", opt-level=3)
- [x] `.github/workflows/release.yml` generated, contains 6 jobs including `publish-homebrew-formula`
- [x] HOMEBREW_TAP_TOKEN secret reference in release.yml
- [x] ci.yml unchanged
- [x] `dist plan` exits 0 with expected matrix
- [x] DIST-01 ldd proof clean
- [x] `cargo publish --dry-run` clean
- [x] `cargo test --all-targets` green
- [x] `cargo build` (debug + release) green

**Plan 04-03 (publish + bootstrap wave) preconditions met for the local-side deliverables.** Out-of-tree blockers remain: `gh repo create True347/ahb`, `gh repo create True347/homebrew-tap`, fine-grained PAT creation for `HOMEBREW_TAP_TOKEN`, and screenshot.png human replacement (carried over from Plan 04-01).

---

*Phase: 04-distribution-release-polish*
*Plan: 02 — cargo-dist init scaffolding*
*Captured: 2026-05-26*
