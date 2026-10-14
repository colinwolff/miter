//! The JSON-RPC envelope: initialize, tools/list and the stdio loop.

use std::path::PathBuf;

use tokio::io::AsyncWriteExt;

use miter_mcp::protocol::{
    InitializeResult, JsonRpcRequest, JsonRpcResponse, MCP_PROTOCOL_VERSION,
};

#[tokio::test]
async fn test_initialize_handshake() {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        id: Some(serde_json::json!(1)),
        method: "initialize".to_owned(),
        params: Some(serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "1.0.0" }
        })),
    };

    let response =
        miter_mcp::process_request(&request, &PathBuf::from("miter.redb"), &PathBuf::from("."))
            .await
            .expect("must produce a response");

    assert_eq!(response.id, serde_json::json!(1));
    assert!(response.error.is_none());

    let result: InitializeResult = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(result.protocol_version, MCP_PROTOCOL_VERSION);
    assert_eq!(result.server_info.name, "miter-mcp");
    assert!(result.capabilities.tools.is_some());
}

#[tokio::test]
async fn test_tools_list() {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        id: Some(serde_json::json!(2)),
        method: "tools/list".to_owned(),
        params: None,
    };

    let response =
        miter_mcp::process_request(&request, &PathBuf::from("miter.redb"), &PathBuf::from("."))
            .await
            .expect("must produce a response");

    assert_eq!(response.id, serde_json::json!(2));
    let result = response.result.expect("result present");
    let tools = result
        .get("tools")
        .and_then(|t| t.as_array())
        .expect("tools array");

    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();

    assert!(
        tool_names.contains(&"miter_lookup"),
        "miter_lookup must be in tools"
    );
    assert!(
        tool_names.contains(&"miter_verify"),
        "miter_verify must be in tools"
    );
    assert!(
        tool_names.contains(&"miter_audit"),
        "miter_audit must be in tools"
    );
    assert!(
        tool_names.contains(&"miter_slice"),
        "miter_slice must be in tools"
    );
    assert!(
        tool_names.contains(&"miter_skeleton"),
        "miter_skeleton must be in tools"
    );
    assert!(
        tool_names.contains(&"miter_references"),
        "miter_references must be in tools"
    );

    // Check schema for miter_lookup
    let lookup_tool = tools.iter().find(|t| t["name"] == "miter_lookup").unwrap();
    let schema = &lookup_tool["inputSchema"];
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["target"].is_object());
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("target"))
    );

    // Check schema for miter_verify
    let verify_tool = tools.iter().find(|t| t["name"] == "miter_verify").unwrap();
    let verify_schema = &verify_tool["inputSchema"];
    assert_eq!(verify_schema["type"], "object");
    assert!(verify_schema["properties"]["path"].is_object());
    assert!(verify_schema["properties"]["sandbox"].is_object());
    assert!(verify_schema["properties"]["timeout_ms"].is_object());

    // Check schema for miter_audit
    let audit_tool = tools.iter().find(|t| t["name"] == "miter_audit").unwrap();
    let audit_schema = &audit_tool["inputSchema"];
    assert_eq!(audit_schema["type"], "object");
    assert!(audit_schema["properties"]["threshold"].is_object());

    // Check schema for miter_slice
    let slice_tool = tools.iter().find(|t| t["name"] == "miter_slice").unwrap();
    let slice_schema = &slice_tool["inputSchema"];
    assert_eq!(slice_schema["type"], "object");
    assert!(slice_schema["properties"]["path"].is_object());
    assert!(
        slice_schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("path"))
    );

    // Check schema for miter_references
    let refs_tool = tools
        .iter()
        .find(|t| t["name"] == "miter_references")
        .unwrap();
    let refs_schema = &refs_tool["inputSchema"];
    assert_eq!(refs_schema["type"], "object");
    assert!(refs_schema["properties"]["symbol"].is_object());
    assert!(
        refs_schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("symbol"))
    );
}

#[tokio::test]
async fn test_server_loop_stream() {
    // Test the line-delimited stdio server loop over in-memory duplex streams
    let (server_stdin, mut client_stdout) = tokio::io::duplex(4096);
    let (client_stdin, server_stdout) = tokio::io::duplex(4096);

    let db_path = PathBuf::from("non_existent_db.redb");

    let server_handle = tokio::spawn(async move {
        miter_mcp::run_server_loop(
            tokio::io::BufReader::new(server_stdin),
            server_stdout,
            db_path,
        )
        .await
    });

    let mut client_reader = tokio::io::BufReader::new(client_stdin);

    // Send initialize request
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    });
    client_stdout
        .write_all(format!("{init_req}\n").as_bytes())
        .await
        .unwrap();

    let mut response_line = String::new();
    use tokio::io::AsyncBufReadExt;
    client_reader.read_line(&mut response_line).await.unwrap();

    let resp: JsonRpcResponse = serde_json::from_str(response_line.trim()).unwrap();
    assert_eq!(resp.id, serde_json::json!(1));
    assert!(resp.result.is_some());

    // Send ping request
    let ping_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "ping"
    });
    client_stdout
        .write_all(format!("{ping_req}\n").as_bytes())
        .await
        .unwrap();

    response_line.clear();
    client_reader.read_line(&mut response_line).await.unwrap();
    let resp: JsonRpcResponse = serde_json::from_str(response_line.trim()).unwrap();
    assert_eq!(resp.id, serde_json::json!(2));
    assert_eq!(resp.result, Some(serde_json::json!({})));

    // Drop client writer to trigger EOF and cleanly close server loop
    drop(client_stdout);
    let _ = server_handle.await;
}
