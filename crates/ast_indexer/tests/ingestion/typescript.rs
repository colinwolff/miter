//! TypeScript ingestion.
//!
//! This module proves the TS half of the crate's extraction contract end to end.

use miter_ast::{AstIndexer, FunctionNode, Language, Param};
use std::path::Path;

const SAMPLE: &str = r#"
export function fetchUser(id: string, retries: number): Promise<User> {
    return client.get(id);
}

export function log(message: string) {
    console.log(message);
}

class Repository {
    constructor(private readonly url: string) {}

    async find(id: string): Promise<User | null> {
        return null;
    }

    static parse(raw: string, options?: ParseOptions): unknown {
        return raw;
    }
}

const helper = (value: number, scale: number): number => value * scale;
const untitled = () => 42;

namespace Internals {
    export function nested(flag: boolean): boolean {
        return !flag;
    }
}
"#;

fn extract(source: &str) -> Vec<FunctionNode> {
    AstIndexer::new()
        .extract(source, Language::TypeScript)
        .expect("typescript parses")
}

fn by_name<'a>(nodes: &'a [FunctionNode], name: &str) -> &'a FunctionNode {
    nodes
        .iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| {
            let found: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
            panic!("no function named {name}; found {found:?}")
        })
}

#[test]
fn extracts_function_declarations_with_types() {
    let nodes = extract(SAMPLE);
    let fetch = by_name(&nodes, "fetchUser");
    assert_eq!(
        fetch.params,
        vec![
            Param {
                name: "id".into(),
                type_name: "string".into()
            },
            Param {
                name: "retries".into(),
                type_name: "number".into()
            },
        ]
    );
    assert_eq!(fetch.return_type.as_deref(), Some("Promise<User>"));
    assert_eq!(fetch.scope_depth, 0);
    assert_eq!(fetch.line, 2);
}

#[test]
fn unannotated_return_types_are_reported_as_absent() {
    let nodes = extract(SAMPLE);
    assert_eq!(by_name(&nodes, "log").return_type, None);
    assert_eq!(by_name(&nodes, "log").params.len(), 1);
}

#[test]
fn methods_arrow_functions_and_namespaces_are_indexed_with_depth() {
    let nodes = extract(SAMPLE);

    // Class methods sit one level below the class declaration.
    let find = by_name(&nodes, "find");
    assert_eq!(find.scope_depth, 1);
    assert_eq!(
        find.params,
        vec![Param {
            name: "id".into(),
            type_name: "string".into()
        }]
    );
    assert_eq!(find.return_type.as_deref(), Some("Promise<User | null>"));

    // Optional parameters keep their annotation.
    let parse = by_name(&nodes, "parse");
    assert_eq!(parse.params.len(), 2);
    assert_eq!(parse.params[1].type_name, "ParseOptions");

    // Arrow functions inherit the name of the binding they are assigned to.
    let helper = by_name(&nodes, "helper");
    assert_eq!(helper.return_type.as_deref(), Some("number"));
    assert_eq!(helper.params.len(), 2);

    // Namespaces add a scope level for nested declarations.
    assert_eq!(by_name(&nodes, "nested").scope_depth, 1);
}

#[test]
fn encoding_matches_the_rust_pipeline() {
    let indexer = AstIndexer::new();
    let symbols = indexer
        .index_source(SAMPLE, Language::TypeScript, Path::new("web/api.ts"), 2)
        .expect("parses");

    let fetch = symbols
        .iter()
        .find(|symbol| symbol.name == "fetchUser")
        .expect("fetchUser is encoded");
    assert_eq!(fetch.language, Language::TypeScript);
    assert_eq!(fetch.depth(), 2);
    assert_eq!(
        fetch.signature(),
        "fetchUser(id: string, retries: number) -> Promise<User>"
    );
}

#[test]
fn tsx_and_declaration_files_are_detected() {
    // a `.tsx` file is no longer filed as `TypeScript` — it has its own
    // variant, because the TSX grammar accepts JSX declarations the TypeScript
    // grammar does not (`Language::family` is what still groups the three).
    assert_eq!(
        Language::from_path(Path::new("app/component.tsx")),
        Some(Language::Tsx)
    );
    assert_eq!(
        Language::from_path(Path::new("types/index.d.ts")),
        Some(Language::TypeScript)
    );
    assert!(Language::TypeScript.is_enabled());
    assert!(Language::Tsx.is_enabled());
}

#[test]
fn a_tsx_file_is_read_with_the_tsx_grammar() {
    // JSX only parses under `LANGUAGE_TSX`, which is the whole reason the variant
    // exists: `<Widget … />` is a syntax error to `LANGUAGE_TYPESCRIPT`.
    let source = r#"
export function Widget(props: { label: string }) {
    return <span className="widget">{props.label}</span>;
}
"#;
    let nodes = AstIndexer::new()
        .extract(source, Language::Tsx)
        .expect("tsx parses");
    assert_eq!(
        by_name(&nodes, "Widget").params[0].type_name,
        "{ label: string }"
    );

    // A JavaScript file is read by the same grammar: it is the superset.
    let script = "const double = (n) => n * 2;\n";
    assert_eq!(
        by_name(
            &AstIndexer::new()
                .extract(script, Language::JavaScript)
                .expect("javascript parses"),
            "double",
        )
        .params
        .len(),
        1
    );
}

#[test]
fn interface_method_signatures_are_extracted() {
    let nodes = extract(
        "interface Client {\n  request(url: string, body: unknown): Promise<Response>;\n}\n",
    );
    let request = by_name(&nodes, "request");
    assert_eq!(request.params.len(), 2);
    assert_eq!(request.return_type.as_deref(), Some("Promise<Response>"));
    assert_eq!(request.scope_depth, 1);
}
