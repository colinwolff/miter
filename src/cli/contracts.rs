//! `miter contracts`: Cross-language polyglot schema contract verification.

use super::error::CliError;
use super::json::Json;
use miter_engine::{ContractReport, StorageError, WorkspaceStore};
use std::path::Path;

/// Format choice for `miter contracts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractsFormat {
    /// Human-readable aligned summary table.
    Table,
    /// Machine-readable JSON output.
    Json,
}

impl ContractsFormat {
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

/// Runs the `miter contracts` command.
pub fn run_contracts(db: &Path, format: ContractsFormat) -> Result<(), CliError> {
    if !db.exists() {
        return Err(CliError::MissingIndex(db.to_path_buf()));
    }

    let store = WorkspaceStore::open(db)
        .map_err(|e: StorageError| CliError::Usage(format!("failed to open database: {e}")))?;
    let symbols = miter_engine::load_redb_universal_from(&store)
        .map_err(|e| CliError::Usage(format!("failed to load universal symbols: {e}")))?;
    let report = miter_engine::verify_contracts_from_symbols(&symbols);

    match format {
        ContractsFormat::Table => render_table(&report),
        ContractsFormat::Json => render_json(&report),
    }

    Ok(())
}

fn render_table(report: &ContractReport) {
    println!("Polyglot Contract Verification across SQL, Rust, and TypeScript");
    println!("{}", "=".repeat(80));
    println!(
        "Verified Multi-Language Entities: {}",
        report.verified_entities
    );
    println!("Entities with Contract Drift:     {}", report.drifts.len());

    if report.drifts.is_empty() {
        println!("\nALL CONTRACTS CONSISTENT: No cross-language schema drifts detected.");
        return;
    }

    println!("\n{:<24} {:<24} {:<30}", "ENTITY", "FIELD", "DRIFT STATUS");
    println!("{}", "-".repeat(80));

    for drift in &report.drifts {
        for missing in &drift.missing_fields {
            let status = format!(
                "in [{}] but missing in [{}]",
                missing.present_in.join(", "),
                missing.missing_in.join(", ")
            );
            println!(
                "{:<24} {:<24} {:<30}",
                drift.entity, missing.field_name, status
            );
        }
    }
}

fn render_json(report: &ContractReport) {
    let drifts_json = report.drifts.iter().map(|d| {
        let missing_json = d.missing_fields.iter().map(|m| {
            let pres = m.present_in.iter().map(|s| Json::string(s.clone()));
            let miss = m.missing_in.iter().map(|s| Json::string(s.clone()));
            Json::object([
                ("field", Json::string(&m.field_name)),
                ("present_in", Json::array(pres)),
                ("missing_in", Json::array(miss)),
            ])
        });
        let srcs = d.sources.iter().map(|s| Json::string(s.clone()));
        Json::object([
            ("entity", Json::string(&d.entity)),
            ("sources", Json::array(srcs)),
            ("missing_fields", Json::array(missing_json)),
        ])
    });

    let doc = Json::object([
        (
            "verified_entities",
            Json::Unsigned(report.verified_entities as u64),
        ),
        ("drift_count", Json::Unsigned(report.drifts.len() as u64)),
        ("drifts", Json::array(drifts_json)),
    ]);

    println!("{}", doc.render());
}
