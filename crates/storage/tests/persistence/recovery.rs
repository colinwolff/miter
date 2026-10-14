//! The schema handshake and its recovery.
//!
//! Three files, three outcomes:
//!
//! | the file | `inspect` | `open_or_recreate` |
//! |---|---|---|
//! | this build's generation | the version | opens it, keeping every row |
//! | another generation | `SchemaMismatch` | purges and re-initializes |
//! | no longer a database at all | `Redb` | purges and re-initializes |
//!
//! The last row of that table is the one worth stating: a *corrupt* file and a *foreign* file take
//! the same path, because from this build's side neither can be read and both are recoverable by
//! rebuilding. What a caller must not get is a silent reinterpretation — the recovery warns, and
//! `inspect` is there for the caller that wants to be told instead.

use miter_storage::tables::ENGINE_STORAGE_VERSION;
use miter_storage::{HASH_BYTES, StorageError, WorkspaceStore};

use crate::support::{TempDir, re_stamp, write_json_header};

/// A workspace with one ledger row in it, so a purge is visible.
fn workspace_with_a_row(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.join("index.redb");
    {
        let store = WorkspaceStore::create(&path).expect("create");
        store.ensure_tables().expect("shape");
        store
            .set_file_hash("src/lib.rs", &[0x5A; HASH_BYTES])
            .expect("write");
    }
    path
}

#[test]
fn inspect_reports_this_builds_generation() {
    let dir = TempDir::new("inspect_ok");
    let path = workspace_with_a_row(&dir);
    assert_eq!(
        WorkspaceStore::inspect(&path).expect("readable"),
        ENGINE_STORAGE_VERSION
    );
}

#[test]
fn a_foreign_generation_is_refused_by_inspect_and_recovered_by_open_or_recreate() {
    let dir = TempDir::new("stale");
    let path = workspace_with_a_row(&dir);
    re_stamp(&path, Some(99));

    let error = WorkspaceStore::inspect(&path).expect_err("inspect refuses");
    match &error {
        StorageError::SchemaMismatch { expected, found } => {
            assert_eq!(*expected, ENGINE_STORAGE_VERSION);
            assert_eq!(*found, Some(99));
        }
        other => panic!("expected a mismatch, got {other:?}"),
    }
    assert!(error.to_string().contains("99"), "{error}");

    let store = WorkspaceStore::open_or_recreate(&path).expect("recovery");
    assert_eq!(
        store.schema_version().expect("read"),
        Some(ENGINE_STORAGE_VERSION)
    );
    // A rebuild, not a migration: no record survived, and the file is this build's again.
    assert_eq!(store.file_hash_count().expect("count"), 0);
    drop(store);
    assert_eq!(
        WorkspaceStore::inspect(&path).expect("readable again"),
        ENGINE_STORAGE_VERSION
    );
}

#[test]
fn a_corrupt_file_is_purged_and_replaced() {
    let dir = TempDir::new("corrupt");
    let path = dir.join("index.redb");
    std::fs::write(&path, b"not a database, just bytes").expect("write garbage");
    assert!(
        WorkspaceStore::inspect(&path).is_err(),
        "a garbage file is not this build's"
    );

    let store = WorkspaceStore::open_or_recreate(&path).expect("recovery");
    assert_eq!(
        store.schema_version().expect("read"),
        Some(ENGINE_STORAGE_VERSION)
    );
    assert_eq!(store.file_hash_count().expect("count"), 0);
}

