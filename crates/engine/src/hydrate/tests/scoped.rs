//! Module- and package-scoped hydration.

use super::*;

#[test]
fn an_unknown_name_in_a_populated_index_is_still_not_found() {
    // The other half of the contract: an index that *has* symbols must still report a genuine
    // miss as a miss, or the new error would have swallowed it.
    let engine = corpus();
    let error =
        hydrate_symbol_context(&engine, "Nonexistent", DEFAULT_MAX_TOKENS).expect_err("refused");
    assert_eq!(error, HydrateError::NotFound("Nonexistent".to_owned()));
}

#[test]
fn an_unknown_name_is_named_in_the_refusal() {
    let engine = corpus();
    let error =
        hydrate_symbol_context(&engine, "Nonexistent", DEFAULT_MAX_TOKENS).expect_err("refused");
    assert_eq!(error, HydrateError::NotFound("Nonexistent".to_owned()));
    assert!(error.to_string().contains("Nonexistent"), "{error}");
}

#[test]
fn a_qualified_path_resolves_and_a_bare_ambiguous_name_reports_its_rivals() {
    // Two modules declaring the same name, indexed into one engine.
    let mut engine = engine_with("pub struct Model;\n", "crates/engine/src/model.rs");
    engine
        .ingest_polyglot_file(
            "pub struct Model;\n",
            Path::new("crates/other/src/model.rs"),
            0,
            Language::Rust,
        )
        .expect("valid Rust");

    // The bare name is a question: it must name its rivals rather than pick silently.
    let context = hydrate_symbol_context(&engine, "Model", DEFAULT_MAX_TOKENS).expect("hydrates");
    assert_eq!(context.name, "Model");
    assert_eq!(
        context.ambiguous_with.len(),
        1,
        "the rival file is reported: {context:?}"
    );
    assert!(context.text.contains("Ambiguous with:"));

    // The qualified path is an answer, and it resolves to exactly one declaration. The path
    // is the parse's own — `engine.model.Model`, module chain included.
    let qualified = hydrate_symbol_context(&engine, "engine.model.Model", DEFAULT_MAX_TOKENS)
        .expect("resolves");
    assert!(qualified.ambiguous_with.is_empty());
    assert_eq!(
        qualified.file,
        Path::new("crates/engine/src/model.rs").to_path_buf()
    );
}

#[test]
fn the_token_estimate_is_conservative_for_punctuation_dense_code() {
    let dense = "foo(&self, x: u32, y: Vec<u32>) -> Result<u64, Error>";
    let estimate = estimate_tokens(dense);
    let whitespace_only = dense.split_whitespace().count();
    // 53 characters over four per token, rounded up, against 8 whitespace fields — the
    // character count is the one that binds, which is the whole point of taking the maximum.
    assert_eq!(estimate, 14, "four chars per token, rounded up");
    assert!(
        estimate > whitespace_only,
        "a whitespace count alone ({whitespace_only}) would under-report {dense:?}"
    );
    assert_eq!(estimate_tokens(""), 0);
}

#[test]
fn a_wide_corpus_is_still_bounded_by_the_budget() {
    // Many declarations at the same level, so the band admits all of them and the render has to
    // shed the surplus.
    let mut source = String::new();
    for index in 0..40 {
        source.push_str(&format!(
            "/// Item {index}.\npub fn item_{index}(a: u32) -> u32 {{ a }}\n"
        ));
    }
    let engine = engine_with(&source, "crates/engine/src/wide.rs");
    let context = hydrate_symbol_context(&engine, "item_0", DEFAULT_MAX_TOKENS).expect("hydrates");
    assert!(
        context.tokens <= DEFAULT_MAX_TOKENS,
        "{} tokens exceeds the budget",
        context.tokens
    );
    assert!(
        context.neighbours.len() <= MAX_NEIGHBOURS,
        "the neighbourhood is bounded too"
    );
    assert!(context.text.contains("Symbol: item_0"));
}

#[test]
fn scoped_hydration_module_and_package_resolves() {
    let engine = corpus();

    // Scope: Symbol default
    let sym_res = hydrate_scoped(
        &engine,
        &ScopedHydrateOptions {
            target: "CacheStore".to_owned(),
            scope: HydrateScope::Symbol,
            depth: 1,
            max_tokens: 256,
        },
    )
    .expect("hydrates symbol");
    assert_eq!(sym_res.scope, HydrateScope::Symbol);
    assert!(sym_res.text.contains("pub struct CacheStore"));

    // Scope: Module
    let mod_res = hydrate_scoped(
        &engine,
        &ScopedHydrateOptions {
            target: "cache".to_owned(),
            scope: HydrateScope::Module,
            depth: 1,
            max_tokens: 1024,
        },
    )
    .expect("hydrates module");
    assert_eq!(mod_res.scope, HydrateScope::Module);
    assert!(mod_res.text.contains("pub struct CacheStore"));
    assert!(mod_res.text.contains("EntryBuffer"));

    // Scope: Package
    let pkg_res = hydrate_scoped(
        &engine,
        &ScopedHydrateOptions {
            target: "engine".to_owned(),
            scope: HydrateScope::Package,
            depth: 1,
            max_tokens: 1024,
        },
    )
    .expect("hydrates package");
    assert_eq!(pkg_res.scope, HydrateScope::Package);
    assert!(pkg_res.text.contains("pub struct CacheStore"));
}

#[test]
fn scoped_hydration_truncates_at_boundary_with_message() {
    let engine = corpus();
    // Tight budget so it must truncate
    let tight_res = hydrate_scoped(
        &engine,
        &ScopedHydrateOptions {
            target: "cache".to_owned(),
            scope: HydrateScope::Module,
            depth: 1,
            max_tokens: 64,
        },
    )
    .expect("hydrates with truncation");

    assert!(tight_res.tokens <= 64 || tight_res.text.contains("[truncated:"));
    if tight_res.included_count < tight_res.match_count {
        assert!(tight_res.text.contains("[truncated:"));
        assert!(
            tight_res
                .text
                .contains("additional declarations omitted. Use scope='module' on submodules]")
        );
    }
}
