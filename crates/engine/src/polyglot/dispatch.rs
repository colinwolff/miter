//! Parsing: source text in, universal declarations out.

use std::path::Path;

use super::{
    Language, PolyglotError, UniversalSymbol, css, docker, env, hcl, html, k8s, markdown, python,
    rust, sql, typescript,
};

/// Parses `source` with a tree-sitter grammar and returns every declaration in it.
///
/// The grammar comes from [`miter_ast::grammar_for`], which is the workspace's one table saying
/// which grammar reads which language, and the visitor is this module's own: the index's walker
/// stops at the node kinds that declare a *function*, while a universal visitor has to see types,
/// modules and doc comments too.
///
/// # Errors
///
/// [`PolyglotError::Grammar`] when the grammar cannot be loaded, [`PolyglotError::Parse`] when
/// `tree-sitter` returns no tree, and [`PolyglotError::Unsupported`] for a declarative format —
/// those are read by [`parse_declarative`] instead.
pub fn parse(
    path: &Path,
    source: &str,
    language: Language,
) -> Result<Vec<UniversalSymbol>, PolyglotError> {
    let Some(index_language) = language.index_language() else {
        return Err(PolyglotError::Unsupported { language });
    };
    let mut parser = tree_sitter::Parser::new();
    let grammar = miter_ast::grammar_for(index_language).map_err(|error| match error {
        miter_ast::IndexError::UnsupportedLanguage { .. } => {
            PolyglotError::Unsupported { language }
        }
        other => PolyglotError::Grammar {
            language,
            message: other.to_string(),
        },
    })?;
    parser
        .set_language(&grammar)
        .map_err(|error| PolyglotError::Grammar {
            language,
            message: error.to_string(),
        })?;
    let tree = parser.parse(source, None).ok_or(PolyglotError::Parse {
        language,
        path: Some(path.to_path_buf()),
    })?;

    match language {
        Language::Rust => Ok(rust::visit(path, source, tree.root_node())),
        Language::TypeScript | Language::Tsx | Language::JavaScript => {
            Ok(typescript::visit(path, source, tree.root_node(), language))
        }
        Language::Python => Ok(python::visit(path, source, tree.root_node())),
        _ => Err(PolyglotError::Unsupported { language }),
    }
}

/// [`parse`] with the format read off `path`.
///
/// `Ok(None)` when the path names no format this harness reads, which is not a failure: a
/// directory walk meets a lockfile and a licence and should keep going.
///
/// # Errors
///
/// As [`parse`].
pub fn parse_detected(
    path: &Path,
    source: &str,
) -> Result<Option<Vec<UniversalSymbol>>, PolyglotError> {
    match Language::from_path(path) {
        None => Ok(None),
        Some(language) if language.is_declarative() => {
            Ok(parse_declarative(path, source, language))
        }
        Some(language) => parse(path, source, language).map(Some),
    }
}

/// `true` when `path` names a format whose declarations are read directly.
#[must_use]
pub fn is_declarative_source(path: &Path) -> bool {
    Language::from_path(path).is_some_and(Language::is_declarative)
}

/// Reads a declarative format's declarations.
///
/// `None` when the format is not declarative, so a caller can tell "nothing declared here" from
/// "this is not a declarative file at all".
pub fn parse_declarative(
    path: &Path,
    source: &str,
    language: Language,
) -> Option<Vec<UniversalSymbol>> {
    Some(match language {
        Language::Sql => sql::extract_sql_symbols(path, source),
        Language::Html => html::extract_html_symbols(path, source),
        Language::Css => css::extract_css_symbols(path, source),
        Language::Markdown => markdown::extract_markdown_symbols(path, source),
        Language::Dockerfile => docker::extract_docker_symbols(path, source),
        Language::Yaml => k8s::extract_yaml_symbols(path, source),
        Language::Hcl => hcl::extract_hcl_symbols(path, source),
        Language::Env => env::extract_env_symbols(path, source),
        _ => return None,
    })
}

/// Reads a declarative file, choosing the reader from its path.
#[must_use]
pub fn declarative_symbols(path: &Path, source: &str) -> Option<Vec<UniversalSymbol>> {
    let language = Language::from_path(path)?;
    parse_declarative(path, source, language)
}
