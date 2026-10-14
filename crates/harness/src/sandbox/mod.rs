//! The isolation boundary: an ephemeral `git worktree` the harness may do anything to.
//!
//! ## Why a worktree and not a copy
//!
//! A copy has to be *kept in step* with the repository — the sandbox would need its own
//! index, its own build cache, and its own answer to "what is HEAD". A worktree answers all
//! three by construction: it shares the object store, it shares `.git`, and it is checked out
//! at a named commit. What that buys is a repair loop that can compile, fail and retry
//! **without the user's working tree ever holding an intermediate state**.
//!
//! ## The jail is not advisory
//!
//! [`SandboxEnvironment::apply_patch`] resolves the requested path against the sandbox root
//! and refuses anything landing outside it. A patch naming `../../.ssh/authorized_keys` is
//! refused rather than clamped: silently redirecting a proposal somewhere other than where it
//! pointed is how a harness becomes a liability.
//!
//! ## Detached, on purpose
//!
//! The worktree is created `--detach` at `HEAD`. A harness that commits to a branch it
//! created has invented a branch nobody asked for; a detached worktree leaves no branch
//! behind, and [`SandboxEnvironment::merge_to_head`] copies the *files* back rather than
//! merging a history — which is the transaction this module wants: the user gets the verified
//! bytes, not a commit they did not write.

use std::path::{Path, PathBuf};

use crate::error::HarnessError;

#[cfg(test)]
mod tests;

/// Where sandboxes are cut, relative to the repository root.
pub const DEFAULT_WORKTREE_ROOT: &str = ".miter/worktrees";

/// An isolated checkout of a repository, and the only place a patch may be written.
#[derive(Clone, Debug)]
pub struct SandboxEnvironment {
    /// The repository the worktree was cut from.
    base_repo: PathBuf,
    /// The task this sandbox belongs to; also its directory name.
    task_id: String,
    /// The sandbox root — the worktree's checkout directory.
    root: PathBuf,
    /// Whether [`SandboxEnvironment::cleanup`] has already run.
    retired: bool,
}

impl SandboxEnvironment {
    /// The sandbox root: the directory a verifier is pointed at.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The repository this sandbox was cut from.
    #[must_use]
    pub fn base_repo(&self) -> &Path {
        &self.base_repo
    }
    /// Removes the worktree and forgets it.
    ///
    /// Idempotent: a second call is a no-op, so a cleanup on the success path needs no guard
    /// from the failure path.
    ///
    /// # Errors
    ///
    /// [`HarnessError::Git`] when `git worktree remove` fails on a directory that still
    /// stands.
    pub fn cleanup(&mut self) -> Result<(), HarnessError> {
        if self.retired {
            return Ok(());
        }
        let root = self.root.to_string_lossy().into_owned();
        let result = self.git(&["worktree", "remove", "--force", &root]);
        // `git worktree remove` refuses a directory it no longer tracks (a test that already
        // deleted it by hand, a crashed earlier run). The sandbox is gone either way, which is
        // what the caller asked for, so an untracked directory is success and only a directory
        // that still *stands* is an error.
        if let Err(error) = result
            && self.root.exists()
        {
            return Err(error);
        }
        self.retired = true;
        tracing::info!(sandbox = %self.root.display(), "sandbox cleaned up");
        Ok(())
    }

    /// The task id this sandbox belongs to.
    #[must_use]
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Whether [`SandboxEnvironment::cleanup`] has already run.
    #[must_use]
    pub fn is_retired(&self) -> bool {
        self.retired
    }

    /// Resolves a proposal path against the jail, refusing anything that escapes.
    /// Runs `git` inside the sandbox and captures its output.
    pub(super) fn git(&self, args: &[&str]) -> Result<std::process::Output, HarnessError> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .map_err(|e| HarnessError::Spawn {
                tool: "git".into(),
                detail: e.to_string(),
            })?;
        if !output.status.success() {
            return Err(HarnessError::Git {
                subcommand: args.join(" "),
                status: output.status.to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        Ok(output)
    }
}
impl Drop for SandboxEnvironment {
    /// Releases the worktree registration if the caller forgot to.
    ///
    /// Best effort by construction: `Drop` cannot report, and a cleanup failure here is
    /// `git worktree prune`'s problem rather than the caller's. The *directory* is deliberately
    /// left in place for that reason — deleting files from a `Drop` is a surprise, and the
    /// next [`SandboxEnvironment::create`] names what it found.
    fn drop(&mut self) {
        if !self.retired {
            let _ = self.cleanup();
        }
    }
}

mod create;
mod files;
