//! What a polyglot parse can fail with.

use std::path::PathBuf;

use super::Language;

/// What a polyglot parse can fail with.
#[derive(Debug)]
pub enum PolyglotError {
    /// The grammar could not be loaded or accepted the parser.
    Grammar {
        /// Language whose grammar failed.
        language: Language,
        /// Underlying message.
        message: String,
    },
    /// `tree-sitter` returned no tree for the source.
    Parse {
        /// Language being parsed.
        language: Language,
        /// File the source came from, when the caller named one.
        path: Option<PathBuf>,
    },
    /// The language is known but its visitor is not compiled into this build.
    Unsupported {
        /// The requested language.
        language: Language,
    },
}

impl core::fmt::Display for PolyglotError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PolyglotError::Grammar { language, message } => write!(
                formatter,
                "failed to load the {} grammar: {message}",
                language.name()
            ),
            PolyglotError::Parse { language, path } => match path {
                Some(path) => write!(
                    formatter,
                    "the {} parser produced no tree for {}",
                    language.name(),
                    path.display()
                ),
                None => write!(formatter, "the {} parser produced no tree", language.name()),
            },
            PolyglotError::Unsupported { language } => write!(
                formatter,
                "no universal visitor for {} is compiled into this build",
                language.name()
            ),
        }
    }
}

impl std::error::Error for PolyglotError {}
