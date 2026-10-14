//! Every format `miter` reads, and the one recognizer that names them.

use std::path::Path;

/// A format the universal model reads.
///
/// The first five are read with a tree-sitter grammar; the rest are *declarative* formats whose
/// declarations are read directly, because a Dockerfile or a stylesheet has no syntax tree worth
/// walking for names. They share one enum because every consumer — the walker, the audit, the
/// dirty-file pass, the dispatcher — asks the same question ("what is this file?") and a second
/// table answering it is a second table that can disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PolyglotLanguage {
    /// Rust.
    Rust,
    /// TypeScript.
    TypeScript,
    /// TSX — TypeScript with JSX.
    Tsx,
    /// JavaScript, including the `.mjs`/`.cjs` module flavours (read with the TSX grammar).
    JavaScript,
    /// Python.
    Python,
    /// SQL DDL: tables, views, indexes and the columns they declare.
    Sql,
    /// HTML documents: ids, custom elements, templates and landmarks.
    Html,
    /// CSS and SCSS: custom properties, selectors, keyframes and layers.
    Css,
    /// Markdown: the ATX heading outline.
    Markdown,
    /// Dockerfile / Containerfile: stages, build arguments and published ports.
    Dockerfile,
    /// Kubernetes and Docker Compose manifests.
    Yaml,
    /// HCL / OpenTofu / Terraform: resources, modules and variables.
    Hcl,
    /// `.env` key files (values are never recorded).
    Env,
}

/// The historical spelling of [`PolyglotLanguage`].
///
/// Kept as an alias so the many call sites that name `polyglot::Language` keep compiling.
pub use PolyglotLanguage as Language;

impl PolyglotLanguage {
    /// Every format, in report order.
    pub const ALL: [PolyglotLanguage; 13] = [
        PolyglotLanguage::Rust,
        PolyglotLanguage::TypeScript,
        PolyglotLanguage::Tsx,
        PolyglotLanguage::JavaScript,
        PolyglotLanguage::Python,
        PolyglotLanguage::Sql,
        PolyglotLanguage::Html,
        PolyglotLanguage::Css,
        PolyglotLanguage::Markdown,
        PolyglotLanguage::Dockerfile,
        PolyglotLanguage::Yaml,
        PolyglotLanguage::Hcl,
        PolyglotLanguage::Env,
    ];

