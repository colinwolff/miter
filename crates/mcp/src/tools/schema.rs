//! The `tools/list` document: one JSON Schema per tool.

use serde_json::json;

use super::ToolDefinition;

/// Returns the list of tools exposed by this MCP server.
#[must_use]
pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "miter_lookup".to_owned(),
            description: "Inspect symbols, module boundaries, or entire crate API surfaces without \
                          reading full source files. Use scope='symbol' for surgical signatures (~120 \
                          tokens), or scope='module'/'package' (up to 2048 tokens) for architectural \
                          audits across monorepo boundaries."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "The symbol name, module path, or crate name (alias 'symbol' for backwards compatibility)."
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Alias for target (backwards compatibility)."
                    },
                    "scope": {
                        "type": "string",
                        "description": "One of ['symbol', 'module', 'package'].",
                        "enum": ["symbol", "module", "package"],
                        "default": "symbol"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "1 = direct declarations; 2 = declarations + immediate neighbor types/callers.",
                        "default": 1
                    },
                    "context": {
                        "type": "boolean",
                        "description": "Whether to include the neighbouring declarations and byte ranges (scope='symbol').",
                        "default": true
                    },
                    "max_tokens": {
                        "type": "number",
                        "description": "Maximum token budget (clamped between 64 and 2048).",
                        "default": 256
                    }
                },
                "required": ["target"]
            }),
        },
        ToolDefinition {
            name: "miter_verify".to_owned(),
            description: "Run isolated, deterministic compiler and type verification on the crate \
                          or workspace. Returns sliced diagnostic spans for errors with zero noisy \
                          build scrollback. Use this immediately after editing code to verify changes."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the target crate (default: current workspace)."
                    },
                    "sandbox": {
                        "type": "boolean",
                        "description": "Whether to run in an ephemeral git worktree at '.miter/worktrees/'.",
                        "default": false
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Unique ID for the task if sandboxed."
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Maximum verification timeout budget in milliseconds (default: 15000).",
                        "default": 15000
                    }
                }
            }),
        },
        ToolDefinition {
            name: "miter_audit".to_owned(),
            description: "Scan workspace source files to flag files exceeding line-count thresholds \
                          (opinionated default: 500 lines). Returns violation severities ('Warning' \
                          500–799 vs 'Critical' >= 800) and declaration counts."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "threshold": {
                        "type": "integer",
                        "description": "Maximum line count threshold (default: 500).",
                        "default": 500
                    },
                    "path": {
                        "type": "string",
                        "description": "Root path to audit (defaults to workspace root)."
                    }
                }
            }),
        },
        ToolDefinition {
            name: "miter_slice".to_owned(),
            description: "Analyze AST co-reference graphs to identify natural module boundaries \
                          and emit structured extraction blueprints targeting §14 modularity (200–250 lines). \
                          Does NOT rewrite files on disk."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Target source file path to analyze."
                    },
                    "target_lines": {
                        "type": "integer",
                        "description": "Target line count per extraction cluster (default: 250).",
                        "default": 250
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "miter_skeleton".to_owned(),
            description: "Extracts token-efficient interface skeletons (types, signatures, trait bounds, \
                          docstrings) with method bodies stripped."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Target source file path to extract skeleton from."
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "miter_references".to_owned(),
            description: "PRIMARY INTERFACE for reverse-lookup symbol references and call sites. \
                          Returns AST-precise coordinates without grep noise."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Target symbol identifier to look up references and call sites for."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of reference records to return (default: 50).",
                        "default": 50
                    }
                },
                "required": ["symbol"]
            }),
        },
        ToolDefinition {
            name: "miter_impact".to_owned(),
            description: "Calculates multi-hop reverse blast radius and test selector. \
                          Identifies impacted unit tests, entrypoints, and transitive callers."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Target symbol identifier to calculate blast radius for."
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum reverse traversal depth hops (default: 4).",
                        "default": 4
                    }
                },
                "required": ["symbol"]
            }),
        },
    ]
}
