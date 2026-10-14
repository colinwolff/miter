//! # `miter-storage` — the workspace database layer
//!
//! The `redb` tables, the schema handshake and the file-level recovery, extracted from
//! `engine` so that anything — a CLI that wants a row count, a test that wants to forge a
//! foreign file — can read or write a workspace without linking a grammar.
//!
//! ```text
//!                 ┌──────────────────────────────────────────────┐
//!   storage  ───► │ meta · symbols · universal_symbols ·          │  one redb file
//!                 │ file_hashes                                  │
//!                 └──────────────────────────────────────────────┘
//!                        ▲
//!              miter_engine::persist::redb_store
//!              (the codec's symbol records)
//! ```
//!
//! | module | responsibility |
//! |---|---|
//! | [`tables`] | the canonical table names, key/value layouts and generation |
//! | [`store`] | [`WorkspaceStore`]: open, create, stamp, inspect, auto-purge and the file-hash ledger |
//! | [`workspace`] | the `.miter/` root heuristic and [`resolve_workspace_root`] |
//! | [`hash`] | the SHA-256 the incremental indexer keys its ledger with |
//! | [`error`] | [`StorageError`] and its aliases |
//!
//! ## What this crate deliberately does not know
//!
//! Nothing here parses a record. A value is bytes; who wrote them and what they mean belongs to
//! `engine`. The one field this crate reads out of a record is the schema generation, and it reads
//! it because the generation is the *only* thing that decides whether the file may be opened at
//! all.
//!
//! ## Example
//!
//! ```
//! use miter_storage::WorkspaceStore;
//!
//! # fn main() -> Result<(), miter_storage::StorageError> {
//! let store = WorkspaceStore::in_memory()?;
//! assert_eq!(store.schema_version()?, Some(miter_storage::ENGINE_STORAGE_VERSION));
//! assert_eq!(store.file_hash_count()?, 0);
//!
//! store.set_file_hash("src/lib.rs", &[0u8; miter_storage::HASH_BYTES])?;
//! assert_eq!(store.file_hash_count()?, 1);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod hash;
pub mod indexed_files;
pub mod references;
pub mod schema;
pub mod store;
pub mod tables;
pub mod workspace;

pub use error::{StorageError, StorageResult};
pub use hash::{DIGEST_BYTES, HASH_HEX_LEN, sha256, sha256_hex};
pub use schema::{
    INDEXED_FILES_TABLE, INDEXED_FILES_TABLE_NAME, IndexedFileRecord, REFERENCES_TABLE,
    REFERENCES_TABLE_NAME, ReferenceEntry, ReferenceKind,
};
pub use store::WorkspaceStore;
pub use tables::{
    ENGINE_STORAGE_VERSION, HASH_BYTES, META_HEADER_KEY, META_SCHEMA_VERSION_KEY,
    TABLE_FILE_HASHES, TABLE_FILE_HASHES_NAME, TABLE_META, TABLE_META_NAME, TABLE_SYMBOLS,
    TABLE_SYMBOLS_NAME, TABLE_UNIVERSAL, TABLE_UNIVERSAL_NAME, WORKSPACE_TABLES,
};
pub use workspace::{
    INDEX_FILE, ROOT_MARKERS, WORKSPACE_DIR, default_db_path, find_workspace_root,
    resolve_workspace_root,
};