#[test]
fn a_file_with_no_stamp_is_read_through_its_json_header() {
    // The shape of every database written before this crate existed: the engine's header, and no
    // stamp row. The generation is still readable — from the document — and nothing is purged.
    let dir = TempDir::new("legacy");
    let path = dir.join("index.redb");
    {
        let store = WorkspaceStore::create(&path).expect("create");
        store.ensure_tables().expect("shape");
        store
            .set_file_hash("src/legacy.rs", &[0x11; HASH_BYTES])
            .expect("write");
    }
    re_stamp(&path, None);
    write_json_header(&path, ENGINE_STORAGE_VERSION);

    let store = WorkspaceStore::open_or_recreate(&path).expect("open");
    assert_eq!(
        store.schema_version().expect("read"),
        Some(ENGINE_STORAGE_VERSION)
    );
    // The row is still there: agreeing with the header is not a reason to rebuild.
    assert_eq!(
        store.file_hash("src/legacy.rs").expect("read"),
        Some([0x11; HASH_BYTES])
    );
    // And the stamp is now written, so the next open does not have to parse the document.
    assert!(store.schema_version_bytes().expect("read").is_some());
}

#[test]
fn a_legacy_file_whose_header_names_another_generation_is_rebuilt() {
    let dir = TempDir::new("legacy_stale");
    let path = dir.join("index.redb");
    {
        let store = WorkspaceStore::create(&path).expect("create");
        store.ensure_tables().expect("shape");
    }
    re_stamp(&path, None);
    write_json_header(&path, 42);

    let store = WorkspaceStore::open_or_recreate(&path).expect("recovery");
    assert_eq!(
        store.schema_version().expect("read"),
        Some(ENGINE_STORAGE_VERSION)
    );
    assert!(
        store
            .table_names()
            .expect("tables")
            .contains(&"file_hashes_v1".to_owned()),
        "the recovered file is this build's"
    );
}

#[test]
fn purge_is_idempotent_and_takes_the_lock_sidecar() {
    let dir = TempDir::new("purge");
    let path = dir.join("index.redb");
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    std::fs::write(std::path::PathBuf::from(&lock), b"sidecar").expect("write sidecar");
    WorkspaceStore::purge(&path).expect("purge a missing database");
    WorkspaceStore::purge(&path).expect("purge again");
    assert!(!std::path::Path::new(&lock).exists());
}

#[test]
fn a_recovered_file_is_written_by_this_build_and_reopens() {
    // The end of the recovery is a *usable* workspace, not merely a new file: tables writable,
    // ledger writable, and the generation readable back.
    let dir = TempDir::new("recovered");
    let path = dir.join("index.redb");
    std::fs::write(&path, b"junk").expect("garbage");
    let store = WorkspaceStore::open_or_recreate(&path).expect("recovery");
    store.ensure_tables().expect("shape");
    store
        .set_file_hash("a.rs", &[0x22; HASH_BYTES])
        .expect("write");
    drop(store);

    let reopened = WorkspaceStore::open_or_recreate(&path).expect("reopen");
    assert_eq!(
        reopened.file_hash("a.rs").expect("read"),
        Some([0x22; HASH_BYTES])
    );
}

#[test]
fn a_database_in_use_is_never_purged() {
    // The distinction the recovery path must keep: a lock conflict is *not* damage. A second
    // handle on a file this process (or another) holds is refused, and the file is left alone —
    // deleting it would destroy a workspace somebody is working in.
    let dir = TempDir::new("busy");
    let path = dir.join("index.redb");
    let held = WorkspaceStore::create(&path).expect("create");
    held.set_file_hash("keep.rs", &[0xEE; HASH_BYTES])
        .expect("write");

    let error = WorkspaceStore::open_or_recreate(&path).expect_err("a held database is refused");
    match &error {
        StorageError::Busy { path: busy } => assert_eq!(busy, &path),
        other => panic!("expected a busy database, got {other:?}"),
    }
    assert!(error.to_string().contains("already open"), "{error}");

    // The file still holds the row the first handle wrote.
    drop(held);
    let reopened = WorkspaceStore::open_or_recreate(&path).expect("reopen after the holder left");
    assert_eq!(
        reopened.file_hash("keep.rs").expect("read"),
        Some([0xEE; HASH_BYTES])
    );
}
