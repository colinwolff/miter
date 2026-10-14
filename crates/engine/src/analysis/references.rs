//! AST Identifier Reference Extraction.
//!
//! Traverses Tree-Sitter concrete syntax trees to extract call expressions,
//! path expressions, type annotations, and trait implementations with exact
//! byte and line coordinates and enclosing scope context.

use std::path::Path;

use miter_storage::schema::ReferenceEntry;

/// Extracts the names `source` refers to, for the file at `path`.
///
/// The format decides the reader: markup, stylesheets and SQL have their own scanners, and the
/// tree-sitter walker reads the five code languages. A format with no reference model at all —
/// Markdown, a Dockerfile, a manifest — reports nothing rather than being parsed as Rust, which
/// is what a default-to-Rust fallback would have done to a stylesheet.
#[must_use]
pub fn extract_references(path: &Path, source: &str) -> Vec<(String, ReferenceEntry)> {
    let file_path = path.to_string_lossy().replace('\\', "/");
    let Some(language) = crate::polyglot::Language::from_path(path) else {
        return Vec::new();
    };
    if language.is_declarative() {
        return match language {
            crate::polyglot::Language::Html => markup::html(&file_path, source),
            crate::polyglot::Language::Css => markup::css(&file_path, source),
            crate::polyglot::Language::Sql => markup::sql(&file_path, source),
            crate::polyglot::Language::Dockerfile => markup::docker(&file_path, source),
            _ => Vec::new(),
        };
    }
    let Some(index_language) = language.index_language() else {
        return Vec::new();
    };
    let Ok(grammar) = miter_ast::grammar_for(index_language) else {
        return Vec::new();
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&grammar).is_err() {
        return Vec::new();
    }

    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };

    let mut collector = Collector {
        source,
        file_path,
        lang: index_language,
        entries: Vec::new(),
    };

    collector.walk(tree.root_node(), None);
    collector.entries
}

mod collector;
mod markup;

use collector::Collector;
pub(super) fn text_of(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    let s = source.get(node.byte_range())?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

pub(super) fn leaf_ident_of(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" | "field_identifier" | "type_identifier" => text_of(node, source),
        "scoped_identifier" | "field_expression" | "member_expression" => {
            let child = node
                .child_by_field_name("field")
                .or_else(|| node.child_by_field_name("name"))
                .or_else(|| node.child_by_field_name("property"));
            child.map_or_else(|| text_of(node, source), |c| text_of(c, source))
        }
        _ => node
            .child_by_field_name("name")
            .map_or_else(|| text_of(node, source), |n| text_of(n, source)),
    }
}
