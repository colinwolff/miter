//! # `miter` — Standalone AST Co-Processor & MCP Server
//!
//! Deterministic AST indexing, interface extraction, and verification engine.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cli;

pub use miter_ast as ast;
pub use miter_engine as engine;
pub use miter_harness as harness;
pub use miter_mcp as mcp;
pub use miter_storage as storage;

use std::path::Path;

pub use miter_ast::{AstIndexer, FunctionSymbol, Language};
pub use miter_engine::{Engine, EngineError};

/// Ingests every supported source file below `root` into `engine` in parallel.
///
/// # Errors
///
/// Returns [`EngineError`] when the directory cannot be traversed.
pub fn index_directory(
    engine: &mut Engine,
    root: &Path,
) -> Result<miter_engine::IngestStats, EngineError> {
    engine.ingest_dir(root)
}

/// Everything needed to drive the engine from a fresh session.
pub mod prelude {
    pub use crate::{AstIndexer, Engine, EngineError, FunctionSymbol, Language, index_directory};
    pub use miter_engine::IngestStats;
}
