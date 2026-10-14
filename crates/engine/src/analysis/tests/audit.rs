//! The line-count auditor, against a real temporary tree.

use std::fs;

use crate::analysis::audit::{AuditConfig, AuditSeverity, WorkspaceAuditor};

#[test]
fn test_workspace_auditor_filtering() {
    let temp = std::env::temp_dir().join(format!("test_audit_{}", std::process::id()));
    fs::create_dir_all(&temp).unwrap();

    let small_file = temp.join("small.rs");
    fs::write(&small_file, "fn small() {}\n").unwrap();

    let large_file = temp.join("large.rs");
    let mut large_content = String::new();
    for i in 0..300 {
        large_content.push_str(&format!("// line {i}\nfn item_{i}() {{}}\n"));
    }
    fs::write(&large_file, &large_content).unwrap();

    let critical_file = temp.join("critical.rs");
    let mut critical_content = String::new();
    for i in 0..450 {
        critical_content.push_str(&format!("// line {i}\nfn crit_{i}() {{}}\n"));
    }
    fs::write(&critical_file, &critical_content).unwrap();

    let auditor = WorkspaceAuditor::with_config(AuditConfig::new(500));
    let violations = auditor.audit_root(&temp).expect("audit succeeds");

    assert_eq!(
        violations.len(),
        2,
        "must find large and critical files only"
    );
    assert_eq!(violations[0].path, critical_file);
    assert_eq!(violations[0].severity, AuditSeverity::Critical);
    assert!(violations[0].line_count >= 850);

    assert_eq!(violations[1].path, large_file);
    assert_eq!(violations[1].severity, AuditSeverity::Warning);
    assert!(violations[1].line_count >= 600);

    let _ = fs::remove_dir_all(&temp);
}
