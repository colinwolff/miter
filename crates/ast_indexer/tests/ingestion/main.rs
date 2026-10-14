//! The ingestion suite: `tree-sitter` extraction and its hierarchy radii, for Rust,
//! TypeScript and Python.
//!
//! One binary, three modules. Every `tests/*.rs` file is a *binary*: cargo links it, starts it,
//! runs its tests and waits for it to exit before it starts the next one, so the gate pays the
//! linker and the process once per file where a suite pays it once.
//!
//! A module is addressable the way its own file used to be — by naming it as the filter:
//!
//! ```text
//! cargo test -p miter-ast --test ingestion extraction
//! cargo test --release -p miter-ast --test ingestion extraction   # the encoding budget
//! cargo test -p miter-ast --test ingestion typescript
//! cargo test -p miter-ast --test ingestion python
//! ```

mod extraction;
mod python;
mod typescript;
