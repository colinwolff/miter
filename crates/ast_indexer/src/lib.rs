//! # `miter-ast` — code ingestion into the geometric-symbolic schema
//!
//! Parses source files with `tree-sitter` and extracts **functions, their inputs and
//! their return types**, together with the provenance a caller needs to find them again:
//! the file, the one-based line, the byte span and the nesting depth.
//!
//! ## Example
//!
//! ```
//! use miter_ast::{AstIndexer, Language};
//! use std::path::Path;
//!
//! let source = r#"
//! fn scale(x: f32, factor: f32) -> f32 {
//!     x * factor
//! }
//! "#;
//!
//! let indexer = AstIndexer::new();
//! let symbols = indexer
//!     .index_source(source, Language::Rust, Path::new("src/math.rs"), 1)
//!     .expect("valid Rust");
//!
//! let symbol = &symbols[0];
//! assert_eq!(symbol.name, "scale");
//! assert_eq!(symbol.signature(), "scale(x: f32, factor: f32) -> f32");
//! assert_eq!(symbol.depth(), 1);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod discover;
pub mod extract;
pub mod progress;
pub mod symbol;

use std::path::PathBuf;

pub use discover::{IGNORED_DIRECTORIES, SourceFile, discover_sources};
pub use extract::{FunctionNode, Language, Param, grammar_for, parse_source, signature_of};
pub use progress::{INDEXING_TEMPLATE, IndexingProgress, SilentProgress, indexing_style};
pub use symbol::{AstIndexer, FunctionSymbol, IndexReport, fnv1a64, seed_from_text};

/// Failures raised while ingesting source code.
#[derive(Debug)]
pub enum IndexError {
    /// A file could not be read.
    Io(std::io::Error),
    /// `tree-sitter` returned no tree for the source.
    Parse {
        /// Language being parsed.
        language: Language,
        /// File the source came from, when known.
        path: Option<PathBuf>,
    },
    /// The grammar could not be loaded.
    Grammar {
        /// Language whose grammar failed to load.
        language: Language,
        /// Underlying loader message.
        message: String,
    },
    /// The language is known but its grammar is not available to this build.
    UnsupportedLanguage {
        /// The requested language.
        language: Language,
    },
}

impl core::fmt::Display for IndexError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            IndexError::Io(error) => write!(formatter, "i/o error while indexing: {error}"),
            IndexError::Parse { language, path } => match path {
                Some(path) => write!(
                    formatter,
                    "{} parser produced no tree for {}",
                    language.name(),
                    path.display()
                ),
                None => write!(formatter, "{} parser produced no tree", language.name()),
            },
            IndexError::Grammar { language, message } => write!(
                formatter,
                "failed to load the {} grammar: {message}",
                language.name()
            ),
            IndexError::UnsupportedLanguage { language } => write!(
                formatter,
                "the {} grammar is not available to this build",
                language.name()
            ),
        }
    }
}

impl std::error::Error for IndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IndexError::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for IndexError {
    fn from(error: std::io::Error) -> Self {
        IndexError::Io(error)
    }
}

/// Ingestion in one call: discover, parse and encode a source tree.
///
/// # Errors
///
/// Propagates traversal failures; per-file failures are reported inside the
/// [`IndexReport`].
pub fn index_root(root: &std::path::Path) -> Result<IndexReport, IndexError> {
    AstIndexer::new().index_dir(root)
}

/// Ingestion in one import.
pub mod prelude {
    pub use crate::IndexError;
    pub use crate::discover::{SourceFile, discover_sources};
    pub use crate::extract::{FunctionNode, Language, Param, parse_source, signature_of};
    pub use crate::symbol::{AstIndexer, FunctionSymbol, IndexReport};
}
