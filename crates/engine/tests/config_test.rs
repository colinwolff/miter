//! Tests for `miter` workspace configuration, inheritance, precedence, and glob matching.

use miter_engine::config::{MiterConfig, is_path_excluded};
use std::fs;
use std::path::Path;
#[test]
fn test_config_precedence_dot_miter_over_root() {
    let unique = format!(
        "miter_cfg_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).expect("create temp root");

    // Root miter.toml
    fs::write(
        root.join("miter.toml"),
        r#"
exclude = ["from_root/**"]
"#,
    )
    .expect("write miter.toml");

    let cfg_root = MiterConfig::load_from_workspace(&root);
    assert_eq!(cfg_root.exclude, vec!["from_root/**"]);

    // Create .miter/config.toml
    let dot_miter = root.join(".miter");
    fs::create_dir_all(&dot_miter).expect("create .miter");
    fs::write(
        dot_miter.join("config.toml"),
        r#"
exclude = ["from_dot_miter/**"]
"#,
    )
    .expect("write .miter/config.toml");

    let cfg_precedence = MiterConfig::load_from_workspace(&root);
    assert_eq!(cfg_precedence.exclude, vec!["from_dot_miter/**"]);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_global_and_tool_specific_exclusion_inheritance() {
    let toml_str = r#"
exclude = ["target/**", "node_modules/**"]

[indexer]
exclude = ["vendor/**", "fixtures/**"]

[audit]
exclude = ["migrations/**", "benches/**"]
threshold = 300
"#;

    let config: MiterConfig = toml::from_str(toml_str).expect("deserialize config");
    assert_eq!(config.audit.threshold, Some(300));

    let indexer_matcher = config.build_indexer_matcher().expect("indexer matcher");
    let audit_matcher = config.build_audit_matcher().expect("audit matcher");

    let root = Path::new("/workspace");

    // Global exclude matches both indexer and audit
    let target_file = root.join("target/debug/build.rs");
    let node_file = root.join("node_modules/package/index.js");
    assert!(is_path_excluded(&indexer_matcher, &target_file, root));
    assert!(is_path_excluded(&audit_matcher, &target_file, root));
    assert!(is_path_excluded(&indexer_matcher, &node_file, root));
    assert!(is_path_excluded(&audit_matcher, &node_file, root));

    // Indexer specific exclude
    let vendor_file = root.join("vendor/lib.rs");
    assert!(is_path_excluded(&indexer_matcher, &vendor_file, root));
    assert!(!is_path_excluded(&audit_matcher, &vendor_file, root));

    // Audit specific exclude
    let mig_file = root.join("migrations/001_init.sql");
    assert!(!is_path_excluded(&indexer_matcher, &mig_file, root));
    assert!(is_path_excluded(&audit_matcher, &mig_file, root));
}

#[test]
fn test_relative_glob_path_matching_and_pruning() {
    let config: MiterConfig = toml::from_str(
        r#"
exclude = ["migrations/**", "docs/*", "build", "src/generated/**/*.rs"]
"#,
    )
    .expect("deserialize config");

    let matcher = config.build_indexer_matcher().expect("build matcher");
    let root = Path::new("/workspace/project");

    // Nested file under migrations/**
    let sql_file = root.join("migrations/001_init.sql");
    assert!(is_path_excluded(&matcher, &sql_file, root));

    // Deeply nested file under migrations/sub/002.sql
    let nested_sql = root.join("migrations/sub/002.sql");
    assert!(is_path_excluded(&matcher, &nested_sql, root));

    // Directory itself: migrations/
    let mig_dir = root.join("migrations");
    assert!(is_path_excluded(&matcher, &mig_dir, root));

    // docs/* matches direct child but not non-child
    let doc_file = root.join("docs/readme.md");
    assert!(is_path_excluded(&matcher, &doc_file, root));

    // src/generated/**/*.rs
    let gen_rs = root.join("src/generated/proto/message.rs");
    assert!(is_path_excluded(&matcher, &gen_rs, root));

    // Non-excluded regular source
    let normal_rs = root.join("src/main.rs");
    assert!(!is_path_excluded(&matcher, &normal_rs, root));
}

#[test]
fn test_audit_threshold_config_precedence() {
    let unique = format!(
        "miter_audit_thresh_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    let dot_miter = root.join(".miter");
    fs::create_dir_all(&dot_miter).expect("create .miter dir");

    fs::write(
        dot_miter.join("config.toml"),
        r#"
[audit]
threshold = 300
"#,
    )
    .expect("write config.toml");

    // Create a 350-line test source file
    let mut file_content = String::new();
    for i in 0..350 {
        file_content.push_str(&format!("// line {}\n", i));
    }
    fs::write(root.join("large.rs"), file_content).expect("write large.rs");

    let config = miter_engine::analysis::AuditConfig::from_workspace_with_threshold(&root, None);
    assert_eq!(config.max_lines, 300);

    let auditor = miter_engine::analysis::WorkspaceAuditor::with_config(config);
    let violations = auditor.audit_root(&root).expect("audit_root");
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].line_count, 350);

    let _ = fs::remove_dir_all(&root);
}
