//! The in-memory storage engine: ingestion, resolution and query entry points.

use std::collections::HashMap;
use std::path::Path;

use crate::{EngineError, IngestStats};
use miter_ast::{AstIndexer, FunctionSymbol, Language, discover_sources};
use rayon::prelude::*;

/// An in-memory, geometric-symbolic index over function symbols.
///
/// The engine owns the [`AstIndexer`] (so ingestion and query encoding always use
/// the same role vectors), the symbol table, and an id → position map for
/// deduplication.
#[derive(Debug)]
pub struct Engine {
    indexer: AstIndexer,
    symbols: Vec<FunctionSymbol>,
    by_id: HashMap<u64, usize>,
    universal_symbols: Vec<crate::polyglot::UniversalSymbol>,
}

impl Engine {
    /// Creates an empty engine with the canonical [`AstIndexer`].
    pub fn new() -> Self {
        Self {
            indexer: AstIndexer::new(),
            symbols: Vec::new(),
            by_id: HashMap::new(),
            universal_symbols: Vec::new(),
        }
    }

    /// Rebuilds an engine from pre-encoded symbols (used by the persistence
    /// loaders and by index sharding).
    pub fn from_symbols<I: IntoIterator<Item = FunctionSymbol>>(symbols: I) -> Self {
        let mut engine = Self::new();
        engine.extend(symbols);
        engine
    }

    /// The indexer used for ingestion and query encoding.
    pub fn indexer(&self) -> &AstIndexer {
        &self.indexer
    }

    /// Number of stored symbols.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// `true` when nothing has been ingested.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Every stored symbol, in insertion order.
    pub fn symbols(&self) -> &[FunctionSymbol] {
        &self.symbols
    }

    /// Inserts a pre-encoded symbol, returning `false` when the id was already
    /// present (re-ingesting a file is therefore idempotent).
    pub fn insert(&mut self, symbol: FunctionSymbol) -> bool {
        if self.by_id.contains_key(&symbol.id) {
            return false;
        }
        self.by_id.insert(symbol.id, self.symbols.len());
        self.symbols.push(symbol);
        true
    }

    /// Inserts many symbols, returning how many were new.
    pub fn extend<I: IntoIterator<Item = FunctionSymbol>>(&mut self, symbols: I) -> usize {
        let mut inserted = 0;
        for symbol in symbols {
            if self.insert(symbol) {
                inserted += 1;
            }
        }
        inserted
    }

    /// Drops every stored symbol.
    pub fn clear(&mut self) {
        self.symbols.clear();
        self.by_id.clear();
        self.universal_symbols.clear();
    }

    /// Returns the universal polyglot symbols, if any were ingested.
    pub fn universal_symbols(&self) -> &[crate::polyglot::UniversalSymbol] {
        &self.universal_symbols
    }

    /// Records every universal symbol a polyglot pass read.
    ///
    /// The two ingestion paths are complementary halves of one parse, and both have to land:
    /// [`Self::insert`] writes the *function-shaped* record the query pipeline ranks, while this
    /// writes the *universal* record the coherence scanner and context hydration read. A `struct`,
    /// an `interface` or a decorated `def` produces no function record at all, so an engine that
    /// skipped this step could not say anything about one — which is why the ingestion passes
    /// call it rather than leaving it to the caller.
    ///
    /// Appends rather than replaces, because a tracked pass is incremental: it reports the files
    /// it re-read, and the symbols of an unchanged file were recorded by the pass that read it.
    pub fn add_universal_symbols(
        &mut self,
        symbols: impl IntoIterator<Item = crate::polyglot::UniversalSymbol>,
    ) {
        self.universal_symbols.extend(symbols);
    }

    /// Parses, encodes and stores a single source buffer.
    ///
    /// # Errors
    ///
    /// Propagates parse failures from [`miter_ast`].
    pub fn ingest_source(
        &mut self,
        source: &str,
        language: Language,
        path: &Path,
        dir_depth: usize,
    ) -> Result<usize, EngineError> {
        let symbols = self
            .indexer
            .index_source(source, language, path, dir_depth)?;
        let mut distinct = 0;
        for symbol in symbols {
            if self.insert(symbol) {
                distinct += 1;
            }
        }
        Ok(distinct)
    }

    /// Discovers every supported file below `root` and ingests them in parallel.
    ///
    /// Parsing is the expensive part of ingestion and is pure, so it runs on the
    /// Rayon pool; insertion happens on the calling thread to keep the symbol
    /// table deterministic.
    ///
    /// # Errors
    ///
    /// * [`EngineError::Index`] when the directory cannot be traversed,
    /// * per-file failures are reported in [`IngestStats::failures`].
    pub fn ingest_dir(&mut self, root: &Path) -> Result<IngestStats, EngineError> {
        self.ingest_dir_with(root, &miter_ast::SilentProgress)
    }

    /// [`Engine::ingest_dir`], reporting each file and symbol through `progress`.
    ///
    /// The parsing half runs on the Rayon pool, so `progress` is reported from several threads at
    /// once — which is why [`miter_ast::IndexingProgress`] is `Sync` and why the bar it is
    /// usually given advances under a lock rather than through anything thread-local.
    ///
    /// # Errors
    ///
    /// As [`Engine::ingest_dir`].
    pub fn ingest_dir_with(
        &mut self,
        root: &Path,
        progress: &dyn miter_ast::IndexingProgress,
    ) -> Result<IngestStats, EngineError> {
        let config = crate::config::MiterConfig::load_from_workspace(root);
        let matcher = config.build_indexer_matcher().ok();
        let files: Vec<_> = discover_sources(root)?
            .into_iter()
            .filter(|file| {
                if let Some(m) = &matcher {
                    !crate::config::is_path_excluded(m, &file.path, root)
                } else {
                    true
                }
            })
            .collect();
        let indexer = &self.indexer;

        let parsed: Vec<Result<Vec<FunctionSymbol>, String>> = files
            .par_iter()
            .map(|file| {
                progress.file(&file.path);
                indexer
                    .index_file(file)
                    .map_err(|error| format!("{}: {error}", file.path.display()))
            })
            .collect();

        let mut stats = IngestStats {
            files_scanned: files.len(),
            ..IngestStats::default()
        };
        let mut batch = Vec::new();
        for result in parsed {
            match result {
                Ok(mut symbols) => {
                    stats.files_indexed += 1;
                    batch.append(&mut symbols);
                }
                Err(message) => stats.failures.push(message),
            }
        }
        for symbol in batch {
            progress.symbol(&symbol.name, &symbol.path);
            if self.insert(symbol) {
                stats.symbols += 1;
            } else {
                stats.duplicates += 1;
            }
        }
        progress.finish();
        Ok(stats)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

mod ingest;
mod persist;
mod stats;

pub use stats::IncrementalStats;

pub(crate) use ingest::relative_to;
