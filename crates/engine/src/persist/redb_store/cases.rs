#[cfg(test)]
mod tests {
    use super::super::*;

    /// Two read-only handles over one file: the guarantee that makes concurrent CLI reads work.
    ///
    /// This is the regression this reader exists for. `Database::open` takes an exclusive lock even
    /// when the caller only reads, so two `miter lookup` runs racing each other used to produce one
    /// success and one `Database already open` — an exit code indistinguishable from a real
    /// failure. Two `ReadOnlyDatabase` handles take a *shared* lock and coexist.
    #[test]
    fn concurrent_reads_on_shared_workspace_store() {
        let path = std::env::temp_dir().join("miter_ro_shared_7c1e.redb");
        let _ = std::fs::remove_file(&path);
        let engine = crate::Engine::new();
        save_redb(&engine, &path).expect("an index is written");

        let store = miter_storage::WorkspaceStore::open(&path).expect("the store opens");
        let first = store
            .database()
            .begin_read()
            .expect("first read transaction");
        let second = store
            .database()
            .begin_read()
            .expect("second read transaction shares");
        drop(first);
        drop(second);
        let _ = std::fs::remove_file(&path);
    }

    /// The read-only loader returns both halves of an index, not just the records table.
    #[test]
    fn the_read_only_loader_reads_both_tables() {
        let path = std::env::temp_dir().join("miter_ro_both_5d92.redb");
        let _ = std::fs::remove_file(&path);
        let engine = crate::Engine::new();
        save_redb(&engine, &path).expect("an index is written");

        let store = miter_storage::WorkspaceStore::open(&path).expect("store opens");
        let records = load_redb_from(&store).expect("records read");
        let universal = load_redb_universal_from(&store).expect("universal read");
        assert!(records.is_empty(), "a fresh engine wrote no records");
        assert!(
            universal.is_empty(),
            "a fresh engine wrote no universal declarations"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A file that is not a database is still an error, not a panic and not an empty index.
    #[test]
    fn a_non_database_file_is_an_error_rather_than_an_empty_index() {
        let path = std::env::temp_dir().join("miter_ro_notadb_3a6f.redb");
        std::fs::write(&path, b"this is not a redb file").expect("the fixture is written");
        let error = miter_storage::WorkspaceStore::open(&path).expect_err("garbage is refused");
        assert!(
            matches!(error, miter_storage::StorageError::Redb(_)),
            "{error:?}"
        );
        let _ = std::fs::remove_file(&path);
    }
}
