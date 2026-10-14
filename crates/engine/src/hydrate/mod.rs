//! Context hydration: enough of a symbol to work with, in a bounded number of tokens.
//!
//! The problem this solves is an agent's. To learn what a declaration is, the
//! obvious move is to read its file, and then the three files it calls into, and then
//! the ones *they* call into — twenty thousand lines of source to answer a question the index
//! already holds the answer to. This module answers it from the index instead, and the answer is
//! assembled from *AST metadata* rather than from text search:
//!
//! ```text
//! prompt ──► miter lookup ──► miter references
//!                                  │
//!                                  ▼
//!                     the declaration's hierarchy level (0..=4)
//!                                  │
//!                                  ▼
//!              neighbours within HYDRATION_LEVEL_BAND levels
//!                                  │
//!                                  ▼
//!                    HydratedContext — a bounded, prompt-ready block
//! ```
//!
//! # What "hydration" means here
//!
//! A [`HydratedContext`] is four things and nothing else:
//!
//! 1. **where it is** — file, line, byte span, kind and hierarchy level;
//! 2. **what it is** — the declaration's own signature line, its parameter contracts and its return
//!    type, read off the parse rather than re-derived from text;
//! 3. **what it holds** — its public members, filtered to the exported ones, because a private
//!    leaf is not part of a caller's problem;
//! 4. **what is near it** — the other declarations within [`HYDRATION_LEVEL_BAND`] levels, nearest
//!    first.
//!
//! What it deliberately does *not* contain is a body. A body is what the agent was trying to avoid
//! reading, and a signature plus a neighbour list is what it actually needed.
//!
//! # The level band is the budget
//!
//! [`HYDRATION_LEVEL_BAND`] (`1`) is the whole reason this module is cheaper than grepping, and its
//! *meaning* is the one thing worth reading before trusting a neighbour list. A declaration's only
//! positional property is its hierarchy level, which the parse reads straight off the syntax tree —
//! a `mod` is level 1, a `struct` level 2, a `fn` level 3. "Near" therefore means "within one level
//! of the anchor's", which admits exactly the siblings and members a reader of the source would
//! call its neighbours, and excludes both shallower scaffolding and deeper unrelated leaves.
//!
//! # The token bound
//!
//! [`estimate_tokens`] is an *estimate*, not an exact count, and [`HydratedContext::render_within`] is
//! written so the estimate is conservative enough for the bound to mean something. The count is
//! the larger of two estimators — four characters per unit (the rate source code and prose share)
//! and whitespace-delimited fields — so a payload cannot slip past the budget by being
//! punctuation-dense. When a payload still exceeds `max_tokens`, the render drops the *farthest*
//! neighbour and re-renders rather than truncating mid-signature: a context that ends in half a
//! line is worse than one that ends a line earlier.
//!
//! ```
//! use miter_engine::{Engine, hydrate_symbol_context, polyglot};
//! use std::path::Path;
//!
//! let mut engine = Engine::new();
//! engine
//!     .ingest_polyglot_file(
//!         "/// Stores weights.\npub struct Model { /* ... */ }\nimpl Model {\n    /// Loads.\n    pub fn load(&self, path: &str) -> u64 { 0 }\n}\n",
//!         Path::new("crates/miter-engine/src/model.rs"),
//!         0,
//!         polyglot::Language::Rust,
//!     )
//!     .expect("valid Rust");
//!
//! let context = hydrate_symbol_context(&engine, "Model", 256).expect("hydrates");
//! assert_eq!(context.name, "Model");
//! assert!(context.text.contains("pub struct Model"));
//! assert!(context.tokens <= 256, "{} tokens", context.tokens);
//! ```
//!
//! ## Layout
//!
//! This module is the facade: it re-exports every historical item, so `crate::hydrate::HydratedContext`
//! and friends keep resolving. The work lives in focused submodules:
//!
//! | module | owns |
//! |---|---|
//! | `types` | the data model — the bounding constants, the context structs, and [`HydrateError`] |
//! | `render` | the payload a context becomes, and the token estimate it is bounded by |
//! | `resolve` | which declaration a name means, and which declarations are its neighbours |
//! | `context` | symbol hydration: one name in, one rendered context out |
//! | `scoped` | module- and package-scoped hydration |

mod context;
mod render;
mod resolve;
mod scoped;
mod types;

pub use context::{hydrate_symbol_context, hydrate_symbol_context_with};
pub use render::estimate_tokens;
pub use resolve::resolve;
pub use scoped::{HydrateScope, ScopedHydrateOptions, ScopedHydratedContext, hydrate_scoped};
pub use types::{
    CHARS_PER_TOKEN, DEFAULT_MAX_TOKENS, HYDRATION_LEVEL_BAND, HydrateError, HydratedContext,
    HydratedNeighbour, MAX_MEMBERS, MAX_NEIGHBOURS, ParamContract,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_wide;
