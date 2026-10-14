//! The machinery every universal visitor shares: hierarchy levels, module paths and
//! docstring harvesting.
//!
//! A language visitor is a recursive walk over the concrete syntax tree that knows
//! *which* node kinds declare something and *which* level they sit at
//! ([`crate::polyglot::rust`] is the reference). Everything else — reading a node's
//! text, collapsing it onto one line, computing a level, building the dotted path,
//! reading the comment block above a declaration — is here, so the three visitors
//! differ only in their node-kind table and never in a rule.
//!
//! # The hierarchy model
//!
//! ```text
//! level 0  root        the crate / package boundary (the file's own container)
//! level 1  module      `mod`, `namespace`/`module`, a package module
//! level 2  scope       struct / enum / trait / class / interface / type alias
//! level 3  member      function / method / exported hook
//! level 4  leaf        closure, inline callback, lambda, inner `def`
//! ```
//!
//! A declaration's level is `min(4, base(kind) + enclosing_functions)`: the *kind*
//! fixes the floor and each enclosing function or closure pushes it one level
//! leaf-ward. That is the rule the objective's table states, checked against each of
//! its examples — a method inside a class or an `impl` is still a member (level 3,
//! because neither `class` nor `impl` is a function), a function inside a function is
//! a leaf (level 4), and a closure is a leaf whatever encloses it.
//!
//! # The two module spellings
//!
//! A universal symbol's `path` is dotted (`my_module.MyClass.my_method`) because it is
//! a *name*, while a module label uses the index's own slash convention
//! (`crate/module`) because a reader wants the crate off it. [`fn@module_path`] and
//! [`fn@module_label`] are the two, and they are one traversal with two joiners — which is
//! why they live side by side rather than in their separate callers.

use std::path::Path;

use tree_sitter::Node;

/// The deepest hierarchy level: closures, callbacks, lambdas and inner definitions.
pub const MAX_LEVEL: usize = 4;

/// The level a declaration of `kind` sits at, before enclosing functions are counted.
pub(crate) const fn base_level(kind: crate::polyglot::SymbolKind) -> usize {
    use crate::polyglot::SymbolKind;
    match kind {
        // A heading's own depth is its `#` count, which the Markdown extractor sets directly;
        // the floor here only keeps the table total.
        SymbolKind::Module | SymbolKind::Heading => 1,
        SymbolKind::Struct
        | SymbolKind::Enum
        | SymbolKind::Trait
        | SymbolKind::Class
        | SymbolKind::Interface
        | SymbolKind::TypeAlias
        | SymbolKind::Table
        | SymbolKind::InfraResource
        | SymbolKind::DockerStage
        | SymbolKind::K8sManifest => 2,
        SymbolKind::Function | SymbolKind::Method | SymbolKind::Property => 3,
    }
}

/// `min(MAX_LEVEL, base + enclosing_functions)`: the leaf-ward push, capped.
pub(crate) fn level(base: usize, enclosing_functions: usize) -> usize {
    (base + enclosing_functions).min(MAX_LEVEL)
}

/// The one-based line a byte offset sits on.
///
/// The declarative readers work from byte offsets into the text they were handed (a tag, a
/// selector, a statement), and a line number is what a report shows.
pub fn line_at(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].matches('\n').count() + 1
}

/// The source text a node covers (empty when the range is out of bounds).
pub(crate) fn text_of<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    source.get(node.byte_range()).unwrap_or("")
}

/// A source slice flattened onto one line, with single spaces between words.
pub(crate) fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The one-based line a node starts on.
pub(crate) fn line_of(node: Node<'_>) -> usize {
    node.start_position().row + 1
}

/// The node's byte span, `(start, end)`.
pub(crate) fn span_of(node: Node<'_>) -> (usize, usize) {
    (node.start_byte(), node.end_byte())
}

/// The declaration's header: its text up to `body`, trailing punctuation removed.
///
/// `body` is the declaration's own body node (`{ … }`, a Python `block`, an arrow
/// function's expression) when it has one. A declaration with no body — a trait method
/// signature, an interface member, a `type` alias — passes `None` and gets its whole
/// text, with the statement's `;` or `:` trimmed off.
pub(crate) fn header_of(node: Node<'_>, source: &str, body: Option<Node<'_>>) -> String {
    let end = body
        .map(|body| body.start_byte())
        .unwrap_or(node.end_byte());
    let text = source.get(node.start_byte()..end).unwrap_or("");
    collapse(text)
        .trim_end_matches(['{', ';', ':', ' ', '\t'])
        .to_string()
}

/// The comment block immediately above `node`, with its markers stripped.
///
/// Comments are gathered from the node's *previous siblings*, which is where every one
/// of these grammars puts them: a doc comment is a sibling of the declaration it
/// documents, not a child. The scan stops at the first sibling that is not a comment
/// and that `accepts` refuses, so an unrelated block ends the association — only
/// comments that are adjacent *and* of the accepted form are joined.
pub(crate) fn comments_above(
    node: Node<'_>,
    source: &str,
    accepts: fn(&str) -> bool,
) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut sibling = node.prev_sibling();
    while let Some(current) = sibling {
        if !is_comment_kind(current.kind()) {
            break;
        }
        let raw = text_of(current, source);
        if !accepts(raw) {
            break;
        }
        lines.push(raw.to_string());
        sibling = current.prev_sibling();
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    let cleaned = strip_comment_markers(&lines.join("\n"));
    if cleaned.trim().is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

mod comments;

use comments::{is_comment_kind, strip_comment_markers};
pub(crate) use comments::{is_jsdoc, is_rust_doc};

/// The directory path of a file, as `crate/module`.
///
/// Read off the path: `crates/engine/src/store.rs` is the module `engine`. A path that
/// does not sit under `crates/` or `src/` falls back to its own parent directory, which
/// is the best a path can say about a tree that does not use the convention. This is the
/// convention the projection's labels are written in, and the one rule lives here
/// rather than being copied by each caller.
pub fn module_label(path: &Path) -> String {
    let (directories, _) = module_parts(path);
    directories.join("/")
}

/// The module of a file between dots, at *file* granularity: `engine.store`.
///
/// The dotted form names the file, because in Python and TypeScript the file *is* the
/// module — which is what makes a universal symbol's `path`
/// (`engine.store.Store.fetch`) a name a reader of the source would recognise.
pub fn module_path(path: &Path) -> String {
    let (directories, stem) = module_parts(path);
    if directories.is_empty() {
        stem
    } else {
        format!("{}.{}", directories.join("."), stem)
    }
}

/// The dotted qualified name of a declaration: `module.chain.name`.
pub fn dotted_path(module: &str, chain: &[String], name: &str) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(chain.len() + 2);
    if !module.is_empty() {
        parts.push(module);
    }
    parts.extend(chain.iter().map(String::as_str));
    parts.push(name);
    parts.join(".")
}

/// `(directory components, file stem)`, anchored at `crates/` or `src/`.
fn module_parts(path: &Path) -> (Vec<String>, String) {
    let components: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    let start = components
        .iter()
        .position(|component| component == "crates" || component == "src")
        .map_or(0, |index| index + 1);
    let mut parts: Vec<String> = components[start.min(components.len())..].to_vec();
    let file = parts.pop().unwrap_or_default();
    let stem = Path::new(&file)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or(file);
    // The `src` hop is the crate root rather than a module.
    parts.retain(|part| part != "src");
    (parts, stem)
}

#[cfg(test)]
mod cases;
