//! Tests for reverse-lookup references table operations.

use std::collections::HashSet;

use miter_storage::WorkspaceStore;
use miter_storage::schema::{REFERENCES_TABLE_NAME, ReferenceEntry, ReferenceKind};

use crate::support::TempDir;

#[test]
fn test_references_table_name_and_operations() {
    assert_eq!(REFERENCES_TABLE_NAME, "references_v1");

    let dir = TempDir::new("refs_ops");
    let path = dir.path.join("index.redb");
    let store = WorkspaceStore::create(&path).expect("create redb");

    // Initially, looking up an unknown symbol returns an empty list
    let initial_refs = store
        .get_references_for_symbol("authenticate")
        .expect("query references");
    assert!(initial_refs.is_empty());

    // Insert references across two files
    let file1 = "crates/auth/src/service.rs";
    let refs1 = vec![
        (
            "authenticate".to_string(),
            ReferenceEntry {
                file_path: file1.to_string(),
                byte_range: (100, 112),
                line_range: (10, 10),
                enclosing_symbol: Some("login_handler".to_string()),
                reference_kind: ReferenceKind::Call,
            },
        ),
        (
            "User".to_string(),
            ReferenceEntry {
                file_path: file1.to_string(),
                byte_range: (150, 154),
                line_range: (12, 12),
                enclosing_symbol: Some("login_handler".to_string()),
                reference_kind: ReferenceKind::TypeUsage,
            },
        ),
    ];

    let file2 = "crates/auth/src/api.rs";
    let refs2 = vec![
        (
            "authenticate".to_string(),
            ReferenceEntry {
                file_path: file2.to_string(),
                byte_range: (200, 212),
                line_range: (18, 18),
                enclosing_symbol: Some("route_post".to_string()),
                reference_kind: ReferenceKind::Call,
            },
        ),
        (
            "WorkerService".to_string(),
            ReferenceEntry {
                file_path: file2.to_string(),
                byte_range: (220, 233),
                line_range: (22, 22),
                enclosing_symbol: None,
                reference_kind: ReferenceKind::TraitImpl,
            },
        ),
    ];

    store.insert_references(file1, refs1).expect("insert file1");
    store.insert_references(file2, refs2).expect("insert file2");

    // Query references for "authenticate" (should find 2 across both files)
    let auth_refs = store
        .get_references_for_symbol("authenticate")
        .expect("query authenticate");
    assert_eq!(auth_refs.len(), 2);
    let files: HashSet<&str> = auth_refs.iter().map(|r| r.file_path.as_str()).collect();
    assert!(files.contains(file1));
    assert!(files.contains(file2));

    // Verify listed symbols
    let symbols = store.list_referenced_symbols().expect("list symbols");
    assert!(symbols.contains("authenticate"));
    assert!(symbols.contains("User"));
    assert!(symbols.contains("WorkerService"));

    // Invalidate references for file1
    let removed = store
        .clear_references_for_file(file1)
        .expect("clear file1 references");
    assert_eq!(removed, 2);

    // Now "authenticate" should only have 1 reference from file2
    let auth_refs_after = store
        .get_references_for_symbol("authenticate")
        .expect("query authenticate after clear");
    assert_eq!(auth_refs_after.len(), 1);
    assert_eq!(auth_refs_after[0].file_path, file2);

    // "User" had only file1 references, so its key should now be removed
    let user_refs_after = store
        .get_references_for_symbol("User")
        .expect("query User after clear");
    assert!(user_refs_after.is_empty());

    let symbols_after = store.list_referenced_symbols().expect("list symbols after");
    assert!(!symbols_after.contains("User"));
    assert!(symbols_after.contains("authenticate"));
}
