//! Acceptance grep: `jiff::Timestamp::now()` is forbidden under `src/provider/`.
//!
//! Phase 0 contract: only `src/main.rs` (and `src/cli/mod.rs` as the main-adjacent
//! CLI dispatcher) call wall-clock. All adapters use `ctx.now` (clock-injection).
//! This test walks `src/provider/**/*.rs` line by line, filters out comments, and
//! asserts no non-comment line matches `Timestamp::now`.

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
fn no_timestamp_now_in_provider_subtree() {
    let provider_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("provider");
    let mut rs_files = Vec::new();
    walk_rs_files(&provider_dir, &mut rs_files);

    assert!(!rs_files.is_empty(), "should find provider/*.rs files");

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
        "Wall-clock injection rule violation — adapters must use ctx.now:\n{}",
        offenders.join("\n")
    );
}
