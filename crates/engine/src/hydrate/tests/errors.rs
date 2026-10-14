//! The refusals: an empty index, an unknown name, an impossible budget.

use super::*;

#[test]
fn a_ceiling_too_small_for_the_header_is_refused_not_truncated() {
    let engine = corpus();
    let error = hydrate_symbol_context(&engine, "CacheStore", 4).expect_err("refused");
    assert!(matches!(error, HydrateError::EmptyBudget(4)), "{error:?}");
    assert!(error.to_string().contains("raise the budget"), "{error}");
}

#[test]
fn an_empty_index_is_refused_before_any_name_is_blamed() {
    // The distinction this error exists for: an engine that has ingested nothing must not answer
    // "no declaration named X", because that is a claim about `X` and the index is what is
    // empty. This is the state a workspace indexed before the universal table existed is in.
    let engine = Engine::new();
    let error =
        hydrate_symbol_context(&engine, "CacheStore", DEFAULT_MAX_TOKENS).expect_err("refused");
    assert_eq!(error, HydrateError::IndexEmpty);
    let message = error.to_string();
    assert!(
        message.contains("miter index"),
        "it names the repair: {message}"
    );
    assert!(
        !message.contains("no declaration named"),
        "it must not blame the symbol: {message}"
    );
}
