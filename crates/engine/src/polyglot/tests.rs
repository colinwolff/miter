use crate::polyglot::*;
use std::path::Path;

#[test]
fn every_extension_is_detected() {
    for (path, expected) in [
        ("src/lib.rs", Language::Rust),
        ("src/api.ts", Language::TypeScript),
        ("src/api.mts", Language::TypeScript),
        ("src/api.cts", Language::TypeScript),
        ("src/api.js", Language::JavaScript),
        ("src/api.jsx", Language::JavaScript),
        ("src/api.mjs", Language::JavaScript),
        ("src/api.cjs", Language::JavaScript),
        ("src/api.d.ts", Language::TypeScript),
        ("app/Widget.tsx", Language::Tsx),
        ("pkg/models.py", Language::Python),
        ("pkg/models.pyi", Language::Python),
    ] {
        assert_eq!(
            Language::from_path(Path::new(path)),
            Some(expected),
            "{path} must be {expected:?}"
        );
    }
    assert_eq!(Language::from_path(Path::new("Cargo.toml")), None);
    assert_eq!(Language::from_path(Path::new("no_extension")), None);
}

#[test]
fn the_extensions_the_watcher_uses_are_all_detected() {
    for extension in EXTENSIONS {
        assert!(
            Language::from_extension(extension).is_some(),
            "{extension} is listed but detects nothing"
        );
    }
}

#[test]
fn the_three_families_are_the_ones_a_dashboard_filters_by() {
    assert_eq!(Language::Rust.family(), "rust");
    assert_eq!(Language::TypeScript.family(), "typescript");
    assert_eq!(Language::Tsx.family(), "typescript");
    assert_eq!(Language::Python.family(), "python");
}

#[test]
fn a_rust_snippet_is_read_into_universal_symbols() {
    let source = "\
/// A helper that scales.
pub fn scale(x: f32, factor: f32) -> f32 {
x * factor
}

pub struct Store {
field: u32,
}
";
    let symbols = parse(
        Path::new("crates/engine/src/math.rs"),
        source,
        Language::Rust,
    )
    .expect("rust parses");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    // A struct's field is a declaration too: `miter lookup Store` lists it as a member, and
    // `miter contracts` reads it as the struct's field list.
    assert_eq!(names, vec!["scale", "Store", "field"]);
    assert_eq!(symbols[2].kind, SymbolKind::Property);
    assert_eq!(symbols[2].depth, 3);
    assert_eq!(symbols[2].path, "engine.math.Store.field");
    assert_eq!(symbols[2].signature, "field: u32");

    let scale = &symbols[0];
    assert_eq!(scale.kind, SymbolKind::Function);
    assert_eq!(scale.depth, 3);
    assert_eq!(scale.docstring.as_deref(), Some("A helper that scales."));
    assert_eq!(scale.path, "engine.math.scale");
    assert_eq!(scale.params.len(), 2);
    assert_eq!(scale.return_type.as_deref(), Some("f32"));
    assert_eq!(scale.line, 2);
    assert_eq!(scale.signature, "pub fn scale(x: f32, factor: f32) -> f32");

    let store = &symbols[1];
    assert_eq!(store.kind, SymbolKind::Struct);
    assert_eq!(store.depth, 2);
    assert_eq!(store.docstring, None);
    assert_eq!(store.signature, "pub struct Store");
}

#[test]
fn a_rust_method_is_a_member_and_an_inner_function_is_a_leaf() {
    let source = "\
impl Store {
/// Fetches a record.
fn fetch(&self, id: u64) -> Option<Record> {
    fn helper() -> u64 { 0 }
    helper()
}
}
";
    let symbols = parse(Path::new("src/store.rs"), source, Language::Rust).expect("parses");
    let names: Vec<&str> = symbols.iter().map(|symbol| symbol.name.as_str()).collect();
    assert_eq!(names, vec!["fetch", "helper"]);

    let fetch = &symbols[0];
    assert_eq!(fetch.kind, SymbolKind::Method);
    assert_eq!(fetch.depth, 3);
    assert_eq!(fetch.path, "store.Store.fetch");
    assert_eq!(fetch.params[0].name, "&self");
    assert_eq!(fetch.params[0].type_name, "Self");
    assert_eq!(fetch.return_type.as_deref(), Some("Option<Record>"));
    assert_eq!(fetch.docstring.as_deref(), Some("Fetches a record."));

    let helper = &symbols[1];
    assert_eq!(helper.kind, SymbolKind::Function);
    assert_eq!(helper.depth, 4, "a function inside a function is a leaf");
    assert_eq!(helper.path, "store.Store.fetch.helper");
}

#[test]
fn a_declaration_carries_the_level_its_syntax_nests_it_at() {
    let source = "pub mod nested {\n    pub fn deep() {}\n}\n";
    let symbols = parse(Path::new("src/lib.rs"), source, Language::Rust).expect("parses");
    let module = symbols
        .iter()
        .find(|symbol| symbol.kind == SymbolKind::Module)
        .expect("the module");
    assert_eq!(module.depth, 1);
}

#[test]
fn the_identity_is_the_one_the_index_derives() {
    let source = "pub fn probe() {}\n";
    let path = Path::new("src/probe.rs");
    let symbols = parse(path, source, Language::Rust).expect("parses");
    let expected = miter_ast::seed_from_text(&format!("{}:1:probe", path.display()));
    assert_eq!(symbols[0].id(), expected);

    // The bridge to the index's own record keeps that id.
    let indexer = miter_ast::AstIndexer::new();
    let record = symbols[0].function_symbol(&indexer, 0);
    assert_eq!(record.id, expected);
    assert_eq!(record.language, miter_ast::Language::Rust);
    assert_eq!(record.scope_depth, symbols[0].depth);
    assert!(symbols[0].exported, "`pub fn probe` is exported");
}

#[test]
fn declarative_files_are_detected_and_parsed() {
    assert!(is_declarative_source(Path::new("migrations/schema.sql")));
    assert!(is_declarative_source(Path::new("infra/main.tf")));
    assert!(is_declarative_source(Path::new("infra/main.tofu")));
    assert!(is_declarative_source(Path::new("Dockerfile")));
    assert!(is_declarative_source(Path::new("Dockerfile.dev")));
    assert!(is_declarative_source(Path::new("deploy/pod.yaml")));
    assert!(is_declarative_source(Path::new("deploy/pod.yml")));
    assert!(is_declarative_source(Path::new("Containerfile")));
    assert!(is_declarative_source(Path::new("Containerfile.dev")));
    assert!(is_declarative_source(Path::new("docker-compose.yml")));
    assert!(is_declarative_source(Path::new("README.md")));
    assert!(is_declarative_source(Path::new("docs/guide.markdown")));
    assert!(!is_declarative_source(Path::new("Cargo.toml")));

    let sql = "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT);";
    let symbols = parse_detected(Path::new("schema.sql"), sql)
        .expect("parses")
        .expect("detected");
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "users");
    assert_eq!(symbols[0].kind, SymbolKind::Table);
}
