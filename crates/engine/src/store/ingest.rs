//! Tracked ingestion: parse only the files whose contents changed.

use std::path::Path;

use crate::EngineError;

use super::{Engine, IncrementalStats};

impl Engine {
    /// Ingests the source tree under `root`, parsing only the files whose contents changed.
    ///
    /// This is `ingest_dir` with a ledger. Every discovered file is read, its SHA-256 digest is
    /// compared against the one recorded for its workspace-relative path in `index`, and only a
    /// *difference* costs a tree-sitter parse: a save that changes nothing, a rebuild that touches
    /// every mtime, or a second `miter index` over the same tree all cost one hash per file.
    ///
    /// Three properties the caller can rely on:
    ///
    /// * **A file that fails to parse is not recorded.** Its digest is written *after* a successful
    ///   parse, so a failure leaves the ledger untouched and the next pass retries the file instead
    ///   of skipping something that was never indexed.
    /// * **Files that disappeared are not forgotten.** Pruning is the writer's job
    ///   ([`miter_storage::WorkspaceStore::remove_file_hash`]): a stale row costs one hash of nothing,
    ///   while an over-eager prune would re-parse a tree that was only temporarily unreadable.
    /// * **A read failure is a failure of that file, not of the pass** — the same rule the
    ///   unconditional scan follows.
    ///
    /// # Errors
    ///
    /// [`EngineError::Index`] when the tree cannot be traversed, and [`EngineError::Persist`] when
    /// the ledger cannot be read or written.
    pub fn ingest_tree_tracked(
        &mut self,
        root: &Path,
        index: &miter_storage::WorkspaceStore,
    ) -> Result<IncrementalStats, EngineError> {
        let config = crate::config::MiterConfig::load_from_workspace(root);
        let matcher = config.build_indexer_matcher().ok();
        let sources: Vec<_> = miter_ast::discover_sources(root)?
            .into_iter()
            .filter(|s| {
                if let Some(m) = &matcher {
                    !crate::config::is_path_excluded(m, &s.path, root)
                } else {
                    true
                }
            })
            .collect();
        let mut stats = IncrementalStats {
            files_scanned: sources.len(),
            ..IncrementalStats::default()
        };
        for source in sources {
            let relative = relative_to(root, &source.path);
            let bytes = match std::fs::read(&source.path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    stats
                        .failures
                        .push(format!("{}: {error}", source.path.display()));
                    continue;
                }
            };
            let digest = miter_storage::sha256(&bytes);
            if index.file_hash(&relative)? == Some(digest) {
                stats.files_skipped += 1;
                continue;
            }
            let text = match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(error) => {
                    stats
                        .failures
                        .push(format!("{}: not UTF-8: {error}", source.path.display()));
                    continue;
                }
            };
            match self.ingest_source(&text, source.language, &source.path, source.dir_depth) {
                Ok(symbols) => {
                    stats.files_indexed += 1;
                    stats.symbols += symbols;
                    index.set_file_hash(&relative, &digest)?;
                }
                Err(error) => {
                    // The ledger is deliberately *not* updated: the file has not been indexed, and
                    // recording its hash would make the next pass skip it forever.
                    stats
                        .failures
                        .push(format!("{}: {error}", source.path.display()));
                }
            }
        }
        Ok(stats)
    }
}

/// The ledger key of a file: its path relative to the workspace root, with `/` separators.
///
/// Relative so that moving a workspace does not invalidate the ledger, and `/`-separated so that a
/// ledger written on one platform is readable on another. A path that does not sit under `root` is
/// keyed by its whole path — the fallback for a caller that indexed a file outside the tree it
/// named, which is a mistake worth *recording* rather than refusing.
/// [`relative_to`], visible to `crate::ingest`'s polyglot pass — one rule for the ledger key,
/// two readers.
pub(crate) fn relative_to(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
