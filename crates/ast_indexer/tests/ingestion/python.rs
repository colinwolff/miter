//! Python ingestion.
//!
//! This module proves the parameter shapes the grammar presents are read into a name
//! and a type rather than into the raw source text: `typed_parameter` (`x: int`),
//! `default_parameter` (`x=1`) and the bare splats (`*args`, `**kwargs`).

use miter_ast::{AstIndexer, FunctionNode, Language, Param};

const SAMPLE: &str = r#"
"""A module of small helpers."""

from typing import Iterable


def scale(x: float, factor: float) -> float:
    """Multiply by a factor."""
    return x * factor


def label(x, prefix="v"):
    return f"{prefix}{x}"


def total(*values, **options) -> int:
    return sum(values)


class Repository:
    """Reads users out of a store."""

    def __init__(self, url: str) -> None:
        self.url = url

    async def find(self, user_id: str, limit: int = 10) -> str | None:
        return None

    def names(self, users: Iterable[str]) -> list[str]:
        def inner() -> None:
            return None
        return list(users)


def driver():
    pass
"#;

fn extract(source: &str) -> Vec<FunctionNode> {
    AstIndexer::new()
        .extract(source, Language::Python)
        .expect("python parses")
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
fn typed_parameters_are_split_into_name_and_type() {
    let nodes = extract(SAMPLE);
    let scale = by_name(&nodes, "scale");
    assert_eq!(
        scale.params,
        vec![
            Param {
                name: "x".into(),
                type_name: "float".into()
            },
            Param {
                name: "factor".into(),
                type_name: "float".into()
            },
        ]
    );
    assert_eq!(scale.return_type.as_deref(), Some("float"));
    assert_eq!(scale.scope_depth, 0);
}

#[test]
fn default_and_splat_parameters_survive() {
    let nodes = extract(SAMPLE);
    let label = by_name(&nodes, "label");
    assert_eq!(label.params[0].name, "x");
    assert_eq!(label.params[1].name, "prefix");
    assert!(label.params[1].type_name.is_empty());
    assert_eq!(label.return_type, None);

    let total = by_name(&nodes, "total");
    assert_eq!(total.params.len(), 2);
    assert_eq!(total.params[0].name, "*values");
    assert_eq!(total.params[1].name, "**options");
    assert_eq!(total.return_type.as_deref(), Some("int"));
}

#[test]
fn methods_are_nested_inside_their_class() {
    let nodes = extract(SAMPLE);
    let find = by_name(&nodes, "find");
    assert_eq!(find.params[0].name, "self");
    assert_eq!(find.params[0].type_name, "Self");
    assert_eq!(find.return_type.as_deref(), Some("str | None"));
    // `class Repository:` is one scope, so every method is one level deeper than
    // a module-level function.
    assert_eq!(find.scope_depth, 1);
    assert_eq!(by_name(&nodes, "names").scope_depth, 1);
}

#[test]
fn a_definition_inside_a_definition_is_a_second_scope() {
    let nodes = extract(SAMPLE);
    assert_eq!(by_name(&nodes, "inner").scope_depth, 2);
}
