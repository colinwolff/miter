//! `redb` backend: one table of `id → encoded record`.

use std::path::{Path, PathBuf};

use miter_ast::FunctionSymbol;
use redb::{Database, ReadableDatabase, ReadableTable};

use crate::Engine;
use crate::codec::{PersistError, decode_symbol, encode_symbol, encode_universal};

/// Table holding the records of a persisted index.
///
/// The definition is `storage`'s ([`miter_storage::tables::TABLE_SYMBOLS`], the `symbols` table) so that
/// a tool can count a workspace's declarations without linking this crate's query pipeline.
pub use miter_storage::tables::TABLE_SYMBOLS as RECORDS;

/// The polyglot declarations, in their own table beside [`RECORDS`].
pub use miter_storage::tables::TABLE_UNIVERSAL as UNIVERSAL;

/// Writes every symbol of `engine` to a fresh `redb` database at `path`.
///
/// The table is **replaced**, not merged: re-indexing a tree that has shrunk must not leave
/// records for symbols that no longer exist.
///
/// # Errors
///
/// [`PersistError`] when the database cannot be created, written or committed.
pub fn save_redb(engine: &Engine, path: &Path) -> Result<(), PersistError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(PersistError::Io)?;
    }
    // `redb`'s `create` opens an existing file rather than truncating it, so a database written
    // by an older record layout would keep its rows beside the new ones and a reader would meet a
    // record it cannot decode. The file is derived data — every row is rebuilt from the source
    // tree — so it is purged rather than migrated.
    let _ = std::fs::remove_file(path);
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    let _ = std::fs::remove_file(PathBuf::from(lock));

    let database = Database::create(path).map_err(|error| PersistError::Redb(error.to_string()))?;
    replace_records(&database, engine)
}

/// Writes every symbol of `engine` through an *already open* workspace database.
///
/// The path-based [`save_redb`] cannot be used by a caller that already holds the file — `redb`
/// refuses a second handle — so an incremental pass writes through the handle it already has.
///
/// # Errors
///
/// [`PersistError`] when the table cannot be replaced or the transaction cannot be committed.
pub fn save_redb_into(
    store: &miter_storage::WorkspaceStore,
    engine: &Engine,
) -> Result<(), PersistError> {
    replace_records(store.database(), engine)
}

/// The shared body of the two writers.
///
/// Both tables are **replaced**, not merged: re-indexing a tree that has shrunk must not leave
/// records for symbols that no longer exist.
fn replace_records(database: &Database, engine: &Engine) -> Result<(), PersistError> {
    let transaction = database
        .begin_write()
        .map_err(|error| PersistError::Redb(error.to_string()))?;
    {
        // Drop any previous contents first (the file may already hold an older index).
        transaction
            .delete_table(RECORDS)
            .map_err(|error| PersistError::Redb(error.to_string()))?;
        let mut table = transaction
            .open_table(RECORDS)
            .map_err(|error| PersistError::Redb(error.to_string()))?;
        for symbol in engine.symbols() {
            let record = encode_symbol(symbol);
            table
                .insert(symbol.id, record.as_slice())
                .map_err(|error| PersistError::Redb(error.to_string()))?;
        }
    }
    // The universal half, in its own table. It is written *unconditionally* rather than only when
    // non-empty: an index that parsed nothing polyglot should hold an empty table, not the table
    // of a previous run, and deleting first is what makes that true.
    {
        transaction
            .delete_table(UNIVERSAL)
            .map_err(|error| PersistError::Redb(error.to_string()))?;
        let mut table = transaction
            .open_table(UNIVERSAL)
            .map_err(|error| PersistError::Redb(error.to_string()))?;
        for symbol in engine.universal_symbols() {
            let record = encode_universal(symbol);
            table
                .insert(symbol.id(), record.as_slice())
                .map_err(|error| PersistError::Redb(error.to_string()))?;
        }
    }
    transaction
        .commit()
        .map_err(|error| PersistError::Redb(error.to_string()))?;
    Ok(())
}