    /// Human-readable name.
    pub const fn name(self) -> &'static str {
        match self {
            PolyglotLanguage::Rust => "rust",
            PolyglotLanguage::TypeScript => "typescript",
            PolyglotLanguage::Tsx => "tsx",
            PolyglotLanguage::JavaScript => "javascript",
            PolyglotLanguage::Python => "python",
            PolyglotLanguage::Sql => "sql",
            PolyglotLanguage::Html => "html",
            PolyglotLanguage::Css => "css",
            PolyglotLanguage::Markdown => "markdown",
            PolyglotLanguage::Dockerfile => "dockerfile",
            PolyglotLanguage::Yaml => "yaml",
            PolyglotLanguage::Hcl => "hcl",
            PolyglotLanguage::Env => "env",
        }
    }

    /// The *family* a dashboard filters by: the three tree-sitter families, then one per
    /// declarative format.
    pub const fn family(self) -> &'static str {
        match self {
            PolyglotLanguage::Rust => "rust",
            PolyglotLanguage::TypeScript | PolyglotLanguage::Tsx | PolyglotLanguage::JavaScript => {
                "typescript"
            }
            PolyglotLanguage::Python => "python",
            PolyglotLanguage::Sql => "sql",
            PolyglotLanguage::Html => "html",
            PolyglotLanguage::Css => "css",
            PolyglotLanguage::Markdown => "markdown",
            PolyglotLanguage::Dockerfile => "docker",
            PolyglotLanguage::Yaml => "yaml",
            PolyglotLanguage::Hcl => "hcl",
            PolyglotLanguage::Env => "env",
        }
    }

    /// File extensions this format is detected from (empty for the two filename-only formats).
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            PolyglotLanguage::Rust => &["rs"],
            PolyglotLanguage::TypeScript => &["ts", "mts", "cts"],
            PolyglotLanguage::Tsx => &["tsx"],
            PolyglotLanguage::JavaScript => &["js", "jsx", "mjs", "cjs"],
            PolyglotLanguage::Python => &["py", "pyi"],
            PolyglotLanguage::Sql => &["sql"],
            PolyglotLanguage::Html => &["html", "htm"],
            PolyglotLanguage::Css => &["css", "scss"],
            PolyglotLanguage::Markdown => &["md", "markdown"],
            PolyglotLanguage::Dockerfile => &["dockerfile"],
            PolyglotLanguage::Yaml => &["yaml", "yml"],
            PolyglotLanguage::Hcl => &["tf", "tofu"],
            PolyglotLanguage::Env => &[],
        }
    }

    /// `true` when a tree-sitter grammar reads this format.
    pub const fn is_tree_sitter(self) -> bool {
        matches!(
            self,
            PolyglotLanguage::Rust
                | PolyglotLanguage::TypeScript
                | PolyglotLanguage::Tsx
                | PolyglotLanguage::JavaScript
                | PolyglotLanguage::Python
        )
    }

    /// `true` when the format's declarations are read directly rather than from a syntax tree.
    pub const fn is_declarative(self) -> bool {
        !self.is_tree_sitter()
    }

    /// `true` when the line-count ceiling applies to this format.
    ///
    /// Documentation and `.env` key files are not code: a long README is not a monolith to
    /// decompose, and counting its lines would flag exactly the files the ceiling exists to keep
    /// *out* of the source tree.
    pub const fn is_line_counted(self) -> bool {
        !matches!(self, PolyglotLanguage::Markdown | PolyglotLanguage::Env)
    }

    /// The `ast_indexer` variant a record of this format is filed under.
    ///
    /// `None` for a declarative format: nothing in `miter-ast` reads a stylesheet.
    pub const fn index_language(self) -> Option<miter_ast::Language> {
        match self {
            PolyglotLanguage::Rust => Some(miter_ast::Language::Rust),
            PolyglotLanguage::TypeScript => Some(miter_ast::Language::TypeScript),
            PolyglotLanguage::Tsx => Some(miter_ast::Language::Tsx),
            PolyglotLanguage::JavaScript => Some(miter_ast::Language::JavaScript),
            PolyglotLanguage::Python => Some(miter_ast::Language::Python),
            _ => None,
        }
    }

    /// Infers the format from a bare extension (case-insensitive).
    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Self> {
        let lowered = extension.to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|format| format.extensions().contains(&lowered.as_str()))
    }

    /// The format a path names, or `None` when it names none of them.
    ///
    /// The **filename is checked before the extension**, because three of the formats have no
    /// usable extension at all: `Dockerfile` has none, `.env.local` has an extension of `local`,
    /// and `api.dockerfile` would otherwise look like a file of type `dockerfile`. One function
    /// answers "what is this file?" for the walker, the audit, the dirty pass and the dispatcher,
    /// so none of them can drift from the others.
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name().and_then(|name| name.to_str())?;
        if let Some(format) = Self::from_file_name(name) {
            return Some(format);
        }
        Self::from_extension(path.extension()?.to_str()?)
    }

    /// The format a bare file *name* names, for the formats an extension cannot carry.
    fn from_file_name(name: &str) -> Option<Self> {
        let lowered = name.to_ascii_lowercase();
        if lowered == ".env" || lowered.starts_with(".env.") {
            return Some(PolyglotLanguage::Env);
        }
        if lowered == "dockerfile"
            || lowered == "containerfile"
            || lowered.starts_with("dockerfile.")
            || lowered.starts_with("containerfile.")
        {
            return Some(PolyglotLanguage::Dockerfile);
        }
        // A `.d.ts` file is TypeScript, not a file whose extension is `ts` in a directory
        // called `d` — the name is checked before the extension.
        if lowered.ends_with(".d.ts") || lowered.ends_with(".d.mts") {
            return Some(PolyglotLanguage::TypeScript);
        }
        None
    }

    /// The universal reading of an `ast_indexer` language, whose walker discovers a tree's files.
    pub const fn from_index(language: miter_ast::Language) -> Self {
        match language {
            miter_ast::Language::Rust => PolyglotLanguage::Rust,
            miter_ast::Language::TypeScript => PolyglotLanguage::TypeScript,
            miter_ast::Language::Tsx => PolyglotLanguage::Tsx,
            miter_ast::Language::JavaScript => PolyglotLanguage::JavaScript,
            miter_ast::Language::Python => PolyglotLanguage::Python,
        }
    }
}

/// Every extension the walker, the audit and the CLI have to agree on.
pub const EXTENSIONS: &[&str] = &[
    "rs",
    "ts",
    "mts",
    "cts",
    "tsx",
    "js",
    "jsx",
    "mjs",
    "cjs",
    "py",
    "pyi",
    "sql",
    "html",
    "htm",
    "css",
    "scss",
    "md",
    "markdown",
    "yaml",
    "yml",
    "tf",
    "tofu",
    "dockerfile",
];
