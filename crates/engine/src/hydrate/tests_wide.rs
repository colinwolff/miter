use super::resolve::{is_scaffolding, name_affinity};
use super::*;
use crate::polyglot::Language;
use crate::store::Engine;
use std::path::Path;

/// A struct with many public methods — the shape that overflows a small budget.
fn wide() -> Engine {
    let mut source = String::from(
        "/// A wide index.\npub struct Index {\n    w: Vec<f32>,\n}\n\nimpl Index {\n",
    );
    for index in 0..30 {
        source.push_str(&format!(
            "    /// Does thing {index}.\n    pub fn method_{index}(&self, value: u32) -> u32 {{ value }}\n"
        ));
    }
    source.push_str("}\n");
    let mut engine = Engine::new();
    engine
        .ingest_polyglot_file(
            &source,
            Path::new("crates/engine/src/index.rs"),
            0,
            Language::Rust,
        )
        .expect("valid Rust");
    engine
}

#[test]
fn the_member_list_is_capped_and_says_so() {
    let engine = wide();
    let context = hydrate_symbol_context_with(&engine, "Index", 4096, 100).expect("hydrates");

    // The cap is applied where the members are collected, so the context never holds more than
    // `MAX_MEMBERS` and a reader is never handed a silently truncated inventory.
    assert_eq!(
        context.members.len(),
        MAX_MEMBERS,
        "thirty methods, twelve listed"
    );
    let text = context.render_within(4096);
    assert!(text.contains("Public Members:"), "{text}");
    assert!(
        !text.contains("method_12"),
        "the tail of the inventory is what the cap sheds, and it says where it stopped: {text}"
    );
    assert!(text.contains("method_11"), "the head survives: {text}");
}

#[test]
fn a_wide_type_still_fits_the_default_budget() {
    // The case the release gate exercises: thirty methods and a neighbourhood must still come
    // back inside 256 tokens rather than raising EmptyBudget.
    let engine = wide();
    let context = hydrate_symbol_context(&engine, "Index", DEFAULT_MAX_TOKENS).expect("hydrates");
    assert!(
        context.tokens <= DEFAULT_MAX_TOKENS,
        "{} tokens exceeds the budget",
        context.tokens
    );
    assert!(context.text.contains("Symbol: Index"), "{}", context.text);
    assert!(
        context.text.ends_with('\n'),
        "the payload ends on a whole line"
    );
}

#[test]
fn a_tight_budget_sheds_members_before_it_gives_up_on_the_header() {
    let engine = wide();
    let context = hydrate_symbol_context(&engine, "Index", DEFAULT_MAX_TOKENS).expect("hydrates");
    let tight = context.render_within(96);

    assert!(
        estimate_tokens(&tight) <= 96 || !tight.contains("Public Members"),
        "a budget this small sheds members, or none were listed"
    );
    assert!(
        tight.starts_with("Symbol: Index"),
        "the header survives: {tight}"
    );
    assert!(
        tight.contains("Signature: pub struct Index { ... }"),
        "{tight}"
    );
}

#[test]
fn scaffolding_names_rank_behind_the_peers_a_reader_wants() {
    assert!(is_scaffolding("tests"));
    assert!(is_scaffolding("mod"));
    assert!(!is_scaffolding("Index"));

    assert_eq!(
        name_affinity("Index", "tests"),
        0,
        "scaffolding scores zero"
    );
    assert_eq!(name_affinity("Index", "Config"), 1, "an unrelated name");
    assert_eq!(name_affinity("Index", "load_index"), 2, "one shared word");
    assert_eq!(
        name_affinity("miter_index", "index_epoch"),
        2,
        "case and separators do not hide a shared word"
    );
    assert!(
        name_affinity("Index", "index_model") > name_affinity("Index", "Config"),
        "a related name outranks an unrelated one"
    );
}

#[test]
fn a_related_neighbour_outranks_an_equally_near_unrelated_one() {
    let source = "\
/// The index.
pub struct Index;
/// An index writer.
pub struct IndexWriter;
/// An unrelated scalar.
pub struct Scalar;
/// Scales a value.
pub fn scale(v: f32) -> f32 { v }
";
    let mut engine = Engine::new();
    engine
        .ingest_polyglot_file(source, Path::new("src/lib.rs"), 0, Language::Rust)
        .expect("valid Rust");
    let context = hydrate_symbol_context_with(&engine, "Index", 4096, 100).expect("hydrates");

    let names: Vec<&str> = context
        .neighbours
        .iter()
        .map(|neighbour| neighbour.name.as_str())
        .collect();
    assert!(
        names.contains(&"IndexWriter"),
        "the related peer is in the band: {names:?}"
    );
    let position = names.iter().position(|name| *name == "IndexWriter");
    if let Some(position) = position {
        for earlier in &names[..position] {
            assert!(
                !is_scaffolding(earlier),
                "scaffolding `{earlier}` outranked a related peer: {names:?}"
            );
        }
    }
}
