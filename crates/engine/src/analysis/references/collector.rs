//! The tree walk: one syntax tree in, one reference per call, type and import out.

use miter_storage::schema::{ReferenceEntry, ReferenceKind};

use super::{leaf_ident_of, text_of};

pub(super) struct Collector<'a> {
    pub(super) source: &'a str,
    pub(super) file_path: String,
    pub(super) lang: miter_ast::Language,
    pub(super) entries: Vec<(String, ReferenceEntry)>,
}

impl Collector<'_> {
    pub(super) fn walk(&mut self, node: tree_sitter::Node<'_>, enclosing: Option<&str>) {
        // 1. Determine if this node introduces or changes the enclosing scope
        let current_scope = self.scope_name_of(node);
        let active_enclosing = current_scope.as_deref().or(enclosing);

        // 2. Identify references based on grammar node kind
        match self.lang {
            miter_ast::Language::Rust => self.extract_rust_reference(node, active_enclosing),
            miter_ast::Language::TypeScript
            | miter_ast::Language::Tsx
            | miter_ast::Language::JavaScript => self.extract_ts_reference(node, active_enclosing),
            _ => {}
        }

        // 3. Recurse into children
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.walk(child, active_enclosing);
            }
        }
    }

    fn extract_rust_reference(&mut self, node: tree_sitter::Node<'_>, enclosing: Option<&str>) {
        match node.kind() {
            "call_expression" => {
                if let Some(function) = node.child_by_field_name("function")
                    && let Some(ident) = leaf_ident_of(function, self.source)
                {
                    self.push_ref(ident, node, enclosing, ReferenceKind::Call);
                }
            }
            "field_identifier" => {
                if let Some(parent) = node.parent()
                    && matches!(parent.kind(), "field_expression" | "method_call_expression")
                    && let Some(ident) = text_of(node, self.source)
                {
                    self.push_ref(ident, node, enclosing, ReferenceKind::Call);
                }
            }
            "type_identifier" => {
                if let Some(parent) = node.parent()
                    && !matches!(
                        parent.kind(),
                        "struct_item" | "enum_item" | "union_item" | "trait_item"
                    )
                    && let Some(ident) = text_of(node, self.source)
                {
                    self.push_ref(ident, node, enclosing, ReferenceKind::TypeUsage);
                }
            }
            "scoped_identifier" | "scoped_type_identifier" => {
                if let Some(parent) = node.parent()
                    && parent.kind() != "use_declaration"
                {
                    if let Some(path) = node
                        .child_by_field_name("path")
                        .and_then(|p| leaf_ident_of(p, self.source))
                    {
                        self.push_ref(path, node, enclosing, ReferenceKind::TypeUsage);
                    }
                    if let Some(name) = node
                        .child_by_field_name("name")
                        .and_then(|n| text_of(n, self.source))
                    {
                        let k = if node.kind() == "scoped_type_identifier" {
                            ReferenceKind::TypeUsage
                        } else {
                            ReferenceKind::Call
                        };
                        self.push_ref(name, node, enclosing, k);
                    }
                }
            }
            "use_declaration" => {
                if let Some(ident) = leaf_ident_of(node, self.source) {
                    self.push_ref(ident, node, enclosing, ReferenceKind::Import);
                }
            }
            "impl_item" => {
                if let Some(trait_node) = node.child_by_field_name("trait")
                    && let Some(ident) = leaf_ident_of(trait_node, self.source)
                {
                    self.push_ref(ident, trait_node, enclosing, ReferenceKind::TraitImpl);
                }
            }
            _ => {}
        }
    }

    fn extract_ts_reference(&mut self, node: tree_sitter::Node<'_>, enclosing: Option<&str>) {
        match node.kind() {
            "call_expression" => {
                if let Some(function) = node.child_by_field_name("function")
                    && let Some(ident) = leaf_ident_of(function, self.source)
                {
                    self.push_ref(ident, node, enclosing, ReferenceKind::Call);
                }
            }
            "property_identifier" => {
                if let Some(parent) = node.parent()
                    && matches!(parent.kind(), "member_expression" | "call_expression")
                    && let Some(ident) = text_of(node, self.source)
                {
                    self.push_ref(ident, node, enclosing, ReferenceKind::Call);
                }
            }
            "type_identifier" | "predefined_type" => {
                if let Some(parent) = node.parent()
                    && !matches!(
                        parent.kind(),
                        "interface_declaration" | "type_alias_declaration" | "class_declaration"
                    )
                    && let Some(ident) = text_of(node, self.source)
                {
                    self.push_ref(ident, node, enclosing, ReferenceKind::TypeUsage);
                }
            }
            "import_specifier" => {
                if let Some(name_node) = node.child_by_field_name("name")
                    && let Some(ident) = text_of(name_node, self.source)
                {
                    self.push_ref(ident, node, enclosing, ReferenceKind::Import);
                }
            }
            _ => {}
        }
    }

    fn push_ref(
        &mut self,
        target_symbol: String,
        node: tree_sitter::Node<'_>,
        enclosing: Option<&str>,
        kind: ReferenceKind,
    ) {
        let bare_symbol = if let Some((_, last)) = target_symbol.rsplit_once("::") {
            last.trim_end_matches(';').to_string()
        } else if let Some((_, last)) = target_symbol.rsplit_once('.') {
            last.trim_end_matches(';').to_string()
        } else {
            target_symbol.trim_end_matches(';').to_string()
        };

        if bare_symbol.is_empty() {
            return;
        }

        let byte_range = (node.start_byte(), node.end_byte());
        let line_range = (node.start_position().row + 1, node.end_position().row + 1);

        self.entries.push((
            bare_symbol,
            ReferenceEntry {
                file_path: self.file_path.clone(),
                byte_range,
                line_range,
                enclosing_symbol: enclosing.map(str::to_owned),
                reference_kind: kind,
            },
        ));
    }

    fn scope_name_of(&self, node: tree_sitter::Node<'_>) -> Option<String> {
        let kind = node.kind();
        if !matches!(
            kind,
            "function_item"
                | "struct_item"
                | "enum_item"
                | "trait_item"
                | "impl_item"
                | "function_declaration"
                | "method_definition"
                | "class_declaration"
                | "interface_declaration"
        ) {
            return None;
        }
        if let Some(name) = node.child_by_field_name("name") {
            text_of(name, self.source)
        } else if kind == "impl_item" {
            node.child_by_field_name("type")
                .and_then(|t| text_of(t, self.source))
        } else {
            None
        }
    }
}
