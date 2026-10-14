//! The universal, language-agnostic symbol model.
//!
//! The index's own reader (`ast_indexer`) is *function-shaped*: it walks a grammar looking
//! for the node kinds that declare a function. A universal model has to see more than that — a
//! `struct`, an
//! `interface`, a `type` alias, a decorated `def` — and it has to know where each
//! declaration sits in a hierarchy that means the same thing in all four languages:
//!
//! ```text
//!                       source file (.rs .ts .tsx .js .py)
//!                                  │
//!                                  ▼
//!                ┌──────────────────────────────────┐
//!                │   Tree-Sitter Grammar            │  ← `miter_ast::grammar_for`
//!                │   (Rust / TS / TSX / Py)         │
//!                └───────────────┬──────────────────┘
//!                                │ concrete syntax tree
//!                                ▼
//!                ┌──────────────────────────────────┐
//!                │   Universal CST Visitor          │  ← `rust`, `typescript`, `python`
//!                │   level 0 ──► level 4            │     over `common`'s rules
//!                └───────────────┬──────────────────┘
//!                                │
//!                     ┌──────────┴──────────┐
//!                     ▼                     ▼
//!             hierarchy level         provenance
//!              (0 ──► 4)        (path, line, name)
//! ```
//!
//! # The hierarchy levels
//!
//! | level | role | Rust | TypeScript | Python |
//! |---|---|---|---|---|
//! | 0 | root | crate root | `package.json` boundary | package root (`__init__.py`) |
//! | 1 | module | `mod` | `namespace` / `module` | package module |
//! | 2 | scope | `struct` / `enum` / `trait` | `class` / `interface` / `type` | `class` |
//! | 3 | member | `fn` / method | function / method / exported hook | `def` / method |
//! | 4 | leaf | closure | inline callback | lambda / inner `def` |
//!
//! The level is what the hydration band is measured against: a neighbour qualifies when it sits
//! within one level of the anchor.
//!
//! # One identity, whichever parser read the file
//!
//! [`UniversalSymbol::id`] is derived from `file:line:name` — the same formula
//! `miter_ast::AstIndexer::encode` uses — so a Rust declaration read by either path lands on
//! the same key. The two readings disagree only in how they count nesting (the universal one
//! reads the syntax tree's level, the index's adds the directory depth), which is why a store

pub mod common;
pub mod docker;
pub mod env;
pub mod hcl;
pub mod k8s;
pub mod python;
pub mod rust;
pub mod sql;
pub mod typescript;

mod dispatch;
mod error;
mod kind;
mod language;
mod markdown;
mod redact;
mod symbol;

pub mod css;
pub mod html;

pub use css::extract_css_symbols;
pub use dispatch::{
    declarative_symbols, is_declarative_source, parse, parse_declarative, parse_detected,
};
pub use error::PolyglotError;
pub use html::extract_html_symbols;
pub use kind::SymbolKind;
pub use language::{EXTENSIONS, Language, PolyglotLanguage};
pub use markdown::extract_markdown_symbols;
pub use redact::redacted_value;
pub use symbol::UniversalSymbol;

#[cfg(test)]
mod tests;
