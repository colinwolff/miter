//! Symbol hydration: one name in, one rendered context out.
//!
//! `hydrate_symbol_context` is the entry point the CLI and the MCP tool both call.

use crate::store::Engine;

use super::render::{estimate_tokens, first_sentence, render_signature};
use super::resolve::{members_of, neighbours_of, resolve};
use super::types::{HYDRATION_LEVEL_BAND, HydrateError, HydratedContext, ParamContract};

/// Hydrates the symbol called `symbol_name` out of `engine`.
///
/// # Errors
///
/// [`HydrateError::IndexEmpty`] when the database holds no declarations at all (so the failure is
/// the index's, not the symbol's), [`HydrateError::NotFound`] when the index is populated but no
/// declaration answers to the name, and [`HydrateError::EmptyBudget`] when `max_tokens` is too
/// small to hold the symbol's own header — which is refused rather than met with a truncated line.
///
/// An ambiguous name is *not* an error: the context reports every other file it resolved to in
/// `ambiguous_with`, because the caller's next move is to disambiguate and that information is
/// exactly what disambiguating needs.
pub fn hydrate_symbol_context(
    engine: &Engine,
    symbol_name: &str,
    max_tokens: usize,
) -> Result<HydratedContext, HydrateError> {
    hydrate_symbol_context_with(engine, symbol_name, max_tokens, HYDRATION_LEVEL_BAND)
}

/// [`hydrate_symbol_context`] with an explicit level band: how many hierarchy steps either way a
/// neighbour may sit and still be listed.
pub fn hydrate_symbol_context_with(
    engine: &Engine,
    symbol_name: &str,
    max_tokens: usize,
    level_band: usize,
) -> Result<HydratedContext, HydrateError> {
    let symbols = engine.universal_symbols();
    // An index that holds nothing cannot answer *any* name, so saying "no declaration named X"
    // would blame the symbol for the database's state. A workspace that was never indexed — or
    // one indexed before the universal table existed — produces this instead, with the command
    // that fixes it. The distinction is the difference between a five-second fix and an hour of
    // searching the source for a declaration that is right there.
    if symbols.is_empty() {
        return Err(HydrateError::IndexEmpty);
    }
    let (target, ambiguous_with) = resolve(symbols, symbol_name)
        .ok_or_else(|| HydrateError::NotFound(symbol_name.to_owned()))?;

    let mut context = HydratedContext {
        name: target.name.clone(),
        qualified: target.path.clone(),
        module: target.module(),
        file: target.file.clone(),
        line: target.line,
        span: target.span,
        kind: target.kind,
        depth: target.depth,
        signature: render_signature(target),
        docstring: target.docstring.as_deref().map(first_sentence),
        params: target
            .params
            .iter()
            .map(|param| ParamContract {
                name: param.name.clone(),
                type_name: param.type_name.clone(),
            })
            .collect(),
        return_type: target.return_type.clone(),
        members: members_of(symbols, target),
        neighbours: neighbours_of(symbols, target, level_band),
        ambiguous_with,
        text: String::new(),
        tokens: 0,
    };

    context.signature = crate::security::scrub_source_text(&context.signature);
    context.text = crate::security::scrub_source_text(&context.render_within(max_tokens));
    context.tokens = estimate_tokens(&context.text);

    // The header is not negotiable: a context that dropped the declaration's own name to fit a
    // neighbour list has answered a different question.
    if estimate_tokens(&context.text) > max_tokens {
        return Err(HydrateError::EmptyBudget(max_tokens));
    }
    Ok(context)
}
