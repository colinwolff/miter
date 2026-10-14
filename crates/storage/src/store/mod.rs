//! `WorkspaceStore` — the file-level owner of a workspace database.
//!
//! One handle, one `redb` file, five tables, and the schema handshake that decides whether the
//! file is readable or has to be rebuilt.
//!
//! ## Why the schema stamp lives in its own row
//!
//! A workspace database carries two statements about its generation, and they are deliberately
//! two *rows of the same table*:
//!
//! * [`crate::tables::META_HEADER_KEY`] (`0`) holds the engine's JSON header — the document that records the
//!   build, the git hash and the substrate dimensions. It has been written since the header
//!   existed, so nothing here may move it.
//! * [`crate::tables::META_SCHEMA_VERSION_KEY`] (`1`) holds the four little-endian bytes of
//!   [`crate::tables::ENGINE_STORAGE_VERSION`]. It is this crate's own stamp: a file written before the
//!   extraction has no such row, and [`WorkspaceStore::schema_version`] falls back to reading
//!   `schema_version` out of the JSON header for exactly those files.
//!
//! The two cannot disagree, because the engine's `SCHEMA_VERSION` *is* this crate's constant
//! re-exported (`miter_engine::codec::VERSION`), not a second copy of `2`.
//!
//! ## What auto-purge does and does not do
//!
//! [`WorkspaceStore::open_or_recreate`] purges a file it cannot read or whose generation is not
//! this build's, and starts a fresh one. That is a *recovery*, not a migration: records are not
//! translated, because a record of the wrong layout has no meaning to translate — reading it as
//! bytes would be the silent misinterpretation the whole handshake exists to prevent. It warns
//! with the reason, deletes the file, and says what it did. A caller that wants the refusal
//! instead of the recovery asks for [`WorkspaceStore::inspect`], which raises
//! [`crate::StorageError::SchemaMismatch`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use redb::{Database, ReadableDatabase, ReadableTableMetadata, TableHandle};

use crate::error::StorageResult;
use crate::tables::{
    TABLE_FILE_HASHES, TABLE_META, TABLE_SYMBOLS, TABLE_UNIVERSAL, WORKSPACE_TABLES,
};

/// A `redb` database read as a miter workspace.
///
/// `Clone` shares the handle rather than reopening the file (as `redb` holds an exclusive OS
/// lock), so two threads can share one handle and both see one set of tables.
#[derive(Debug, Clone)]
pub struct WorkspaceStore {
    database: Arc<Database>,
    path: Option<PathBuf>,
}

/// What a probe of an existing file found.
#[derive(Debug)]
enum Verdict {
    /// The file is this build's, and the handle is usable.
    Current,
    /// The file reports a different generation (or none at all).
    Stale(Option<u32>),
    /// The file could not be opened or read as a database at all.
    Unreadable(String),
    /// The file is held by another handle, so nothing may be done with it at all.
    Busy,
}

impl WorkspaceStore {
    /// Opens a workspace database, creating it when it is not there yet.
    ///
    /// No purge: a file whose generation is not this one still opens, so a caller that wants to
    /// change it *deliberately* (a migration tool, a test) can. [`WorkspaceStore::open_or_recreate`]
    /// is the recovering form.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the database cannot be opened or its tables cannot be created.
    pub fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        let path = path.as_ref();
        let database = if path.exists() {
            open_database(path, false)?
        } else {
            open_database(path, true)?
        };
        Self::initialize(Arc::new(database), Some(path.to_path_buf()))
    }

    /// Creates a workspace database at `path`, replacing whatever is there.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the file cannot be created.
    pub fn create(path: impl AsRef<Path>) -> StorageResult<Self> {
        let path = path.as_ref();
        Self::purge(path)?;
        let database = open_database(path, true)?;
        Self::initialize(Arc::new(database), Some(path.to_path_buf()))
    }

    /// A workspace that lives only for the lifetime of the process.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the in-memory database cannot be created.
    pub fn in_memory() -> StorageResult<Self> {
        let backend = redb::backends::InMemoryBackend::new();
        let database = Database::builder().create_with_backend(backend)?;
        Self::initialize(Arc::new(database), None)
    }

    /// A workspace view over a database handle another layer already owns.
    ///
    /// The two handles share one `redb` database (and therefore one file lock), which is what lets
    /// the engine's record layer — [`crate::WorkspaceStore`]'s own caller — read the tables this
    /// crate defines without opening the file a second time (an exclusive lock makes a second
    /// `Database::open` on the same file *fail*, not share).
    ///
    /// The tables this crate owns are created and the generation is stamped, exactly as
    /// [`WorkspaceStore::open`] does: the caller already holds the database, so the file is not
    /// touched twice.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the tables cannot be created or the stamp cannot be written.
    pub fn from_shared(database: Arc<Database>, path: Option<PathBuf>) -> StorageResult<Self> {
        Self::initialize(database, path)
    }

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
    pub(super) fn initialize(
        database: Arc<Database>,
        path: Option<PathBuf>,
    ) -> StorageResult<Self> {
        let store = Self { database, path };
        let transaction = store.database.begin_write()?;
        {
            transaction.open_table(TABLE_META)?;
            transaction.open_table(TABLE_FILE_HASHES)?;
        }
        transaction.commit()?;
        store.stamp_schema_version()?;
        Ok(store)
    }

    /// Opens every table this crate defines, creating the missing ones.
    ///
    /// A tool that wants to *inspect* a workspace calls this; the writers call it implicitly.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the tables cannot be created.
    pub fn ensure_tables(&self) -> StorageResult<()> {
        let transaction = self.database.begin_write()?;
        {
            transaction.open_table(TABLE_META)?;
            transaction.open_table(TABLE_SYMBOLS)?;
            transaction.open_table(TABLE_UNIVERSAL)?;
            transaction.open_table(TABLE_FILE_HASHES)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// The tables the file actually holds, sorted by name.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the transaction cannot be started or the table list cannot be read.
    pub fn table_names(&self) -> StorageResult<Vec<String>> {
        let transaction = self.database.begin_read()?;
        let mut names: Vec<String> = transaction
            .list_tables()?
            .map(|handle| handle.name().to_owned())
            .collect();
        names.sort();
        Ok(names)
    }

    /// The file the store lives in, when it is file backed.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The shared database handle, for a record layer (the engine) that owns its own tables.
    pub fn database(&self) -> &Arc<Database> {
        &self.database
    }

    /// The names of every table this crate defines.
    pub fn defined_tables() -> &'static [&'static str] {
        WORKSPACE_TABLES
    }

    /// The store's own stamp: `schema_version`, little-endian, in four bytes.
    ///
    /// # Errors
    ///
    /// [`crate::StorageError`] when the meta row cannot be read or is not four bytes long.
    pub(super) fn count<K>(
        &self,
        table: redb::TableDefinition<'static, K, &[u8]>,
    ) -> StorageResult<usize>
    where
        K: redb::Key + 'static,
    {
        let transaction = self.database.begin_read()?;
        // A table that is not there yet is empty, not an error: a workspace only grows the
        // tables it uses, and a count of nothing is zero.
        match transaction.open_table(table) {
            Ok(table) => Ok(table.len()? as usize),
            Err(redb::TableError::TableDoesNotExist(_)) => Ok(0),
            Err(error) => Err(error.into()),
        }
    }
}
mod db;
mod files;
mod schema;
mod symbols;

pub(crate) use db::open_database;
