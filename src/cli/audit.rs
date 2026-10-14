//! `miter audit`: Scans workspace source files for line-count threshold violations.

use super::error::CliError;
use super::json::Json;
use miter_engine::analysis::{AuditConfig, AuditSeverity, AuditViolation, WorkspaceAuditor};
use std::path::{Path, PathBuf};

/// Format choice for `miter audit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditFormat {
    /// Human-readable aligned table.
    Table,
    /// Raw JSON document.
    Json,
}

impl AuditFormat {
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

/// Runs the `miter audit` command.
pub fn run_audit(
    path: Option<PathBuf>,
    explicit_threshold: Option<usize>,
    format: AuditFormat,
) -> Result<(), CliError> {
    let root = path.unwrap_or_else(|| PathBuf::from("."));
    let config = AuditConfig::from_workspace_with_threshold(&root, explicit_threshold);
    let threshold = config.max_lines;
    let auditor = WorkspaceAuditor::with_config(config);

    let violations = auditor.audit_root(&root).map_err(CliError::Io)?;

    match format {
        AuditFormat::Json => {
            println!("{}", audit_json(&violations, threshold, &root).render());
        }
        AuditFormat::Table => {
            render_audit_table(&violations, threshold, &root);
        }
    }

    Ok(())
}

fn audit_json(violations: &[AuditViolation], threshold: usize, root: &Path) -> Json {
    let items = violations.iter().map(|v| {
        Json::object([
            ("path", Json::string(v.path.display().to_string())),
            ("lines", Json::Unsigned(v.line_count as u64)),
            ("declarations", Json::Unsigned(v.declaration_count as u64)),
            ("severity", Json::string(v.severity.label())),
        ])
    });

    Json::object([
        ("root", Json::string(root.display().to_string())),
        ("threshold", Json::Unsigned(threshold as u64)),
        ("violation_count", Json::Unsigned(violations.len() as u64)),
        ("violations", Json::array(items)),
    ])
}

fn render_audit_table(violations: &[AuditViolation], threshold: usize, root: &Path) {
    if violations.is_empty() {
        println!(
            "AUDIT PASSED: No files exceed {} lines in {}.",
            threshold,
            root.display()
        );
        return;
    }

    println!(
        "AUDIT: Found {} file(s) exceeding threshold ({} lines) in {}:\n",
        violations.len(),
        threshold,
        root.display()
    );

    println!(
        "{:<10} {:>7}  {:>13}  FILE PATH",
        "SEVERITY", "LINES", "DECLARATIONS"
    );
    println!("{:-<10} {:-<7}  {:-<13}  {:-<40}", "", "", "", "");

    for v in violations {
        let sev_tag = match v.severity {
            AuditSeverity::Critical => "! CRITICAL",
            AuditSeverity::Warning => "  WARNING ",
        };
        println!(
            "{:<10} {:>7}  {:>13}  {}",
            sev_tag,
            v.line_count,
            v.declaration_count,
            v.path.display()
        );
    }
}
