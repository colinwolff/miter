//! Unified ingestion: one pass that reads all five languages, indexes them and projects them.

use std::path::{Path, PathBuf};

use miter_ast::discover_sources;

use crate::polyglot::{self, UniversalSymbol};
use crate::store::IncrementalStats;

mod polyglot_pass;
mod tracked;

/// Recursively discovers both language source files and declarative schema/manifest files.
pub fn discover_all_sources(
    root: &Path,
) -> Result<Vec<(PathBuf, Option<miter_ast::Language>, usize)>, miter_ast::IndexError> {
    let config = crate::config::MiterConfig::load_from_workspace(root);
    let matcher = config.build_indexer_matcher().ok();

    let code_files = discover_sources(root)?;
    let mut all: Vec<(PathBuf, Option<miter_ast::Language>, usize)> = code_files
        .into_iter()
        .filter(|f| {
            if let Some(m) = &matcher {
                !crate::config::is_path_excluded(m, &f.path, root)
            } else {
                true
            }
        })
        .map(|f| (f.path, Some(f.language), f.dir_depth))
        .collect();

    // Walk for declarative files that discover_sources ignores (sql, tf, dockerfile, yaml)
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    while let Some((directory, depth)) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let mut dirs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(m) = &matcher
                && crate::config::is_path_excluded(m, &path, root)
            {
                continue;
            }
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !name.starts_with('.') && !miter_ast::IGNORED_DIRECTORIES.contains(&name) {
                        dirs.push(path);
                    }
                } else if file_type.is_file() && polyglot::is_declarative_source(&path) {
                    all.push((path, None, depth));
                }
            }
        }
        for dir in dirs {
            pending.push((dir, depth + 1));
        }
    }

    all.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(all)
}

/// What a polyglot ingestion pass did, and what it read.
#[derive(Clone, Debug, Default)]
pub struct PolyglotIngest {
    /// The pass's own summary: files scanned, indexed, skipped, symbols and failures.
    pub stats: IncrementalStats,
    /// Every declaration read, with its docstring, its level and its signature.
    pub symbols: Vec<UniversalSymbol>,
    /// AST identifier references extracted across all scanned files, grouped by relative file path.
    pub references: Vec<(String, Vec<(String, miter_storage::ReferenceEntry)>)>,
}

impl PolyglotIngest {
    /// One line for a log.
    pub fn summary(&self) -> String {
        format!(
            "{} declarations from {} of {} {}",
            self.symbols.len(),
            self.stats.files_indexed,
            self.stats.files_scanned,
            if self.stats.files_scanned == 1 {
                "file"
            } else {
                "files"
            }
        )
    }
}
