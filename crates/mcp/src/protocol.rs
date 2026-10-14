//! JSON-RPC 2.0 and MCP protocol message definitions.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 version string.
pub const JSONRPC_VERSION: &str = "2.0";

/// Supported MCP protocol version.
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// An incoming JSON-RPC request or notification.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct JsonRpcRequest {
    /// JSON-RPC protocol version (must be "2.0").
    #[serde(default = "default_jsonrpc")]
    pub jsonrpc: String,
    /// Request identifier (omitted for notifications).
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    /// Method name.
    pub method: String,
    /// Method parameters.
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

fn default_jsonrpc() -> String {
    JSONRPC_VERSION.to_owned()
}

/// An outgoing JSON-RPC response.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct JsonRpcResponse {
    /// JSON-RPC protocol version ("2.0").
    pub jsonrpc: String,
    /// Request identifier being responded to.
    pub id: serde_json::Value,
    /// Result payload if successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error payload if unsuccessful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Constructs a successful response for a request id.
    #[must_use]
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Constructs an error response for a request id.
    #[must_use]
    pub fn error(id: serde_json::Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// JSON-RPC error payload.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct JsonRpcError {
    /// Standard or application error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional diagnostic data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Method not found (-32601).
    #[must_use]
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }

    /// Invalid params (-32602).
    #[must_use]
    pub fn invalid_params(detail: &str) -> Self {
        Self {
            code: -32602,
            message: format!("Invalid params: {detail}"),
            data: None,
        }
    }

    /// Internal error (-32603).
    #[must_use]
    pub fn internal_error(detail: &str) -> Self {
        Self {
            code: -32603,
            message: format!("Internal error: {detail}"),
            data: None,
        }
    }

    /// Parse error (-32700).
    #[must_use]
    pub fn parse_error(detail: &str) -> Self {
        Self {
            code: -32700,
            message: format!("Parse error: {detail}"),
            data: None,
        }
    }
}

/// MCP server information for `initialize`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ImplementationInfo {
    /// Name of the server implementation.
    pub name: String,
    /// Version string of the implementation.
    pub version: String,
}

/// Server capabilities declared during initialization.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct ServerCapabilities {
    /// Tool execution capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
}

/// Tools capability descriptor.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct ToolsCapability {
    /// Whether the server emits tool list changes.
    #[serde(default)]
    pub list_changed: bool,
}

/// MCP initialize result payload.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct InitializeResult {
    /// Protocol version negotiated.
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    /// Capabilities supported by this server.
    pub capabilities: ServerCapabilities,
    /// Information about the server.
    #[serde(rename = "serverInfo")]
    pub server_info: ImplementationInfo,
}

/// One item in the content array returned by `tools/call`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolContent {
    /// Text response.
    Text {
        /// The text string.
        text: String,
    },
}

/// Payload returned by `tools/call`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CallToolResult {
    /// Formatted content items.
    pub content: Vec<ToolContent>,
    /// Whether the tool execution resulted in an error.
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl CallToolResult {
    /// Creates a success text result.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            is_error: None,
        }
    }

    /// Creates an error text result.
    #[must_use]
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            is_error: Some(true),
        }
    }

    /// Passes all text content in this result through the secret scrubber.
    #[must_use]
    pub fn scrub_egress(mut self) -> Self {
        for item in &mut self.content {
            let ToolContent::Text { text } = item;
            *text = miter_engine::scrub_source_text(text);
        }
        self
    }
}
