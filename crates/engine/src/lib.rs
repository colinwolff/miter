//! # `miter-engine` — deterministic AST storage and analysis engine
//!
//! Holds the AST indexed symbols, polyglot representations, and interface extractions.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod analysis;
pub mod codec;
pub mod config;
pub mod hydrate;
pub mod ingest;
pub mod persist;
pub mod polyglot;
pub mod security;
pub mod store;

use miter_ast::IndexError;

pub use analysis::{
    AffectedTest, AuditConfig, AuditSeverity, AuditViolation, CloneDetectionReport, ContractDrift,
    ContractReport, CycleDetectionReport, DeadSymbol, DependencyCycle, DirtyScan,
    EntrypointHandler, ExtractionBlueprint, FileDependencyGraph, FunctionFingerprint,
    HygieneReport, ImpactAnalysis, SeamSlicer, SkeletonOptions, SkeletonResult,
    StructuralClonePair, TransitiveCaller, VisibilityCandidate, WorkspaceAuditor, analyze_hygiene,
    canonical_entity_name, compute_impact, detect_dirty_files, extract_fingerprints_from_rust,
    extract_references, extract_skeleton, find_clones, ingest_dirty, normalize_identifier,
    verify_contracts_from_symbols,
};
pub use codec::{
    DecodeError, MAGIC, PersistError, VERSION, decode_symbol, decode_universal, encode_symbol,
    encode_universal,
};
pub use config::{AuditToolConfig, IndexerConfig, MiterConfig, is_path_excluded};
pub use hydrate::{
    DEFAULT_MAX_TOKENS, HYDRATION_LEVEL_BAND, HydrateError, HydrateScope, HydratedContext,
    ScopedHydrateOptions, ScopedHydratedContext, estimate_tokens, hydrate_scoped,
    hydrate_symbol_context, hydrate_symbol_context_with,
};
pub use ingest::PolyglotIngest;
pub use miter_ast::{FunctionSymbol, Language};
pub use miter_storage::{
    ReferenceEntry, ReferenceKind, StorageError, WorkspaceStore, default_db_path,
};
pub use persist::{load_redb, load_redb_from, load_redb_universal, load_redb_universal_from};
pub use polyglot::{SymbolKind, UniversalSymbol};
pub use security::{
    REDACTED_SECRET, is_high_entropy_secret, is_secret_literal, is_sensitive_identifier,
    scrub_source_text,
};
pub use store::{Engine, IncrementalStats};

/// What an ingestion pass did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IngestStats {
    /// Files discovered by the traversal.
    pub files_scanned: usize,
    /// Files that parsed and encoded successfully.
    pub files_indexed: usize,
    /// Symbols added to the index.
    pub symbols: usize,
    /// Symbols skipped because their id was already present.
    pub duplicates: usize,
    /// Human-readable failures of individual files (the scan keeps going).
    pub failures: Vec<String>,
}

/// Failures raised by the engine.
#[derive(Debug)]
pub enum EngineError {
    /// Ingestion failed.
    Index(IndexError),
    /// Persistence failed.
    Persist(PersistError),
    /// The workspace ledger (or another `storage` operation) failed.
    Storage(StorageError),
    /// The polyglot grammar could not be loaded, or the source did not parse.
    Polyglot(crate::polyglot::PolyglotError),
}

impl core::fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EngineError::Index(error) => write!(formatter, "ingestion error: {error}"),
            EngineError::Persist(error) => write!(formatter, "persistence error: {error}"),
            EngineError::Storage(error) => write!(formatter, "{error}"),
            EngineError::Polyglot(error) => write!(formatter, "polyglot ingestion error: {error}"),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EngineError::Index(error) => Some(error),
            EngineError::Persist(error) => Some(error),
            EngineError::Storage(error) => Some(error),
            EngineError::Polyglot(error) => Some(error),
        }
    }
}

impl From<IndexError> for EngineError {
    fn from(error: IndexError) -> Self {
        EngineError::Index(error)
    }
}

impl From<PersistError> for EngineError {
    fn from(error: PersistError) -> Self {
        EngineError::Persist(error)
    }
}

impl From<StorageError> for EngineError {
    fn from(error: StorageError) -> Self {
        EngineError::Storage(error)
    }
}

impl From<crate::polyglot::PolyglotError> for EngineError {
    fn from(error: crate::polyglot::PolyglotError) -> Self {
        EngineError::Polyglot(error)
    }
}

/// Everything needed to drive the engine.
pub mod prelude {
    pub use crate::store::Engine;
    pub use crate::{EngineError, IngestStats};
    pub use miter_ast::{AstIndexer, FunctionSymbol, Language};
}
