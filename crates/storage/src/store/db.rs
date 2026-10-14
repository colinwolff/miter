//! The `redb` handle: opening a file, and the lock sidecar `redb` keeps beside it.

use std::path::{Path, PathBuf};

use redb::Database;

use crate::error::{StorageError, StorageResult};
use crate::tables::HASH_BYTES;

/// Reads a stored digest, rejecting a row that is not [`HASH_BYTES`] long.
pub(super) fn digest(bytes: &[u8]) -> StorageResult<[u8; HASH_BYTES]> {
    bytes.try_into().map_err(|_| StorageError::Malformed {
        expected: "a 32-byte SHA-256 digest",
        found: format!("{} bytes", bytes.len()),
    })
}

/// Opens or creates a database, reporting a *lock conflict* as [`StorageError::Busy`].
///
/// `redb` reports both "another process holds this file" and "this process already holds it" as
/// [`redb::DatabaseError::DatabaseAlreadyOpen`], and that distinction is the one this crate has to
/// keep: every other open failure is damage that the recovery path may rebuild, while a lock
/// conflict is a database *in use* that nothing may touch.
pub(crate) fn open_database(path: &Path, create: bool) -> StorageResult<Database> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| StorageError::Io {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    let opened = if create {
        Database::create(path)
    } else {
        Database::open(path)
    };
    opened.map_err(|error| match error {
        redb::DatabaseError::DatabaseAlreadyOpen => StorageError::Busy {
            path: path.to_path_buf(),
        },
        other => StorageError::Redb(other.to_string()),
    })
}

/// The lock sidecar's path: `<database>.lock`.
pub(super) fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}
