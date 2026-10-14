//! Integration tests for the MCP protocol server.
//!
//! `handshake` covers the JSON-RPC envelope and the stdio loop; `tools` covers one test per
//! tool against a real index.

mod handshake;
mod tools;
