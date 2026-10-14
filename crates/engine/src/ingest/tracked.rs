//! Incremental tracked polyglot ingestion.

use std::path::Path;
use std::time::SystemTime;

use crate::EngineError;
use crate::polyglot::{self, Language, UniversalSymbol};
use crate::store::relative_to;
use crate::store::{Engine, IncrementalStats};

use super::{PolyglotIngest, discover_all_sources};

impl Engine {
    /// [`Engine::ingest_polyglot_tree`], parsing only the files whose contents changed.
    ///
    /// # Errors
    ///
    /// [`EngineError::Index`] when the tree cannot be traversed, and
    /// [`EngineError::Persist`] when the ledger cannot be read or written.
    pub fn ingest_polyglot_tree_tracked(
        &mut self,
        root: &Path,
        index: &miter_storage::WorkspaceStore,
    ) -> Result<PolyglotIngest, EngineError> {
        let sources = discover_all_sources(root)?;
        let mut stats = IncrementalStats {
            files_scanned: sources.len(),
            ..IncrementalStats::default()
        };
        let mut symbols: Vec<UniversalSymbol> = Vec::new();
        let indexer = self.indexer().clone();
        for (path, lang_opt, dir_depth) in sources {
            let relative = relative_to(root, &path);
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    stats.failures.push(format!("{}: {error}", path.display()));
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
                        .push(format!("{}: not UTF-8: {error}", path.display()));
                    continue;
                }
            };
            let read_res = if let Some(idx_lang) = lang_opt {
                let language = Language::from_index(idx_lang);
                polyglot::parse(&path, &text, language).map_err(|e| e.to_string())
            } else {
                Ok(polyglot::declarative_symbols(&path, &text).unwrap_or_default())
            };
            match read_res {
                Ok(read) => {
                    for symbol in &read {
                        if self.insert(symbol.function_symbol(&indexer, dir_depth)) {
                            stats.symbols += 1;
                        }
                    }
                    stats.files_indexed += 1;
                    symbols.extend(read);
                    index.set_file_hash(&relative, &digest)?;
                    let mtime_info = path
                        .metadata()
                        .ok()
                        .and_then(|meta| meta.modified().ok().map(|m| (meta, m)))
                        .and_then(|(meta, mtime)| {
                            mtime
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .ok()
                                .map(|dur| (dur.as_secs(), dur.subsec_nanos(), meta.len()))
                        });
                    if let Some((secs, nanos, len)) = mtime_info {
                        let record =
                            miter_storage::IndexedFileRecord::new(secs, nanos, digest, len);
                        let _ = index.set_indexed_file(&relative, &record);
                    }
                    let refs = crate::analysis::extract_references(Path::new(&relative), &text);
                    let _ = index.clear_references_for_file(&relative);
                    let _ = index.insert_references(&relative, refs);
                }
                Err(error) => stats.failures.push(format!("{}: {error}", path.display())),
            }
        }
        self.add_universal_symbols(symbols.clone());
        Ok(PolyglotIngest {
            stats,
            symbols,
            references: Vec::new(),
        })
    }
}
