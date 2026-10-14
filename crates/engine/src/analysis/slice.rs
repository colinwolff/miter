//! AST Seam Slicer.
//!
//! Analyzes AST co-reference graphs to identify natural module boundaries
//! and emits structured extraction blueprints targeting §14 modularity (200–250 lines).

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::cluster;

/// Structured extraction blueprint for refactoring large files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionBlueprint {
    /// Original source file containing the cluster.
    pub source_file: String,
    /// Derived candidate module file path (e.g., "src/cli/init.rs").
    pub candidate_file: String,
    /// 1-indexed (start_line, end_line) span in source file.
    pub line_range: (usize, usize),
    /// (start_byte, end_byte) span in source file.
    pub byte_range: (usize, usize),
    /// Symbol identifiers to move into candidate file.
    pub symbols_to_move: Vec<String>,
    /// Suggested visibility updates needed in child submodule (e.g. `["pub(crate) struct InitArgs"]`).
    pub suggested_exports: Vec<String>,
    /// Suggested visibility updates needed in parent module (e.g. `["pub(crate) fn helper"]`).
    pub parent_elevations: Vec<String>,
    /// Types or symbols required from parent or sibling modules.
    pub suggested_imports: Vec<String>,
    /// Module declaration to add in parent file (e.g. "mod init;").
    pub suggested_mod_decl: String,
    /// Estimated remaining line count of source file after extraction.
    pub estimated_remaining_lines: usize,
    /// Whether any cyclic dependency risk exists in the proposed candidate structure.
    pub has_circular_risk: bool,
}

/// Seam Slicer analyzer.
#[derive(Clone, Debug)]
pub struct SeamSlicer {
    target_lines: usize,
}

impl Default for SeamSlicer {
    fn default() -> Self {
        Self { target_lines: 220 }
    }
}

impl SeamSlicer {
    /// Creates a slicer with default target lines (250).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the target cluster line size.
    #[must_use]
    pub fn with_target_lines(target_lines: usize) -> Self {
        Self { target_lines }
    }

    /// Parses the target source file and generates extraction blueprints.
    pub fn slice_file(&self, path: &Path, content: &str) -> Vec<ExtractionBlueprint> {
        let total_lines = content.lines().count();
        let declarations = extract_declarations(content);
        if declarations.is_empty() {
            return Vec::new();
        }

        let clusters = cluster::partition_into_clusters(&declarations, self.target_lines);
        let mut blueprints = Vec::new();

        for cluster in clusters {
            if cluster.declarations.is_empty() {
                continue;
            }
            let blueprint = build_blueprint(path, total_lines, &cluster, &declarations);
            blueprints.push(blueprint);
        }

        blueprints
    }
}

mod blueprint;
mod declarations;

use blueprint::build_blueprint;
use declarations::extract_declarations;
