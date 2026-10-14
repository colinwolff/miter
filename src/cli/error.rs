//! The CLI's error type and its exit-code mapping.
//!
//! One error for the whole binary, because a subcommand's failure is the process's failure: every
//! recoverable condition the CLI can meet is a variant here, `Display` renders the sentence a user
//! reads, and `run` maps it to exit code 1. The conversions below are what let a `?` in any
//! subcommand reach it.

use std::path::PathBuf;

use miter_engine::{EngineError, PersistError};

/// Failures reported by the CLI, mapped to exit code 1.
#[derive(Debug)]
pub enum CliError {
    /// A database file could not be read or written.
    Persist(PersistError),
    /// The traversal or ingestion failed.
    Engine(EngineError),
    /// The index file does not exist yet.
    MissingIndex(PathBuf),
    /// The requested node id is not in the index.
    UnknownNode(u64),
    /// A subcommand was used without the arguments it needs.
    Usage(String),
    /// An I/O error occurred during file traversal or execution.
    Io(std::io::Error),
    /// A verification ran, and the tree was rejected.
    Rejected(String),
}

impl core::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CliError::Persist(PersistError::Redb(message))
                if message.contains("already open") || message.contains("lock") =>
            {
                write!(
                    formatter,
                    "the database is held by another process: {message}.\n\
                     Stop the holding process, or point --db at a different file."
                )
            }
            CliError::Persist(error) => write!(formatter, "{error}"),
            CliError::Engine(error) => write!(formatter, "{error}"),
            CliError::MissingIndex(path) => write!(
                formatter,
                "no index at {}: run `miter index <PATH> --db {}` first",
                path.display(),
                path.display()
            ),
            CliError::UnknownNode(id) => write!(formatter, "no symbol with id {id} in the index"),
            CliError::Usage(message) => write!(formatter, "{message}"),
            CliError::Io(error) => write!(formatter, "{error}"),
            CliError::Rejected(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CliError::Persist(error) => Some(error),
            CliError::Engine(error) => Some(error),
            CliError::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        CliError::Io(error)
    }
}

impl From<PersistError> for CliError {
    fn from(error: PersistError) -> Self {
        CliError::Persist(error)
    }
}

impl From<EngineError> for CliError {
    fn from(error: EngineError) -> Self {
        CliError::Engine(error)
    }
}
