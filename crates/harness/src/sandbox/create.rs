//! Cutting a sandbox: the worktree, and the id that names it.

use std::path::{Component, Path, PathBuf};

use crate::error::HarnessError;

use super::{DEFAULT_WORKTREE_ROOT, SandboxEnvironment};

impl SandboxEnvironment {
    /// Cuts a sandbox for `task_id` from `base_repo`.
    ///
    /// The id becomes a directory name, so it is validated first: non-empty, and made only of
    /// `[A-Za-z0-9._-]`. A task id is generated from a symbol name or a timestamp by the
    /// caller, and a caller that builds one from untrusted text must not be able to walk out of
    /// `.miter/worktrees` with it.
    ///
    /// # Errors
    ///
    /// [`HarnessError::Refused`] for an unusable `task_id` or a sandbox already standing,
    /// [`HarnessError::NotARepository`] when `base_repo` is not inside a git repository, and
    /// [`HarnessError::Git`] when the `git worktree add` itself fails.
    pub fn create(base_repo: &Path, task_id: &str) -> Result<Self, HarnessError> {
        Self::create_in(base_repo, task_id, DEFAULT_WORKTREE_ROOT)
    }

    /// As [`SandboxEnvironment::create`], with the sandbox parent directory chosen by the
    /// caller.
    ///
    /// # Errors
    ///
    /// As [`SandboxEnvironment::create`].
    pub fn create_in(base_repo: &Path, task_id: &str, parent: &str) -> Result<Self, HarnessError> {
        Self::validate_task_id(task_id)?;
        let base_repo = absolute(base_repo)?;
        if !base_repo.join(".git").exists() {
            return Err(HarnessError::NotARepository { path: base_repo });
        }

        let root = base_repo.join(parent).join(task_id);
        if root.exists() {
            // A leftover worktree from a crashed run would make `git worktree add` fail with
            // a message about the path being in use, which names the fix less precisely than
            // saying what was found and where.
            return Err(HarnessError::Refused(format!(
                "{} already exists: a sandbox for task `{task_id}` is still standing",
                root.display()
            )));
        }
        if let Some(parent_dir) = root.parent() {
            std::fs::create_dir_all(parent_dir).map_err(|e| HarnessError::io(parent_dir, e))?;
        }

        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&base_repo)
            .args(["worktree", "add", "--detach", "--quiet"])
            .arg(&root)
            .arg("HEAD")
            .output()
            .map_err(|e| HarnessError::Spawn {
                tool: "git".into(),
                detail: e.to_string(),
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let _ = std::fs::remove_dir_all(&root);
            return Err(HarnessError::Git {
                subcommand: "worktree add".into(),
                status: output.status.to_string(),
                stderr,
            });
        }

        tracing::info!(sandbox = %root.display(), task = %task_id, "sandbox created");
        Ok(Self {
            base_repo,
            task_id: task_id.to_owned(),
            root,
            retired: false,
        })
    }

    /// Rejects a task id that could not safely be a directory name.
    fn validate_task_id(task_id: &str) -> Result<(), HarnessError> {
        if task_id.is_empty() {
            return Err(HarnessError::Refused(
                "a sandbox task id must not be empty".into(),
            ));
        }
        if matches!(task_id, "." | "..") {
            return Err(HarnessError::Refused(format!(
                "task id `{task_id}` is not a usable directory name"
            )));
        }
        if !task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err(HarnessError::Refused(format!(
                "task id `{task_id}` may hold only letters, digits, `.`, `_` and `-`: it \
                 becomes a directory name"
            )));
        }
        Ok(())
    }
}

fn absolute(path: &Path) -> Result<PathBuf, HarnessError> {
    if path.is_absolute() {
        return Ok(normalize(path));
    }
    let cwd = std::env::current_dir().map_err(|e| HarnessError::io(path, e))?;
    Ok(normalize(&cwd.join(path)))
}

/// Lexically removes `.` and `..` components without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
