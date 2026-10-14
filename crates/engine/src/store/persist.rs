//! Persistence wrappers: the whole index in one `redb` file, and back again.

use std::path::Path;

use super::Engine;

/// # Persistence
/// Thin wrappers over [`crate::persist`] so callers do not have to name the backend module.
impl Engine {
    /// Writes the whole index to a `redb` database (replacing its contents).
    ///
    /// # Errors
    ///
    /// [`crate::PersistError`] when the database cannot be written.
    pub fn save_redb(&self, path: &Path) -> Result<(), crate::PersistError> {
        crate::persist::save_redb(self, path)
    }

    /// Loads an index from a `redb` database.
    ///
    /// # Errors
    ///
    /// [`crate::PersistError`] when the database cannot be read or a record is
    /// malformed.
    pub fn open_redb(path: &Path) -> Result<Self, crate::PersistError> {
        if !path.exists() {
            return Err(crate::PersistError::Redb(
                "database file does not exist".into(),
            ));
        }
        let store = miter_storage::WorkspaceStore::open(path)
            .map_err(|e| crate::PersistError::Redb(e.to_string()))?;
        let records = crate::persist::load_redb_from(&store)?;
        let universal = crate::persist::load_redb_universal_from(&store)?;
        let mut engine = Self::from_symbols(records);
        engine.add_universal_symbols(universal);
        Ok(engine)
    }
}
