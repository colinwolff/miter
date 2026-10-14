//! `tree-sitter` ingestion: source text → function nodes.
//!
//! The grammar-specific knowledge lives in a `GrammarSpec` table (node kind
//! names + the return-type sigil), so adding a language means adding a spec, not
//! another traversal. Five specs ship today, over four grammars: Rust,
//! the TypeScript family — `TypeScript` for `.ts`, `Tsx` for `.tsx`
//! and `JavaScript` for `.js`/`.jsx`, the last two read with
//! `LANGUAGE_TSX` because that grammar accepts the JSX dialects — and `Python`.
//!
//! The *universal* model — declaration kinds, hierarchy levels and
//! docstrings — is `miter_engine::polyglot`, which parses the same grammars through
//! [`grammar_for`]; this module stays the fast, function-shaped reader the index
//! and the query pipeline are built on.

use tree_sitter::{Node, Parser};

use crate::IndexError;

/// A single function input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    /// Parameter name (for Rust receivers: `self`).
    pub name: String,
    /// Parameter type as written in the source; `Self` for Rust receivers.
    pub type_name: String,
}

/// A function as read from the syntax tree, before symbolic encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionNode {
    /// Bare function name.
    pub name: String,
    /// Inputs in declaration order.
    pub params: Vec<Param>,
    /// Return type as written in the source, if annotated.
    pub return_type: Option<String>,
    /// Nesting depth inside the file (`mod`/`impl`/`trait`/`class`/`fn`).
    pub scope_depth: usize,
    /// One-based line of the declaration.
    pub line: usize,
    /// Byte range of the declaration.
    pub start_byte: usize,
    /// Byte offset one past the declaration.
    pub end_byte: usize,
}

/// Source languages supported by the indexer.
///
/// The variant names the language a record was read from. `Tsx` and `JavaScript`
/// are separate from `TypeScript` because a `.tsx` file is not a `.ts` file (the
/// grammar that parses JSX rejects several declarations the TypeScript grammar
/// accepts, and vice versa), while all three share one *family* for a report
/// ([`Language::family`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    /// Rust.
    Rust,
    /// TypeScript.
    TypeScript,
    /// TSX — TypeScript with JSX.
    Tsx,
    /// JavaScript, including JSX and the `.mjs`/`.cjs` module flavours.
    JavaScript,
    /// Python.
    Python,
}

impl Language {
    /// Every language this crate knows about, whether or not it is enabled.
    pub const ALL: [Language; 5] = [
        Language::Rust,
        Language::TypeScript,
        Language::Tsx,
        Language::JavaScript,
        Language::Python,
    ];

    /// Human-readable name.
    pub const fn name(self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::Tsx => "tsx",
            Language::JavaScript => "javascript",
            Language::Python => "python",
        }
    }

    /// The language *family* a report groups by: `rust`, `typescript` or `python`.
    ///
    /// The three TypeScript-family members answer `typescript`, because a
    /// dashboard that filtered by the exact grammar would split one project's
    /// front end into three colours for a distinction its readers do not draw.
    /// The family is what a stored record is labelled with
    /// ([`FunctionSymbol::language`](crate::FunctionSymbol)), so a client never
    /// has to know which grammar read the file.
    pub const fn family(self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript | Language::Tsx | Language::JavaScript => "typescript",
            Language::Python => "python",
        }
    }

    /// `true` when the grammar for this language is compiled into the binary.
    ///
    /// Every grammar this crate names is compiled in unconditionally.
    pub const fn is_enabled(self) -> bool {
        true
    }

    /// File extensions mapped to this language.
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            Language::Rust => &["rs"],
            // `.d.ts` is handled by name in [`Language::from_path`].
            Language::TypeScript => &["ts", "mts", "cts"],
            Language::Tsx => &["tsx"],
            Language::JavaScript => &["js", "jsx", "mjs", "cjs"],
            Language::Python => &["py", "pyi"],
        }
    }

    /// Infers the language from a bare extension (case-insensitive).
    pub fn from_extension(extension: &str) -> Option<Self> {
        let lowered = extension.to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|language| language.extensions().contains(&lowered.as_str()))
    }

    /// Infers the language from a path, including the `.d.ts` suffix.
    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        if name.ends_with(".d.ts") {
            return Some(Language::TypeScript);
        }
        Self::from_extension(path.extension()?.to_str()?)
    }
}

use grammar::spec_for;
use read::{read_params, read_return_type, walk};

mod grammar;
mod read;

pub use grammar::grammar_for;

/// Extracts every named function from `source`.
///
/// Anonymous closures are skipped; a TypeScript arrow function inherits the name
/// of the binding it is assigned to (`const f = () => …` yields `f`).
///
/// # Errors
///
/// * [`IndexError::UnsupportedLanguage`] when the grammar is not compiled in,
/// * [`IndexError::Grammar`] when the grammar cannot be loaded,
/// * [`IndexError::Parse`] when the parser returns no tree.
pub fn parse_source(source: &str, language: Language) -> Result<Vec<FunctionNode>, IndexError> {
    let mut parser = Parser::new();
    let grammar = grammar_for(language)?;
    parser
        .set_language(&grammar)
        .map_err(|error| IndexError::Grammar {
            language,
            message: error.to_string(),
        })?;
    let tree = parser.parse(source, None).ok_or(IndexError::Parse {
        language,
        path: None,
    })?;

    let spec = spec_for(language).ok_or(IndexError::UnsupportedLanguage { language })?;
    let mut functions = Vec::new();
    walk(tree.root_node(), 0, spec, source, &mut functions);
    Ok(functions)
}

/// The parameter list and annotated return type of one declaration node.
///
/// The same reader [`crate::AstIndexer::extract`] uses, exposed for a caller that walks the
/// tree itself: a universal visitor (`miter_engine::polyglot`) has to recognise
/// kinds this crate's function-shaped walker ignores — a `struct`, an `interface`, a
/// decorated `def` — and reconstructing "what a parameter is in this grammar" a second
/// time is exactly the kind of drift one table exists to prevent. `node` is the
/// declaration node (a `function_item`, a `function_definition`, a `method_definition`
/// …), not its parameter list.
///
/// Returns an empty vector and `None` for a grammar this build does not carry.
pub fn signature_of(
    node: Node<'_>,
    source: &str,
    language: Language,
) -> (Vec<Param>, Option<String>) {
    let Some(spec) = spec_for(language) else {
        return (Vec::new(), None);
    };
    (
        read_params(node, spec, source),
        read_return_type(node, spec, source),
    )
}
