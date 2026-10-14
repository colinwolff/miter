//! The structural analysis tools: audit, slice, skeleton, references and impact.

use std::path::Path;

use crate::protocol::CallToolResult;
use crate::tools::{AuditArgs, ImpactArgs, ReferencesArgs, SkeletonArgs, SliceArgs};

use super::{EMPTY_INDEX_MESSAGE, auto_refresh_dirty};

/// Executes the `miter_audit` tool.
pub fn handle_audit(workspace_root: &Path, args: AuditArgs) -> CallToolResult {
    let target_root = match &args.path {
        Some(p) => workspace_root.join(p),
        None => workspace_root.to_path_buf(),
    };

    let config =
        miter_engine::AuditConfig::from_workspace_with_threshold(&target_root, args.threshold);
    let effective_threshold = config.max_lines;
    let auditor = miter_engine::WorkspaceAuditor::with_config(config);
    match auditor.audit_root(&target_root) {
        Ok(violations) => {
            if violations.is_empty() {
                return CallToolResult::text(format!(
                    "AUDIT PASSED: No source files exceed {} lines in {}.",
                    effective_threshold,
                    target_root.display()
                ));
            }

            let mut out = format!(
                "AUDIT WARNING: Found {} file(s) exceeding {} lines:\n\n",
                violations.len(),
                effective_threshold
            );
            for v in &violations {
                out.push_str(&format!(
                    "- [{}] {} ({} lines, ~{} declarations)\n",
                    v.severity.label(),
                    v.path.display(),
                    v.line_count,
                    v.declaration_count
                ));
            }
            CallToolResult::text(out.trim_end())
        }
        Err(err) => CallToolResult::error(format!("Audit failed: {err}")),
    }
}

/// Executes the `miter_slice` tool.
pub fn handle_slice(db_path: &Path, workspace_root: &Path, args: SliceArgs) -> CallToolResult {
    auto_refresh_dirty(db_path, workspace_root);

    let target_path = workspace_root.join(&args.path);
    let content = match std::fs::read_to_string(&target_path) {
        Ok(c) => c,
        Err(err) => {
            return CallToolResult::error(format!(
                "Failed to read source file at {}: {err}",
                target_path.display()
            ));
        }
    };

    let slicer = miter_engine::SeamSlicer::with_target_lines(args.target_lines);
    let blueprints = slicer.slice_file(&target_path, &content);

    if blueprints.is_empty() {
        return CallToolResult::text(format!(
            "No extractable clusters found for {} (target: {} lines). File may already be small or indivisible.",
            args.path, args.target_lines
        ));
    }

    match serde_json::to_string_pretty(&blueprints) {
        Ok(json_str) => CallToolResult::text(json_str),
        Err(err) => CallToolResult::error(format!("Failed to serialize blueprints: {err}")),
    }
}

/// Executes the `miter_skeleton` tool.
pub fn handle_skeleton(workspace_root: &Path, args: SkeletonArgs) -> CallToolResult {
    let target_path = workspace_root.join(&args.path);
    let content = match std::fs::read_to_string(&target_path) {
        Ok(c) => c,
        Err(err) => {
            return CallToolResult::error(format!(
                "Failed to read source file at {}: {err}",
                target_path.display()
            ));
        }
    };

    let result = miter_engine::extract_skeleton(
        &target_path,
        &content,
        miter_engine::SkeletonOptions::default(),
    );
    CallToolResult::text(result.skeleton_text)
}

/// Executes the `miter_references` tool.
pub fn handle_references(
    db_path: &Path,
    workspace_root: &Path,
    args: ReferencesArgs,
) -> CallToolResult {
    auto_refresh_dirty(db_path, workspace_root);

    if !db_path.exists() {
        return CallToolResult::text(EMPTY_INDEX_MESSAGE);
    }

    let store = match miter_storage::WorkspaceStore::open(db_path) {
        Ok(s) => s,
        Err(err) => {
            return CallToolResult::error(format!(
                "Failed to open database at {}: {err}",
                db_path.display()
            ));
        }
    };

    let mut entries = match store.get_references_for_symbol(&args.symbol) {
        Ok(e) => e,
        Err(err) => {
            return CallToolResult::error(format!(
                "Failed to query references for '{}': {err}",
                args.symbol
            ));
        }
    };

    let total_found = entries.len();
    if args.limit > 0 && entries.len() > args.limit {
        entries.truncate(args.limit);
    }

    if entries.is_empty() {
        return CallToolResult::text(format!("No references found for symbol '{}'.", args.symbol));
    }

    let response_data = serde_json::json!({
        "symbol": args.symbol,
        "total_references": total_found,
        "returned": entries.len(),
        "references": entries,
    });

    match serde_json::to_string_pretty(&response_data) {
        Ok(json_str) => CallToolResult::text(json_str),
        Err(err) => CallToolResult::error(format!("Failed to serialize references: {err}")),
    }
}

/// Executes the `miter_impact` tool.
pub fn handle_impact(db_path: &Path, _workspace_root: &Path, args: ImpactArgs) -> CallToolResult {
    let store = match miter_storage::store::WorkspaceStore::open(db_path) {
        Ok(s) => s,
        Err(err) => {
            return CallToolResult::error(format!(
                "Failed to open database at {}: {err}",
                db_path.display()
            ));
        }
    };

    match miter_engine::compute_impact(&store, &args.symbol, args.max_depth) {
        Ok(impact) => match serde_json::to_string_pretty(&impact) {
            Ok(json_str) => CallToolResult::text(json_str),
            Err(err) => {
                CallToolResult::error(format!("Failed to serialize impact analysis: {err}"))
            }
        },
        Err(err) => CallToolResult::error(format!(
            "Failed to compute blast radius for '{}': {err}",
            args.symbol
        )),
    }
}
