//! Reading the universal (polyglot) declarations of a persisted index.

use std::path::Path;

use redb::{Database, ReadableTable};

use crate::codec::{PersistError, decode_universal};

use super::{Readable, UNIVERSAL};

/// Reads the universal declarations of a `redb` database.
///
/// # Errors
///
/// [`PersistError`] when the database cannot be opened or read, or a record does not decode.
pub fn load_redb_universal(
    path: &Path,
) -> Result<Vec<crate::polyglot::UniversalSymbol>, PersistError> {
    let database = Database::open(path).map_err(|error| PersistError::Redb(error.to_string()))?;
    read_universal(&database)
}

/// Reads the universal declarations through an *already open* workspace database.
///
/// # Errors
///
/// [`PersistError`] when the table cannot be read or a record does not decode.
pub fn load_redb_universal_from(
    store: &miter_storage::WorkspaceStore,
) -> Result<Vec<crate::polyglot::UniversalSymbol>, PersistError> {
    read_universal(store.database())
}

/// The shared body of the two universal readers.
///
/// A workspace with no `universal_symbols_v1` table — one indexed before the universal table existed — reads as an
/// empty context rather than an error, the same reading [`read_records`] gives a workspace that
/// was never indexed. That is what lets an old database keep opening.
pub(super) fn read_universal<D: Readable>(
    database: &D,
) -> Result<Vec<crate::polyglot::UniversalSymbol>, PersistError> {
    let transaction = database.snapshot()?;
    let table = match transaction.open_table(UNIVERSAL) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
        Err(error) => return Err(PersistError::Redb(error.to_string())),
    };

    let mut symbols = Vec::new();
    for entry in table
        .iter()
        .map_err(|error| PersistError::Redb(error.to_string()))?
    {
        let (_key, value) = entry.map_err(|error| PersistError::Redb(error.to_string()))?;
        symbols.push(decode_universal(value.value())?);
    }
    // Ordered by id so two loads of the same file produce the same context — the order a
    // hydration walks its neighbours in must not depend on redb's iteration.
    symbols.sort_by_key(crate::polyglot::UniversalSymbol::id);
    Ok(symbols)
}
