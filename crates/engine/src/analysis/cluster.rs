//! AST declaration node representation and graph clustering.
//!
//! Partitions top-level declarations into extraction clusters targeting
//! ~200–250 lines based on co-reference dependencies.

use std::collections::HashSet;

/// A top-level declaration node extracted from an AST.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationNode {
    /// Identifier / symbol name (e.g., "InitArgs", "run_init").
    pub name: String,
    /// Kind of declaration (e.g., "struct", "function", "enum").
    pub kind: String,
    /// 0-indexed byte span: `(start_byte, end_byte)`.
    pub byte_span: (usize, usize),
    /// 1-indexed line span: `(start_line, end_line)`.
    pub line_span: (usize, usize),
    /// Names of other local declarations referenced within this node.
    pub local_references: HashSet<String>,
    /// Whether this declaration is marked `pub` or exported.
    pub is_exported: bool,
    /// Number of associated `impl` blocks attached to this declaration.
    pub associated_impl_count: usize,
}

impl DeclarationNode {
    /// Number of physical lines spanned by this declaration.
    #[must_use]
    pub fn line_count(&self) -> usize {
        (self.line_span.1.saturating_sub(self.line_span.0)) + 1
    }

    /// Whether this declaration is a root command orchestrator / entrypoint.
    #[must_use]
    pub fn is_root_orchestrator(&self) -> bool {
        (self.kind == "function_item" || self.kind == "function" || self.kind == "fn")
            && (self.name == "run"
                || self.name.starts_with("run_")
                || self.name.starts_with("handle_"))
    }
}

/// A cohesive cluster of declarations candidate for extraction into a separate module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoupledCluster {
    /// Declarations belonging to this cluster.
    pub declarations: Vec<DeclarationNode>,
    /// Combined 1-indexed line span: `(start_line, end_line)`.
    pub line_range: (usize, usize),
    /// Combined byte span: `(start_byte, end_byte)`.
    pub byte_range: (usize, usize),
    /// Estimated line count of the cluster.
    pub total_lines: usize,
}

/// Partitions declaration nodes into clusters targeting `target_lines` (default ~200–250).
///
/// Root command orchestrators (e.g., `run_*`, `handle_*`, `run`) that depend on other local
/// declarations are retained in the parent file to prevent circular dependencies.
pub fn partition_into_clusters(
    declarations: &[DeclarationNode],
    target_lines: usize,
) -> Vec<CoupledCluster> {
    if declarations.is_empty() {
        return Vec::new();
    }

    // Filter out root orchestrators that depend on local declarations
    let extractable: Vec<DeclarationNode> = declarations
        .iter()
        .filter(|d| !(d.is_root_orchestrator() && !d.local_references.is_empty()))
        .cloned()
        .collect();

    if extractable.is_empty() {
        return Vec::new();
    }

    let mut sorted_decls = extractable;
    sorted_decls.sort_by_key(|d| d.line_span.0);

    let mut clusters = Vec::new();
    let mut current_cluster: Vec<DeclarationNode> = Vec::new();
    let mut current_lines = 0;

    for decl in sorted_decls {
        let decl_lines = decl.line_count();

        // If the declaration itself is large (>= target_lines), emit current cluster
        // and emit this decl as its own cluster.
        if decl_lines >= target_lines {
            if !current_cluster.is_empty() {
                clusters.push(make_cluster(std::mem::take(&mut current_cluster)));
                current_lines = 0;
            }
            clusters.push(make_cluster(vec![decl]));
            continue;
        }

        // If adding this decl would exceed target_lines and we already have decls
        if current_lines + decl_lines > target_lines && !current_cluster.is_empty() {
            clusters.push(make_cluster(std::mem::take(&mut current_cluster)));
            current_lines = 0;
        }

        current_lines += decl_lines;
        current_cluster.push(decl);
    }

    if !current_cluster.is_empty() {
        clusters.push(make_cluster(current_cluster));
    }

    clusters.sort_by_key(|c| c.line_range.0);
    clusters
}

fn make_cluster(mut decls: Vec<DeclarationNode>) -> CoupledCluster {
    decls.sort_by_key(|d| d.line_span.0);
    let start_line = decls.first().map(|d| d.line_span.0).unwrap_or(1);
    let end_line = decls.last().map(|d| d.line_span.1).unwrap_or(1);
    let start_byte = decls.first().map(|d| d.byte_span.0).unwrap_or(0);
    let end_byte = decls.last().map(|d| d.byte_span.1).unwrap_or(0);
    let total_lines = decls.iter().map(|d| d.line_count()).sum();

    CoupledCluster {
        declarations: decls,
        line_range: (start_line, end_line),
        byte_range: (start_byte, end_byte),
        total_lines,
    }
}

/// Derives a candidate submodule name from cluster declarations using smart prioritization.
///
/// Priority 1 (Type Sovereignty): If types exist, pick the type with the most associated
/// impl items (dominant group), or the first type in line order.
/// Priority 2 (Function Fallback): If only functions exist, pick the first non-helper function.
#[must_use]
pub fn derive_candidate_module_name(cluster: &CoupledCluster) -> String {
    // 1. Check for structs and enums
    let mut types: Vec<&DeclarationNode> = cluster
        .declarations
        .iter()
        .filter(|d| is_type_kind(&d.kind))
        .collect();

    if !types.is_empty() {
        // Sort by associated_impl_count descending, keeping stable line order for ties
        types.sort_by(|a, b| {
            b.associated_impl_count
                .cmp(&a.associated_impl_count)
                .then_with(|| a.line_span.0.cmp(&b.line_span.0))
        });
        return to_snake_case(&types[0].name);
    }

    // 2. Fallback to standalone functions (prefer non-helper)
    let fns: Vec<&DeclarationNode> = cluster
        .declarations
        .iter()
        .filter(|d| is_fn_kind(&d.kind))
        .collect();

    if let Some(target_fn) = fns.iter().find(|d| !is_helper_name(&d.name)) {
        return to_snake_case(&target_fn.name);
    }
    if let Some(first_fn) = fns.first() {
        return to_snake_case(&first_fn.name);
    }

    // 3. Last resort: first declaration name or "submodule"
    cluster
        .declarations
        .first()
        .map(|d| to_snake_case(&d.name))
        .unwrap_or_else(|| "submodule".to_string())
}

fn is_type_kind(kind: &str) -> bool {
    matches!(
        kind,
        "struct_item" | "enum_item" | "struct" | "enum" | "type_item" | "type"
    )
}

fn is_fn_kind(kind: &str) -> bool {
    matches!(kind, "function_item" | "function" | "fn")
}

fn is_helper_name(name: &str) -> bool {
    name.starts_with('_') || name.starts_with("test_") || name.starts_with("helper_")
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    let trimmed = out.trim_start_matches('_');
    if trimmed.is_empty() {
        "submodule".to_string()
    } else {
        trimmed.to_string()
    }
}
