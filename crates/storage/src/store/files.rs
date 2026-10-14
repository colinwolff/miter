//! The file-hash ledger: which files an indexing pass has already seen.

use redb::{ReadableDatabase, ReadableTable};

use crate::error::StorageResult;
use crate::tables::{HASH_BYTES, TABLE_FILE_HASHES};

use super::WorkspaceStore;
use super::db::digest;

impl WorkspaceStore {
    /// How many files the incremental ledger remembers.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the table cannot be read.
    pub fn file_hash_count(&self) -> StorageResult<usize> {
        self.count(TABLE_FILE_HASHES)
    }

    /// The digest recorded for a workspace-relative path.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the row cannot be read or is not [`HASH_BYTES`] long.
    pub fn file_hash(&self, relative: &str) -> StorageResult<Option<[u8; HASH_BYTES]>> {
        let transaction = self.database.begin_read()?;
        let table = transaction.open_table(TABLE_FILE_HASHES)?;
        let bytes = table
            .get(relative.as_bytes())?
            .map(|row| row.value().to_vec());
        match bytes {
            None => Ok(None),
            Some(bytes) => digest(&bytes).map(Some),
        }
    }

    /// Records the digest of a workspace-relative path, replacing any earlier one.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the row cannot be written.
    pub fn set_file_hash(&self, relative: &str, digest: &[u8; HASH_BYTES]) -> StorageResult<()> {
        let transaction = self.database.begin_write()?;
        {
            let mut table = transaction.open_table(TABLE_FILE_HASHES)?;
            table.insert(relative.as_bytes(), digest.as_slice())?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Removes a ledger row, reporting whether one was there.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the row cannot be removed.
    pub fn remove_file_hash(&self, relative: &str) -> StorageResult<bool> {
        let transaction = self.database.begin_write()?;
        let removed = {
            let mut table = transaction.open_table(TABLE_FILE_HASHES)?;
            table.remove(relative.as_bytes())?.is_some()
        };
        transaction.commit()?;
        Ok(removed)
    }

    /// Every ledger row, sorted by path.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the table cannot be read or a row is not a digest.
    pub fn file_hashes(&self) -> StorageResult<Vec<(String, [u8; HASH_BYTES])>> {
        let transaction = self.database.begin_read()?;
        let table = transaction.open_table(TABLE_FILE_HASHES)?;
        let mut rows = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            let path = String::from_utf8_lossy(key.value()).into_owned();
            rows.push((path, digest(value.value())?));
        }
        rows.sort();
        Ok(rows)
    }
}
