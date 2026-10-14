//! Scoring a cluster and naming the file it should move to.

use std::collections::HashSet;
use std::path::Path;

use super::cluster::{self, CoupledCluster, DeclarationNode};

use super::ExtractionBlueprint;

pub(super) fn resolve_type_name(node: tree_sitter::Node<'_>, content: &str) -> Option<String> {
    match node.kind() {
        "type_identifier" => {
            let s = content.get(node.byte_range())?.trim();
            if !s.is_empty() {
                Some(s.to_string())
            } else {
                None
            }
        }
        "generic_type" => {
            let type_id = node.child_by_field_name("type")?;
            resolve_type_name(type_id, content)
        }
        "scoped_type_identifier" => {
            let name_node = node.child_by_field_name("name")?;
            resolve_type_name(name_node, content)
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(res) = resolve_type_name(child, content) {
                    return Some(res);
                }
            }
            let s = content.get(node.byte_range())?.trim();
            if !s.is_empty() {
                Some(s.to_string())
            } else {
                None
            }
        }
    }
}

pub(super) fn build_blueprint(
    path: &Path,
    total_lines: usize,
    cluster: &CoupledCluster,
    all_decls: &[DeclarationNode],
) -> ExtractionBlueprint {
    let source_file = path.display().to_string();

    // Order-preserving deduplication for symbols_to_move
    let mut seen_symbols = HashSet::new();
    let mut symbols = Vec::new();
    for decl in &cluster.declarations {
        if seen_symbols.insert(decl.name.clone()) {
            symbols.push(decl.name.clone());
        }
    }

    let mod_name = cluster::derive_candidate_module_name(cluster);
    let candidate_file = derive_candidate_path(path, &mod_name);

    let mut suggested_exports = Vec::new();
    let mut seen_exports = HashSet::new();
    let mut parent_elevations = Vec::new();
    let mut seen_parent_elevations = HashSet::new();
    let mut suggested_imports = HashSet::new();
    let cluster_syms: HashSet<&str> = symbols.iter().map(String::as_str).collect();

    for decl in &cluster.declarations {
        if !decl.is_exported && is_exportable_kind(&decl.kind) && !decl.name.starts_with("impl") {
            let export_decl = format!("pub(crate) {}", decl.name);
            if seen_exports.insert(export_decl.clone()) {
                suggested_exports.push(export_decl);
            }
        }
        for ref_name in &decl.local_references {
            if !cluster_syms.contains(ref_name.as_str()) {
                suggested_imports.insert(format!("super::{ref_name}"));
                // If the referenced symbol in the parent is not exported, elevate it in the parent!
                if let Some(parent_decl) = all_decls.iter().find(|d| d.name == *ref_name)
                    && !parent_decl.is_exported
                    && is_exportable_kind(&parent_decl.kind)
                    && !ref_name.starts_with("impl")
                {
                    let elevation = format!("pub(crate) {ref_name}");
                    if seen_parent_elevations.insert(elevation.clone()) {
                        parent_elevations.push(elevation);
                    }
                }
            }
        }
    }

    for other in all_decls {
        if !cluster_syms.contains(other.name.as_str()) {
            for ref_name in &other.local_references {
                if cluster_syms.contains(ref_name.as_str()) {
                    let decl_match = cluster.declarations.iter().find(|d| d.name == *ref_name);
                    let is_exportable = decl_match.is_none_or(|d| is_exportable_kind(&d.kind));
                    if is_exportable && !ref_name.starts_with("impl") {
                        let export_decl = format!("pub(crate) {ref_name}");
                        if seen_exports.insert(export_decl.clone()) {
                            suggested_exports.push(export_decl);
                        }
                    }
                }
            }
        }
    }

    let mut imports: Vec<String> = suggested_imports.into_iter().collect();
    imports.sort();

    let estimated_remaining_lines = total_lines.saturating_sub(cluster.total_lines);
    // Circular risk if parent elevation is required while candidate imports symbols from parent
    let has_circular_risk = !parent_elevations.is_empty() && !imports.is_empty();

    ExtractionBlueprint {
        source_file,
        candidate_file,
        line_range: cluster.line_range,
        byte_range: cluster.byte_range,
        symbols_to_move: symbols,
        suggested_exports,
        parent_elevations,
        suggested_imports: imports,
        suggested_mod_decl: format!("mod {mod_name};"),
        estimated_remaining_lines,
        has_circular_risk,
    }
}

fn derive_candidate_path(path: &Path, mod_name: &str) -> String {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("rs");

    if stem == "mod" || stem == "lib" || stem == "main" {
        parent
            .join(format!("{mod_name}.{ext}"))
            .display()
            .to_string()
    } else {
        parent
            .join(stem)
            .join(format!("{mod_name}.{ext}"))
            .display()
            .to_string()
    }
}

fn is_exportable_kind(kind: &str) -> bool {
    matches!(
        kind,
        "struct_item"
            | "enum_item"
            | "type_item"
            | "function_item"
            | "trait_item"
            | "struct"
            | "enum"
            | "type"
            | "fn"
            | "trait"
    )
}
