//! The `.miter/` workspace root and the database inside it.
//!
//! ```text
//! repo/
//!   .git/          ← the fallback marker (a worktree has a `.git` *file*)
//!   .miter/        ← the marker this crate creates
//!     miter.redb   ← the workspace database: symbols, declarations, references, file hashes
//!     config.toml  ← optional glob exclusions (`miter_engine::config::MiterConfig`)
//!   src/…
//! ```
//!
//! A workspace is a *directory*, not a daemon and not a registry entry. `miter` resolves exactly
//! one root per invocation, opens `<root>/.miter/miter.redb` in-process, and reads
//! `<root>/.miter/config.toml` from the same directory. Nothing is attached, remembered or kept
//! alive between runs, so there is no attachment state to get out of step with the files on disk.
//!
//! ## Why the marker is a directory and not the database file
//!
//! A `.git/` beside a `.redb` would tie "this is a project root" to "someone has indexed it",
//! and the two are different questions: a repository is a project before it is ever indexed, and
//! `miter` has to be able to answer where it is *before* it writes anything.
//!
//! ## Why the search is upward
//!
//! A command runs in the directory a human is standing in, which is usually *inside* the project
//! — `crates/engine/src` is four levels below the root. Walking up to the first marker is what
//! makes `miter` work from anywhere in the tree without a flag, and picking the *nearest* marker
//! is what makes a nested repository (a vendored crate with its own `.git`) its own workspace
//! rather than a fragment of the outer one.

use std::path::{Path, PathBuf};

/// The directory that marks a project root.
pub const WORKSPACE_DIR: &str = ".miter";

/// The database inside [`WORKSPACE_DIR`].
pub const INDEX_FILE: &str = "miter.redb";

/// The database path for a workspace root: `<workspace_root>/.miter/miter.redb`.
#[must_use]
pub fn default_db_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(WORKSPACE_DIR).join(INDEX_FILE)
}

/// The markers an upward search accepts.
///
/// `.miter` outranks `.git` *at the same level*: a repository that has been initialized keeps its
/// own root even if the surrounding tree is also a repository. The order is not a preference
/// between the two across levels — the nearest marker of either kind wins, which is what makes a
/// nested checkout its own workspace.
pub const ROOT_MARKERS: [&str; 2] = [WORKSPACE_DIR, ".git"];

/// Walks up from `start_dir` to the nearest directory that looks like a project root.
///
/// A marker counts when it exists, whether it is a directory or a file — a linked worktree and a
/// submodule both have a `.git` *file*, and treating those as "not a repository" would make the
/// workspace of a `git worktree` the parent repository's database.
///
/// The returned path is canonicalized when the filesystem allows it, so that `/tmp/x` and
/// `/private/tmp/x` (the same directory on macOS) cannot resolve to two different roots.
#[must_use]
pub fn find_workspace_root(start_dir: &Path) -> Option<PathBuf> {
    let mut candidate = Some(absolute(start_dir));
    while let Some(directory) = candidate {
        if ROOT_MARKERS
            .iter()
            .any(|marker| directory.join(marker).exists())
        {
            return Some(canonical(&directory));
        }
        candidate = directory.parent().map(Path::to_path_buf);
    }
    None
}

/// The single workspace root a command works in.
///
/// The nearest marker at or above `start_dir`, or `start_dir` itself when the walk finds none:
/// a caller always has exactly one root to open, and a directory outside any project simply
/// becomes its own workspace rather than an error.
#[must_use]
pub fn resolve_workspace_root(start_dir: &Path) -> PathBuf {
    find_workspace_root(start_dir).unwrap_or_else(|| canonical(&absolute(start_dir)))
}

/// The directory a relative `start_dir` means, read against the process's own directory.
fn absolute(start_dir: &Path) -> PathBuf {
    if start_dir.is_absolute() {
        start_dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(start_dir))
            .unwrap_or_else(|_| start_dir.to_path_buf())
    }
}

/// The canonical form of a directory that exists, or the path unchanged when it does not.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
