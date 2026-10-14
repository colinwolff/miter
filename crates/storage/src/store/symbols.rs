//! The `symbols` table: row counts, the id list, and removing one row.

use redb::{ReadableDatabase, ReadableTable};

use crate::error::StorageResult;
use crate::tables::TABLE_SYMBOLS;

use super::WorkspaceStore;

impl WorkspaceStore {
    /// How many encoded declarations the file holds.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the table cannot be read.
    pub fn symbol_count(&self) -> StorageResult<usize> {
        self.count(TABLE_SYMBOLS)
    }

    /// The ids of every encoded declaration, sorted.
    ///
    /// The *key* of the `symbols` table is the id, so a tool can enumerate what a workspace holds —
    /// and, together with [`WorkspaceStore::remove_symbol_row`], drop what it no longer wants —
    /// without decoding a single record. Decoding is the engine's business; deciding *which* rows
    /// are orphans is a policy the engine owns, and this is the half both halves need.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the table cannot be read.
    pub fn symbol_ids(&self) -> StorageResult<Vec<u64>> {
        let transaction = self.database.begin_read()?;
        let mut ids = Vec::new();
        match transaction.open_table(TABLE_SYMBOLS) {
            Ok(table) => {
                for entry in table.iter()? {
                    let (key, _value) = entry?;
                    ids.push(key.value());
                }
            }
            Err(redb::TableError::TableDoesNotExist(_)) => {}
            Err(error) => return Err(error.into()),
        }
        ids.sort_unstable();
        Ok(ids)
    }

    /// Removes the encoded declaration `id`, reporting whether one was there.
    ///
    /// The one write the *sleep cycle* makes to this table: a pruning pass deletes the records of
    /// declarations whose source file is gone. Nothing else removes a symbol row — an
    /// indexing pass replaces the whole table through `miter_engine::persist`, which is a different
    /// operation with a different reason.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the row cannot be removed.
    pub fn remove_symbol_row(&self, id: u64) -> StorageResult<bool> {
        let transaction = self.database.begin_write()?;
        let removed = {
            let mut table = transaction.open_table(TABLE_SYMBOLS)?;
            table.remove(id)?.is_some()
        };
        transaction.commit()?;
        Ok(removed)
    }
}
