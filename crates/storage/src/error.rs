//! Failures a workspace database raises.

use std::path::PathBuf;

/// Result alias for the storage layer.
pub type StorageResult<T> = core::result::Result<T, StorageError>;

/// What went wrong while reading or writing a workspace database.
#[derive(Debug)]
pub enum StorageError {
    /// `redb` refused the operation (open, transaction, table, commit).
    Redb(String),
    /// The filesystem refused the operation (create, purge, canonicalize).
    Io {
        /// The path the operation was aimed at.
        path: PathBuf,
        /// The underlying message.
        message: String,
    },
    /// The database is already open (by this process or another), so no second handle can be taken.
    ///
    /// Reported rather than recovered from: an open database is *in use*, and a recovery that
    /// treated a lock conflict as damage would delete a workspace somebody is working in.
    Busy {
        /// The database that is held.
        path: PathBuf,
    },
    /// A stored document did not decode, or was not the shape the layout requires.
    Malformed {
        /// What was expected, in a sentence a human can act on.
        expected: &'static str,
        /// What was found.
        found: String,
    },
    /// The file reports a schema generation this build does not write.
    ///
    /// Only [`crate::WorkspaceStore::inspect`] raises this: [`crate::WorkspaceStore::open_or_recreate`]
    /// purges the file instead, and the purge is reported as a warning rather than as an error.
    SchemaMismatch {
        /// Generation this build writes ([`crate::tables::ENGINE_STORAGE_VERSION`]).
        expected: u32,
        /// Generation the file reports; `None` when it reports none at all.
        found: Option<u32>,
    },
}

impl core::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StorageError::Redb(message) => write!(formatter, "storage error: {message}"),
            StorageError::Io { path, message } => {
                write!(
                    formatter,
                    "storage i/o error at {}: {message}",
                    path.display()
                )
            }
            StorageError::Busy { path } => write!(
                formatter,
                "the database at {} is already open: close the process that holds it, or point \
                 at another path",
                path.display()
            ),
            StorageError::Malformed { expected, found } => {
                write!(
                    formatter,
                    "storage record error: expected {expected}, found {found}"
                )
            }
            StorageError::SchemaMismatch { expected, found } => match found {
                Some(found) => write!(
                    formatter,
                    "storage schema mismatch: the file is generation {found}, this build writes {expected}"
                ),
                None => write!(
                    formatter,
                    "storage schema mismatch: the file reports no generation, this build writes {expected}"
                ),
            },
        }
    }
}

impl std::error::Error for StorageError {}

impl From<redb::DatabaseError> for StorageError {
    fn from(error: redb::DatabaseError) -> Self {
        StorageError::Redb(error.to_string())
    }
}

impl From<redb::TransactionError> for StorageError {
    fn from(error: redb::TransactionError) -> Self {
        StorageError::Redb(error.to_string())
    }
}

impl From<redb::TableError> for StorageError {
    fn from(error: redb::TableError) -> Self {
        StorageError::Redb(error.to_string())
    }
}

impl From<redb::StorageError> for StorageError {
    fn from(error: redb::StorageError) -> Self {
        StorageError::Redb(error.to_string())
    }
}

impl From<redb::CommitError> for StorageError {
    fn from(error: redb::CommitError) -> Self {
        StorageError::Redb(error.to_string())
    }
}
