//! What a Rust source file yields: names, parameters, return types and provenance.

use std::path::Path;

use miter_ast::{AstIndexer, Language};

use super::*;

#[test]
fn extracts_names_parameters_and_return_types() {
    let nodes = extract(SAMPLE);
    assert!(
        names(&nodes).contains(&"scale".to_string()),
        "{:?}",
        names(&nodes)
    );

    let scale = by_name(&nodes, "scale");
    assert_eq!(
        scale.params,
        vec![
            Param {
                name: "x".into(),
                type_name: "f32".into()
            },
            Param {
                name: "factor".into(),
                type_name: "f32".into()
            },
        ]
    );
    assert_eq!(scale.return_type.as_deref(), Some("f32"));
    assert_eq!(scale.scope_depth, 0, "free function at file scope");
    assert_eq!(scale.line, 2, "declaration line is one-based");
    assert!(scale.end_byte > scale.start_byte);
}

#[test]
fn tracks_scope_depth_through_impl_trait_mod_and_nesting() {
    let nodes = extract(SAMPLE);
    // `impl Widget { fn new }` and `trait Draw { fn draw }` are one level deep.
    assert_eq!(by_name(&nodes, "new").scope_depth, 1);
    assert_eq!(by_name(&nodes, "area").scope_depth, 1);
    assert_eq!(by_name(&nodes, "consume").scope_depth, 1);
    assert_eq!(by_name(&nodes, "draw").scope_depth, 1, "trait declaration");
    // `mod inner { fn nested }` is one level; `deeper` is nested inside `nested`.
    assert_eq!(by_name(&nodes, "nested").scope_depth, 1);
    assert_eq!(by_name(&nodes, "deeper").scope_depth, 2);
}

#[test]
fn receivers_and_ancillary_parameters_are_labelled() {
    let nodes = extract(SAMPLE);
    let area = by_name(&nodes, "area");
    assert_eq!(
        area.params.len(),
        2,
        "receiver plus one argument: {:?}",
        area.params
    );
    assert!(
        area.params[0].name.contains("self"),
        "receiver name should mention self, got {:?}",
        area.params[0]
    );
    assert_eq!(area.params[0].type_name, "Self");
    assert_eq!(area.params[1].type_name, "f32");

    let consume = by_name(&nodes, "consume");
    assert_eq!(consume.params.len(), 1);
    assert_eq!(consume.params[0].type_name, "Self");

    let new = by_name(&nodes, "new");
    assert_eq!(new.return_type.as_deref(), Some("Self"));
    assert_eq!(by_name(&nodes, "draw").return_type, None);
}

#[test]
fn signatures_are_flattened_onto_one_line() {
    let nodes = extract(
        r#"
fn complex<'a, T: Clone>(
    values: &'a [T],
    mapper: impl Fn(&T) -> T,
) -> Result<Vec<T>, std::io::Error>
where
    T: Send,
{
    unimplemented!()
}
"#,
    );
    let node = by_name(&nodes, "complex");
    assert_eq!(
        node.params,
        vec![
            Param {
                name: "values".into(),
                type_name: "&'a [T]".into()
            },
            Param {
                name: "mapper".into(),
                type_name: "impl Fn(&T) -> T".into()
            },
        ]
    );
    assert_eq!(
        node.return_type.as_deref(),
        Some("Result<Vec<T>, std::io::Error>")
    );
}

#[test]
fn closures_are_not_indexed_as_named_functions() {
    let nodes = extract("fn outer() { let f = |x: i32| x + 1; let _ = f(1); }");
    assert_eq!(names(&nodes), vec!["outer".to_string()]);
}

#[test]
fn broken_source_still_yields_the_functions_it_can() {
    // tree-sitter is error tolerant: the well-formed function is still found.
    let nodes = extract("fn good() -> u8 { 1 }\nfn broken( {");
    assert!(names(&nodes).contains(&"good".to_string()));
}

#[test]
fn symbols_carry_geometry_and_provenance() {
    let indexer = AstIndexer::new();
    let symbols = indexer
        .index_source(
            "fn hash_password(secret: &str) -> Vec<u8> { unimplemented!() }",
            Language::Rust,
            Path::new("src/auth.rs"),
            1,
        )
        .expect("parses");

    let symbol = &symbols[0];
    assert_eq!(symbol.name, "hash_password");
    assert_eq!(symbol.dir_depth, 1);
    assert_eq!(symbol.scope_depth, 0);
    assert_eq!(symbol.depth(), 1);
    assert_eq!(symbol.language, Language::Rust);
    assert_eq!(symbol.params.len(), 1);
    assert_eq!(symbol.signature(), "hash_password(secret: &str) -> Vec<u8>");
    assert_eq!(symbol.qualified_name(), "src/auth.rs::hash_password");
    assert!(symbol.search_text().contains("hash_password"));

    // Provenance: ids are deterministic, so re-ingestion is idempotent.
    let again = indexer
        .index_source(
            "fn hash_password(secret: &str) -> Vec<u8> { unimplemented!() }",
            Language::Rust,
            Path::new("src/auth.rs"),
            1,
        )
        .expect("parses");
    assert_eq!(again[0].id, symbol.id);
    assert_eq!(
        again[0], *symbol,
        "identical input yields identical symbols"
    );
}
