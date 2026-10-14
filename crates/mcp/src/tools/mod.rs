//! Tool schemas, definitions, and argument validation.

use serde::{Deserialize, Serialize};

/// Definition of an MCP tool for `tools/list`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ToolDefinition {
    /// Distinct identifier of the tool.
    pub name: String,
    /// Detailed description for LLM usage and prompting.
    pub description: String,
    /// JSON Schema describing input arguments.
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// Arguments parsed for the `miter_lookup` tool.
#[derive(Clone, Debug, PartialEq)]
pub struct LookupArgs {
    /// Target identifier, module path, or crate name.
    pub target: String,
    /// The scope resolution level: "symbol", "module", or "package".
    pub scope: String,
    /// Depth: 1 = direct declarations; 2 = declarations + immediate neighbor types/callers.
    pub depth: usize,
    /// Whether to include the neighbouring declarations and byte ranges (in symbol scope).
    pub context: bool,
    /// Maximum token budget for the hydrated signature (clamped between 64 and 2048).
    pub max_tokens: usize,
}

impl LookupArgs {
    /// Backward-compatibility accessor for the target symbol.
    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.target
    }
}

/// Arguments parsed for the `miter_verify` tool.
#[derive(Clone, Debug, PartialEq)]
pub struct VerifyArgs {
    /// Path to target crate or workspace (defaults to current dir).
    pub path: Option<String>,
    /// Whether to run in an ephemeral git worktree at `.miter/worktrees/`.
    pub sandbox: bool,
    /// Unique task ID if sandboxed.
    pub task_id: Option<String>,
    /// Optional timeout budget in milliseconds (default: 15,000ms).
    pub timeout_ms: Option<u64>,
}

/// Arguments parsed for the `miter_audit` tool.
#[derive(Clone, Debug, PartialEq)]
pub struct AuditArgs {
    /// Line threshold above which files are flagged (defaults to workspace config or 500).
    pub threshold: Option<usize>,
    /// Root path to audit (defaults to workspace root).
    pub path: Option<String>,
}

/// Arguments parsed for the `miter_slice` tool.
#[derive(Clone, Debug, PartialEq)]
pub struct SliceArgs {
    /// Target file path to slice.
    pub path: String,
    /// Target lines for extraction clusters (default: 250).
    pub target_lines: usize,
}

/// Arguments parsed for the `miter_skeleton` tool.
#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonArgs {
    /// Target source file path to extract.
    pub path: String,
}

/// Arguments parsed for the `miter_references` tool.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferencesArgs {
    /// Target symbol identifier to resolve references for.
    pub symbol: String,
    /// Maximum number of reference records to return (default: 50).
    pub limit: usize,
}

/// Arguments parsed for the `miter_impact` tool.
#[derive(Clone, Debug, PartialEq)]
pub struct ImpactArgs {
    /// Target symbol identifier to calculate blast radius for.
    pub symbol: String,
    /// Maximum reverse traversal depth hops (default: 4).
    pub max_depth: usize,
}

mod parse;
mod schema;

pub use parse::{
    parse_audit_args, parse_impact_args, parse_lookup_args, parse_references_args,
    parse_skeleton_args, parse_slice_args, parse_verify_args,
};
pub use schema::list_tools;
