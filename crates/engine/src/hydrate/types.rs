//! The hydration data model: the constants that bound a context, the types it is made of, and the
//! error a hydration can fail with.
//!
//! Nothing here renders and nothing here resolves: the structs are plain data and the constants are
//! the bounds the rest of the module is measured against.

use std::path::PathBuf;

use crate::polyglot::SymbolKind;

/// How many hierarchy levels away a declaration may sit and still count as a neighbour.
///
/// # Why a level band and not a distance
///
/// A declaration's only positional property is its hierarchy level (`0..=4`), which the parse
/// reads straight off the syntax tree — a `mod` is level 1, a `struct` level 2, a `fn` level 3.
/// "Near" therefore means "within this many steps of the anchor's level": the band admits the
/// siblings and members that share the anchor's level, plus one step either way so a genuinely
/// flat hierarchy still yields a neighbourhood. It excludes both shallower scaffolding (the
/// enclosing `mod`) and deeper unrelated leaves.
///
/// ```text
///            anchor level
///                 │
///   ──────────────┼──────────────
///     band        │       band
///   ──────────────┼──────────────
///       shallower │ deeper
/// ```
pub const HYDRATION_LEVEL_BAND: usize = 1;

/// The default token ceiling for [`super::hydrate_symbol_context`].
pub const DEFAULT_MAX_TOKENS: usize = 256;

/// The characters per token the estimate assumes.
pub const CHARS_PER_TOKEN: usize = 4;

/// The most neighbours a context carries, nearest first.
///
/// A bound on the *rendering*: the level band says who is near, this says how much of that a
/// reader is asked to hold at once.
pub const MAX_NEIGHBOURS: usize = 6;

/// The most public members a context lists.
///
/// Also a rendering bound. A `struct` with sixty public methods is real, and listing all of them
/// would spend the whole budget on an inventory the reader did not ask for while the
/// neighbourhood, which is the part that answers "what else touches this", never appeared. The
/// members that survive are the ones nearest the top of the source file.
pub const MAX_MEMBERS: usize = 12;

/// The human name of each hierarchy level, indexed by the level itself.
pub(super) const LEVEL_NAMES: [&str; 5] = ["Root", "Module", "Scope", "Member", "Leaf"];

/// A parameter contract, as the parse read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamContract {
    /// Parameter name (a Rust receiver reads `self`).
    pub name: String,
    /// Type as written in the source.
    pub type_name: String,
}

/// A declaration within [`HYDRATION_LEVEL_BAND`] levels of the hydrated one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydratedNeighbour {
    /// Bare declaration name.
    pub name: String,
    /// File it was read from.
    pub file: PathBuf,
    /// One-based declaration line.
    pub line: usize,
    /// Hierarchy level, when the index holds the declaration.
    pub depth: usize,
    /// `|neighbour level − anchor level|`: the band filters on this and the ordering ranks by it,
    /// so it is the one number a caller reads to know *why* a neighbour was admitted.
    pub level_delta: usize,
}

/// Everything a caller is told about one symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydratedContext {
    /// The symbol that was hydrated.
    pub name: String,
    /// Qualified name as the parse recorded it.
    pub qualified: String,
    /// Module label, `crate/module`.
    pub module: String,
    /// File the declaration was read from.
    pub file: PathBuf,
    /// One-based line.
    pub line: usize,
    /// Byte span, `(start, end)`.
    pub span: (usize, usize),
    /// What the declaration is.
    pub kind: SymbolKind,
    /// Hierarchy level, `0..=4`.
    pub depth: usize,
    /// The declaration's own signature line.
    pub signature: String,
    /// The doc comment's first sentence, if the declaration had one.
    pub docstring: Option<String>,
    /// Parameter contracts, in declaration order.
    pub params: Vec<ParamContract>,
    /// Annotated return type, if the source carried one.
    pub return_type: Option<String>,
    /// Public members of the declaration.
    pub members: Vec<HydratedNeighbour>,
    /// Neighbours within [`HYDRATION_LEVEL_BAND`], nearest first.
    pub neighbours: Vec<HydratedNeighbour>,
    /// Other files the same name resolved to.
    pub ambiguous_with: Vec<PathBuf>,
    /// The rendered, token-bounded payload.
    pub text: String,
    /// [`super::estimate_tokens`] of `text`.
    pub tokens: usize,
}

/// Why a hydration could not be produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HydrateError {
    /// The index holds no declarations at all, so no name could have resolved.
    ///
    /// Distinct from [`HydrateError::NotFound`] on purpose: that error is a claim about a
    /// *symbol*, and it is wrong when the database is merely empty. This one names the cause and
    /// the command that repairs it.
    IndexEmpty,
    /// No declaration answers to the name.
    NotFound(String),
    /// The ceiling is too small to hold the symbol's own header.
    EmptyBudget(usize),
}

impl core::fmt::Display for HydrateError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HydrateError::IndexEmpty => write!(
                formatter,
                "the symbol index is empty or stale — it holds no declarations, so no name can \
                 resolve. Run `miter index .` to generate fresh AST bindings."
            ),
            HydrateError::NotFound(name) => write!(
                formatter,
                "no declaration named `{name}` in the index (a struct, a method or a qualified \
                 path such as `module.name`)"
            ),
            HydrateError::EmptyBudget(budget) => write!(
                formatter,
                "{budget} tokens cannot hold this declaration's own header; raise the budget"
            ),
        }
    }
}

impl std::error::Error for HydrateError {}
