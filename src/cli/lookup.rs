//! `miter lookup`: Locate a declaration and print its hydrated context.

use std::path::Path;

use miter_engine::hydrate::{HydrateError, HydratedContext, HydratedNeighbour};

use super::CliError;
use super::json::Json;

/// `miter lookup <SYMBOL> [--context] [--json] [--db <PATH>]`.
///
/// Without `--context` the command prints the declaration's identity only;
/// with it, the hydrated payload.
pub fn run_lookup(
    symbol: &str,
    db: &Path,
    context: bool,
    json: bool,
    max_tokens: usize,
) -> Result<(), CliError> {
    let engine = super::store::load(db)?;
    let hydrated = miter_engine::hydrate_symbol_context(&engine, symbol, max_tokens)?;

    if json {
        println!("{}", lookup_json(&hydrated).render());
    } else if context {
        print!("{}", hydrated.text);
    } else {
        print!("{}", identity_only(&hydrated));
    }
    Ok(())
}

/// The bare identity block, for a caller that wants the address and not the neighbourhood.
fn identity_only(context: &HydratedContext) -> String {
    format!(
        "{}\n  {}:{}\n  {} · level {}\n",
        context.name,
        context.file.display(),
        context.line,
        context.kind.name(),
        context.depth,
    )
}

/// The `lookup --json` document: everything hydration measured, not just what it rendered.
fn lookup_json(context: &HydratedContext) -> Json {
    Json::object([
        ("command", Json::string("lookup")),
        ("symbol", Json::string(context.name.clone())),
        ("qualified", Json::string(context.qualified.clone())),
        ("module", Json::string(context.module.clone())),
        ("file", Json::string(context.file.display().to_string())),
        ("line", Json::Unsigned(context.line as u64)),
        (
            "span",
            Json::array([
                Json::Unsigned(context.span.0 as u64),
                Json::Unsigned(context.span.1 as u64),
            ]),
        ),
        ("kind", Json::string(context.kind.name())),
        ("depth", Json::Unsigned(context.depth as u64)),
        ("signature", Json::string(context.signature.clone())),
        (
            "docstring",
            Json::optional(context.docstring.clone().map(Json::string)),
        ),
        (
            "params",
            Json::array(context.params.iter().map(|param| {
                Json::object([
                    ("name", Json::string(param.name.clone())),
                    ("type", Json::string(param.type_name.clone())),
                ])
            })),
        ),
        (
            "returns",
            Json::optional(context.return_type.clone().map(Json::string)),
        ),
        (
            "members",
            Json::array(context.members.iter().map(neighbour_json)),
        ),
        (
            "neighbours",
            Json::array(context.neighbours.iter().map(neighbour_json)),
        ),
        (
            "ambiguous_with",
            Json::array(
                context
                    .ambiguous_with
                    .iter()
                    .map(|path| Json::string(path.display().to_string())),
            ),
        ),
        ("tokens", Json::Unsigned(context.tokens as u64)),
        ("text", Json::string(context.text.clone())),
    ])
}

fn neighbour_json(neighbour: &HydratedNeighbour) -> Json {
    Json::object([
        ("name", Json::string(neighbour.name.clone())),
        ("file", Json::string(neighbour.file.display().to_string())),
        ("line", Json::Unsigned(neighbour.line as u64)),
        ("depth", Json::Unsigned(neighbour.depth as u64)),
        ("level_delta", Json::Unsigned(neighbour.level_delta as u64)),
    ])
}

impl From<HydrateError> for CliError {
    fn from(error: HydrateError) -> Self {
        match error {
            HydrateError::IndexEmpty => CliError::Usage(error.to_string()),
            HydrateError::NotFound(_) => CliError::Usage(error.to_string()),
            other => CliError::Usage(format!("lookup failed: {other}")),
        }
    }
}
