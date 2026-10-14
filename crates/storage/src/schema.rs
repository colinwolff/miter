//! Canonical schema definitions and constants for indexed files and symbols.
//!
//! Provides table definitions and helper records for incremental dirty-file tracking
//! and AST co-processor index caches.

use redb::TableDefinition;

/// Name of [`INDEXED_FILES_TABLE`]: tracks file mtimes and SHA-256 digests.
pub const INDEXED_FILES_TABLE_NAME: &str = "indexed_files_v1";

/// Table definition for `INDEXED_FILES_TABLE`: workspace-relative path -> serialized record.
pub const INDEXED_FILES_TABLE: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new(INDEXED_FILES_TABLE_NAME);

/// Record stored in [`INDEXED_FILES_TABLE`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndexedFileRecord {
    /// Filesystem modification time in seconds since UNIX epoch.
    pub mtime_secs: u64,
    /// Filesystem modification time subsecond nanoseconds.
    pub mtime_nanos: u32,
    /// 32-byte SHA-256 digest of file content.
    pub content_hash: [u8; 32],
    /// File size in bytes.
    pub size_bytes: u64,
}

impl IndexedFileRecord {
    /// Creates a new record.
    #[must_use]
    pub fn new(mtime_secs: u64, mtime_nanos: u32, content_hash: [u8; 32], size_bytes: u64) -> Self {
        Self {
            mtime_secs,
            mtime_nanos,
            content_hash,
            size_bytes,
        }
    }
}

/// Name of [`REFERENCES_TABLE`]: inverted index mapping symbol names to references.
pub const REFERENCES_TABLE_NAME: &str = "references_v1";

/// Table definition for [`REFERENCES_TABLE`]: target symbol name -> serialized `Vec<ReferenceEntry>`.
pub const REFERENCES_TABLE: TableDefinition<'static, &str, &[u8]> =
    TableDefinition::new(REFERENCES_TABLE_NAME);

/// Classification of an AST identifier reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReferenceKind {
    /// Function or method invocation / call site.
    Call,
    /// Type annotation, struct construction, or generic parameter usage.
    TypeUsage,
    /// Implementation of a trait or interface method.
    TraitImpl,
    /// Import or use statement (`use foo::bar`, `import { bar } from 'foo'`).
    Import,
    /// A bare name used as a value: a CSS `var(--token)`, an HTML `class`/`id` reference, or a
    /// SQL `FROM`/`JOIN`/`REFERENCES` table. It names something without calling or typing it.
    Usage,
}

/// A single reference occurrence pointing to an AST symbol.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReferenceEntry {
    /// Workspace-relative file path.
    pub file_path: String,
    /// 0-indexed byte span: `(start_byte, end_byte)`.
    pub byte_range: (usize, usize),
    /// 1-indexed line span: `(start_line, end_line)`.
    pub line_range: (usize, usize),
    /// Name of enclosing function, method, struct, or module (if any).
    pub enclosing_symbol: Option<String>,
    /// Semantic classification of the reference.
    pub reference_kind: ReferenceKind,
}
