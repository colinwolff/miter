//! The Rust visitor: the reference implementation of the universal walk.
//!
//! | node | kind | level |
//! |---|---|---|
//! | `mod_item` | [`SymbolKind::Module`] | 1 |
//! | `struct_item` / `union_item` | [`SymbolKind::Struct`] | 2 |
//! | `enum_item` | [`SymbolKind::Enum`] | 2 |
//! | `trait_item` | [`SymbolKind::Trait`] | 2 |
//! | `type_item` | [`SymbolKind::TypeAlias`] | 2 |
//! | `field_declaration` | [`SymbolKind::Property`] | 3 |
//! | `function_item` | [`SymbolKind::Function`] or [`SymbolKind::Method`] | 3 (+1 per enclosing function) |
//!
//! `impl_item` is *transparent*: it declares nothing, but it contributes the type it
//! implements as a path element, so `impl Store { fn fetch(&self) }` yields
//! `engine.store.Store.fetch` rather than two `fetch`es that only a line number tells apart.
//! A `closure_expression` declares nothing either, but it *is* a function frame: a `fn`
//! written inside a closure's body is at level 4, which is what "leaf" means.
//!
//! Doc comments are the `///` and `/** */` forms only (`common::is_rust_doc`): `//!`
//! documents the item's *container*, and attaching it to the next declaration would credit
//! that declaration with a comment that was never about it.

use std::path::Path;

use tree_sitter::Node;

use super::{Language, SymbolKind, UniversalSymbol, common};

/// Every declaration in a parsed Rust file, in source order.
pub(crate) fn visit(path: &Path, source: &str, root: Node<'_>) -> Vec<UniversalSymbol> {
    let mut visitor = Visitor {
        path,
        source,
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
    module: String,
    symbols: Vec<UniversalSymbol>,
}

impl Visitor<'_> {
    /// Walks one node: records it if it declares something, then descends.
    ///
    /// `functions` counts the function frames this node sits inside (a `fn` or a closure),
    /// and `in_type` says whether it sits directly inside a type's body — which is the whole
    /// difference between a `Function` and a `Method`.
    fn walk(&mut self, node: Node<'_>, chain: &mut Vec<String>, functions: usize, in_type: bool) {
        match node.kind() {
            "impl_item" => {
                let pushed = self.push_impl_type(node, chain);
                if let Some(body) = node.child_by_field_name("body") {
                    for index in 0..body.named_child_count() {
                        if let Some(child) = body.named_child(index) {
                            self.walk(child, chain, functions, true);
                        }
                    }
                }
                if pushed {
                    chain.pop();
                }
                return;
            }
            // A closure declares nothing, but its body is a function frame.
            "closure_expression" => {
                self.descend(node, chain, functions + 1, false);
                return;
            }
            _ => {}
        }

        let declared = declaration(node, in_type);
        let mut pushed = false;
        let mut frame = false;
        if let Some(kind) = declared
            && let Some(name) = self.name_of(node)
        {
            let depth = common::level(common::base_level(kind), functions);
            let body = node.child_by_field_name("body");
            let (params, return_type) =
                miter_ast::signature_of(node, self.source, miter_ast::Language::Rust);
            self.symbols.push(UniversalSymbol {
                path: common::dotted_path(&self.module, chain, &name),
                name: name.clone(),
                language: Language::Rust,
                kind,
                depth,
                docstring: common::comments_above(node, self.source, common::is_rust_doc),
                signature: common::header_of(node, self.source, body),
                span: common::span_of(node),
                file: self.path.to_path_buf(),
                line: common::line_of(node),
                params,
                return_type,
                exported: is_public(node, self.source),
            });
            chain.push(name);
            pushed = true;
            frame = kind.is_callable();
        }

        self.descend(
            node,
            chain,
            functions + usize::from(frame),
            is_type_body(declared),
        );
        if pushed {
            chain.pop();
        }
    }

    /// Walks every named child of `node`.
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

    /// Pushes the type an `impl` block is about, when it can be read.
    fn push_impl_type(&self, node: Node<'_>, chain: &mut Vec<String>) -> bool {
        let Some(target) = node.child_by_field_name("type") else {
            return false;
        };
        let label = common::collapse(common::text_of(target, self.source)).replace(' ', "");
        // A generic target can be an entire line of text; a path element that long would be
        // noise, so only a plain type name qualifies.
        if label.is_empty() || label.len() > 48 {
            return false;
        }
        chain.push(label);
        true
    }

    /// The declared name, from the node's `name` field.
    fn name_of(&self, node: Node<'_>) -> Option<String> {
        let name = node.child_by_field_name("name")?;
        let text = common::collapse(common::text_of(name, self.source));
        (!text.is_empty()).then_some(text)
    }
}

/// The kind a node declares, or `None` when it declares nothing.
fn declaration(node: Node<'_>, in_type: bool) -> Option<SymbolKind> {
    match node.kind() {
        "mod_item" => Some(SymbolKind::Module),
        "struct_item" | "union_item" => Some(SymbolKind::Struct),
        "enum_item" => Some(SymbolKind::Enum),
        "trait_item" => Some(SymbolKind::Trait),
        "type_item" => Some(SymbolKind::TypeAlias),
        "const_item" | "static_item" | "field_declaration" => Some(SymbolKind::Property),
        "function_item" | "function_signature_item" => Some(if in_type {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        }),
        _ => None,
    }
}

/// `true` when a declaration's own body holds members rather than statements.
fn is_type_body(declared: Option<SymbolKind>) -> bool {
    declared.is_some_and(|kind| kind.is_scope() || kind == SymbolKind::Trait)
}

/// `true` when the declaration carries a `pub`-family visibility modifier.
///
/// The *node kind* is what is tested rather than a field name, because the Rust grammar does
/// not expose the modifier as a field: a `visibility_modifier` child is the whole test, and it
/// is the same test for an item, a field and a `mod`. `pub(crate)` and `pub(super)` count as
/// exported — the declaration is reachable from another module of the crate, which is the
/// question the flag answers — while `pub(self)`, the one spelling that is *not* wider than
/// the module, is excluded by name.
fn is_public(node: Node<'_>, source: &str) -> bool {
    let Some(visibility) = (0..node.child_count())
        .filter_map(|index| node.child(index))
        .find(|child| child.kind() == "visibility_modifier")
    else {
        return false;
    };
    !common::collapse(common::text_of(visibility, source)).starts_with("pub(self)")
}
