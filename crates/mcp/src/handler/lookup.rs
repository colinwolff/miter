//! The `miter_lookup` tool: symbol hydration over the index.

use std::path::Path;

use miter_engine::hydrate::HydratedContext;

use crate::protocol::CallToolResult;
use crate::tools::LookupArgs;

use super::{EMPTY_INDEX_MESSAGE, auto_refresh_dirty};

/// Executes the `miter_lookup` tool.
///
/// Reads universal symbols from `db_path` using a read-only database handle to avoid lock contention.
/// If the database or table is missing or empty, returns a helpful indexing prompt.
#[must_use]
pub fn handle_lookup(db_path: &Path, workspace_root: &Path, args: LookupArgs) -> CallToolResult {
    auto_refresh_dirty(db_path, workspace_root);

    if !db_path.exists() {
        return CallToolResult::text(EMPTY_INDEX_MESSAGE);
    }

    let store = match miter_storage::WorkspaceStore::open(db_path) {
        Ok(s) => s,
        Err(error) => {
            return CallToolResult::error(format!(
                "Failed to open database at {}: {error}",
                db_path.display()
            ));
        }
    };

    let symbols = match miter_engine::load_redb_universal_from(&store) {
        Ok(res) => res,
        Err(error) => {
            return CallToolResult::error(format!(
                "Failed to read database at {}: {error}",
                db_path.display()
            ));
        }
    };

    if symbols.is_empty() {
        return CallToolResult::text(EMPTY_INDEX_MESSAGE);
    }

    let mut engine = miter_engine::Engine::new();
    engine.add_universal_symbols(symbols);

    let scope = miter_engine::HydrateScope::parse(&args.scope)
        .unwrap_or(miter_engine::HydrateScope::Symbol);

    // If scope is symbol and context is false, preserve minimal identity behavior
    if scope == miter_engine::HydrateScope::Symbol && !args.context {
        return match miter_engine::hydrate_symbol_context(&engine, &args.target, args.max_tokens) {
            Ok(context) => CallToolResult::text(render_identity_only(&context)),
            Err(miter_engine::HydrateError::IndexEmpty) => {
                CallToolResult::text(EMPTY_INDEX_MESSAGE)
            }
            Err(error) => CallToolResult::error(error.to_string()),
        };
    }

    let options = miter_engine::ScopedHydrateOptions {
        target: args.target.clone(),
        scope,
        depth: args.depth,
        max_tokens: args.max_tokens,
    };

    match miter_engine::hydrate_scoped(&engine, &options) {
        Ok(scoped) => CallToolResult::text(scoped.text),
        Err(miter_engine::HydrateError::IndexEmpty) => CallToolResult::text(EMPTY_INDEX_MESSAGE),
        Err(error) => CallToolResult::error(error.to_string()),
    }
}

/// Renders the bare identity block of a symbol without its neighborhood.
fn render_identity_only(context: &HydratedContext) -> String {
    format!(
        "{}\n  {}:{}\n  {} · level {}\n",
        context.name,
        context.file.display(),
        context.line,
        context.kind.name(),
        context.depth,
    )
}
