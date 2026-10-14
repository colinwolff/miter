//! Reading a file's declarations out of its syntax tree.

use std::collections::HashSet;

use super::blueprint::resolve_type_name;
use super::cluster::DeclarationNode;

pub(super) fn extract_declarations(content: &str) -> Vec<DeclarationNode> {
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

    let root = tree.root_node();
    let mut all_names = HashSet::new();
    let mut raw_decls = Vec::new();
    let mut raw_impls = Vec::new();

    for i in 0..root.child_count() {
        let Some(child) = root.child(i) else { continue };
        let kind = child.kind();
        if kind == "impl_item" {
            let target_type = child
                .child_by_field_name("type")
                .and_then(|t| resolve_type_name(t, content));
            raw_impls.push((child, target_type));
            continue;
        }

        if !matches!(
            kind,
            "function_item" | "struct_item" | "enum_item" | "trait_item" | "type_item"
        ) {
            continue;
        }

        let name = child
            .child_by_field_name("name")
            .map(|n| content.get(n.byte_range()).unwrap_or(""))
            .filter(|s| !s.is_empty())
            .unwrap_or("");

        if !name.is_empty() {
            all_names.insert(name.to_string());
            raw_decls.push((child, name.to_string(), kind.to_string()));
        }
    }

    let mut declarations = Vec::new();
    for (node, name, kind) in raw_decls {
        let mut byte_span = (node.start_byte(), node.end_byte());
        let mut line_span = (node.start_position().row + 1, node.end_position().row + 1);

        // Include preceding doc comments or comments attached to this declaration
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "line_comment" || sibling.kind() == "block_comment" {
                byte_span.0 = byte_span.0.min(sibling.start_byte());
                line_span.0 = line_span.0.min(sibling.start_position().row + 1);
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }

        let mut local_refs = HashSet::new();
        let decl_text = content.get(byte_span.0..byte_span.1).unwrap_or("");
        for candidate in &all_names {
            if candidate != &name && decl_text.contains(candidate.as_str()) {
                local_refs.insert(candidate.clone());
            }
        }

        // Attach associated inherent and trait impl blocks in the same file
        let mut impl_count = 0;
        for (impl_node, target_type) in &raw_impls {
            if target_type.as_deref() == Some(&name) {
                impl_count += 1;
                let mut i_byte = (impl_node.start_byte(), impl_node.end_byte());
                let mut i_line = (
                    impl_node.start_position().row + 1,
                    impl_node.end_position().row + 1,
                );

                let mut i_prev = impl_node.prev_sibling();
                while let Some(sibling) = i_prev {
                    if sibling.kind() == "line_comment" || sibling.kind() == "block_comment" {
                        i_byte.0 = i_byte.0.min(sibling.start_byte());
                        i_line.0 = i_line.0.min(sibling.start_position().row + 1);
                        i_prev = sibling.prev_sibling();
                    } else {
                        break;
                    }
                }

                byte_span.0 = byte_span.0.min(i_byte.0);
                byte_span.1 = byte_span.1.max(i_byte.1);
                line_span.0 = line_line_min(line_span.0, i_line.0);
                line_span.1 = line_span.1.max(i_line.1);

                let impl_text = content.get(i_byte.0..i_byte.1).unwrap_or("");
                for candidate in &all_names {
                    if candidate != &name && impl_text.contains(candidate.as_str()) {
                        local_refs.insert(candidate.clone());
                    }
                }
            }
        }

        let is_exported = decl_text.trim_start().starts_with("pub");

        declarations.push(DeclarationNode {
            name,
            kind,
            byte_span,
            line_span,
            local_references: local_refs,
            is_exported,
            associated_impl_count: impl_count,
        });
    }

    declarations
}

#[inline]
fn line_line_min(a: usize, b: usize) -> usize {
    a.min(b)
}
