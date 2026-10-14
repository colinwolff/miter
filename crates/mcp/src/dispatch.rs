//! The JSON-RPC dispatch layer: one request in, one response out.
//!
//! Nothing here reads a stream; [`crate::run_server_loop`] owns the transport and hands each
//! parsed line to [`process_request`].

use std::path::Path;

use crate::protocol::{
    CallToolResult, ImplementationInfo, InitializeResult, JsonRpcError, JsonRpcRequest,
    JsonRpcResponse, MCP_PROTOCOL_VERSION, ServerCapabilities, ToolsCapability,
};
use crate::{handler, tools};

/// Processes a single incoming JSON-RPC request or notification.
/// Returns `Some(response)` for requests, and `None` for notifications.
pub async fn process_request(
    request: &JsonRpcRequest,
    db_path: &Path,
    workspace_root: &Path,
) -> Option<JsonRpcResponse> {
    let id = match &request.id {
        Some(id) => id.clone(),
        None => {
            // Notifications (no id) have no response
            handle_notification(request);
            return None;
        }
    };

    let response = match request.method.as_str() {
        "initialize" => {
            let result = InitializeResult {
                protocol_version: MCP_PROTOCOL_VERSION.to_owned(),
                capabilities: ServerCapabilities {
                    tools: Some(ToolsCapability {
                        list_changed: false,
                    }),
                },
                server_info: ImplementationInfo {
                    name: "miter-mcp".to_owned(),
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                },
            };
            JsonRpcResponse::success(id, serde_json::to_value(result).unwrap_or_default())
        }

        "ping" => JsonRpcResponse::success(id, serde_json::json!({})),

        "tools/list" => {
            let tools = tools::list_tools();
            let result = serde_json::json!({ "tools": tools });
            JsonRpcResponse::success(id, result)
        }

        "tools/call" => {
            let result = dispatch_tool_call(request, db_path, workspace_root).await;
            match result {
                Ok(tool_result) => JsonRpcResponse::success(
                    id,
                    serde_json::to_value(tool_result).unwrap_or_default(),
                ),
                Err(error) => JsonRpcResponse::error(id, error),
            }
        }

        unknown => JsonRpcResponse::error(id, JsonRpcError::method_not_found(unknown)),
    };

    Some(response)
}

/// Handles notifications (like `notifications/initialized`).
fn handle_notification(request: &JsonRpcRequest) {
    match request.method.as_str() {
        "notifications/initialized" => {
            tracing::info!("MCP client initialized notification received");
        }
        other => {
            tracing::debug!(method = %other, "Unhandled notification");
        }
    }
}

/// Dispatches a `tools/call` request to its handler.
async fn dispatch_tool_call(
    request: &JsonRpcRequest,
    db_path: &Path,
    workspace_root: &Path,
) -> Result<CallToolResult, JsonRpcError> {
    let Some(params) = request
        .params
        .as_ref()
        .and_then(serde_json::Value::as_object)
    else {
        return Err(JsonRpcError::invalid_params("Expected params object"));
    };

    let name = params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| JsonRpcError::invalid_params("Missing tool 'name'"))?;

    let arguments = params.get("arguments").cloned();

    match name {
        "miter_lookup" => {
            let args = tools::parse_lookup_args(&arguments)
                .map_err(|e| JsonRpcError::invalid_params(&e))?;
            Ok(handler::handle_lookup(db_path, workspace_root, args).scrub_egress())
        }

        "miter_verify" => {
            let args = tools::parse_verify_args(&arguments)
                .map_err(|e| JsonRpcError::invalid_params(&e))?;
            Ok(handler::handle_verify(workspace_root, args).await)
        }

        "miter_audit" => {
            let args = tools::parse_audit_args(&arguments)
                .map_err(|e| JsonRpcError::invalid_params(&e))?;
            Ok(handler::handle_audit(workspace_root, args))
        }

        "miter_slice" => {
            let args = tools::parse_slice_args(&arguments)
                .map_err(|e| JsonRpcError::invalid_params(&e))?;
            Ok(handler::handle_slice(db_path, workspace_root, args).scrub_egress())
        }

        "miter_skeleton" => {
            let args = tools::parse_skeleton_args(&arguments)
                .map_err(|e| JsonRpcError::invalid_params(&e))?;
            Ok(handler::handle_skeleton(workspace_root, args).scrub_egress())
        }

        "miter_references" => {
            let args = tools::parse_references_args(&arguments)
                .map_err(|e| JsonRpcError::invalid_params(&e))?;
            Ok(handler::handle_references(db_path, workspace_root, args))
        }

        "miter_impact" => {
            let args = tools::parse_impact_args(&arguments)
                .map_err(|e| JsonRpcError::invalid_params(&e))?;
            Ok(handler::handle_impact(db_path, workspace_root, args))
        }

        unknown => Err(JsonRpcError::method_not_found(&format!(
            "Unknown tool: {unknown}"
        ))),
    }
}
