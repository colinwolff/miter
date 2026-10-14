//! `miter slice`: Analyzes AST co-reference graphs and emits extraction blueprints.

use super::error::CliError;
use super::json::Json;
use miter_engine::analysis::{ExtractionBlueprint, SeamSlicer};
use std::fs;
use std::path::Path;

/// Format choice for `miter slice`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliceFormat {
    /// Human-readable blueprint report.
    Human,
    /// Raw JSON representation.
    Json,
}

impl SliceFormat {
    /// Parses format string (`human` or `json`).
    #[must_use]
    pub fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("json") {
            Self::Json
        } else {
            Self::Human
        }
    }
}

/// Runs the `miter slice` command.
pub fn run_slice(path: &Path, target_lines: usize, format: SliceFormat) -> Result<(), CliError> {
    let content = fs::read_to_string(path).map_err(CliError::Io)?;
    let slicer = SeamSlicer::with_target_lines(target_lines);
    let blueprints = slicer.slice_file(path, &content);

    match format {
        SliceFormat::Json => {
            println!("{}", slice_json(&blueprints, path).render());
        }
        SliceFormat::Human => {
            render_slice_human(&blueprints, path, target_lines);
        }
    }

    Ok(())
}

fn slice_json(blueprints: &[ExtractionBlueprint], path: &Path) -> Json {
    let items = blueprints.iter().map(|bp| {
        let syms = bp.symbols_to_move.iter().map(|s| Json::string(s.clone()));
        let exports = bp.suggested_exports.iter().map(|s| Json::string(s.clone()));
        let imports = bp.suggested_imports.iter().map(|s| Json::string(s.clone()));

        Json::object([
            ("source_file", Json::string(bp.source_file.clone())),
            ("candidate_file", Json::string(bp.candidate_file.clone())),
            ("line_start", Json::Unsigned(bp.line_range.0 as u64)),
            ("line_end", Json::Unsigned(bp.line_range.1 as u64)),
            ("byte_start", Json::Unsigned(bp.byte_range.0 as u64)),
            ("byte_end", Json::Unsigned(bp.byte_range.1 as u64)),
            ("symbols_to_move", Json::array(syms)),
            ("suggested_exports", Json::array(exports)),
            ("suggested_imports", Json::array(imports)),
            (
                "suggested_mod_decl",
                Json::string(bp.suggested_mod_decl.clone()),
            ),
            (
                "estimated_remaining_lines",
                Json::Unsigned(bp.estimated_remaining_lines as u64),
            ),
            ("has_circular_risk", Json::Bool(bp.has_circular_risk)),
        ])
    });

    Json::object([
        ("target_file", Json::string(path.display().to_string())),
        ("cluster_count", Json::Unsigned(blueprints.len() as u64)),
        ("blueprints", Json::array(items)),
    ])
}

fn render_slice_human(blueprints: &[ExtractionBlueprint], path: &Path, target_lines: usize) {
    if blueprints.is_empty() {
        println!(
            "No extractable clusters found for {} (target: {} lines). File may already be small or indivisible.",
            path.display(),
            target_lines
        );
        return;
    }

    println!(
        "EXTRACTION BLUEPRINTS: Found {} cluster(s) for {} targeting ≤{} lines:\n",
        blueprints.len(),
        path.display(),
        target_lines
    );

    for (idx, bp) in blueprints.iter().enumerate() {
        println!("=== Cluster #{} -> {} ===", idx + 1, bp.candidate_file);
        println!(
            "  Source Lines: {}-{} ({} lines)",
            bp.line_range.0,
            bp.line_range.1,
            (bp.line_range.1.saturating_sub(bp.line_range.0)) + 1
        );
        println!("  Symbols to Move: {}", bp.symbols_to_move.join(", "));
        println!("  Parent mod decl: {}", bp.suggested_mod_decl);
        if !bp.suggested_exports.is_empty() {
            println!("  Suggested Exports: {}", bp.suggested_exports.join(", "));
        }
        if !bp.suggested_imports.is_empty() {
            println!("  Suggested Imports: {}", bp.suggested_imports.join(", "));
        }
        println!(
            "  Est. Remaining Lines in Parent: {}\n",
            bp.estimated_remaining_lines
        );
    }
}
