//! The Python visitor: classes, definitions, PEP 257 docstrings and type hints.
//!
//! | node | kind | level |
//! |---|---|---|
//! | `class_definition` | [`SymbolKind::Class`] | 2 |
//! | `function_definition` | [`SymbolKind::Function`] or [`SymbolKind::Method`] | 3 (+1 per enclosing `def`) |
//!
//! Three things make a Python file different from a Rust or TypeScript one:
//!
//! * **A module is a file.** There is no `mod` keyword, so the file's own stem is the only
//!   module name there is — which is what [`common::module_path`] hands the dotted path.
//! * **Documentation is *inside* the body.** PEP 257 puts the docstring in the first
//!   statement of the block rather than in a comment above it, so the visitor reads the
//!   body's first `expression_statement` for its `string_content` children — no quoting
//!   convention is part of the text, and a multi-line docstring keeps its lines.
//! * **A decorator is a sibling.** `@staticmethod` sits between the comment block and the
//!   `def`, so a decorated definition is read from its `decorated_definition` wrapper: that
//!   is where the comment scan starts, and the decorators are rendered into the signature so
//!   what a reader sees is what the source says.
//!
//! An inner `def` is a leaf (level 4): a function written inside another function is nested
//! code, which is what `common::level`'s leaf-ward push says.
//!
//! `exported` follows the language's own convention — a name with no leading underscore is
//! part of the module's surface, and `_private` is not.

use std::path::Path;

use tree_sitter::Node;

use super::{Language, SymbolKind, UniversalSymbol, common};

/// Every declaration in a parsed Python file, in source order.
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
    fn walk(&mut self, node: Node<'_>, chain: &mut Vec<String>, functions: usize, in_type: bool) {
        // `decorated_definition` is transparent: it wraps a definition and its decorators, and
        // walking through it here keeps the definition's own identity (its span and its line)
        // rather than the wrapper's.
        let declared = declaration(node, in_type);

        let mut pushed = false;
        let mut frame = false;
        if let Some(kind) = declared
            && let Some(name) = self.name_of(node)
        {
            let depth = common::level(common::base_level(kind), functions);
            let body = node.child_by_field_name("body");
            let (params, return_type) =
                miter_ast::signature_of(node, self.source, miter_ast::Language::Python);
            let mut signature = common::header_of(node, self.source, body);
            if let Some(decorators) = self.decorators(node) {
                signature = format!("{decorators} {signature}");
            }
            self.symbols.push(UniversalSymbol {
                path: common::dotted_path(&self.module, chain, &name),
                name: name.clone(),
                language: Language::Python,
                kind,
                depth,
                docstring: docstring(node, self.source),
                signature,
                span: common::span_of(node),
                file: self.path.to_path_buf(),
                line: common::line_of(node),
                params,
                return_type,
                exported: !name.starts_with('_'),
            });
            chain.push(name);
            pushed = true;
            frame = kind.is_callable();
        }

        // A class's own body holds members; every other node passes the flag through.
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

    /// The decorators of a definition, rendered as `@name @other`, or `None` when undecorated.
    fn decorators(&self, node: Node<'_>) -> Option<String> {
        let wrapper = node.parent()?;
        if wrapper.kind() != "decorated_definition" {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        for index in 0..wrapper.named_child_count() {
            let Some(child) = wrapper.named_child(index) else {
                continue;
            };
            if child.kind() == "decorator" {
                parts.push(common::collapse(common::text_of(child, self.source)));
            }
        }
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    /// The declared name, from the definition's `name` field.
    fn name_of(&self, node: Node<'_>) -> Option<String> {
        let name = node.child_by_field_name("name")?;
        let text = common::collapse(common::text_of(name, self.source));
        (!text.is_empty()).then_some(text)
    }
}

/// The kind a node declares, or `None` when it declares nothing.
fn declaration(node: Node<'_>, in_type: bool) -> Option<SymbolKind> {
    match node.kind() {
        "class_definition" => Some(SymbolKind::Class),
        // `async def` is the same node kind with an `async` child, so one arm covers it.
        "function_definition" => Some(if in_type {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        }),
        _ => None,
    }
}

/// The PEP 257 docstring of a definition: the first statement of its body, when that
/// statement is a string.
///
/// Read from the `string_content` children rather than from the node's text, so no quoting
/// convention (`"""`, `'''`, a single-quoted one-liner, an `r` prefix) can leak into the
/// answer. A definition whose first statement is anything else has no docstring, and a
/// *later* string is a value rather than documentation — which is what "first statement"
/// means, and why this does not search the body.
fn docstring(node: Node<'_>, source: &str) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let statement = first_named_child(body)?;
    if statement.kind() != "expression_statement" {
        return None;
    }
    let string = first_named_child(statement)?;
    let mut parts: Vec<&str> = Vec::new();
    collect_string_contents(string, source, &mut parts);
    let text = parts.join("").trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Appends the literal text of a `string` (or of a `concatenated_string`) to `parts`.
///
/// A `string` holds `string_start`, `string_content` and `string_end` children, so the
/// contents are the text between the quotes whatever the quote is; a concatenation of two
/// literals is one docstring written on two lines, which is why it is joined rather than
/// truncated at the first part.
fn collect_string_contents<'a>(node: Node<'_>, source: &'a str, parts: &mut Vec<&'a str>) {
    if node.kind() == "string_content" {
        parts.push(common::text_of(node, source));
        return;
    }
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            collect_string_contents(child, source, parts);
        }
    }
}

/// The first named child of `node`, if it has one.
fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    (0..node.named_child_count()).find_map(|index| node.named_child(index))
}

#[cfg(test)]
mod cases;
