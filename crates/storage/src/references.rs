//! Operations on [`crate::schema::REFERENCES_TABLE`].
//!
//! Provides reverse-lookup mapping from target symbol identifiers to cross-file call sites,
//! type usages, and exact AST coordinates.

use std::collections::{HashMap, HashSet};

use redb::{ReadableDatabase, ReadableTable};

use crate::error::{StorageError, StorageResult};
use crate::schema::{REFERENCES_TABLE, ReferenceEntry};
use crate::store::WorkspaceStore;

impl WorkspaceStore {
    /// Retrieves all reference entries pointing to `symbol`.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if redb transaction fails or deserialize fails.
    pub fn get_references_for_symbol(&self, symbol: &str) -> StorageResult<Vec<ReferenceEntry>> {
        let tx = self.database().begin_read()?;
        let table = match tx.open_table(REFERENCES_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let Some(value) = table.get(symbol)? else {
            return Ok(Vec::new());
        };
        let entries: Vec<ReferenceEntry> =
            serde_json::from_slice(value.value()).map_err(|e| StorageError::Malformed {
                expected: "valid Vec<ReferenceEntry> JSON",
                found: e.to_string(),
            })?;
        Ok(entries)
    }

    /// Replaces all reference entries in the workspace database with the given iterator.
    ///
    /// Clears any existing table entries, groups incoming references by symbol,
    /// and commits all entries in a single write transaction.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if serialization or transaction fails.
    pub fn replace_all_references<I>(&self, refs: I) -> StorageResult<()>
    where
        I: IntoIterator<Item = (String, ReferenceEntry)>,
    {
        let mut grouped: HashMap<String, Vec<ReferenceEntry>> = HashMap::new();
        for (sym, entry) in refs {
            grouped.entry(sym).or_default().push(entry);
        }

        let tx = self.database().begin_write()?;
        {
            let _ = tx.delete_table(REFERENCES_TABLE);
            let mut table = tx.open_table(REFERENCES_TABLE)?;
            for (sym, mut entries) in grouped {
                entries.dedup();
                let bytes = serde_json::to_vec(&entries).map_err(|e| StorageError::Malformed {
                    expected: "serializable Vec<ReferenceEntry>",
                    found: e.to_string(),
                })?;
                table.insert(sym.as_str(), bytes.as_slice())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Inserts or merges a batch of reference entries in a single write transaction.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if serialization or transaction fails.
    pub fn insert_references_batch<I>(&self, refs: I) -> StorageResult<()>
    where
        I: IntoIterator<Item = (String, ReferenceEntry)>,
    {
        let mut grouped: HashMap<String, Vec<ReferenceEntry>> = HashMap::new();
        for (sym, entry) in refs {
            grouped.entry(sym).or_default().push(entry);
        }
        if grouped.is_empty() {
            return Ok(());
        }

        let tx = self.database().begin_write()?;
        {
            let mut table = tx.open_table(REFERENCES_TABLE)?;
            for (sym, new_entries) in grouped {
                let mut existing: Vec<ReferenceEntry> =
                    if let Some(val) = table.get(sym.as_str())? {
                        serde_json::from_slice(val.value()).unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                existing.extend(new_entries);
                existing.dedup();
                let bytes = serde_json::to_vec(&existing).map_err(|e| StorageError::Malformed {
                    expected: "serializable Vec<ReferenceEntry>",
                    found: e.to_string(),
                })?;
                table.insert(sym.as_str(), bytes.as_slice())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Inserts or appends reference entries for `file_path`.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if serialization or transaction fails.
    pub fn insert_references(
        &self,
        _file_path: &str,
        refs: Vec<(String, ReferenceEntry)>,
    ) -> StorageResult<()> {
        self.insert_references_batch(refs)
    }

    /// Clears all existing reference entries associated with `file_path`.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if transaction or update fails.
    pub fn clear_references_for_file(&self, file_path: &str) -> StorageResult<usize> {
        let tx = self.database().begin_write()?;
        let mut removed_count = 0;
        {
            let mut table = match tx.open_table(REFERENCES_TABLE) {
                Ok(t) => t,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(0),
                Err(e) => return Err(e.into()),
            };

            // Identify keys that have references in file_path
            let mut keys_to_update = Vec::new();
            let mut keys_to_delete = Vec::new();

            // We iterate over all entries in the references table
            for item_res in table.iter()? {
                let (key, val) = item_res?;
                let key_str = key.value().to_string();
                if let Ok(mut entries) = serde_json::from_slice::<Vec<ReferenceEntry>>(val.value())
                {
                    let before_len = entries.len();
                    entries.retain(|e| e.file_path != file_path);
                    let diff = before_len - entries.len();
                    if diff > 0 {
                        removed_count += diff;
                        if entries.is_empty() {
                            keys_to_delete.push(key_str);
                        } else {
                            keys_to_update.push((key_str, entries));
                        }
                    }
                }
            }

            for key in keys_to_delete {
                table.remove(key.as_str())?;
            }
            for (key, entries) in keys_to_update {
                let bytes = serde_json::to_vec(&entries).map_err(|e| StorageError::Malformed {
                    expected: "serializable Vec<ReferenceEntry>",
                    found: e.to_string(),
                })?;
                table.insert(key.as_str(), bytes.as_slice())?;
            }
        }
        tx.commit()?;
        Ok(removed_count)
    }

    /// Lists all unique symbol keys currently tracked in the references table.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if read transaction fails.
    pub fn list_referenced_symbols(&self) -> StorageResult<HashSet<String>> {
        let tx = self.database().begin_read()?;
        let table = match tx.open_table(REFERENCES_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(HashSet::new()),
            Err(e) => return Err(e.into()),
        };
        let mut symbols = HashSet::new();
        for item_res in table.iter()? {
            let (key, _) = item_res?;
            symbols.insert(key.value().to_string());
        }
        Ok(symbols)
    }
}
