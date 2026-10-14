//! Opening, generation stamping and recovery: whether a file may be read at all.

use std::path::Path;

use redb::{ReadableDatabase, ReadableTable};

use crate::error::{StorageError, StorageResult};
use crate::tables::{
    ENGINE_STORAGE_VERSION, META_HEADER_KEY, META_SCHEMA_VERSION_FIELD, META_SCHEMA_VERSION_KEY,
    TABLE_META,
};

use super::db::lock_path;
use super::{Verdict, WorkspaceStore};

impl WorkspaceStore {
    /// Opens the workspace at `path`, rebuilding it when this build cannot read it.
    ///
    /// Three outcomes, all explicit:
    ///
    /// * **The file is current** — it opens, and its schema stamp is written if it predates this
    ///   crate (the fallback path in [`WorkspaceStore::schema_version`]).
    /// * **The generation differs, or the file is unreadable** — a warning names the reason and
    ///   the path, the old file is deleted with its lock sidecar, and a fresh one is created.
    /// * **There is no file** — a fresh one is created, with no warning: the first run of anything
    ///   is not a recovery.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the purge or the recreation fails. A *readable* file that this build
    /// can only purge is never an error — it is a warning.
    pub fn open_or_recreate(path: impl AsRef<Path>) -> StorageResult<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Self::create(path);
        }

        let (verdict, usable) = Self::classify(path);
        if let Some(store) = usable {
            return Ok(store);
        }

        match verdict {
            Verdict::Stale(found) => tracing::warn!(
                path = %path.display(),
                found = found.map_or_else(|| "none".to_owned(), |value| value.to_string()),
                expected = ENGINE_STORAGE_VERSION,
                "workspace database is from another schema generation: purging and re-initializing"
            ),
            Verdict::Unreadable(reason) => tracing::warn!(
                path = %path.display(),
                reason = %reason,
                "workspace database could not be read: purging and re-initializing"
            ),
            // A database in use is not damage, and deleting it would be the worst possible
            // recovery: the caller is told, and the file is left exactly as it is.
            Verdict::Busy => {
                return Err(StorageError::Busy {
                    path: path.to_path_buf(),
                });
            }
            Verdict::Current => unreachable!("a current file is returned as a usable store"),
        }
        Self::create(path)
    }

    /// The schema generation of the database at `path`, without touching it.
    ///
    /// This is the *refusing* form of the handshake: a caller that wants to be told, rather than
    /// recovering, asks this first. A file that reports no generation at all is a mismatch too —
    /// "unversioned" is not a generation this build writes.
    ///
    /// # Errors
    ///
    /// [`StorageError::SchemaMismatch`] when the file is not this build's generation, and
    /// [`StorageError::Redb`] when it cannot be read as a database.
    pub fn inspect(path: impl AsRef<Path>) -> StorageResult<u32> {
        let store = Self::open(path)?;
        match store.schema_version()? {
            Some(version) if version == ENGINE_STORAGE_VERSION => Ok(version),
            found => Err(StorageError::SchemaMismatch {
                expected: ENGINE_STORAGE_VERSION,
                found,
            }),
        }
    }

    /// Deletes the database at `path` and the lock sidecar `redb` keeps beside it.
    ///
    /// A missing file is not an error: purging something that is not there has already succeeded.
    ///
    /// # Errors
    ///
    /// [`StorageError::Io`] when a file exists and cannot be removed.
    pub fn purge(path: impl AsRef<Path>) -> StorageResult<()> {
        let path = path.as_ref();
        for candidate in [path.to_path_buf(), lock_path(path)] {
            match std::fs::remove_file(&candidate) {
                Ok(()) => tracing::debug!(path = %candidate.display(), "purged"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(StorageError::Io {
                        path: candidate,
                        message: error.to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Creates the storage-owned tables and stamps the file with this build's generation.
    ///
    /// Deliberately *not* every table: `symbols` and `universal_symbols_v1` are created by the
    /// index writer, and creating them here would make a brand-new file look like one written by
    /// a full index pass. [`WorkspaceStore::ensure_tables`] creates everything, for a tool that
    /// wants the full shape.
    pub fn schema_version_bytes(&self) -> StorageResult<Option<[u8; 4]>> {
        let transaction = self.database.begin_read()?;
        let table = transaction.open_table(TABLE_META)?;
        let bytes = table
            .get(&[META_SCHEMA_VERSION_KEY][..])?
            .map(|row| row.value().to_vec());
        match bytes {
            None => Ok(None),
            Some(bytes) => {
                let found: [u8; 4] =
                    bytes
                        .as_slice()
                        .try_into()
                        .map_err(|_| StorageError::Malformed {
                            expected: "a four-byte schema version",
                            found: format!("{} bytes", bytes.len()),
                        })?;
                Ok(Some(found))
            }
        }
    }

    /// The schema generation the file reports.
    ///
    /// The stamp row is read first; a file that predates it is read through its JSON header, which
    /// is the only other place the generation has ever been written. `None` means the file reports
    /// no generation at all, which [`WorkspaceStore::open_or_recreate`] treats as a purge.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the tables cannot be read or the header row is not the JSON object
    /// the layout requires.
    pub fn schema_version(&self) -> StorageResult<Option<u32>> {
        if let Some(bytes) = self.schema_version_bytes()? {
            return Ok(Some(u32::from_le_bytes(bytes)));
        }
        let transaction = self.database.begin_read()?;
        let table = transaction.open_table(TABLE_META)?;
        let header = table
            .get(&[META_HEADER_KEY][..])?
            .map(|row| row.value().to_vec());
        let Some(header) = header else {
            return Ok(None);
        };
        let document: serde_json::Value =
            serde_json::from_slice(&header).map_err(|error| StorageError::Malformed {
                expected: "a JSON header document in the meta table",
                found: error.to_string(),
            })?;
        Ok(document
            .get(META_SCHEMA_VERSION_FIELD)
            .and_then(serde_json::Value::as_u64)
            .map(|value| value as u32))
    }

    /// Writes the store's own stamp when it is missing, so a file from before this crate is
    /// stamped on first open and never re-derived.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the row cannot be written.
    pub fn stamp_schema_version(&self) -> StorageResult<u32> {
        let transaction = self.database.begin_write()?;
        {
            let mut table = transaction.open_table(TABLE_META)?;
            let stamped = table.get(&[META_SCHEMA_VERSION_KEY][..])?.is_some();
            if !stamped {
                table.insert(
                    &[META_SCHEMA_VERSION_KEY][..],
                    ENGINE_STORAGE_VERSION.to_le_bytes().as_slice(),
                )?;
            }
        }
        transaction.commit()?;
        Ok(ENGINE_STORAGE_VERSION)
    }

    /// How many encoded declarations the file holds.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the table cannot be read.
    pub(super) fn classify(path: &Path) -> (Verdict, Option<Self>) {
        match Self::open(path) {
            Ok(store) => match store.schema_version() {
                Ok(Some(version)) if version == ENGINE_STORAGE_VERSION => {
                    (Verdict::Current, Some(store))
                }
                Ok(found) => (Verdict::Stale(found), None),
                Err(error) => (Verdict::Unreadable(error.to_string()), None),
            },
            Err(StorageError::Busy { .. }) => (Verdict::Busy, None),
            Err(error) => (Verdict::Unreadable(error.to_string()), None),
        }
    }
}
