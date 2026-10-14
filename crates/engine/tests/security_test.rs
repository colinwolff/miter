//! Automated tests for AST-aware secret protection and egress sanitization.

use miter_engine::{
    Engine, MiterConfig, SkeletonOptions, extract_skeleton, hydrate_symbol_context,
    scrub_source_text,
};
use miter_storage::WorkspaceStore;
use std::fs;
use std::path::PathBuf;

struct TestWorkspace {
    root: PathBuf,
    db_path: PathBuf,
}

impl TestWorkspace {
    fn new(label: &str) -> Self {
        let unique = format!(
            "miter_sec_test_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let src = root.join("src");
        fs::create_dir_all(&src).expect("create workspace src");
        let db_path = root.join("test.redb");
        Self { root, db_path }
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn test_sensitive_identifier_preservation_and_redaction() {
    let ws = TestWorkspace::new("stripe_secret");
    let file_path = ws.root.join("src/config.rs");
    let secret_val = "sk_live_51Nz987654321abcdef";
    let code = format!("pub const STRIPE_SECRET_KEY: &str = \"{secret_val}\";\n");
    fs::write(&file_path, &code).expect("write rust file");

    let mut engine = Engine::new();
    let store = WorkspaceStore::open_or_recreate(&ws.db_path).expect("open store");
    engine
        .ingest_polyglot_tree_tracked(&ws.root, &store)
        .expect("ingest");
    miter_engine::persist::save_redb_into(&store, &engine).expect("save redb");

    // 1. Verify symbol name is preserved and hydrated context returns [REDACTED_SECRET]
    let context =
        hydrate_symbol_context(&engine, "STRIPE_SECRET_KEY", 256).expect("hydrate symbol");
    assert_eq!(context.name, "STRIPE_SECRET_KEY");
    assert!(context.signature.contains("[REDACTED_SECRET]"));
    assert!(!context.signature.contains(secret_val));
    assert!(context.text.contains("[REDACTED_SECRET]"));
    assert!(!context.text.contains(secret_val));

    // 2. Verify skeleton strips or redacts literal
    let skel = extract_skeleton(&file_path, &code, SkeletonOptions::default());
    assert!(skel.skeleton_text.contains("[REDACTED_SECRET]"));
    assert!(!skel.skeleton_text.contains(secret_val));

    // 3. Verify raw secret value does NOT appear in persisted redb bytes
    let redb_bytes = fs::read(&ws.db_path).expect("read redb");
    let found = redb_bytes
        .windows(secret_val.len())
        .any(|window| window == secret_val.as_bytes());
    assert!(!found, "raw secret found in redb database bytes!");
}

#[test]
fn test_generic_identifier_with_known_prefix_redaction() {
    let raw_gh_token = "ghp_1234567890abcdefghijklmnopqrstuvwxyz";
    let ts_code = format!("const default_header = \"{raw_gh_token}\";\n");
    let scrubbed = scrub_source_text(&ts_code);

    assert!(scrubbed.contains("const default_header = \"[REDACTED_SECRET]\";"));
    assert!(!scrubbed.contains(raw_gh_token));
}

#[test]
fn test_false_positive_guard_ast_types() {
    let rust_code =
        "pub struct SyntaxToken { pub kind: String }\nconst MAX_TOKENS: usize = 4096;\n";
    let scrubbed = scrub_source_text(rust_code);
    assert_eq!(scrubbed, rust_code);
}

#[test]
fn test_env_variable_name_discovery_and_redaction() {
    let ws = TestWorkspace::new("env_test");
    let secret_pass = "supersecret";
    let env_content =
        format!("DATABASE_URL=postgres://admin:{secret_pass}@localhost:5432/db\nPORT=3000\n");
    fs::write(ws.root.join(".env"), &env_content).expect("write .env");

    let mut engine = Engine::new();
    let store = WorkspaceStore::open_or_recreate(&ws.db_path).expect("open store");
    engine
        .ingest_polyglot_tree_tracked(&ws.root, &store)
        .expect("ingest");

    // Verify DATABASE_URL can be looked up by name
    let context =
        hydrate_symbol_context(&engine, "DATABASE_URL", 256).expect("hydrate DATABASE_URL");
    assert_eq!(context.name, "DATABASE_URL");
    assert!(context.signature.contains("[REDACTED_SECRET]"));
    assert!(!context.signature.contains(secret_pass));

    // Verify baseline security exclusions in MiterConfig
    let cfg = MiterConfig::default();
    let matcher = cfg.build_indexer_matcher().expect("matcher");
    assert!(miter_engine::is_path_excluded(
        &matcher,
        &PathBuf::from("server.key"),
        &ws.root
    ));
    assert!(miter_engine::is_path_excluded(
        &matcher,
        &PathBuf::from("certs/server.crt"),
        &ws.root
    ));
    assert!(miter_engine::is_path_excluded(
        &matcher,
        &PathBuf::from("credentials.json"),
        &ws.root
    ));
}
