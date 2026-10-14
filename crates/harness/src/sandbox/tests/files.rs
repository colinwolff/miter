//! The file surface: writing a patch, reading it back, diffing and merging.

use std::path::Path;

use super::fixtures::{fixture_repo, remove_repo};
use crate::error::HarnessError;
use crate::sandbox::SandboxEnvironment;

#[test]
fn a_patch_lands_in_the_sandbox_and_nowhere_else() {
    let repo = fixture_repo("patch");
    let mut sandbox = SandboxEnvironment::create(&repo, "task-patch").expect("cut a sandbox");

    let written = sandbox
        .apply_patch(Path::new("src/lib.rs"), "pub fn patched() -> u8 { 2 }\n")
        .expect("apply the patch");
    assert!(
        written.starts_with(sandbox.root()),
        "{written:?} escaped the jail"
    );

    // The sandbox holds the patch …
    let sandbox_text = sandbox
        .read(Path::new("src/lib.rs"))
        .expect("read from the sandbox");
    assert!(sandbox_text.contains("patched"), "{sandbox_text}");

    // … and the user's working tree does not.
    let primary = std::fs::read_to_string(repo.join("src/lib.rs")).expect("read the primary");
    assert!(primary.contains("original"), "{primary}");

    sandbox.cleanup().expect("clean up");
    remove_repo(&repo);
}

#[test]
fn a_new_file_and_its_parents_are_created_inside_the_jail() {
    let repo = fixture_repo("newfile");
    let mut sandbox = SandboxEnvironment::create(&repo, "task-new").expect("cut a sandbox");

    sandbox
        .apply_patch(Path::new("src/deep/nested/mod.rs"), "// added\n")
        .expect("create parents");
    assert!(sandbox.root().join("src/deep/nested/mod.rs").exists());

    sandbox.cleanup().expect("clean up");
    remove_repo(&repo);
}

#[test]
fn a_path_that_climbs_out_is_refused() {
    let repo = fixture_repo("escape");
    let mut sandbox = SandboxEnvironment::create(&repo, "task-escape").expect("cut a sandbox");

    // Both spellings are refused: a plain `..`, and one that leaves and returns. The second is
    // the interesting one — it *would* normalise back inside the jail, so a check made after
    // normalisation would wave it through.
    for attempt in ["../escaped.rs", "src/../../escaped.rs"] {
        let error = sandbox
            .apply_patch(Path::new(attempt), "// should never be written\n")
            .expect_err("a climbing path must be refused");
        assert!(
            matches!(error, HarnessError::PathEscapes { .. }),
            "{attempt} produced {error:?}"
        );
    }
    assert!(!repo.join("escaped.rs").exists());

    // An absolute path is refused for the same reason.
    let absolute = repo.join("src/absolute.rs");
    assert!(matches!(
        sandbox.apply_patch(&absolute, "// nope\n"),
        Err(HarnessError::PathEscapes { .. })
    ));

    sandbox.cleanup().expect("clean up");
    remove_repo(&repo);
}

#[test]
fn merging_transfers_the_verified_bytes_and_leaves_history_alone() {
    let repo = fixture_repo("merge");
    let head_before = rev_parse_head(&repo);

    let mut sandbox = SandboxEnvironment::create(&repo, "task-merge").expect("cut a sandbox");
    sandbox
        .apply_patch(Path::new("src/lib.rs"), "pub fn merged() -> u8 { 3 }\n")
        .expect("patch");
    sandbox
        .apply_patch(Path::new("src/extra.rs"), "// new file\n")
        .expect("add");

    let transferred = sandbox.merge_to_head().expect("merge");
    assert_eq!(transferred.len(), 2, "{transferred:?}");
    let primary = std::fs::read_to_string(repo.join("src/lib.rs")).expect("read the primary");
    assert!(primary.contains("merged"), "{primary}");
    assert!(repo.join("src/extra.rs").exists());

    // HEAD is untouched: the user gets files, not a commit.
    assert_eq!(rev_parse_head(&repo), head_before, "the merge moved HEAD");

    sandbox.cleanup().expect("clean up");
    remove_repo(&repo);
}

/// The fixture repository's current `HEAD`.
fn rev_parse_head(repo: &Path) -> String {
    let output = std::process::Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .expect("read HEAD");
    assert!(output.status.success(), "rev-parse HEAD failed");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn a_deleted_file_does_not_survive_the_merge() {
    let repo = fixture_repo("delete");
    let mut sandbox = SandboxEnvironment::create(&repo, "task-delete").expect("cut a sandbox");

    std::fs::remove_file(sandbox.root().join("src/lib.rs")).expect("delete inside the sandbox");
    sandbox.merge_to_head().expect("merge");

    assert!(
        !repo.join("src/lib.rs").exists(),
        "a file the verified tree no longer held came back anyway"
    );

    sandbox.cleanup().expect("clean up");
    remove_repo(&repo);
}
