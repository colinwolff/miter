//! # `mcp` — Model Context Protocol stdio server for MITER
//!
//! Exposes AST symbol hydration (`miter_lookup`) and compiler verification (`miter_verify`)
//! over standard I/O (JSON-RPC 2.0 lines).

pub mod handler;
pub mod protocol;
pub mod tools;

use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

/// Resolves the database the server reads.
///
/// An empty path means `<workspace_root>/.miter/miter.redb`; a relative path is taken against the
/// workspace root; an absolute path is used as given.
fn resolve_db_path(db_path: PathBuf, workspace_root: &Path) -> PathBuf {
    if db_path.as_os_str().is_empty() {
        miter_storage::default_db_path(workspace_root)
    } else if db_path.is_relative() {
        workspace_root.join(db_path)
    } else {
        db_path
    }
}

/// Starts the MCP stdio server on the current stdin and stdout.
///
/// Ensures all log outputs or warnings go to stderr, as stdout is strictly reserved
/// for newline-delimited JSON-RPC messages.
///
/// # Errors
/// Returns an I/O error if reading stdin or writing stdout fails.
pub async fn run_stdio_server(db_path: PathBuf) -> Result<(), std::io::Error> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let reader = BufReader::new(stdin);
    let writer = stdout;

    run_server_loop(reader, writer, db_path).await
}

/// Runs the protocol read/write loop over generic async I/O streams.
pub async fn run_server_loop<R, W>(
    mut reader: R,
    mut writer: W,
    db_path: PathBuf,
) -> Result<(), std::io::Error>
where
    R: AsyncBufReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let mut line = String::new();
    // One root, resolved once: the nearest `.miter/` or `.git/` marker at or above the server's
    // working directory, or that directory itself. The database is `<root>/.miter/miter.redb`.
    let workspace_root = miter_storage::resolve_workspace_root(
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    );
    let db_path = resolve_db_path(db_path, &workspace_root);

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            // EOF reached on stdin
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(e) => {
                let error_resp = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    JsonRpcError::parse_error(&e.to_string()),
                );
                send_response(&mut writer, &error_resp).await?;
                continue;
            }
        };

        if let Some(response) = process_request(&request, &db_path, &workspace_root).await {
            send_response(&mut writer, &response).await?;
        }
    }

    Ok(())
}

/// Sends a JSON-RPC response as a newline-delimited JSON string.
async fn send_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    response: &JsonRpcResponse,
) -> Result<(), std::io::Error> {
    let mut payload = serde_json::to_string(response).unwrap_or_default();
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

mod dispatch;

pub use dispatch::process_request;
