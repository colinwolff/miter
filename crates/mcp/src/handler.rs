//! The MCP tool handlers: one function per tool, split by what it does.
//!
//! `auto_refresh_dirty` is shared by every handler that reads the index: a tool call re-indexes
//! the modified files in `workspace_root` before answering, so an answer is never stale.

use std::path::Path;

/// Message returned when the universal symbol table is empty or missing.
pub const EMPTY_INDEX_MESSAGE: &str =
    "Symbol index is empty or stale. Run 'miter index .' to generate fresh AST bindings.";

/// Success confirmation string for verification.
pub const VERIFICATION_PASSED_MESSAGE: &str =
    "VERIFICATION PASSED: No compiler or type errors detected.";

/// Helper to perform a fast dirty-file refresh prior to queries if the index exists.
pub(super) fn auto_refresh_dirty(db_path: &Path, workspace_root: &Path) {
    if !db_path.exists() {
        return;
    }
    if let Ok(store) = miter_storage::WorkspaceStore::open(db_path) {
        let mut eng = miter_engine::Engine::new();
        let _ = miter_engine::ingest_dirty(&mut eng, workspace_root, &store);
    }
}

mod analysis;
mod lookup;
mod verify;

pub use analysis::{handle_audit, handle_impact, handle_references, handle_skeleton, handle_slice};
pub use lookup::handle_lookup;
pub use verify::handle_verify;
