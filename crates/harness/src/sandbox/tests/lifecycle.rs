//! The worktree lifecycle: create, retire, refuse.

use super::fixtures::{fixture_repo, remove_repo, unique_dir};
use crate::error::HarnessError;
use crate::sandbox::SandboxEnvironment;

#[test]
fn cleanup_is_idempotent_and_the_directory_is_gone() {
    let repo = fixture_repo("cleanup");
    let mut sandbox = SandboxEnvironment::create(&repo, "task-clean").expect("cut a sandbox");
    let root = sandbox.root().to_path_buf();

    sandbox.cleanup().expect("first cleanup");
    assert!(sandbox.is_retired());
    assert!(!root.exists(), "{} still stands", root.display());
    // A second cleanup is a no-op rather than a second `git worktree remove`.
    sandbox.cleanup().expect("second cleanup");
    assert!(sandbox.is_retired());

    remove_repo(&repo);
}

#[test]
fn dropping_the_sandbox_releases_the_worktree() {
    let repo = fixture_repo("drop");
    let root = {
        let sandbox = SandboxEnvironment::create(&repo, "task-drop").expect("cut a sandbox");
        sandbox.root().to_path_buf()
    };
    assert!(!root.exists(), "the worktree outlived its environment");

    let listed = std::process::Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "worktree", "list"])
        .output()
        .expect("list worktrees");
    let listing = String::from_utf8_lossy(&listed.stdout);
    assert!(
        !listing.contains("task-drop"),
        "the worktree is still registered: {listing}"
    );

    remove_repo(&repo);
}

#[test]
fn a_task_id_that_is_not_a_directory_name_is_refused() {
    let repo = fixture_repo("taskid");
    for bad in ["", "..", "../escape", "with/slash", "with space"] {
        assert!(
            matches!(
                SandboxEnvironment::create(&repo, bad),
                Err(HarnessError::Refused(_))
            ),
            "`{bad}` was accepted"
        );
    }
    assert!(!repo.join(".miter/worktrees").exists());
    remove_repo(&repo);
}

#[test]
fn a_directory_outside_a_repository_is_named_as_such() {
    let bare = unique_dir("notarepo");
    std::fs::create_dir_all(&bare).expect("create the bare directory");
    let error = SandboxEnvironment::create(&bare, "task-bare").expect_err("not a repository");
    assert!(
        matches!(error, HarnessError::NotARepository { .. }),
        "{error:?}"
    );
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn a_sandbox_standing_for_the_same_task_is_reported_not_overwritten() {
    let repo = fixture_repo("standing");
    let mut sandbox = SandboxEnvironment::create(&repo, "task-standing").expect("cut a sandbox");

    let error = SandboxEnvironment::create(&repo, "task-standing")
        .expect_err("the second sandbox must be refused");
    assert!(
        matches!(error, HarnessError::Refused(ref message) if message.contains("still standing")),
        "{error:?}"
    );

    sandbox.cleanup().expect("clean up");
    remove_repo(&repo);
}
