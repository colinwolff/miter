//! The hydrated context: facts, members and the rendered sections.

use super::*;

#[test]
fn a_struct_hydrates_with_its_own_declaration_facts() {
    let engine = corpus();
    let context =
        hydrate_symbol_context(&engine, "CacheStore", DEFAULT_MAX_TOKENS).expect("hydrates");

    assert_eq!(context.name, "CacheStore");
    assert_eq!(context.kind, SymbolKind::Struct);
    assert_eq!(context.depth, 2, "a struct is a scope");
    assert!(
        context.span.1 > context.span.0,
        "a declaration's byte span is non-empty: {:?}",
        context.span
    );
    assert!(context.signature.contains("pub struct CacheStore"));
    assert!(
        context.signature.ends_with("{ ... }"),
        "a type opens a body"
    );
    assert_eq!(
        context.docstring.as_deref(),
        Some("A cache of decoded index entries."),
        "the doc comment's first sentence"
    );
}

#[test]
fn public_members_are_listed_and_private_ones_are_not() {
    let engine = corpus();
    let context =
        hydrate_symbol_context(&engine, "EntryBuffer", DEFAULT_MAX_TOKENS).expect("hydrates");

    let names: Vec<&str> = context.members.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["read", "write"],
        "members as the source declares them, exported only"
    );
    assert!(
        !names.contains(&"secret_tweak"),
        "a private method is not part of a caller's problem"
    );
}

#[test]
fn the_payload_carries_the_documented_sections() {
    let engine = corpus();
    let context =
        hydrate_symbol_context(&engine, "CacheStore", DEFAULT_MAX_TOKENS).expect("hydrates");

    assert!(context.text.starts_with("Symbol: CacheStore\n"));
    assert!(context.text.contains("crates/engine/src/cache.rs"));
    assert!(
        context
            .text
            .contains("Kind: struct | Depth: Level 2 (Scope)")
    );
    assert!(
        context
            .text
            .contains("Signature: pub struct CacheStore { ... }")
    );
}
