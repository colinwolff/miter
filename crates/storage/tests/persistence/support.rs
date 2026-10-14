//! Shared scaffolding for the storage suite: a throwaway directory, and the two ways a file from
//! *another* build is forged.
//!
//! The forgery goes through raw `redb` on purpose. The handshake exists for files this crate did
//! not write, and going through `WorkspaceStore` would write this build's own stamp — the
//! criterion would then be about a file that already agrees.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use miter_storage::tables::{META_HEADER_KEY, META_SCHEMA_VERSION_KEY, TABLE_META};
use redb::Database;

/// A directory that removes itself when dropped.
#[derive(Debug)]
pub(crate) struct TempDir {
    /// The directory.
    pub(crate) path: PathBuf,
}

impl TempDir {
    /// Creates `<temp>/miter_storage_<label>_<pid>_<nanos>`.
    pub(crate) fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "miter_storage_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).expect("create temp directory");
        Self { path }
    }

    /// A path inside the directory.
    pub(crate) fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Overwrites the file's schema stamp, the way another build would have written it.
///
/// `None` removes the stamp row entirely, leaving whatever else is there — the shape of a file
/// written before this crate existed.
pub(crate) fn re_stamp(path: &Path, version: Option<u32>) {
    let database = Database::open(path).expect("open");
    let transaction = database.begin_write().expect("write");
    {
        let mut table = transaction.open_table(TABLE_META).expect("meta");
        match version {
            Some(version) => {
                table
                    .insert(
                        &[META_SCHEMA_VERSION_KEY][..],
                        version.to_le_bytes().as_slice(),
                    )
                    .expect("insert stamp");
            }
            None => {
                table
                    .remove(&[META_SCHEMA_VERSION_KEY][..])
                    .expect("remove stamp");
            }
        }
    }
    transaction.commit().expect("commit");
}

/// Writes the JSON header row the way the engine does — the file from before this crate, whose
/// only statement about its generation is that document.
pub(crate) fn write_json_header(path: &Path, schema_version: u32) {
    let database = Database::open(path).expect("open");
    let transaction = database.begin_write().expect("write");
    {
        let mut table = transaction.open_table(TABLE_META).expect("meta");
        let document = serde_json::json!({
            "schema_version": schema_version,
            "engine_version": "0.9.0",
            "git_hash": "deadbee",
            "created_at": 1_700_000_000u64,
        });
        table
            .insert(
                &[META_HEADER_KEY][..],
                serde_json::to_vec(&document).expect("encode").as_slice(),
            )
            .expect("insert header");
    }
    transaction.commit().expect("commit");
}
