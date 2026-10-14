//! Workspace root resolution and the database inside it.
//!
//! The criteria: an upward search finds the *nearest* marker, `.miter/` outranks a `.git/` at the
//! same level, a `.git` *file* (a linked worktree) counts as a marker just as a directory does,
//! a tree with no marker resolves to the directory itself, and a root's database path opens a
//! file that reopens with its rows.

use std::path::Path;

use miter_storage::{
    HASH_BYTES, INDEX_FILE, WORKSPACE_DIR, WorkspaceStore, default_db_path, find_workspace_root,
    resolve_workspace_root,
};

use crate::support::TempDir;

/// `mkdirs` a path and returns it.
fn mkdirs(path: &Path) -> &Path {
    std::fs::create_dir_all(path).expect("create directory");
    path
}

#[test]
fn the_nearest_marker_wins() {
    let dir = TempDir::new("discovery");
    let outer = dir.join("outer");
    let inner = outer.join("nested");
    let deep = inner.join("src").join("engine");
    mkdirs(&deep);
    std::fs::create_dir_all(outer.join(".git")).expect("outer marker");
    std::fs::create_dir_all(inner.join(WORKSPACE_DIR)).expect("inner marker");

    // From the deep directory the *nearest* marker up the chain is the nested one.
    let found = find_workspace_root(&deep).expect("a workspace");
    assert_eq!(found, canonical(&inner));
    // From a directory inside the outer project only, it is the outer one.
    let sibling = outer.join("other");
    mkdirs(&sibling);
    assert_eq!(
        find_workspace_root(&sibling).expect("the outer project"),
        canonical(&outer)
    );
}

#[test]
fn a_workspace_directory_outranks_a_git_directory_at_the_same_level() {
    let dir = TempDir::new("rank");
    let root = dir.join("project");
    mkdirs(&root.join(WORKSPACE_DIR));
    mkdirs(&root.join(".git"));
    assert_eq!(
        find_workspace_root(&root).expect("a workspace"),
        canonical(&root)
    );
}

#[test]
fn a_git_file_counts_as_a_marker() {
    // A linked worktree and a submodule carry `.git` as a *file* pointing at the real directory.
    let dir = TempDir::new("worktree");
    let root = dir.join("wt");
    mkdirs(&root);
    std::fs::write(root.join(".git"), "gitdir: /somewhere/else\n").expect("write .git");
    assert_eq!(
        find_workspace_root(&root).expect("a workspace"),
        canonical(&root)
    );
}

#[test]
fn a_tree_with_no_marker_resolves_to_its_own_directory() {
    let dir = TempDir::new("markerless");
    let deep = dir.join("a").join("b");
    mkdirs(&deep);
    // `std::env::temp_dir()` is neither a repository nor a workspace, so the walk ends at `/` …
    assert!(find_workspace_root(&deep).is_none());
    // … and the caller still has exactly one root to open.
    assert_eq!(resolve_workspace_root(&deep), canonical(&deep));
}

#[test]
fn a_root_resolves_to_a_reopenable_database() {
    let dir = TempDir::new("resolve");
    let root = dir.join("repo");
    mkdirs(&root.join(WORKSPACE_DIR));

    let db = default_db_path(&resolve_workspace_root(&root));
    assert_eq!(db, canonical(&root).join(WORKSPACE_DIR).join(INDEX_FILE));

    let store = WorkspaceStore::open_or_recreate(&db).expect("create");
    store
        .set_file_hash("src/lib.rs", &[9u8; HASH_BYTES])
        .expect("write");
    drop(store);

    // A second open reuses the same database rather than replacing it.
    let reopened = WorkspaceStore::open(&db).expect("reopen");
    assert_eq!(
        reopened.file_hash("src/lib.rs").expect("read"),
        Some([9u8; HASH_BYTES])
    );
}

/// The canonical form of a directory that exists.
fn canonical(path: &Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).expect("canonicalize")
}
