//! Shared fixtures: one throwaway repository per test.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A counter that keeps every fixture repository in its own directory.
///
/// The alternative — one directory cleaned between tests — has parallel tests deleting each
/// other's trees, and a failure that leaves state behind then poisons every test after it.
pub(super) fn unique_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let ordinal = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "miter-harness-{label}-{}-{ordinal}",
        std::process::id()
    ))
}

/// A git repository with one commit, so a worktree can be cut from it.
pub(super) fn fixture_repo(label: &str) -> PathBuf {
    let root = unique_dir(label);
    std::fs::create_dir_all(root.join("src")).expect("create the fixture tree");
    std::fs::write(root.join("src/lib.rs"), "pub fn original() -> u8 { 1 }\n")
        .expect("write the fixture source");

    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    // Identity is pinned so the fixture commits identically wherever it runs.
    git(&["init", "--quiet", "--initial-branch", "main"]);
    git(&["config", "user.email", "harness@example.invalid"]);
    git(&["config", "user.name", "Harness Fixture"]);
    git(&["config", "commit.gpgsign", "false"]);
    git(&["add", "-A"]);
    git(&["commit", "--quiet", "-m", "fixture"]);
    root
}

/// Removes a fixture tree and its worktree registrations.
pub(super) fn remove_repo(root: &Path) {
    let _ = std::process::Command::new("git")
        .args(["-C", &root.to_string_lossy(), "worktree", "prune"])
        .output();
    let _ = std::fs::remove_dir_all(root);
}
