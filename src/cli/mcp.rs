//! CLI entrypoint for `miter mcp`.
//!
//! Launches the Model Context Protocol stdio server for AI coding agents.

use std::path::PathBuf;

use crate::cli::CliError;

/// Runs the MCP stdio server.
///
/// Ensures all tracing logs are routed strictly to stderr, so stdout remains
/// pure JSON-RPC newline-delimited frames.
pub fn run_mcp(db: PathBuf) -> Result<(), CliError> {
    // Route any tracing to stderr
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Usage(format!("failed to start tokio runtime: {error}")))?;

    rt.block_on(async move { miter_mcp::run_stdio_server(db).await })
        .map_err(|error| {
            CliError::Usage(format!("mcp stdio server terminated with error: {error}"))
        })
}
