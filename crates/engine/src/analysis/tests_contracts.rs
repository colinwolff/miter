//! Unit tests for Polyglot Contract Bridge.

use super::contracts::{
    canonical_entity_name, normalize_identifier, verify_contracts_from_symbols,
};
use crate::polyglot::{Language, SymbolKind, UniversalSymbol};
use std::path::{Path, PathBuf};

#[test]
fn test_identifier_normalization() {
    assert_eq!(normalize_identifier("resident_ledger"), "resident_ledger");
    assert_eq!(normalize_identifier("ResidentLedger"), "resident_ledger");
    assert_eq!(normalize_identifier("residentLedger"), "resident_ledger");
    assert_eq!(normalize_identifier("building-id"), "building_id");
    assert_eq!(canonical_entity_name("residents"), "resident");
    assert_eq!(canonical_entity_name("Resident"), "resident");
    assert_eq!(canonical_entity_name("ResidentLedger"), "resident_ledger");
}

#[test]
fn test_contract_drift_detection_across_sql_rust_and_ts() {
    // 1. SQL Table with columns: id, name, building_id
    let sql_table = UniversalSymbol {
        name: "residents".to_string(),
        language: Language::Rust,
        kind: SymbolKind::Table,
        path: "migrations::residents".to_string(),
        depth: 2,
        docstring: None,
        signature:
            "TABLE residents (id: INTEGER PRIMARY KEY, name: TEXT NOT NULL, building_id: INTEGER)"
                .to_string(),
        span: (0, 100),
        file: PathBuf::from("migrations/001_residents.sql"),
        line: 1,
        params: Vec::new(),
        return_type: None,
        exported: true,
    };

    // 2. Rust struct with fields: id, name, building_id
    let rust_struct = UniversalSymbol {
        name: "Resident".to_string(),
        language: Language::Rust,
        kind: SymbolKind::Struct,
        path: "crate::models::Resident".to_string(),
        depth: 2,
        docstring: None,
        signature: "pub struct Resident".to_string(),
        span: (0, 80),
        file: PathBuf::from("src/models.rs"),
        line: 10,
        params: vec![
            miter_ast::Param {
                name: "id".to_string(),
                type_name: "i64".to_string(),
            },
            miter_ast::Param {
                name: "name".to_string(),
                type_name: "String".to_string(),
            },
            miter_ast::Param {
                name: "building_id".to_string(),
                type_name: "Option<i64>".to_string(),
            },
        ],
        return_type: None,
        exported: true,
    };

    // 3. TypeScript interface missing building_id (contract drift!)
    let ts_interface = UniversalSymbol {
        name: "Resident".to_string(),
        language: Language::TypeScript,
        kind: SymbolKind::Interface,
        path: "types::Resident".to_string(),
        depth: 2,
        docstring: None,
        signature: "export interface Resident".to_string(),
        span: (0, 60),
        file: PathBuf::from("types/resident.ts"),
        line: 5,
        params: vec![
            miter_ast::Param {
                name: "id".to_string(),
                type_name: "number".to_string(),
            },
            miter_ast::Param {
                name: "name".to_string(),
                type_name: "string".to_string(),
            },
        ],
        return_type: None,
        exported: true,
    };

    let symbols = vec![sql_table, rust_struct, ts_interface];
    let report = verify_contracts_from_symbols(&symbols);

    assert_eq!(report.verified_entities, 1, "expected 1 verified entity");
    assert_eq!(
        report.drifts.len(),
        1,
        "expected 1 entity with contract drift"
    );

    let drift = &report.drifts[0];
    assert_eq!(drift.entity, "resident");
    assert_eq!(drift.missing_fields.len(), 1);

    let missing = &drift.missing_fields[0];
    assert_eq!(missing.field_name, "building_id");
    assert!(missing.present_in.contains(&"sql".to_string()));
    assert!(missing.present_in.contains(&"rust".to_string()));
    assert!(missing.missing_in.contains(&"typescript".to_string()));
}

#[test]
fn a_real_sql_schema_verifies_against_a_real_rust_struct() {
    // The extractors run for real: the SQL goes through `sqlparser` and the struct through
    // tree-sitter, so the contract bridge is exercised end to end rather than on hand-built rows.
    let sql = "CREATE TABLE residents (\n  id INTEGER PRIMARY KEY,\n  name TEXT NOT NULL,\n  building_id INTEGER\n);\n";
    let mut symbols =
        crate::polyglot::sql::extract_sql_symbols(Path::new("migrations/001.sql"), sql);
    let rust = "pub struct Resident {\n    pub id: i64,\n    pub name: String,\n    pub building_id: i64,\n}\n";
    symbols.extend(
        crate::polyglot::parse(Path::new("src/models.rs"), rust, Language::Rust).expect("parses"),
    );

    let report = verify_contracts_from_symbols(&symbols);
    assert_eq!(report.verified_entities, 1, "{report:?}");
    assert!(report.drifts.is_empty(), "{:?}", report.drifts);
}

#[test]
fn a_column_missing_from_the_struct_is_reported() {
    let sql = "CREATE TABLE residents (\n  id INTEGER PRIMARY KEY,\n  name TEXT NOT NULL,\n  building_id INTEGER\n);\n";
    let mut symbols =
        crate::polyglot::sql::extract_sql_symbols(Path::new("migrations/001.sql"), sql);
    let rust = "pub struct Resident {\n    pub id: i64,\n    pub name: String,\n}\n";
    symbols.extend(
        crate::polyglot::parse(Path::new("src/models.rs"), rust, Language::Rust).expect("parses"),
    );

    let report = verify_contracts_from_symbols(&symbols);
    assert_eq!(report.drifts.len(), 1, "{report:?}");
    let drift = &report.drifts[0];
    assert_eq!(drift.entity, "resident");
    assert!(
        drift
            .missing_fields
            .iter()
            .any(|missing| missing.field_name == "building_id"),
        "{drift:?}"
    );
}
