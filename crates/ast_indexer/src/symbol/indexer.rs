//! The indexer: discovery, parsing and encoding in one pass.

use std::path::Path;

use crate::IndexError;
use crate::discover::{SourceFile, discover_sources};
use crate::extract::{FunctionNode, Language, parse_source};

use super::{FunctionSymbol, seed_from_text};

/// Outcome of indexing a directory.
#[derive(Debug)]
pub struct IndexReport {
    /// Every function found, ordered by file path and declaration order.
    pub symbols: Vec<FunctionSymbol>,
    /// Number of files that parsed successfully.
    pub files_indexed: usize,
    /// Files that failed to read or parse (kept going, see [`IndexReport::failures`]).
    pub failures: Vec<IndexError>,
}

impl IndexReport {
    /// Number of symbols.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// `true` when no function was found.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Number of files that could not be indexed.
    pub fn failed_files(&self) -> usize {
        self.failures.len()
    }
}

/// Turns source code into encoded [`FunctionSymbol`]s.
///
/// The indexer is stateless, so it is `Sync` and can be shared by the parallel
/// ingestion pipeline in the engine.
#[derive(Clone, Debug, Default)]
pub struct AstIndexer;

impl AstIndexer {
    /// Creates an indexer.
    pub fn new() -> Self {
        Self
    }

    /// Pure AST extraction, without symbolic encoding.
    ///
    /// # Errors
    ///
    /// See [`parse_source`].
    pub fn extract(
        &self,
        source: &str,
        language: Language,
    ) -> Result<Vec<FunctionNode>, IndexError> {
        parse_source(source, language)
    }

    /// Extracts and encodes every function in `source`.
    ///
    /// # Errors
    ///
    /// See [`parse_source`].
    pub fn index_source(
        &self,
        source: &str,
        language: Language,
        path: &Path,
        dir_depth: usize,
    ) -> Result<Vec<FunctionSymbol>, IndexError> {
        let functions = self.extract(source, language)?;
        Ok(functions
            .into_iter()
            .map(|function| self.encode(function, path, dir_depth, language))
            .collect())
    }

    /// Reads and indexes a discovered file.
    ///
    /// # Errors
    ///
    /// Propagates read failures and parse failures.
    pub fn index_file(&self, file: &SourceFile) -> Result<Vec<FunctionSymbol>, IndexError> {
        let source = std::fs::read_to_string(&file.path)?;
        self.index_source(&source, file.language, &file.path, file.dir_depth)
    }

    /// Discovers and indexes every supported file below `root`, single threaded.
    ///
    /// Failures on individual files are collected in [`IndexReport::failures`]
    /// instead of aborting the scan; I/O failures of the traversal itself are
    /// returned.
    ///
    /// # Errors
    ///
    /// Propagates traversal failures from [`discover_sources`].
    pub fn index_dir(&self, root: &Path) -> Result<IndexReport, IndexError> {
        self.index_dir_with(root, &crate::progress::SilentProgress)
    }

    /// [`AstIndexer::index_dir`], reporting each file and symbol through `progress`.
    ///
    /// The reporting happens *here* rather than in a caller because this loop is what knows how
    /// many files there are and which one is being read: a caller that only saw the finished
    /// report would have to discover the tree a second time to draw a bar for it.
    ///
    /// # Errors
    ///
    /// As [`AstIndexer::index_dir`].
    pub fn index_dir_with(
        &self,
        root: &Path,
        progress: &dyn crate::progress::IndexingProgress,
    ) -> Result<IndexReport, IndexError> {
        let mut report = IndexReport {
            symbols: Vec::new(),
            files_indexed: 0,
            failures: Vec::new(),
        };
        for file in discover_sources(root)? {
            progress.file(&file.path);
            match self.index_file(&file) {
                Ok(mut symbols) => {
                    for symbol in &symbols {
                        progress.symbol(&symbol.name, &symbol.path);
                    }
                    report.files_indexed += 1;
                    report.symbols.append(&mut symbols);
                }
                Err(error) => report.failures.push(error),
            }
        }
        progress.finish();
        Ok(report)
    }

    /// Encodes one extracted function.
    pub fn encode(
        &self,
        function: FunctionNode,
        path: &Path,
        dir_depth: usize,
        language: Language,
    ) -> FunctionSymbol {
        // The declaration identity seeds the stable id, so `path:line:name` is fully
        // reproducible while two same-named declarations (two `fn new` in one file, say)
        // stay distinct.
        let identity = format!("{}:{}:{}", path.display(), function.line, function.name);
        let id = seed_from_text(&identity);
        FunctionSymbol {
            id,
            name: function.name,
            path: path.to_path_buf(),
            language,
            params: function.params,
            return_type: function.return_type,
            dir_depth,
            scope_depth: function.scope_depth,
            line: function.line,
            start_byte: function.start_byte,
            end_byte: function.end_byte,
        }
    }
}
