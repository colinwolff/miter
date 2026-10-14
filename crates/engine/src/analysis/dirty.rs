//! Fast filesystem modification time (mtime) and git status change detector.
//!
//! Powers `miter index --dirty` to enable sub-10ms incremental re-indexing passes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use miter_storage::IndexedFileRecord;

use sources::is_supported_source;

/// Outcome of scanning for dirty, modified, or deleted files in a workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirtyScan {
    /// Newly added or modified files that must be re-indexed.
    pub modified_files: Vec<PathBuf>,
    /// Deleted files whose indexed state must be pruned.
    pub deleted_files: Vec<String>,
}

/// Detects dirty files within `root` by combining `git status --porcelain` and filesystem mtime checks.
#[must_use]
pub fn detect_dirty_files(root: &Path, indexed_files: &[(String, IndexedFileRecord)]) -> DirtyScan {
    let mut modified = HashSet::new();
    let mut deleted = HashSet::new();

    let config = crate::config::MiterConfig::load_from_workspace(root);
    let matcher = config.build_indexer_matcher().ok();

    // 1. Try git status --porcelain for ultra-fast delta detection in git repositories.
    if root.join(".git").exists() {
        let output_res = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(root)
            .output();
        if let Ok(output) = output_res {
            if !output.status.success() {
                // Command failed or non-zero status
            } else {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.len() < 4 {
                        continue;
                    }
                    let status = &line[..2];
                    let file_part = line[3..].trim();
                    let path_str = if let Some((_, to)) = file_part.split_once(" -> ") {
                        to
                    } else {
                        file_part
                    };
                    let rel = path_str.replace('\\', "/");
                    let full_path = root.join(&rel);

                    if let Some(m) = &matcher
                        && crate::config::is_path_excluded(m, &full_path, root)
                    {
                        continue;
                    }

                    if status.contains('D') {
                        deleted.insert(rel);
                    } else if full_path.is_file() && is_supported_source(&full_path) {
                        modified.insert(full_path);
                    }
                }
            }
        }
    }

    // 2. Cross-reference indexed_files for filesystem mtime differences and missing files.
    for (rel, record) in indexed_files {
        let full = root.join(rel);
        if let Some(m) = &matcher
            && crate::config::is_path_excluded(m, &full, root)
        {
            deleted.insert(rel.clone());
            continue;
        }
        if !full.exists() {
            deleted.insert(rel.clone());
            continue;
        }

        let mtime_match = full
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok().map(|m| (meta, m)))
            .and_then(|(meta, mtime)| {
                mtime
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .ok()
                    .map(|dur| (meta.len(), dur.as_secs(), dur.subsec_nanos()))
            });

        let is_dirty = match mtime_match {
            Some((size, secs, nanos)) => {
                secs != record.mtime_secs
                    || nanos != record.mtime_nanos
                    || size != record.size_bytes
            }
            None => false,
        };
        if is_dirty {
            modified.insert(full);
        }
    }

    // Convert sets to sorted vectors for determinism
    let mut modified_vec: Vec<PathBuf> = modified.into_iter().collect();
    modified_vec.sort();

    let mut deleted_vec: Vec<String> = deleted.into_iter().collect();
    deleted_vec.sort();

    DirtyScan {
        modified_files: modified_vec,
        deleted_files: deleted_vec,
    }
}

mod ingest;
mod sources;

pub use ingest::ingest_dirty;
