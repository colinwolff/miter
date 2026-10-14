//! The TypeScript and TSX visitor: declarations and JSDoc.
//!
//! | node | kind | level |
//! |---|---|---|
//! | `internal_module` / `module` | [`SymbolKind::Module`] | 1 |
//! | `class_declaration` / `abstract_class_declaration` / `class` | [`SymbolKind::Class`] | 2 |
//! | `interface_declaration` | [`SymbolKind::Interface`] | 2 |
//! | `type_alias_declaration` | [`SymbolKind::TypeAlias`] | 2 |
//! | `enum_declaration` | [`SymbolKind::Enum`] | 2 |
//! | `function_declaration` / `generator_function_declaration` | [`SymbolKind::Function`] | 3 |
//! | `method_definition` / `method_signature` / `abstract_method_signature` | [`SymbolKind::Method`] | 3 |
//! | `variable_declarator` holding a function | [`SymbolKind::Method`] or [`SymbolKind::Function`] | 3 |
//! | `property_signature` / `public_field_definition` | [`SymbolKind::Property`] | 3 |
//!
//! Two shapes make a TypeScript file different from a Rust one, and both are read rather
//! than approximated:
//!
//! * **A function often has no declaration node.** `const fetchUser = async (id: string)
//!   => …` is a `variable_declarator` whose *value* is the function, and the name comes from
//!   the binding. The visitor therefore resolves the function node first and reads the
//!   parameters, the return type and the body off whichever node actually holds them — so a
//!   `const` hook and a `function` declaration are described by the same code path.
//! * **A namespace is a module.** `namespace Internals { … }` is an `internal_module`, Level
//!   1 like Rust's `mod`, so a helper declared inside it is depth 3 rather than depth 2.
//!
//! ## JSDoc
//!
//! The doc block is a `/** … */` comment immediately above the declaration
//! (`common::is_jsdoc`): a `//` line comment is not a doc comment, and a block separated
//! from its declaration by anything is not attached. A comment written *outside* an `export`
//! belongs to the export statement rather than to the function, so the scan starts at the
//! outermost node that could own it.
//!
//! ## Exports
//!
//! [`UniversalSymbol::exported`] is `true` when an `export` statement encloses the
//! declaration (`export class`, `export const`, `export default function`). A re-export
//! (`export { a }`) carries no declaration and is therefore not a symbol.

use std::path::Path;

use tree_sitter::Node;

use super::{Language, SymbolKind, UniversalSymbol, common};

/// Every declaration in a parsed TypeScript or TSX file, in source order.
pub(crate) fn visit(
    path: &Path,
    source: &str,
    root: Node<'_>,
    language: Language,
) -> Vec<UniversalSymbol> {
    let mut visitor = Visitor {
        path,
        source,
        language,
        module: common::module_path(path),
        symbols: Vec::new(),
    };
    let mut chain: Vec<String> = Vec::new();
    visitor.walk(root, &mut chain, 0, false);
    visitor.symbols
}

struct Visitor<'a> {
    path: &'a Path,
    source: &'a str,
    language: Language,
    module: String,
    symbols: Vec<UniversalSymbol>,
}