/// Reads every record of a `redb` database.
///
/// # Errors
///
/// [`PersistError`] when the database cannot be opened or read, or a record does
/// not decode.
pub fn load_redb(path: &Path) -> Result<Vec<FunctionSymbol>, PersistError> {
    let database = Database::open(path).map_err(|error| PersistError::Redb(error.to_string()))?;
    read_records(&database)
}

/// Reads both halves of a `redb` index through **one read-only handle**, for a caller that writes
/// nothing.
///
/// Two problems are solved at once. `redb` is single-writer and a plain `Database::open` takes an
/// *exclusive* lock even when the caller only reads, so two `miter lookup` invocations racing each
/// other could not both succeed — the second failed with "Database already open" and the exit code
/// was indistinguishable from a real failure. `ReadOnlyDatabase` takes a *shared* lock, so readers
/// coexist. Opening the file once for both tables also removes a second lock acquisition, which
/// could independently fail against a writer.
///
/// What this does **not** fix, and callers must not assume: a read-only handle still cannot open
/// while a *writer* holds the file. On macOS the OS file lock admits one writer, so a read against
/// a workspace another process is writing still reports contention. That is a property of `redb`,
/// not of this function, and the honest answer is the one the CLI gives: say which process holds it.
///
#[deprecated(note = "use WorkspaceStore and load_redb_from / load_redb_universal_from instead")]
pub fn load_redb_index_read_only(
    path: &Path,
) -> Result<(Vec<FunctionSymbol>, Vec<crate::polyglot::UniversalSymbol>), PersistError> {
    let store = miter_storage::WorkspaceStore::open(path)
        .map_err(|error| PersistError::Redb(error.to_string()))?;
    Ok((
        read_records(store.database())?,
        read_universal(store.database())?,
    ))
}

/// Reads every record through an *already open* workspace database.
///
/// # Errors
///
/// [`PersistError`] when the table cannot be read or a record does not decode.
pub fn load_redb_from(
    store: &miter_storage::WorkspaceStore,
) -> Result<Vec<FunctionSymbol>, PersistError> {
    read_records(store.database())
}

/// What [`read_records`] and [`read_universal`] need from a database.
///
/// `redb` seals its own `ReadableDatabase`, so it cannot be used as a bound outside the crate —
/// hence this one-method trait. It exists so the read-only handle and the read-write handle share
/// a single decode loop: the two differ only in the lock they take, and duplicating the loop to
/// express that would be a second place for it to be wrong.
trait Readable {
    /// A consistent snapshot of the tables.
    fn snapshot(&self) -> Result<redb::ReadTransaction, PersistError>;
}

impl Readable for Database {
    fn snapshot(&self) -> Result<redb::ReadTransaction, PersistError> {
        self.begin_read()
            .map_err(|error| PersistError::Redb(error.to_string()))
    }
}

/// `WorkspaceStore` hands out its handle behind an `Arc`; the blanket impl keeps the shared store
/// (`load_redb_from`, and an incremental pass) on the same reader as a bare handle.
impl<T: Readable + ?Sized> Readable for std::sync::Arc<T> {
    fn snapshot(&self) -> Result<redb::ReadTransaction, PersistError> {
        (**self).snapshot()
    }
}

/// The shared body of the two readers.
fn read_records<D: Readable>(database: &D) -> Result<Vec<FunctionSymbol>, PersistError> {
    let transaction = database.snapshot()?;
    // A workspace that has never been indexed has no `symbols` table at all, which is an empty
    // index rather than an error.
    let table = match transaction.open_table(RECORDS) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(error) => return Err(PersistError::Redb(error.to_string())),
    };

    let mut symbols = Vec::new();
    for entry in table
        .iter()
        .map_err(|error| PersistError::Redb(error.to_string()))?
    {
        let (key, value) = entry.map_err(|error| PersistError::Redb(error.to_string()))?;
        let mut symbol = decode_symbol(value.value())?;
        // Trust the key over the payload for identity, and keep the ordering
        // deterministic.
        symbol.id = key.value();
        symbols.push(symbol);
    }
    symbols.sort_by_key(|symbol| symbol.id);
    Ok(symbols)
}

mod universal;

use universal::read_universal;

pub use universal::{load_redb_universal, load_redb_universal_from};
#[cfg(test)]
mod cases;
