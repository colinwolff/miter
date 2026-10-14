//! `miter clones`: Deterministic AST structural clone detection CLI.

use super::error::CliError;
use super::json::Json;
use miter_engine::{
    CloneDetectionReport, FunctionFingerprint, extract_fingerprints_from_rust, find_clones,
};
use std::fs;
use std::path::Path;

/// Format choice for `miter clones`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClonesFormat {
    /// Human-readable aligned summary table.
    Table,
    /// Machine-readable JSON output.
    Json,
}

impl ClonesFormat {
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

/// Runs the `miter clones` command.
pub fn run_clones(
    root: &Path,
    min_lines: usize,
    min_similarity: f32,
    format: ClonesFormat,
) -> Result<(), CliError> {
    let mut fingerprints = Vec::new();
    collect_fingerprints(root, min_lines, &mut fingerprints)?;

    let report = find_clones(&fingerprints, min_similarity);

    match format {
        ClonesFormat::Table => render_table(&report, min_similarity),
        ClonesFormat::Json => render_json(&report),
    }

    Ok(())
}

fn collect_fingerprints(
    dir: &Path,
    min_lines: usize,
    out: &mut Vec<FunctionFingerprint>,
) -> Result<(), CliError> {
    if dir.is_file() {
        if dir.extension().and_then(|e| e.to_str()) == Some("rs")
            && let Ok(content) = fs::read_to_string(dir)
        {
            out.extend(extract_fingerprints_from_rust(dir, &content, min_lines));
        }
        return Ok(());
    }

    let mut queue = vec![dir.to_path_buf()];
    while let Some(current) = queue.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || matches!(name, "target" | "node_modules" | "dist" | "build")
            {
                continue;
            }
            if path.is_dir() {
                queue.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Ok(content) = fs::read_to_string(&path)
            {
                out.extend(extract_fingerprints_from_rust(&path, &content, min_lines));
            }
        }
    }

    Ok(())
}

fn render_table(report: &CloneDetectionReport, min_similarity: f32) {
    println!(
        "AST Structural Clone Detection (simhash similarity >= {:.0}%):",
        min_similarity * 100.0
    );
    println!("{}", "=".repeat(80));
    println!("Functions Analyzed: {}", report.functions_analyzed);
    println!("Clone Pairs Found:  {}", report.clone_pairs.len());

    if report.clone_pairs.is_empty() {
        println!("\nNO STRUCTURAL CLONES FOUND: Control-flow fingerprints are cleanly decoupled.");
        return;
    }

    println!("\n{:<6} {:<34} {:<34}", "SIM", "FUNCTION A", "FUNCTION B");
    println!("{}", "-".repeat(80));

    for pair in &report.clone_pairs {
        let f1 = format!(
            "{}:{} ({})",
            pair.first.file_path, pair.first.line_span.0, pair.first.name
        );
        let f2 = format!(
            "{}:{} ({})",
            pair.second.file_path, pair.second.line_span.0, pair.second.name
        );
        println!("{:<5.1}% {:<34} {:<34}", pair.similarity * 100.0, f1, f2);
    }
}

fn render_json(report: &CloneDetectionReport) {
    let pairs_json = report.clone_pairs.iter().map(|p| {
        Json::object([
            ("similarity", Json::string(format!("{:.4}", p.similarity))),
            (
                "hamming_distance",
                Json::Unsigned(p.hamming_distance as u64),
            ),
            (
                "first",
                Json::object([
                    ("file", Json::string(&p.first.file_path)),
                    ("name", Json::string(&p.first.name)),
                    ("start_line", Json::Unsigned(p.first.line_span.0 as u64)),
                    ("end_line", Json::Unsigned(p.first.line_span.1 as u64)),
                    ("lines", Json::Unsigned(p.first.line_count as u64)),
                ]),
            ),
            (
                "second",
                Json::object([
                    ("file", Json::string(&p.second.file_path)),
                    ("name", Json::string(&p.second.name)),
                    ("start_line", Json::Unsigned(p.second.line_span.0 as u64)),
                    ("end_line", Json::Unsigned(p.second.line_span.1 as u64)),
                    ("lines", Json::Unsigned(p.second.line_count as u64)),
                ]),
            ),
        ])
    });

    let doc = Json::object([
        (
            "functions_analyzed",
            Json::Unsigned(report.functions_analyzed as u64),
        ),
        (
            "clone_count",
            Json::Unsigned(report.clone_pairs.len() as u64),
        ),
        ("clones", Json::array(pairs_json)),
    ]);

    println!("{}", doc.render());
}
