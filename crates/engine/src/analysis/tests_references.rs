//! Unit tests for AST identifier reference extraction.

use std::path::Path;

use miter_storage::schema::ReferenceKind;

use super::references::extract_references;

#[test]
fn test_rust_references_extracts_storage_engine_and_call_sites() {
    let source = r#"
use miter_storage::StorageEngine;

fn init_storage() {
    let engine = StorageEngine::open("test.db");
    let res: Option<StorageEngine> = None;
}
"#;

    let refs = extract_references(Path::new("src/main.rs"), source);
    let storage_engine_refs: Vec<_> = refs
        .iter()
        .filter(|(sym, _)| sym == "StorageEngine")
        .collect();

    assert!(
        !storage_engine_refs.is_empty(),
        "StorageEngine must be extracted as a reference, found: {:?}",
        refs
    );

    // 1. Check import from `use miter_storage::StorageEngine;`
    let has_import = storage_engine_refs
        .iter()
        .any(|(_, entry)| entry.reference_kind == ReferenceKind::Import);
    assert!(has_import, "expected Import reference for StorageEngine");

    // 2. Check TypeUsage/Call from `StorageEngine::open`
    let has_usage = storage_engine_refs.iter().any(|(_, entry)| {
        entry.reference_kind == ReferenceKind::TypeUsage
            && entry.enclosing_symbol.as_deref() == Some("init_storage")
    });
    assert!(
        has_usage,
        "expected TypeUsage reference for StorageEngine in init_storage"
    );

    // 3. Verify open call site is also extracted
    let has_open_call = refs.iter().any(|(sym, entry)| {
        sym == "open"
            && entry.reference_kind == ReferenceKind::Call
            && entry.enclosing_symbol.as_deref() == Some("init_storage")
    });
    assert!(
        has_open_call,
        "expected Call reference for open in init_storage"
    );
}

#[test]
fn test_compute_impact_three_hop_chain_selects_affected_test() {
    let unique = format!(
        "miter_impact_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let db_path = std::env::temp_dir().join(format!("{unique}.redb"));
    let store = miter_storage::store::WorkspaceStore::create(&db_path).expect("create test store");

    // helper -> service -> test_service_flow
    let refs = vec![
        (
            "helper".to_string(),
            miter_storage::schema::ReferenceEntry {
                file_path: "src/service.rs".to_string(),
                byte_range: (10, 25),
                line_range: (5, 5),
                enclosing_symbol: Some("service".to_string()),
                reference_kind: ReferenceKind::Call,
            },
        ),
        (
            "service".to_string(),
            miter_storage::schema::ReferenceEntry {
                file_path: "tests/service_test.rs".to_string(),
                byte_range: (50, 70),
                line_range: (12, 12),
                enclosing_symbol: Some("test_service_flow".to_string()),
                reference_kind: ReferenceKind::Call,
            },
        ),
    ];

    store.insert_references_batch(refs).expect("insert refs");

    let impact = super::impact::compute_impact(&store, "helper", 4).expect("compute impact");
    assert_eq!(impact.target_symbol, "helper");

    let has_affected_test = impact.affected_tests.iter().any(|t| {
        t.symbol == "test_service_flow" && t.test_command == "cargo test test_service_flow"
    });
    assert!(
        has_affected_test,
        "expected test_service_flow in affected_tests: {:?}",
        impact.affected_tests
    );

    let has_transitive_caller = impact
        .transitive_callers
        .iter()
        .any(|c| c.symbol == "service" && c.depth == 1);
    assert!(
        has_transitive_caller,
        "expected service at depth 1 in transitive_callers"
    );

    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_tarjan_cycle_detector_identifies_three_file_cycle() {
    let mut graph = super::cycles::FileDependencyGraph::new();
    // A -> B -> C -> A
    graph.add_edge("src/a.rs", "src/b.rs");
    graph.add_edge("src/b.rs", "src/c.rs");
    graph.add_edge("src/c.rs", "src/a.rs");
    // D is independent
    graph.add_edge("src/d.rs", "src/a.rs");

    let report = graph.find_cycles();
    assert!(report.has_circular_risk, "expected circular risk");
    assert_eq!(report.cycles.len(), 1, "expected 1 cycle cluster");
    assert_eq!(report.cycles[0].size, 3);
    assert_eq!(
        report.cycles[0].files,
        vec![
            "src/a.rs".to_string(),
            "src/b.rs".to_string(),
            "src/c.rs".to_string()
        ]
    );
}

#[test]
fn test_hygiene_visibility_downgrade_and_dead_symbol() {
    let unique = format!(
        "miter_hygiene_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let db_path = std::env::temp_dir().join(format!("{unique}.redb"));
    let store = miter_storage::store::WorkspaceStore::create(&db_path).expect("create test store");

    // Persist universal symbols:
    // 1. helper_func: pub fn, only referenced internally in src/internal.rs -> candidate for downgrade
    // 2. dead_func: private fn (depth 2), 0 references -> dead symbol
    let symbols = vec![
        crate::polyglot::UniversalSymbol {
            name: "helper_func".to_string(),
            language: crate::polyglot::Language::Rust,
            kind: crate::polyglot::SymbolKind::Function,
            path: "crate::helper_func".to_string(),
            depth: 2,
            docstring: None,
            signature: "pub fn helper_func()".to_string(),
            span: (0, 30),
            file: std::path::PathBuf::from("src/internal.rs"),
            line: 10,
            params: Vec::new(),
            return_type: None,
            exported: true,
        },
        crate::polyglot::UniversalSymbol {
            name: "dead_func".to_string(),
            language: crate::polyglot::Language::Rust,
            kind: crate::polyglot::SymbolKind::Function,
            path: "crate::dead_func".to_string(),
            depth: 2,
            docstring: None,
            signature: "fn dead_func()".to_string(),
            span: (0, 25),
            file: std::path::PathBuf::from("src/unused.rs"),
            line: 20,
            params: Vec::new(),
            return_type: None,
            exported: false,
        },
    ];

    {
        let tx = store.database().begin_write().expect("begin write");
        {
            let mut table = tx
                .open_table(miter_storage::tables::TABLE_UNIVERSAL)
                .expect("open table");
            for sym in &symbols {
                let rec = crate::codec::encode_universal(sym);
                table.insert(sym.id(), rec.as_slice()).expect("insert sym");
            }
        }
        tx.commit().expect("commit");
    }

    // Add reference to helper_func ONLY inside src/internal.rs
    let refs = vec![(
        "helper_func".to_string(),
        miter_storage::schema::ReferenceEntry {
            file_path: "src/internal.rs".to_string(),
            byte_range: (100, 115),
            line_range: (15, 15),
            enclosing_symbol: Some("caller".to_string()),
            reference_kind: ReferenceKind::Call,
        },
    )];
    store.insert_references_batch(refs).expect("insert refs");

    let report = super::hygiene::analyze_hygiene(&store).expect("analyze hygiene");

    let has_dead = report.dead_symbols.iter().any(|d| d.name == "dead_func");
    assert!(
        has_dead,
        "expected dead_func in dead_symbols: {:?}",
        report.dead_symbols
    );

    let has_downgrade = report
        .visibility_downgrades
        .iter()
        .any(|v| v.name == "helper_func" && v.local_references_count == 1);
    assert!(
        has_downgrade,
        "expected helper_func in visibility_downgrades: {:?}",
        report.visibility_downgrades
    );

    let _ = std::fs::remove_file(&db_path);
}
