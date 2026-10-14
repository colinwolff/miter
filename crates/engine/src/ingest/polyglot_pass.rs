//! Polyglot parsing pass: parses all files in parallel via Rayon.

use std::path::Path;

use rayon::prelude::*;

use crate::polyglot::{self, Language, UniversalSymbol};
use crate::store::relative_to;
use crate::store::{Engine, IncrementalStats};
use crate::{EngineError, IngestStats};

use super::{PolyglotIngest, discover_all_sources};

impl Engine {
    /// Parses, encodes and stores every declaration in one source buffer.
    ///
    /// # Errors
    ///
    /// [`EngineError::Polyglot`] when grammar cannot be loaded or file does not parse.
    pub fn ingest_polyglot_file(
        &mut self,
        source: &str,
        path: &Path,
        dir_depth: usize,
        language: Language,
    ) -> Result<Vec<UniversalSymbol>, EngineError> {
        let symbols = polyglot::parse(path, source, language)?;
        let indexer = self.indexer().clone();
        for symbol in &symbols {
            self.insert(symbol.function_symbol(&indexer, dir_depth));
        }
        self.add_universal_symbols(symbols.clone());
        Ok(symbols)
    }

    /// Reads and ingests every polyglot file below `root`, parsing on the Rayon pool.
    ///
    /// # Errors
    ///
    /// [`EngineError::Index`] when the tree cannot be traversed.
    pub fn ingest_polyglot_tree(&mut self, root: &Path) -> Result<PolyglotIngest, EngineError> {
        self.ingest_polyglot_tree_with(root, &miter_ast::SilentProgress)
    }

    /// Reads and ingests every polyglot file below `root` reporting progress via `progress`.
    ///
    /// # Errors
    ///
    /// [`EngineError::Index`] when the tree cannot be traversed.
    pub fn ingest_polyglot_tree_with(
        &mut self,
        root: &Path,
        progress: &dyn miter_ast::IndexingProgress,
    ) -> Result<PolyglotIngest, EngineError> {
        let files = discover_all_sources(root)?;
        let mut stats = IngestStats {
            files_scanned: files.len(),
            ..IngestStats::default()
        };
        type ParsedFile = (
            Vec<UniversalSymbol>,
            String,
            Vec<(String, miter_storage::ReferenceEntry)>,
        );
        let parsed: Vec<Result<ParsedFile, String>> = files
            .par_iter()
            .map(|(path, lang_opt, _depth)| {
                progress.file(path);
                match std::fs::read_to_string(path) {
                    Ok(source) => {
                        let relative = relative_to(root, path);
                        let refs =
                            crate::analysis::extract_references(Path::new(&relative), &source);
                        let syms_res = if let Some(idx_lang) = lang_opt {
                            let language = Language::from_index(*idx_lang);
                            polyglot::parse(path, &source, language)
                                .map_err(|error| format!("{}: {error}", path.display()))
                        } else {
                            Ok(polyglot::declarative_symbols(path, &source).unwrap_or_default())
                        };
                        syms_res.map(|syms| (syms, relative, refs))
                    }
                    Err(error) => Err(format!("{}: {error}", path.display())),
                }
            })
            .collect();

        let mut symbols: Vec<UniversalSymbol> = Vec::new();
        let mut file_refs: Vec<(String, Vec<(String, miter_storage::ReferenceEntry)>)> = Vec::new();
        for result in parsed {
            match result {
                Ok((mut read, rel, refs)) => {
                    stats.files_indexed += 1;
                    symbols.append(&mut read);
                    if !refs.is_empty() {
                        file_refs.push((rel, refs));
                    }
                }
                Err(message) => stats.failures.push(message),
            }
        }
        let indexer = self.indexer().clone();
        for symbol in &symbols {
            if self.insert(symbol.function_symbol(&indexer, 0)) {
                stats.symbols += 1;
            } else {
                stats.duplicates += 1;
            }
        }
        self.add_universal_symbols(symbols.clone());
        Ok(PolyglotIngest {
            stats: IncrementalStats {
                files_scanned: stats.files_scanned,
                files_indexed: stats.files_indexed,
                files_skipped: 0,
                symbols: stats.symbols,
                failures: stats.failures,
            },
            symbols,
            references: file_refs,
        })
    }
}
