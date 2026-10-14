//! `miter skeleton`: Extracts AST interface skeletons with implementation bodies stripped.

use std::fs;
use std::path::Path;

use miter_engine::analysis::{SkeletonOptions, SkeletonResult, extract_skeleton};

use super::error::CliError;
use super::json::Json;

/// Format choice for `miter skeleton`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkeletonFormat {
    /// Pure source text of stripped skeleton.
    Text,
    /// JSON format with metadata and compression statistics.
    Json,
}

impl SkeletonFormat {
    /// Parses format string (`text` or `json`).
    #[must_use]
    pub fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("json") {
            Self::Json
        } else {
            Self::Text
        }
    }
}

/// Runs the `miter skeleton` command.
pub fn run_skeleton(path: &Path, format: SkeletonFormat) -> Result<(), CliError> {
    let content = fs::read_to_string(path).map_err(CliError::Io)?;
    let options = SkeletonOptions::default();
    let result = extract_skeleton(path, &content, options);

    match format {
        SkeletonFormat::Text => {
            println!("{}", result.skeleton_text);
        }
        SkeletonFormat::Json => {
            println!("{}", skeleton_json(path, &result).render());
        }
    }

    Ok(())
}

fn skeleton_json(path: &Path, result: &SkeletonResult) -> Json {
    Json::object([
        ("file", Json::string(path.display().to_string())),
        (
            "original_lines",
            Json::Unsigned(result.original_lines as u64),
        ),
        (
            "skeleton_lines",
            Json::Unsigned(result.skeleton_lines as u64),
        ),
        (
            "original_chars",
            Json::Unsigned(result.original_chars as u64),
        ),
        (
            "skeleton_chars",
            Json::Unsigned(result.skeleton_chars as u64),
        ),
        (
            "compression_ratio",
            Json::fixed(result.compression_ratio(), 4),
        ),
        ("skeleton", Json::string(result.skeleton_text.clone())),
    ])
}
