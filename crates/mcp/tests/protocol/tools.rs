//! One test per tool, against a real index built in a temporary directory.

use std::path::PathBuf;

use miter_mcp::protocol::{CallToolResult, JsonRpcRequest};

#[tokio::test]
async fn test_tools_call_lookup_on_fixture() {
    let temp_dir = std::env::temp_dir().join(format!("miter_mcp_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let db_path = temp_dir.join("test.redb");

    // Ingest a dummy symbol into an in-memory engine and save it to test.redb
    let mut engine = miter_engine::Engine::new();
    engine
        .ingest_polyglot_file(
            "/// The symbol table.\npub struct SymbolTable { pub name: String }\nimpl SymbolTable {\n    pub fn run(&self) -> bool { true }\n}\n",
            std::path::Path::new("src/verifier.rs"),
            0,
            miter_engine::polyglot::Language::Rust,
        )
        .expect("valid rust ingestion");

    engine.save_redb(&db_path).expect("saved redb");

    // Test 1: backwards compatibility fallback when only `symbol` is passed
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        id: Some(serde_json::json!(3)),
        method: "tools/call".to_owned(),
        params: Some(serde_json::json!({
            "name": "miter_lookup",
            "arguments": {
                "symbol": "SymbolTable",
                "context": true,
                "max_tokens": 256
            }
        })),
    };

    let response = miter_mcp::process_request(&request, &db_path, &PathBuf::from("."))
        .await
        .unwrap();
    assert_eq!(response.id, serde_json::json!(3));
    assert!(response.error.is_none());

    let result: CallToolResult = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(result.is_error, None);
    assert!(!result.content.is_empty());

    let miter_mcp::protocol::ToolContent::Text { text } = &result.content[0];
    assert!(
        text.contains("SymbolTable"),
        "output contains symbol name: {text}"
    );
    assert!(
        text.contains("pub struct SymbolTable"),
        "output contains signature: {text}"
    );

    // Test 2: tools/call with target and scope: "module"
    let mod_request = JsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        id: Some(serde_json::json!(4)),
        method: "tools/call".to_owned(),
        params: Some(serde_json::json!({
            "name": "miter_lookup",
            "arguments": {
                "target": "verifier",
                "scope": "module",
                "depth": 1,
                "max_tokens": 512
            }
        })),
    };

    let mod_response = miter_mcp::process_request(&mod_request, &db_path, &PathBuf::from("."))
        .await
        .unwrap();
    assert_eq!(mod_response.id, serde_json::json!(4));
    assert!(mod_response.error.is_none());

    let mod_result: CallToolResult = serde_json::from_value(mod_response.result.unwrap()).unwrap();
    assert_eq!(mod_result.is_error, None);
    let miter_mcp::protocol::ToolContent::Text { text: mod_text } = &mod_result.content[0];
    assert!(
        mod_text.contains("Module Interface"),
        "output contains module interface header: {mod_text}"
    );
    assert!(
        mod_text.contains("pub struct SymbolTable"),
        "output contains exported item: {mod_text}"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_audit_and_slice_tools() {
    let temp_dir =
        std::env::temp_dir().join(format!("miter_mcp_audit_slice_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let db_path = temp_dir.join("test.redb");

    // Create a dummy source file
    let fixture_file = temp_dir.join("sample.rs");
    let mut fixture = String::new();
    for i in 0..60 {
        fixture.push_str(&format!("pub struct Struct{i} {{ pub id: u64 }}\n"));
    }
    std::fs::write(&fixture_file, &fixture).unwrap();

    // 1. Call miter_audit with low threshold
    let audit_req = JsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        id: Some(serde_json::json!(20)),
        method: "tools/call".to_owned(),
        params: Some(serde_json::json!({
            "name": "miter_audit",
            "arguments": {
                "threshold": 50,
                "path": "."
            }
        })),
    };

    let audit_resp = miter_mcp::process_request(&audit_req, &db_path, &temp_dir)
        .await
        .unwrap();
    assert_eq!(audit_resp.id, serde_json::json!(20));
    assert!(audit_resp.error.is_none());
    let audit_result: CallToolResult = serde_json::from_value(audit_resp.result.unwrap()).unwrap();
    let miter_mcp::protocol::ToolContent::Text { text: audit_text } = &audit_result.content[0];
    assert!(
        audit_text.contains("AUDIT WARNING"),
        "must flag violation: {audit_text}"
    );

    // 2. Call miter_slice
    let slice_req = JsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        id: Some(serde_json::json!(21)),
        method: "tools/call".to_owned(),
        params: Some(serde_json::json!({
            "name": "miter_slice",
            "arguments": {
                "path": "sample.rs",
                "target_lines": 30
            }
        })),
    };

    let slice_resp = miter_mcp::process_request(&slice_req, &db_path, &temp_dir)
        .await
        .unwrap();
    assert_eq!(slice_resp.id, serde_json::json!(21));
    assert!(slice_resp.error.is_none());
    let slice_result: CallToolResult = serde_json::from_value(slice_resp.result.unwrap()).unwrap();
    let miter_mcp::protocol::ToolContent::Text { text: slice_text } = &slice_result.content[0];
    assert!(
        slice_text.contains("candidate_file"),
        "must emit blueprint: {slice_text}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_references_tool() {
    let temp_dir = std::env::temp_dir().join(format!("miter_refs_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let db_path = temp_dir.join("miter.redb");
    let store = miter_storage::WorkspaceStore::create(&db_path).unwrap();

    // Insert dummy reference
    let refs = vec![(
        "authenticate".to_string(),
        miter_storage::ReferenceEntry {
            file_path: "src/main.rs".to_string(),
            byte_range: (10, 50),
            line_range: (2, 3),
            enclosing_symbol: Some("run".to_string()),
            reference_kind: miter_storage::ReferenceKind::Call,
        },
    )];
    store.insert_references("src/main.rs", refs).unwrap();
    drop(store);

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_owned(),
        id: Some(serde_json::json!(42)),
        method: "tools/call".to_owned(),
        params: Some(serde_json::json!({
            "name": "miter_references",
            "arguments": {
                "symbol": "authenticate",
                "limit": 10
            }
        })),
    };

    let resp = miter_mcp::process_request(&req, &db_path, &temp_dir)
        .await
        .unwrap();
    assert_eq!(resp.id, serde_json::json!(42));
    assert!(resp.error.is_none());
    let res: CallToolResult = serde_json::from_value(resp.result.unwrap()).unwrap();
    let miter_mcp::protocol::ToolContent::Text { text } = &res.content[0];
    assert!(text.contains("authenticate"));
    assert!(text.contains("src/main.rs"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
