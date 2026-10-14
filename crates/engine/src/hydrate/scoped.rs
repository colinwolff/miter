//! Scoped hydration: a whole module's or package's public surface, within one budget.
//!
//! Where [`super::context::hydrate_symbol_context`] answers "what is this declaration", this answers
//! "what does this module or crate export" — the shape a caller wants before reading any of it. The
//! budget is enforced by dropping whole declarations, and the payload says how many it dropped rather
//! than truncating one.

use crate::store::Engine;

use super::context::hydrate_symbol_context;
use super::render::estimate_tokens;
use super::types::{DEFAULT_MAX_TOKENS, HydrateError};

/// The scope level of a context hydration query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HydrateScope {
    /// Exact declaration signature, docstring, and immediate fields/methods.
    #[default]
    Symbol,
    /// Exported public types, traits, and functions within a target module file.
    Module,
    /// Crate-level public API surface (stripping internal function bodies and private items).
    Package,
}

impl HydrateScope {
    /// Parses a string slice into a `HydrateScope`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "symbol" => Some(Self::Symbol),
            "module" => Some(Self::Module),
            "package" => Some(Self::Package),
            _ => None,
        }
    }

    /// String representation of the scope.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::Module => "module",
            Self::Package => "package",
        }
    }
}

/// Options for scoped hydration queries.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopedHydrateOptions {
    /// Target symbol, module path, or crate name.
    pub target: String,
    /// Scope resolution level.
    pub scope: HydrateScope,
    /// Depth: 1 = direct declarations; 2 = declarations + immediate neighbor types/callers.
    pub depth: usize,
    /// Maximum token budget (clamped between 64 and 2048).
    pub max_tokens: usize,
}

impl Default for ScopedHydrateOptions {
    fn default() -> Self {
        Self {
            target: String::new(),
            scope: HydrateScope::Symbol,
            depth: 1,
            max_tokens: DEFAULT_MAX_TOKENS,
        }
    }
}

/// Result of a scoped context hydration query.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopedHydratedContext {
    /// Target query string.
    pub target: String,
    /// Scope applied.
    pub scope: HydrateScope,
    /// Rendered text payload within the token budget.
    pub text: String,
    /// Estimated token count of `text`.
    pub tokens: usize,
    /// Number of matching declarations found.
    pub match_count: usize,
    /// Number of declarations included in `text`.
    pub included_count: usize,
}

/// Hydrates context according to the provided `ScopedHydrateOptions`.
///
/// # Errors
/// Returns [`HydrateError::IndexEmpty`] if the universal symbol table holds no symbols,
/// [`HydrateError::NotFound`] if no symbols match the target, or [`HydrateError::EmptyBudget`]
/// if the token ceiling cannot hold even the header.
pub fn hydrate_scoped(
    engine: &Engine,
    options: &ScopedHydrateOptions,
) -> Result<ScopedHydratedContext, HydrateError> {
    let symbols = engine.universal_symbols();
    if symbols.is_empty() {
        return Err(HydrateError::IndexEmpty);
    }

    let budget = options.max_tokens.clamp(64, 2048);

    match options.scope {
        HydrateScope::Symbol => {
            let ctx = hydrate_symbol_context(engine, &options.target, budget)?;
            let text = if options.depth > 1 {
                ctx.text.clone()
            } else {
                // Depth 1: direct declarations without neighbours
                crate::security::scrub_source_text(&ctx.render(0, ctx.members.len()))
            };
            let tokens = estimate_tokens(&text);
            Ok(ScopedHydratedContext {
                target: options.target.clone(),
                scope: HydrateScope::Symbol,
                text,
                tokens,
                match_count: 1,
                included_count: 1,
            })
        }
        HydrateScope::Module => {
            hydrate_module_scope(symbols, &options.target, options.depth, budget)
        }
        HydrateScope::Package => {
            hydrate_package_scope(symbols, &options.target, options.depth, budget)
        }
    }
}

mod module;
mod package;
mod render;

use module::hydrate_module_scope;
use package::hydrate_package_scope;
use render::render_declarations_block;
