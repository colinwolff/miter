//! The traversal: one syntax tree in, one `FunctionNode` per declaration out.

use tree_sitter::Node;

use super::grammar::GrammarSpec;
use super::{FunctionNode, Param};

pub(super) fn walk(
    node: Node<'_>,
    depth: usize,
    spec: &GrammarSpec,
    source: &str,
    functions: &mut Vec<FunctionNode>,
) {
    let kind = node.kind();
    if spec.function_kinds.contains(&kind)
        && let Some(function) = read_function(node, depth, spec, source)
    {
        functions.push(function);
    }
    let child_depth = if spec.scope_kinds.contains(&kind) {
        depth + 1
    } else {
        depth
    };
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index) {
            walk(child, child_depth, spec, source, functions);
        }
    }
}

/// Reads a function declaration, or `None` when it has no determinable name.
fn read_function(
    node: Node<'_>,
    depth: usize,
    spec: &GrammarSpec,
    source: &str,
) -> Option<FunctionNode> {
    let name = function_name(node, source)?;
    let start = node.start_position();
    Some(FunctionNode {
        name,
        params: read_params(node, spec, source),
        return_type: read_return_type(node, spec, source),
        scope_depth: depth,
        line: start.row + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    })
}

/// Resolves the function name, following one level of binding for anonymous
/// functions (`const f = () => …`, `{ f: () => … }`).
fn function_name(node: Node<'_>, source: &str) -> Option<String> {
    if let Some(name) = node.child_by_field_name("name") {
        let text = text_of(name, source);
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    let parent = node.parent()?;
    if matches!(
        parent.kind(),
        "variable_declarator" | "pair" | "public_field_definition" | "property_signature"
    ) {
        let binding = parent
            .child_by_field_name("name")
            .or_else(|| parent.child_by_field_name("key"))?;
        let text = text_of(binding, source);
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    None
}

/// Reads the parameter list, in declaration order.
pub(super) fn read_params(node: Node<'_>, spec: &GrammarSpec, source: &str) -> Vec<Param> {
    let Some(list) = node.child_by_field_name("parameters") else {
        return Vec::new();
    };
    let mut params = Vec::new();
    for index in 0..list.named_child_count() {
        let Some(child) = list.named_child(index) else {
            continue;
        };
        params.push(read_param(child, spec, source));
    }
    params
}

/// Reads one parameter: `pattern`/`name` field for the identifier, `type` field
/// for the annotation. Rust receivers carry no type field and become `Self`.
///
/// When neither field names the parameter the *first named child* is the name.
/// That is Python's shape: `typed_parameter` is `x: int`, whose only field is
/// `type` and whose first child is the identifier `x` — reading the whole node
/// would make the parameter's name `x: int`. A parameter that is a bare
/// `identifier` (no fields, no children) still falls back to its own text, which
/// is what every `*args`/`**kwargs` splat needs.
fn read_param(child: Node<'_>, spec: &GrammarSpec, source: &str) -> Param {
    let name = child
        .child_by_field_name("pattern")
        .or_else(|| child.child_by_field_name("name"))
        .or_else(|| {
            // `typed_parameter` is `x: int`: its only field is the type, so the name is
            // its first child — reading the whole node would make the parameter's name
            // `x: int`. The `type` field is what distinguishes it from `*args`, which has
            // no fields at all and whose own text is the name.
            child
                .child_by_field_name("type")
                .and_then(|_| first_named_child(child))
        })
        .map(|pattern| text_of(pattern, source))
        .unwrap_or_else(|| text_of(child, source))
        .to_string();
    let type_name = match child.child_by_field_name("type") {
        Some(annotation) => strip_sigil(text_of(annotation, source), spec.return_sigil),
        None if name.contains("self") => String::from("Self"),
        None => String::new(),
    };
    Param { name, type_name }
}

/// The first *named* child of `node`, if it has one.
fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    (0..node.named_child_count()).find_map(|index| node.named_child(index))
}

/// Reads the annotated return type, if any.
pub(super) fn read_return_type(node: Node<'_>, spec: &GrammarSpec, source: &str) -> Option<String> {
    let annotation = node.child_by_field_name("return_type")?;
    let text = strip_sigil(text_of(annotation, source), spec.return_sigil);
    if text.is_empty() { None } else { Some(text) }
}

/// Source text covered by a node (empty when the range is out of bounds).
fn text_of<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    source.get(node.byte_range()).unwrap_or("")
}

/// Removes a leading sigil (`->`, `:`) and flattens the annotation onto one line.
fn strip_sigil(text: &str, sigil: &str) -> String {
    let trimmed = text.trim();
    let body = trimmed.strip_prefix(sigil).unwrap_or(trimmed).trim();
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}
