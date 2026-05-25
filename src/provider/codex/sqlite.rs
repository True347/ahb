//! Codex SQLite discovery + read-only open (REQ ADP-04 / D-45 / D-46).
//!
//! Phase 2 contract: we DISCOVER the highest-version `state_{N}.sqlite` file
//! and prove the read-only + `busy_timeout` open path works — but we run ZERO
//! `SELECT` queries. RESEARCH §Codex SQLite Schema recommends this because:
//! - Codex schema is internal-unstable (migrations rename / drop tables; #23984
//!   documents post-drop reads against `thread_goals` causing breakage).
//! - JSONL rollouts are the primary signal source (D-45) — SQLite is "supplemental
//!   metadata" and Phase 2 does not yet surface any of it.
//! - Honoring the contract (open + busy_timeout) is enough to prove SEC-04
//!   compliance and Pitfall 3 (lock-resilience) without exposing schema-drift surface.
//!
//! Version-glob: picks the file with the highest `_N` integer suffix in the
//! `state_{N}.sqlite` filename. `state_10.sqlite` sorts above `state_5.sqlite`
//! (the integer parse + sort beats lexicographic sort that would invert "10" < "5").
//! When > 1 file coexists (mid-migration), emit `tracing::warn!` listing the
//! candidates (D-46).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(clippy::pedantic)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::model::ProviderError;

/// Discover all `<codex_dir>/state_*.sqlite` files and return the one with
/// the highest version number `N`. Returns `None` for empty / nonexistent dirs.
/// Emits `tracing::warn!` when > 1 file coexists (D-46 mid-migration warning).
#[must_use]
pub fn discover_state_sqlite(codex_dir: &Path) -> Option<PathBuf> {
    if !codex_dir.exists() {
        return None;
    }
    let pattern = codex_dir.join("state_*.sqlite");
    let pattern_str = pattern.to_string_lossy();
    let paths: Vec<PathBuf> = match glob::glob(&pattern_str) {
        Ok(iter) => iter.filter_map(Result::ok).collect(),
        Err(e) => {
            tracing::warn!("glob error for {}: {e}", pattern_str);
            return None;
        }
    };
    if paths.is_empty() {
        return None;
    }
    // Parse the trailing _N from each filename; default to 0 if unparseable
    // (so a malformed name sorts below valid ones).
    let mut paths_with_version: Vec<(PathBuf, u32)> = paths
        .into_iter()
        .map(|p| {
            let n = p
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_prefix("state_"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            (p, n)
        })
        .collect();
    paths_with_version.sort_by_key(|(_, n)| *n);
    let highest = paths_with_version.pop()?;
    if !paths_with_version.is_empty() {
        let names: Vec<String> = paths_with_version
            .iter()
            .map(|(p, _)| {
                p.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
            .collect();
        tracing::warn!(
            "multi-version Codex state files detected — picked {}, found {}",
            highest.0.display(),
            names.join(", ")
        );
    }
    Some(highest.0)
}

/// Open a SQLite connection in read-only mode with a 250ms `busy_timeout`.
/// Maps any rusqlite error to `ProviderError::Internal { source: anyhow }`.
///
/// Per the Phase 2 contract (D-45), the caller MUST NOT run any `SELECT` against
/// the connection — opening + `busy_timeout` is the entire interaction in Phase 2.
/// Caller `drop(conn)` immediately. Schema reads are deferred to Phase 3.
pub fn open_readonly(path: &Path) -> Result<Connection, ProviderError> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| ProviderError::Internal {
        source: anyhow::anyhow!("codex sqlite open ({}): {e}", path.display()),
    })?;
    conn.busy_timeout(Duration::from_millis(250))
        .map_err(|e| ProviderError::Internal {
            source: anyhow::anyhow!("codex sqlite busy_timeout: {e}"),
        })?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch_state_file(dir: &Path, n: u32) -> PathBuf {
        let p = dir.join(format!("state_{n}.sqlite"));
        // Use rusqlite to create a real (empty) SQLite file so version detection
        // is realistic — `Connection::open` creates the file if missing.
        let conn = Connection::open(&p).unwrap();
        drop(conn);
        p
    }

    #[test]
    fn test_6_returns_none_for_empty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(discover_state_sqlite(tmp.path()).is_none());
    }

    #[test]
    fn test_6_returns_highest_n_among_coexisting_versions() {
        let tmp = tempfile::tempdir().unwrap();
        let _p2 = touch_state_file(tmp.path(), 2);
        let _p5 = touch_state_file(tmp.path(), 5);
        let p10 = touch_state_file(tmp.path(), 10);
        let picked = discover_state_sqlite(tmp.path()).unwrap();
        assert_eq!(
            picked, p10,
            "must pick state_10.sqlite over state_5/state_2 (integer sort, not lex)"
        );
    }

    #[test]
    fn returns_none_for_nonexistent_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("does-not-exist");
        assert!(discover_state_sqlite(&nonexistent).is_none());
    }

    #[test]
    fn open_readonly_succeeds_on_freshly_created_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = touch_state_file(tmp.path(), 5);
        let conn = open_readonly(&path).unwrap();
        drop(conn);
    }

    #[test]
    fn open_readonly_rejects_nonexistent_path() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("missing.sqlite");
        let err = open_readonly(&path).unwrap_err();
        matches!(err, ProviderError::Internal { .. });
    }
}
