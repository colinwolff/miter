//! `miter impact`: Multi-hop reverse blast radius and test selector CLI.

use super::error::CliError;
use super::json::Json;
use miter_engine::{StorageError, WorkspaceStore, compute_impact};
use std::path::Path;

/// Format choice for `miter impact`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpactFormat {
    /// Human-readable aligned summary table.
    Table,
    /// Machine-readable JSON output.
    Json,
}

impl ImpactFormat {
    /// Parses format string (`table` or `json`).
    #[must_use]
    pub fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("json") {
            Self::Json
        } else {
            Self::Table
        }
    }
}

/// Runs the `miter impact` command.
pub fn run_impact(
    symbol: &str,
    depth: usize,
    db: &Path,
    format: ImpactFormat,
) -> Result<(), CliError> {
    if !db.exists() {
        return Err(CliError::MissingIndex(db.to_path_buf()));
    }

    let store = WorkspaceStore::open(db)
        .map_err(|e: StorageError| CliError::Usage(format!("failed to open database: {e}")))?;
    let impact = compute_impact(&store, symbol, depth).map_err(|e: StorageError| {
        CliError::Usage(format!("failed to compute blast radius: {e}"))
    })?;

    match format {
        ImpactFormat::Table => render_table(&impact),
        ImpactFormat::Json => render_json(&impact),
    }

    Ok(())
}

fn render_table(impact: &miter_engine::ImpactAnalysis) {
    println!(
        "Blast Radius for '{}' (max depth {}):",
        impact.target_symbol, impact.max_depth
    );
    println!("{}", "=".repeat(80));

    if !impact.affected_tests.is_empty() {
        println!("\n[Affected Tests] (Run these to verify changes):");
        println!("{:<32} {:<30} RUN COMMAND", "TEST SYMBOL", "FILE");
        println!("{}", "-".repeat(80));
        for t in &impact.affected_tests {
            println!("{:<32} {:<30} {}", t.symbol, t.file_path, t.test_command);
        }
    } else {
        println!(
            "\n[Affected Tests]: None detected within {} hops.",
            impact.max_depth
        );
    }

    if !impact.entrypoints_and_handlers.is_empty() {
        println!("\n[Entrypoints & Handlers]:");
        println!("{:<32} {:<30} {:<6}", "ENTRYPOINT", "FILE", "DEPTH");
        println!("{}", "-".repeat(80));
        for e in &impact.entrypoints_and_handlers {
            println!("{:<32} {:<30} {:<6}", e.symbol, e.file_path, e.depth);
        }
    }

    if !impact.transitive_callers.is_empty() {
        println!("\n[Transitive Callers]:");
        println!(
            "{:<32} {:<30} {:<10} {:<6}",
            "CALLER", "FILE", "LINES", "DEPTH"
        );
        println!("{}", "-".repeat(80));
        for c in &impact.transitive_callers {
            let lines_str = format!("{}:{}", c.line_range.0, c.line_range.1);
            println!(
                "{:<32} {:<30} {:<10} {:<6}",
                c.symbol, c.file_path, lines_str, c.depth
            );
        }
    }
}

fn render_json(impact: &miter_engine::ImpactAnalysis) {
    let json_tests: Vec<Json> = impact
        .affected_tests
        .iter()
        .map(|t| {
            Json::object([
                ("symbol", Json::string(&t.symbol)),
                ("file", Json::string(&t.file_path)),
                (
                    "lines",
                    Json::array([
                        Json::Unsigned(t.line_range.0 as u64),
                        Json::Unsigned(t.line_range.1 as u64),
                    ]),
                ),
                ("test_command", Json::string(&t.test_command)),
            ])
        })
        .collect();

    let json_entrypoints: Vec<Json> = impact
        .entrypoints_and_handlers
        .iter()
        .map(|e| {
            Json::object([
                ("symbol", Json::string(&e.symbol)),
                ("file", Json::string(&e.file_path)),
                (
                    "lines",
                    Json::array([
                        Json::Unsigned(e.line_range.0 as u64),
                        Json::Unsigned(e.line_range.1 as u64),
                    ]),
                ),
                ("depth", Json::Unsigned(e.depth as u64)),
            ])
        })
        .collect();

    let json_callers: Vec<Json> = impact
        .transitive_callers
        .iter()
        .map(|c| {
            Json::object([
                ("symbol", Json::string(&c.symbol)),
                ("file", Json::string(&c.file_path)),
                (
                    "lines",
                    Json::array([
                        Json::Unsigned(c.line_range.0 as u64),
                        Json::Unsigned(c.line_range.1 as u64),
                    ]),
                ),
                ("depth", Json::Unsigned(c.depth as u64)),
            ])
        })
        .collect();

    let root = Json::object([
        ("target_symbol", Json::string(&impact.target_symbol)),
        ("max_depth", Json::Unsigned(impact.max_depth as u64)),
        ("affected_tests", Json::Array(json_tests)),
        ("entrypoints_and_handlers", Json::Array(json_entrypoints)),
        ("transitive_callers", Json::Array(json_callers)),
    ]);

    println!("{}", root.render());
}
