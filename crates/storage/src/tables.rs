//! The canonical `redb` table definitions of a miter workspace.
//!
//! This module is the *whole* on-disk contract in one place: the table names, the key and
//! value layouts, and the schema generation. Nothing here knows about tree-sitter — a record is
//! bytes, and who wrote them is the caller's business.
//!
//! | table | key | value |
//! |---|---|---|
//! | [`TABLE_META`] | a one-byte key (`0` = the header row) | a JSON object (the engine's `DatabaseHeader`) |
//! | [`TABLE_SYMBOLS`] | symbol id, `u64` | the codec's self-describing record |
//! | [`TABLE_UNIVERSAL`] | declaration id, `u64` | the codec's universal record |
//! | [`TABLE_FILE_HASHES`] | a workspace-relative path, UTF-8 | a 32-byte SHA-256 digest |
//!
//! The `_v1` suffix and [`ENGINE_STORAGE_VERSION`] are two statements of the same thing at two
//! levels: the suffix is the *name* of the layout a record lives in, and the version is the
//! number a file reports about itself in [`TABLE_META`]. Both move only together.

use redb::TableDefinition;

/// Schema generation this build writes and understands.
///
/// The number lives in [`TABLE_META`] under [`META_HEADER_KEY`], inside the JSON header row's
/// `schema_version` field, and [`crate::WorkspaceStore::open_or_recreate`] purges a file whose
/// value is anything else. Bumping it is therefore a statement that the *record layout* changed.
pub const ENGINE_STORAGE_VERSION: u32 = 3;

/// Name of [`TABLE_META`].
///
/// Spelled `schema_metadata_v1` rather than `meta`: this is the name the engine has written since
/// the header existed, and a reader of an old file has to find it there.
pub const TABLE_META_NAME: &str = "schema_metadata_v1";

/// Configuration and schema generation: one JSON object per key.
pub const TABLE_META: TableDefinition<'static, &[u8], &[u8]> =
    TableDefinition::new(TABLE_META_NAME);

/// The key of the single header row: one byte, little-endian `0`.
pub const META_HEADER_KEY: u8 = 0;

/// The key of this crate's own schema stamp: one byte, little-endian `1`.
///
/// A second key rather than a second table, because the JSON header at [`META_HEADER_KEY`] has
/// been at its key since it existed and moving it would make every database in the wild look
/// corrupted. See [`crate::WorkspaceStore::schema_version`] for how the two are read.
pub const META_SCHEMA_VERSION_KEY: u8 = 1;

/// The `schema_version` field of the header object, and the only field storage reads.
///
/// Named as a constant because two modules depend on the spelling agreeing: the engine writes
/// the document, this crate parses it, and a rename on one side alone would look like a purge.
pub const META_SCHEMA_VERSION_FIELD: &str = "schema_version";

/// Name of [`TABLE_SYMBOLS`]: the encoded declarations of a persisted index.
pub const TABLE_SYMBOLS_NAME: &str = "symbols";

/// Encoded declarations, keyed by the symbol id the codec's record also carries.
pub const TABLE_SYMBOLS: TableDefinition<'static, u64, &[u8]> =
    TableDefinition::new(TABLE_SYMBOLS_NAME);

/// Name of [`TABLE_UNIVERSAL`]: the polyglot declarations of a persisted index.
///
/// A *second* table beside [`TABLE_SYMBOLS`] rather than a second record kind inside it, because
/// the two hold different shapes with different lifetimes: `symbols` is the function-shaped record,
/// and this is the universal one that can name a `struct`. Keying them alike would make "one
/// declaration" ambiguous between the two, and a reader that had to guess which shape a blob held
/// is a reader that eventually guesses wrong.
pub const TABLE_UNIVERSAL_NAME: &str = "universal_symbols_v1";

/// Encoded universal declarations, keyed by the same `file:line:name` id the universal record
/// derives and carries.
pub const TABLE_UNIVERSAL: TableDefinition<'static, u64, &[u8]> =
    TableDefinition::new(TABLE_UNIVERSAL_NAME);

/// Name of [`TABLE_FILE_HASHES`]: the incremental indexer's ledger.
pub const TABLE_FILE_HASHES_NAME: &str = "file_hashes_v1";

/// Workspace-relative path → SHA-256 digest of the file's bytes, so an indexing pass can skip
/// a file whose contents have not changed since the last one.
///
/// The path is relative to the workspace root on purpose: moving a workspace to another
/// directory must not invalidate the ledger, and an absolute path would.
pub const TABLE_FILE_HASHES: TableDefinition<'static, &[u8], &[u8]> =
    TableDefinition::new(TABLE_FILE_HASHES_NAME);

/// Length of a SHA-256 digest in bytes.
pub const HASH_BYTES: usize = 32;

/// Every table a workspace database owns, as `(name, kind)` for a reader that wants to
/// enumerate rather than to open one.
///
/// The list is what `WorkspaceStore::ensure_tables` creates and what a test asserts against,
/// so a table added to this module without being added here fails that test rather than being
/// silently absent from a fresh file.
pub const WORKSPACE_TABLES: &[&str] = &[
    TABLE_META_NAME,
    TABLE_SYMBOLS_NAME,
    TABLE_UNIVERSAL_NAME,
    TABLE_FILE_HASHES_NAME,
];
