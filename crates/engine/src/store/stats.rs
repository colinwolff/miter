//! What a tracked ingestion pass did.

/// What a tracked ingestion pass did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IncrementalStats {
    /// Files discovered by the traversal.
    pub files_scanned: usize,
    /// Files that parsed and were encoded this pass.
    pub files_indexed: usize,
    /// Files whose digest was already in the ledger, so they were not parsed.
    pub files_skipped: usize,
    /// Symbols added to the index.
    pub symbols: usize,
    /// Human-readable failures of individual files (the scan keeps going).
    pub failures: Vec<String>,
}

impl IncrementalStats {
    /// A one-line summary, in the shape the CLIs print.
    pub fn summary(&self) -> String {
        format!(
            "{} symbols from {} changed of {} files ({} unchanged)",
            self.symbols, self.files_indexed, self.files_scanned, self.files_skipped
        )
    }
}
