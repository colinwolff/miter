//! The clap surface: the `Cli` parser and its `Command` subcommands.
//!
//! Every subcommand the binary exposes is declared here and nothing else — no logic, no I/O — so
//! the command set can be read (and reviewed) in one place. `execute` in [`super`] is the only
//! thing that dispatches it, and the types are re-exported from `crate::cli`, so a caller naming
//! `miter::cli::Command` does not have to know this module exists.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// `miter` — deterministic AST co-processor and MCP server.
#[derive(Debug, Parser)]
#[command(
    name = "miter",
    version,
    about = "Deterministic AST co-processor and Model Context Protocol (MCP) server",
    long_about = "Indexes a polyglot source tree into an embedded database and answers structural \n\
                  queries: symbol hydration, interface skeletons, reverse references, blast radius, \n\
                  schema contracts and clone detection. Intended as a tool interface for autonomous \n\
                  coding agents: every command can emit JSON.\n\n\
                  DATABASE LOCKING:\n\
                  a database is one writer at a time. Read-only commands (lookup, references, \n\
                  impact, contracts) take a shared lock and can run concurrently with each other, \n\
                  but a command that writes (index) holds an exclusive lock, and a read against it \n\
                  fails with \"Database already open\". Point --db at a different file to work in \n\
                  parallel."
)]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Subcommand definitions.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Parse a source tree and write the encoded symbols into the database.
    Index {
        /// Directory (or file) to index.
        path: PathBuf,
        /// Database file to write (defaults to `<workspace_root>/.miter/miter.redb`).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Incremental dirty-file indexing mode driven by mtime and git status.
        #[arg(long)]
        dirty: bool,
        /// Emit JSON instead of a human-readable summary.
        #[arg(long)]
        json: bool,
    },
    /// Locate a declaration and print its hydrated context.
    Lookup {
        /// Declaration name, or a qualified path such as `miter_engine::store::Engine`.
        symbol: String,
        /// Database file to read (defaults to `<workspace_root>/.miter/miter.redb`).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Print the hydrated neighbourhood rather than the bare identity.
        #[arg(long)]
        context: bool,
        /// Emit JSON instead of the rendered payload.
        #[arg(long)]
        json: bool,
        /// Token ceiling for the rendered payload.
        #[arg(long, default_value_t = miter_engine::DEFAULT_MAX_TOKENS)]
        max_tokens: usize,
    },
    /// Launch the Model Context Protocol (MCP) stdio server for AI coding agents.
    ///
    /// Speaks JSON-RPC 2.0 over standard I/O (stdin/stdout), exposing `miter_audit`,
    /// `miter_skeleton`, `miter_references`, `miter_lookup`, `miter_slice`, `miter_verify`,
    /// `miter_impact`.
    Mcp {
        /// Database file to read (defaults to `<workspace_root>/.miter/miter.redb`).
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Scan workspace source files to flag files exceeding line-count thresholds.
    Audit {
        /// Target root directory to audit (defaults to current workspace).
        #[arg(long)]
        path: Option<PathBuf>,
        /// Line count threshold to trigger warning/critical violations (default: 500).
        #[arg(long)]
        threshold: Option<usize>,
        /// Output format (`table` or `json`).
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Analyze AST co-reference graphs and generate extraction blueprints.
    Slice {
        /// Target source file path to analyze.
        path: PathBuf,
        /// Target line count per extraction cluster (default: 250).
        #[arg(long, default_value_t = 250)]
        target_lines: usize,
        /// Output format (`human` or `json`).
        #[arg(long, default_value = "human")]
        format: String,
    },
    /// Extract token-compressed AST interface skeleton with implementation bodies stripped.
    Skeleton {
        /// Target source file path to extract.
        path: PathBuf,
        /// Output format (`text` or `json`).
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Inverted reverse-lookup of symbol references and call sites across the workspace.
    References {
        /// Target symbol name to find references for.
        symbol: String,
        /// Database file to read (defaults to `<workspace_root>/.miter/miter.redb`).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Maximum number of reference records to return.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Output format (`table` or `json`).
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Multi-hop blast radius analysis and affected unit test selector.
    Impact {
        /// Target symbol name to analyze blast radius for.
        symbol: String,
        /// Maximum reverse traversal depth hops (default: 4).
        #[arg(long, default_value_t = 4)]
        depth: usize,
        /// Database file to read (defaults to `<workspace_root>/.miter/miter.redb`).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Output format (`table` or `json`).
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Verify polyglot schema contracts across SQL, Rust, and TypeScript.
    Contracts {
        /// Database file to read (defaults to `<workspace_root>/.miter/miter.redb`).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Output format (`table` or `json`).
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Detect deterministic AST structural clones and duplication candidates.
    Clones {
        /// Target path or directory to scan for structural clones (defaults to current dir).
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Minimum line count for function analysis (default: 15).
        #[arg(long, default_value_t = 15)]
        min_lines: usize,
        /// Minimum structural similarity threshold in 0.0..=1.0 (default: 0.90).
        #[arg(long, default_value_t = 0.90)]
        similarity: f32,
        /// Output format (`table` or `json`).
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Run isolated compiler verification on the workspace or crate.
    Verify {
        /// Target path to verify (defaults to current directory).
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Run in an ephemeral git worktree sandbox.
        #[arg(long)]
        sandbox: bool,
        /// Task ID for the sandbox.
        #[arg(long)]
        task_id: Option<String>,
        /// Timeout budget in milliseconds (default: 15,000ms).
        #[arg(long, default_value_t = 15_000)]
        timeout_ms: u64,
        /// Emit JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
}
