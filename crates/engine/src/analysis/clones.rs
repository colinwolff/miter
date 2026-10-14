//! Deterministic AST Structural Clone Detection.
//!
//! Extracts structural control-flow sequences from function ASTs (ignoring variable names
//! and literal values), computes deterministic 128-bit SimHash bitvectors, and identifies
//! functions with >= 90% structural similarity across files.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tree_sitter::Node;

/// A function AST structural fingerprint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionFingerprint {
    /// Defining file path.
    pub file_path: String,
    /// Function name.
    pub name: String,
    /// 1-indexed (start_line, end_line) span.
    pub line_span: (usize, usize),
    /// Line count of the function body.
    pub line_count: usize,
    /// 128-bit SimHash structural fingerprint (stored as two u64s: high, low).
    pub simhash: (u64, u64),
}

/// A pair of structurally duplicated functions flagged for consolidation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StructuralClonePair {
    /// Primary function definition.
    pub first: FunctionFingerprint,
    /// Duplicate/clone function definition.
    pub second: FunctionFingerprint,
    /// Structural similarity score in [0.0, 1.0].
    pub similarity: f32,
    /// Bitwise Hamming distance (0..=128).
    pub hamming_distance: u32,
}

/// Summary report of detected AST structural clones.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CloneDetectionReport {
    /// Total functions evaluated (>= min_lines).
    pub functions_analyzed: usize,
    /// Detected clone pairs with similarity >= min_similarity.
    pub clone_pairs: Vec<StructuralClonePair>,
}

/// Computes bitwise Hamming distance between two 128-bit fingerprints.
#[must_use]
pub fn hamming_distance(a: (u64, u64), b: (u64, u64)) -> u32 {
    (a.0 ^ b.0).count_ones() + (a.1 ^ b.1).count_ones()
}

/// Computes normalized structural similarity in [0.0, 1.0] from Hamming distance.
#[must_use]
pub fn similarity_score(dist: u32) -> f32 {
    1.0 - (dist as f32 / 128.0)
}

mod simhash;

pub use simhash::{collect_structural_tokens, compute_simhash};
/// Extracts fingerprints for all functions in `content` >= `min_lines`.
#[must_use]
pub fn extract_fingerprints_from_rust(
    file_path: &Path,
    content: &str,
    min_lines: usize,
) -> Vec<FunctionFingerprint> {
    let mut parser = tree_sitter::Parser::new();
    let Ok(grammar) = miter_ast::grammar_for(miter_ast::Language::Rust) else {
        return Vec::new();
    };
    if parser.set_language(&grammar).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(content, None) else {
        return Vec::new();
    };

    let mut fingerprints = Vec::new();
    find_functions(
        tree.root_node(),
        file_path,
        content,
        min_lines,
        &mut fingerprints,
    );
    fingerprints
}

fn find_functions(
    node: Node<'_>,
    file_path: &Path,
    content: &str,
    min_lines: usize,
    out: &mut Vec<FunctionFingerprint>,
) {
    if node.kind() == "function_item" {
        let line_span = (node.start_position().row + 1, node.end_position().row + 1);
        let line_count = line_span.1.saturating_sub(line_span.0) + 1;

        if line_count >= min_lines {
            let name = node
                .child_by_field_name("name")
                .and_then(|n| content.get(n.byte_range()))
                .unwrap_or("unnamed")
                .to_string();

            let mut tokens = Vec::new();
            if let Some(body) = node.child_by_field_name("body") {
                collect_structural_tokens(body, 1, &mut tokens);
            }

            let simhash = compute_simhash(&tokens);
            out.push(FunctionFingerprint {
                file_path: file_path.to_string_lossy().replace('\\', "/"),
                name,
                line_span,
                line_count,
                simhash,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_functions(child, file_path, content, min_lines, out);
    }
}

/// Analyzes fingerprints and finds structural clone pairs across different files.
#[must_use]
pub fn find_clones(
    fingerprints: &[FunctionFingerprint],
    min_similarity: f32,
) -> CloneDetectionReport {
    let mut clone_pairs = Vec::new();
    let max_allowed_distance = ((1.0 - min_similarity) * 128.0).floor() as u32;

    for i in 0..fingerprints.len() {
        for j in (i + 1)..fingerprints.len() {
            let f1 = &fingerprints[i];
            let f2 = &fingerprints[j];

            // Only compare across different files
            if f1.file_path == f2.file_path {
                continue;
            }

            let dist = hamming_distance(f1.simhash, f2.simhash);
            if dist <= max_allowed_distance {
                let similarity = similarity_score(dist);
                clone_pairs.push(StructuralClonePair {
                    first: f1.clone(),
                    second: f2.clone(),
                    similarity,
                    hamming_distance: dist,
                });
            }
        }
    }

    clone_pairs.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
    CloneDetectionReport {
        functions_analyzed: fingerprints.len(),
        clone_pairs,
    }
}
