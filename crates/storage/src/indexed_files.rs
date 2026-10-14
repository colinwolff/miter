//! Operations on [`crate::schema::INDEXED_FILES_TABLE`].
//!
//! Tracks file modification times and SHA-256 digests to support sub-10ms dirty checks.

use crate::error::{StorageError, StorageResult};
use crate::schema::{INDEXED_FILES_TABLE, IndexedFileRecord};
use crate::store::WorkspaceStore;
use redb::{ReadableDatabase, ReadableTable};

impl WorkspaceStore {
    /// Retrieves the indexed record for a relative path.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if redb transaction fails or deserialize fails.
    pub fn get_indexed_file(&self, relative: &str) -> StorageResult<Option<IndexedFileRecord>> {
        let tx = self.database().begin_read()?;
        let table = match tx.open_table(INDEXED_FILES_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let Some(value) = table.get(relative)? else {
            return Ok(None);
        };
        let record: IndexedFileRecord =
            serde_json::from_slice(value.value()).map_err(|e| StorageError::Malformed {
                expected: "valid IndexedFileRecord JSON",
                found: e.to_string(),
            })?;
        Ok(Some(record))
    }

    /// Stores the indexed record for a relative path.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if serialization or transaction fails.
    pub fn set_indexed_file(
        &self,
        relative: &str,
        record: &IndexedFileRecord,
    ) -> StorageResult<()> {
        let bytes = serde_json::to_vec(record).map_err(|e| StorageError::Malformed {
            expected: "serializable IndexedFileRecord",
            found: e.to_string(),
        })?;
        let tx = self.database().begin_write()?;
        {
            let mut table = tx.open_table(INDEXED_FILES_TABLE)?;
            table.insert(relative, bytes.as_slice())?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Removes an indexed record for a relative path.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if redb operation fails.
    pub fn remove_indexed_file(&self, relative: &str) -> StorageResult<bool> {
        let tx = self.database().begin_write()?;
        let removed = {
            let mut table = match tx.open_table(INDEXED_FILES_TABLE) {
                Ok(t) => t,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(false),
                Err(e) => return Err(e.into()),
            };
            table.remove(relative)?.is_some()
        };
        tx.commit()?;
        Ok(removed)
    }

    /// Lists all currently indexed file relative paths and their records.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if table scan fails.
    pub fn list_indexed_files(&self) -> StorageResult<Vec<(String, IndexedFileRecord)>> {
        let tx = self.database().begin_read()?;
        let table = match tx.open_table(INDEXED_FILES_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut list = Vec::new();
        for entry in table.iter()? {
            let (k, v) = entry?;
            let record: IndexedFileRecord =
                serde_json::from_slice(v.value()).map_err(|e| StorageError::Malformed {
                    expected: "valid IndexedFileRecord JSON",
                    found: e.to_string(),
                })?;
            list.push((k.value().to_string(), record));
        }
        list.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(list)
    }
}
