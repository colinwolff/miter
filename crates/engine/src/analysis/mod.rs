//! Architectural analysis and AST seam slicing.
//!
//! Exposes:
//! - [`audit`]: Scans workspace source files for line-count threshold violations.
//! - [`mod@slice`]: Analyzes AST co-reference graphs to identify module seams and blueprint extractions.
//! - [`cluster`]: Partitions symbol subgraphs into cohesive ~200–250 line extraction clusters.

pub mod audit;
pub mod clones;
pub mod cluster;
pub mod contracts;
pub mod cycles;
pub mod dirty;
pub mod hygiene;
pub mod impact;
pub mod references;
pub mod skeleton;
pub mod slice;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_clones;
#[cfg(test)]
mod tests_contracts;
#[cfg(test)]
mod tests_references;
#[cfg(test)]
mod tests_skeleton;

pub use audit::{AuditConfig, AuditSeverity, AuditViolation, WorkspaceAuditor};
pub use clones::{
    CloneDetectionReport, FunctionFingerprint, StructuralClonePair, compute_simhash,
    extract_fingerprints_from_rust, find_clones, hamming_distance, similarity_score,
};
pub use cluster::{CoupledCluster, DeclarationNode};
pub use contracts::{
    ContractDrift, ContractReport, EntityField, MissingFieldDrift, PolyglotEntity,
    canonical_entity_name, normalize_identifier, verify_contracts_from_symbols,
};
pub use cycles::{CycleDetectionReport, DependencyCycle, FileDependencyGraph};
pub use dirty::{DirtyScan, detect_dirty_files, ingest_dirty};
pub use hygiene::{DeadSymbol, HygieneReport, VisibilityCandidate, analyze_hygiene};
pub use impact::{
    AffectedTest, EntrypointHandler, ImpactAnalysis, TransitiveCaller, compute_impact,
};
pub use references::extract_references;
pub use skeleton::{SkeletonOptions, SkeletonResult, extract_skeleton};
pub use slice::{ExtractionBlueprint, SeamSlicer};
