//! The canonical table definitions: the names are the on-disk contract, and the layouts are the
//! ones `engine` has always written.
//!
//! The names are asserted as *literals* rather than through the constants they come from: a test
//! that read the constant would pass after a rename, and a rename is exactly the change that makes
//! every database in the wild unreadable.

use crate::support::TempDir;
use miter_storage::tables::{
    TABLE_FILE_HASHES_NAME, TABLE_META_NAME, TABLE_SYMBOLS_NAME, TABLE_UNIVERSAL_NAME,
    WORKSPACE_TABLES,
};
use miter_storage::{HASH_BYTES, WorkspaceStore};
use redb::ReadableDatabase;

#[test]
fn the_table_names_are_the_ones_this_engine_has_always_written() {
    assert_eq!(TABLE_META_NAME, "schema_metadata_v1");
    assert_eq!(TABLE_SYMBOLS_NAME, "symbols");
    assert_eq!(TABLE_FILE_HASHES_NAME, "file_hashes_v1");
    assert_eq!(TABLE_UNIVERSAL_NAME, "universal_symbols_v1");
    assert_eq!(
        WorkspaceStore::defined_tables(),
        WORKSPACE_TABLES,
        "one list, two accessors"
    );
    assert_eq!(
        WORKSPACE_TABLES.len(),
        4,
        "meta, symbols, universal and the file ledger"
    );
}

#[test]
fn a_fresh_workspace_is_stamped_and_holds_the_full_shape() {
    let dir = TempDir::new("shape");
    let path = dir.path.join("index.redb");
    let store = WorkspaceStore::create(&path).expect("create");
    assert_eq!(store.path(), Some(path.as_path()));

    // The storage-owned tables exist immediately …
    let names = store.table_names().expect("tables");
    assert!(names.contains(&TABLE_META_NAME.to_owned()), "{names:?}");
    assert!(
        names.contains(&TABLE_FILE_HASHES_NAME.to_owned()),
        "{names:?}"
    );
    assert_eq!(
        store.schema_version().expect("read"),
        Some(miter_storage::ENGINE_STORAGE_VERSION)
    );

    // … and the record tables appear when something asks for the full shape.
    store.ensure_tables().expect("shape");
    let names = store.table_names().expect("tables");
    for table in WORKSPACE_TABLES {
        assert!(names.contains(&(*table).to_owned()), "`{table}`: {names:?}");
    }

    // A table nothing has written is empty rather than absent.
    assert_eq!(store.symbol_count().expect("count"), 0);
    assert_eq!(store.file_hash_count().expect("count"), 0);
}

#[test]
fn the_ledger_is_keyed_by_relative_path_and_holds_whole_digests() {
    let store = WorkspaceStore::in_memory().expect("store");
    assert!(
        store
            .file_hash("crates/a/src/lib.rs")
            .expect("read")
            .is_none()
    );
    store
        .set_file_hash("crates/a/src/lib.rs", &[0xAB; HASH_BYTES])
        .expect("set");
    store
        .set_file_hash("b.rs", &[0x01; HASH_BYTES])
        .expect("set");
    assert_eq!(
        store.file_hashes().expect("rows"),
        vec![
            ("b.rs".to_owned(), [0x01; HASH_BYTES]),
            ("crates/a/src/lib.rs".to_owned(), [0xAB; HASH_BYTES]),
        ]
    );
    assert!(store.remove_file_hash("b.rs").expect("remove"));
    assert!(!store.remove_file_hash("b.rs").expect("remove again"));
}

#[test]
fn a_store_lives_in_memory_without_a_path() {
    let store = WorkspaceStore::in_memory().expect("store");
    assert!(store.path().is_none());
    assert_eq!(
        store.schema_version().expect("read"),
        Some(miter_storage::ENGINE_STORAGE_VERSION)
    );
}

#[test]
fn symbol_rows_are_enumerable_and_removable_by_id() {
    // The key of the `symbols` table is the id, so a pass can decide *which* rows are orphans from
    // the records and then drop them without the storage layer having to decode one. The two
    // accessors are the whole contract: what ids are there, and remove one.
    let store = WorkspaceStore::in_memory().expect("create");
    assert_eq!(store.symbol_count().expect("count"), 0);
    assert!(store.symbol_ids().expect("ids").is_empty());

    for id in [7u64, 3, 11] {
        let transaction = store.database().begin_write().expect("write");
        {
            let mut table = transaction
                .open_table(miter_storage::tables::TABLE_SYMBOLS)
                .expect("symbols");
            table
                .insert(id, b"an opaque record".as_slice())
                .expect("insert");
        }
        transaction.commit().expect("commit");
    }
    assert_eq!(store.symbol_count().expect("count"), 3);
    assert_eq!(store.symbol_ids().expect("ids"), vec![3, 7, 11]);

    assert!(store.remove_symbol_row(7).expect("remove"), "it was there");
    assert!(!store.remove_symbol_row(7).expect("remove again"));
    assert_eq!(store.symbol_ids().expect("ids"), vec![3, 11]);
    assert_eq!(store.symbol_count().expect("count"), 2);
}

#[test]
fn concurrent_reads_immediately_following_writes() {
    let store = WorkspaceStore::in_memory().expect("in memory store");
    store
        .set_file_hash("crates/storage/src/lib.rs", &[42u8; HASH_BYTES])
        .expect("write succeeds");

    let read_tx1 = store
        .database()
        .begin_read()
        .expect("first read transaction");
    let read_tx2 = store
        .database()
        .begin_read()
        .expect("concurrent read transaction");

    let hash1 = store
        .file_hash("crates/storage/src/lib.rs")
        .expect("read via store");
    assert_eq!(hash1, Some([42u8; HASH_BYTES]));

    drop(read_tx1);
    drop(read_tx2);
}
