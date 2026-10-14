//! SQLite DDL declarative symbol extractor.
//!
//! Parses `.sql` schema files and migration scripts with pure-Rust `sqlparser`.
//! Extracts compact declaration contracts (~60–120 tokens) for tables, views, and indexes:
//!
//! ```text
//! TABLE <name> (<col1>: <TYPE>, <col2>: <TYPE>, FK -> <ref_table>(<ref_col>))
//! INDEX <name> ON <table>(<col1>, <col2>)
//! VIEW <name>
//! ```

use sqlparser::ast::{ColumnDef, ColumnOption, Statement, TableConstraint};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;
use std::path::Path;

use super::{Language, SymbolKind, UniversalSymbol, common};

/// Extracts declarative schema symbols from SQLite SQL source.
pub fn extract_sql_symbols(path: &Path, content: &str) -> Vec<UniversalSymbol> {
    let dialect = SQLiteDialect {};
    let statements = match Parser::parse_sql(&dialect, content) {
        Ok(stmts) => stmts,
        Err(_) => return Vec::new(),
    };

    let module = common::module_path(path);
    let mut symbols = Vec::new();

    for statement in statements {
        match statement {
            Statement::CreateTable(create_table) => {
                let table_name = create_table.name.to_string();
                let line = 1; // sqlparser statement doesn't always track 1-based start line
                let signature = format_table_contract(
                    &table_name,
                    &create_table.columns,
                    &create_table.constraints,
                );
                symbols.push(UniversalSymbol {
                    path: common::dotted_path(&module, &[], &table_name),
                    name: table_name,
                    language: Language::Rust,
                    kind: SymbolKind::Table,
                    depth: 2, // level 2: scope / schema entity
                    docstring: None,
                    signature,
                    span: (0, content.len()),
                    file: path.to_path_buf(),
                    line,
                    params: Vec::new(),
                    return_type: None,
                    exported: true,
                });
            }
            Statement::CreateIndex(create_index) => {
                let index_name = create_index
                    .name
                    .as_ref()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "unnamed_idx".to_string());
                let table_name = create_index.table_name.to_string();
                let cols: Vec<String> = create_index
                    .columns
                    .iter()
                    .map(|c| c.expr.to_string())
                    .collect();
                let signature = format!(
                    "INDEX {} ON {}({})",
                    index_name,
                    table_name,
                    cols.join(", ")
                );
                symbols.push(UniversalSymbol {
                    path: common::dotted_path(&module, &[table_name], &index_name),
                    name: index_name,
                    language: Language::Rust,
                    kind: SymbolKind::Table,
                    depth: 3, // member of table
                    docstring: None,
                    signature,
                    span: (0, content.len()),
                    file: path.to_path_buf(),
                    line: 1,
                    params: Vec::new(),
                    return_type: None,
                    exported: true,
                });
            }
            Statement::CreateView { name, .. } => {
                let view_name = name.to_string();
                let signature = format!("VIEW {view_name}");
                symbols.push(UniversalSymbol {
                    path: common::dotted_path(&module, &[], &view_name),
                    name: view_name,
                    language: Language::Rust,
                    kind: SymbolKind::Table,
                    depth: 2,
                    docstring: None,
                    signature,
                    span: (0, content.len()),
                    file: path.to_path_buf(),
                    line: 1,
                    params: Vec::new(),
                    return_type: None,
                    exported: true,
                });
            }
            _ => {}
        }
    }

    symbols
}

/// Formats a compact table declaration contract:
/// `TABLE <name> (<col1>: <TYPE>, <col2>: <TYPE>, FK -> <ref_table>(<ref_col>))`
fn format_table_contract(
    table_name: &str,
    columns: &[ColumnDef],
    constraints: &[TableConstraint],
) -> String {
    let mut parts: Vec<String> = Vec::new();

    for col in columns {
        let mut col_str = format!("{}: {}", col.name.value, col.data_type);
        for opt in &col.options {
            if let ColumnOption::ForeignKey {
                foreign_table,
                referred_columns,
                ..
            } = &opt.option
            {
                let ref_cols: Vec<String> =
                    referred_columns.iter().map(|c| c.value.clone()).collect();
                col_str.push_str(&format!(
                    " FK -> {}({})",
                    foreign_table,
                    ref_cols.join(", ")
                ));
            }
        }
        parts.push(col_str);
    }

    for constraint in constraints {
        if let TableConstraint::ForeignKey {
            foreign_table,
            referred_columns,
            columns,
            ..
        } = constraint
        {
            let src_cols: Vec<String> = columns.iter().map(|c| c.value.clone()).collect();
            let ref_cols: Vec<String> = referred_columns.iter().map(|c| c.value.clone()).collect();
            parts.push(format!(
                "FK ({}) -> {}({})",
                src_cols.join(", "),
                foreign_table,
                ref_cols.join(", ")
            ));
        }
    }

    format!("TABLE {} ({})", table_name, parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_ddl_extraction() {
        let sql = r#"
            CREATE TABLE residents (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                building_id INTEGER,
                FOREIGN KEY (building_id) REFERENCES buildings(id)
            );
            CREATE INDEX idx_residents_building ON residents(building_id);
        "#;
        let symbols = extract_sql_symbols(Path::new("migrations/001_residents.sql"), sql);
        assert_eq!(symbols.len(), 2);

        let table = &symbols[0];
        assert_eq!(table.name, "residents");
        assert_eq!(table.kind, SymbolKind::Table);
        assert_eq!(table.depth, 2);
        assert!(table.signature.starts_with("TABLE residents ("));
        assert!(table.signature.contains("id: INT"));
        assert!(table.signature.contains("name: TEXT"));
        assert!(table.signature.contains("building_id: INT"));
        assert!(
            table
                .signature
                .contains("FK (building_id) -> buildings(id)")
        );

        let index = &symbols[1];
        assert_eq!(index.name, "idx_residents_building");
        assert_eq!(index.kind, SymbolKind::Table);
        assert_eq!(index.depth, 3);
        assert_eq!(
            index.signature,
            "INDEX idx_residents_building ON residents(building_id)"
        );
    }
}
