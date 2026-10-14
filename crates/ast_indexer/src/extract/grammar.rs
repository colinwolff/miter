//! The grammar table: which node kinds declare a function, per language.
//!
//! Adding a language means adding a spec here, not another traversal.

use crate::IndexError;

use super::Language;

/// Grammar-specific node kinds and syntax details.
#[derive(Debug)]
pub(super) struct GrammarSpec {
    /// Node kinds that declare a function.
    pub(super) function_kinds: &'static [&'static str],
    /// Node kinds that introduce a scope for [`FunctionNode::scope_depth`].
    pub(super) scope_kinds: &'static [&'static str],
    /// Sigil that introduces a return type in the source.
    pub(super) return_sigil: &'static str,
}

const RUST_SPEC: GrammarSpec = GrammarSpec {
    function_kinds: &["function_item", "function_signature_item"],
    scope_kinds: &["mod_item", "impl_item", "trait_item", "function_item"],
    return_sigil: "->",
};

const TYPESCRIPT_SPEC: GrammarSpec = GrammarSpec {
    function_kinds: &[
        "function_declaration",
        "function_signature",
        "method_definition",
        "method_signature",
        "abstract_method_signature",
        "arrow_function",
    ],
    scope_kinds: &[
        "class_declaration",
        "class",
        "internal_module",
        "module",
        "interface_declaration",
        "function_declaration",
        "method_definition",
        "arrow_function",
    ],
    return_sigil: ":",
};

/// Python's declarations, read with `LANGUAGE`.
///
/// Three node kinds carry a `def`, and all three are named so a decorated or an
/// `async` method is not silently dropped: `function_definition` is the plain
/// `def`, and tree-sitter-python wraps `async def` inside the same kind (with an
/// `async` child), so a single entry covers it. A `class_definition` and a nested
/// `function_definition` both introduce a scope, which is what makes a method's
/// `scope_depth` larger than a module-level function's.
///
/// The parameters arrive as `identifier` (no fields at all), `default_parameter`
/// (`name`/`value`), `typed_parameter` (`type`, with the name as the first child)
/// and `typed_default_parameter` (`name`/`type`/`value`) — which is what
/// [`read_param`]'s first-child fallback is for.
const PYTHON_SPEC: GrammarSpec = GrammarSpec {
    function_kinds: &["function_definition"],
    scope_kinds: &["class_definition", "function_definition"],
    return_sigil: "->",
};

pub(super) fn spec_for(language: Language) -> Option<&'static GrammarSpec> {
    match language {
        Language::Rust => Some(&RUST_SPEC),
        Language::TypeScript | Language::Tsx | Language::JavaScript => Some(&TYPESCRIPT_SPEC),
        Language::Python => Some(&PYTHON_SPEC),
    }
}

/// Loads the grammar for `language`.
///
/// `Tsx` and `JavaScript` both answer `LANGUAGE_TSX`: the TSX grammar is the
/// superset that accepts JSX, which is the reason a `.jsx` file exists.
///
/// # Errors
///
/// [`IndexError::UnsupportedLanguage`] when the grammar is not compiled in.
pub fn grammar_for(language: Language) -> Result<tree_sitter::Language, IndexError> {
    match language {
        Language::Rust => Ok(tree_sitter_rust::LANGUAGE.into()),
        Language::TypeScript => Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        Language::Tsx | Language::JavaScript => Ok(tree_sitter_typescript::LANGUAGE_TSX.into()),
        Language::Python => Ok(tree_sitter_python::LANGUAGE.into()),
    }
}
