//! Acceptance grep: `jiff::Timestamp::now()` is forbidden under `src/provider/`
//! AND `src/tui/widgets/`.
//!
//! Phase 0 contract: only `src/main.rs`, `src/cli/mod.rs::run_compact`, and
//! `src/tui/mod.rs::tui_loop` call wall-clock. All adapters and TUI leaf widgets
//! use injected timestamps (clock-injection rule — Phase 0 + BL-01 Plan 04 extension).
//!
//! Plan 04 (BL-01 fix): the scope expanded from `src/provider/` only to BOTH
//! `src/provider/` AND `src/tui/widgets/`. The render-tick arm in `tui_loop` is the
//! single authorized wall-clock site for the TUI render path; `AppState.now` is the
//! data path into the leaf widget. Future agents must NOT re-narrow the scan.

use std::fs;
use std::path::{Path, PathBuf};

fn walk_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_timestamp_now_in_provider_or_tui_widgets_subtree() {
    // BL-01 (Plan 04): scan BOTH `src/provider/` AND `src/tui/widgets/`. The
    // `src/tui/mod.rs` callsite remains authorized (tui_loop is the canonical
    // wall-clock site for the TUI render path) and is intentionally NOT scanned.
    let scan_dirs: [&str; 2] = ["src/provider", "src/tui/widgets"];

    let mut rs_files = Vec::new();
    for dir_rel in scan_dirs {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir_rel);
        walk_rs_files(&dir, &mut rs_files);
    }

    assert!(
        !rs_files.is_empty(),
        "should find rs files under src/provider/ AND src/tui/widgets/"
    );

    let mut offenders: Vec<String> = Vec::new();
    for path in &rs_files {
        let content = fs::read_to_string(path).expect("read rs file");
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            // Filter out comment lines.
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }
            if line.contains("Timestamp::now") {
                offenders.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    line_num + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "Wall-clock injection rule violation — adapters and TUI leaf widgets must use injected `now`:\n{}",
        offenders.join("\n")
    );
}
