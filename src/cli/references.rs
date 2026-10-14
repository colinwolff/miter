//! `miter references`: Inverted lookup of symbol references, call sites, and AST coordinates.

use std::path::Path;

use miter_engine::{ReferenceEntry, ReferenceKind, StorageError, WorkspaceStore};

use super::error::CliError;
use super::json::Json;

/// Format choice for `miter references`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferencesFormat {
    /// Human-readable aligned table.
    Table,
    /// Machine-readable JSON output.
    Json,
}

impl ReferencesFormat {
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

/// Runs the `miter references` command.
pub fn run_references(
    symbol: &str,
    db: &Path,
    limit: usize,
    format: ReferencesFormat,
) -> Result<(), CliError> {
    if !db.exists() {
        return Err(CliError::MissingIndex(db.to_path_buf()));
    }

    let store = WorkspaceStore::open(db)
        .map_err(|e: StorageError| CliError::Usage(format!("failed to open database: {e}")))?;
    let mut entries = store
        .get_references_for_symbol(symbol)
        .map_err(|e: StorageError| CliError::Usage(format!("failed to query references: {e}")))?;

    if limit > 0 && entries.len() > limit {
        entries.truncate(limit);
    }

    match format {
        ReferencesFormat::Table => render_table(symbol, &entries),
        ReferencesFormat::Json => render_json(symbol, &entries),
    }

    Ok(())
}

fn render_table(symbol: &str, entries: &[ReferenceEntry]) {
    if entries.is_empty() {
        println!("No references found for symbol '{symbol}'.");
        return;
    }

    println!("{:<12} {:<36} {:<12} ENCLOSING", "KIND", "FILE", "LINES");
    println!("{}", "-".repeat(80));

    for entry in entries {
        let kind_str = match entry.reference_kind {
            ReferenceKind::Call => "call",
            ReferenceKind::TypeUsage => "type_usage",
            ReferenceKind::TraitImpl => "trait_impl",
            ReferenceKind::Import => "import",
            ReferenceKind::Usage => "usage",
        };
        let lines_str = format!("{}:{}", entry.line_range.0, entry.line_range.1);
        let enclosing_str = entry.enclosing_symbol.as_deref().unwrap_or("-");

        println!(
            "{:<12} {:<36} {:<12} {}",
            kind_str, entry.file_path, lines_str, enclosing_str
        );
    }
}

fn render_json(symbol: &str, entries: &[ReferenceEntry]) {
    let list: Vec<Json> = entries
        .iter()
        .map(|e| {
            let kind_str = match e.reference_kind {
                ReferenceKind::Call => "call",
                ReferenceKind::TypeUsage => "type_usage",
                ReferenceKind::TraitImpl => "trait_impl",
                ReferenceKind::Import => "import",
                ReferenceKind::Usage => "usage",
            };
            Json::object([
                ("file", Json::string(&e.file_path)),
                (
                    "byte_range",
                    Json::array([
                        Json::Unsigned(e.byte_range.0 as u64),
                        Json::Unsigned(e.byte_range.1 as u64),
                    ]),
                ),
                (
                    "line_range",
                    Json::array([
                        Json::Unsigned(e.line_range.0 as u64),
                        Json::Unsigned(e.line_range.1 as u64),
                    ]),
                ),
                (
                    "enclosing_symbol",
                    match &e.enclosing_symbol {
                        Some(enc) => Json::string(enc),
                        None => Json::Null,
                    },
                ),
                ("kind", Json::string(kind_str)),
            ])
        })
        .collect();

    let output = Json::object([
        ("symbol", Json::string(symbol)),
        ("count", Json::Unsigned(entries.len() as u64)),
        ("references", Json::Array(list)),
    ]);

    println!("{}", output.render());
}