impl Visitor<'_> {
    /// Walks one node: records it if it declares something, then descends.
    fn walk(&mut self, node: Node<'_>, chain: &mut Vec<String>, functions: usize, in_type: bool) {
        // A function-valued binding (`const f = () => …`, `fetch = async () => …`) is a
        // declaration whose *value* is the function; `function_node` is what tells the two
        // apart, and the declarator keeps the identity.
        let declared = declaration(node, in_type);
        let function = function_node(node);

        let mut pushed = false;
        let mut frame = false;
        if let Some(kind) = declared
            && let Some(name) = self.name_of(node)
        {
            let depth = common::level(common::base_level(kind), functions);
            // The node that *holds* the parameters, the return type and the body: the binding's
            // value when the declaration is a `const`-style binding, and the declaration node
            // itself when it is a `function`/`method` declaration. One expression, so the two
            // shapes cannot be described differently.
            let signature_node = function.unwrap_or(node);
            let body = signature_node.child_by_field_name("body");
            let (params, return_type) = miter_ast::signature_of(
                signature_node,
                self.source,
                self.language
                    .index_language()
                    .unwrap_or(miter_ast::Language::Tsx),
            );
            let doc = common::comments_above(self.doc_node(node), self.source, common::is_jsdoc);
            self.symbols.push(UniversalSymbol {
                path: common::dotted_path(&self.module, chain, &name),
                name: name.clone(),
                language: self.language,
                kind,
                depth,
                docstring: doc,
                signature: common::header_of(node, self.source, body),
                span: common::span_of(node),
                file: self.path.to_path_buf(),
                line: common::line_of(node),
                params,
                return_type,
                exported: is_exported(node),
            });
            chain.push(name);
            pushed = true;
            frame = kind.is_callable();
        }

        // A type's *own* body holds members; every other node passes the flag through, which is
        // what makes `class_body` — a node that is not itself a declaration — keep its
        // children at member level instead of resetting them to the file's.
        let child_in_type = declared.map_or(in_type, SymbolKind::is_scope);
        self.descend(node, chain, functions + usize::from(frame), child_in_type);
        if pushed {
            chain.pop();
        }
    }

    fn descend(
        &mut self,
        node: Node<'_>,
        chain: &mut Vec<String>,
        functions: usize,
        in_type: bool,
    ) {
        for index in 0..node.named_child_count() {
            if let Some(child) = node.named_child(index) {
                self.walk(child, chain, functions, in_type);
            }
        }
    }

    /// The node a JSDoc block would be written above: the outermost wrapper the declaration
    /// sits in, because that is the node the comment precedes.
    ///
    /// Three wrappers can sit between a comment and a declaration — `export const f = …`
    /// nests a `variable_declarator` inside a `lexical_declaration` inside an
    /// `export_statement` — and a JSDoc is written above the outermost of them in every
    /// convention this reader has seen.
    fn doc_node<'a>(&self, node: Node<'a>) -> Node<'a> {
        let mut current = node;
        while let Some(parent) = current.parent() {
            if matches!(
                parent.kind(),
                "lexical_declaration" | "variable_declaration" | "export_statement"
            ) {
                current = parent;
            } else {
                break;
            }
        }
        current
    }

    /// The declared name: the `name` field (a `type_identifier`, an `identifier`, a
    /// `property_identifier`, or a computed name, whose text is used as written).
    fn name_of(&self, node: Node<'_>) -> Option<String> {
        let name = node.child_by_field_name("name")?;
        let text = common::collapse(common::text_of(name, self.source));
        (!text.is_empty()).then_some(text)
    }
}

/// The kind a node declares, or `None` when it declares nothing.
fn declaration(node: Node<'_>, in_type: bool) -> Option<SymbolKind> {
    match node.kind() {
        "internal_module" | "module" => Some(SymbolKind::Module),
        "class_declaration" | "abstract_class_declaration" | "class" => Some(SymbolKind::Class),
        "interface_declaration" => Some(SymbolKind::Interface),
        "type_alias_declaration" => Some(SymbolKind::TypeAlias),
        "enum_declaration" => Some(SymbolKind::Enum),
        "function_declaration" | "generator_function_declaration" => Some(if in_type {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        }),
        "method_definition" | "method_signature" | "abstract_method_signature" => {
            Some(SymbolKind::Method)
        }
        // A class field holding an arrow function *is* a method ("an exported hook"), and a
        // module-scope `const f = () => …` is a function. `in_type` is what tells them apart.
        "variable_declarator" | "public_field_definition" if function_node(node).is_some() => {
            Some(if in_type {
                SymbolKind::Method
            } else {
                SymbolKind::Function
            })
        }
        "property_signature" | "public_field_definition" => Some(SymbolKind::Property),
        _ => None,
    }
}

/// The function a declaration node *holds*, when the declaration is a binding.
///
/// `None` for a node that is itself a function (`function_declaration`,
/// `method_definition`), which the caller reads directly — the two cases are distinguished so
/// that a `const` hook and a `function` declaration are described by one reader without that
/// reader having to guess which node it was handed.
fn function_node(node: Node<'_>) -> Option<Node<'_>> {
    let value = node.child_by_field_name("value")?;
    matches!(
        value.kind(),
        "arrow_function" | "function" | "function_expression" | "generator_function"
    )
    .then_some(value)
}

/// `true` when an `export` statement encloses the declaration.
///
/// The climb stops at the first statement that contains the declaration, because a
/// declaration is only exported from the top level of its own file: `export const f = () => {
/// const g = 1 }` exports `f` and not `g`, and stopping early is what says so.
fn is_exported(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() == "export_statement" {
        return true;
    }
    // A `variable_declarator` sits inside a `lexical_declaration` inside the export.
    parent
        .parent()
        .is_some_and(|grand| grand.kind() == "export_statement")
}

#[cfg(test)]
mod cases;
