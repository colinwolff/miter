//! One declaration, as every language's visitor reports it.

use std::path::PathBuf;

use super::{Language, SymbolKind};

/// One declaration, as every language's visitor reports it.
///
/// The first eight fields are the objective's universal declaration; the rest are what makes
/// one *usable*:
///
/// * `file` and `line` are the identity [`UniversalSymbol::id`] is derived from. Without the
///   path a coordinate could not depend on the crate, and without the line two same-named
///   declarations in one file would be one symbol (the determinism rule);
/// * `params` and `return_type` are the structured form of `signature`, read by the *same*
///   reader the index uses (`miter_ast::signature_of`) because `FunctionSymbol` — the
///   record the `symbols` table holds — needs them, and re-parsing a rendered signature to
///   recover them would be a parser that could disagree with the one that wrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniversalSymbol {
    /// Bare declaration name (`fetchUser`, `Repository`, `scale`).
    pub name: String,
    /// The grammar that read it.
    pub language: Language,
    /// What it is.
    pub kind: SymbolKind,
    /// Qualified name, `my_module.MyClass.my_method`.
    pub path: String,
    /// Hierarchy level, `0..=4`.
    pub depth: usize,
    /// The doc comment or docstring that documents it, markers stripped.
    pub docstring: Option<String>,
    /// The declaration's header on one line, body removed.
    pub signature: String,
    /// Byte span of the declaration in its source, `(start, end)`.
    pub span: (usize, usize),
    /// File the declaration was read from.
    pub file: PathBuf,
    /// One-based line of the declaration.
    pub line: usize,
    /// Parameter list, in declaration order.
    pub params: Vec<miter_ast::Param>,
    /// Annotated return type, if the source carries one.
    pub return_type: Option<String>,
    /// Whether the declaration is visible outside its own module.
    ///
    /// Rust's `pub`, TypeScript's `export`, Python's `__all__`/leading-underscore
    /// convention. A universal field rather than three: "is this declaration part of the
    /// module's surface" is the same question in all three, and a per-language note would
    /// make every caller answer it instead of reading it.
    pub exported: bool,
}

impl UniversalSymbol {
    /// The stable identifier: `file:line:name`, hashed exactly as the index hashes it.
    ///
    /// The same formula `miter_ast::AstIndexer::encode` uses, so a declaration's key is
    /// the same whichever reader produced it — which is what lets a re-projection update a
    /// row instead of adding a second one.
    pub fn id(&self) -> u64 {
        miter_ast::seed_from_text(&format!(
            "{}:{}:{}",
            self.file.display(),
            self.line,
            self.name
        ))
    }

    /// The module the declaration lives in, as `crate/module` — the convention a projected
    /// label and the dashboard's crate filter are written in.
    pub fn module(&self) -> String {
        super::common::module_label(&self.file)
    }

    /// Everything the projection's label needs: `crate/module · name`.
    pub fn label(&self) -> String {
        format!("{} · {}", self.module(), self.name)
    }

    /// The index's record for this declaration.
    ///
    /// The bridge to the half of the engine that is function-shaped: the `symbols` table,
    /// the persistence codec and the Rayon query pipeline all speak
    /// [`miter_ast::FunctionSymbol`], so a polyglot declaration enters them through the
    /// index's own encoder — with the *same* id, so nothing is duplicated.
    pub fn function_symbol(
        &self,
        indexer: &miter_ast::AstIndexer,
        dir_depth: usize,
    ) -> miter_ast::FunctionSymbol {
        let node = miter_ast::FunctionNode {
            name: self.name.clone(),
            params: self.params.clone(),
            return_type: self.return_type.clone(),
            scope_depth: self.depth,
            line: self.line,
            start_byte: self.span.0,
            end_byte: self.span.1,
        };
        // A declarative symbol has no tree-sitter language. The function-shaped table files
        // it under Rust because that table has no other home for it, and nothing reads the
        // field.
        let language = self
            .language
            .index_language()
            .unwrap_or(miter_ast::Language::Rust);
        indexer.encode(node, &self.file, dir_depth, language)
    }
}
