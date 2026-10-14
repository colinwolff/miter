//! File operations inside the jail: write a patch, read a file back, diff, merge.
//!
//! Every path a caller names is resolved against the sandbox root first — see
//! [`super::SandboxEnvironment::apply_patch`] for why the jail is not advisory.

use std::path::{Component, Path, PathBuf};

use crate::error::HarnessError;

use super::SandboxEnvironment;

impl SandboxEnvironment {
    pub fn apply_patch(&self, path: &Path, content: &str) -> Result<PathBuf, HarnessError> {
        let target = self.resolve(path)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| HarnessError::io(parent, e))?;
        }
        std::fs::write(&target, content).map_err(|e| HarnessError::io(&target, e))?;
        Ok(target)
    }

    /// Reads a file from inside the sandbox.
    ///
    /// # Errors
    ///
    /// [`HarnessError::PathEscapes`] when the path is outside the jail, [`HarnessError::Io`]
    /// when the file cannot be read.
    pub fn read(&self, path: &Path) -> Result<String, HarnessError> {
        let target = self.resolve(path)?;
        std::fs::read_to_string(&target).map_err(|e| HarnessError::io(&target, e))
    }

    /// The paths that differ from the sandbox's `HEAD`.
    ///
    /// Read from `git status --porcelain`, so it reports what git itself considers changed —
    /// including a file the harness created and a file it deleted.
    ///
    /// # Errors
    ///
    /// [`HarnessError::Git`] when `git status` fails.
    pub fn changed_files(&self) -> Result<Vec<PathBuf>, HarnessError> {
        let output = self.git(&["status", "--porcelain"])?;
        let mut changed = Vec::new();
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            // Porcelain v1: two status characters, a space, then the path. A rename reads
            // `R  old -> new`, and the new path is the one the working tree now holds.
            if line.len() < 4 {
                continue;
            }
            let entry = &line[3..];
            let path = match entry.split_once(" -> ") {
                Some((_, new)) => new,
                None => entry,
            };
            changed.push(PathBuf::from(path.trim_matches('"')));
        }
        Ok(changed)
    }
    /// Copies the verified files back into the primary working tree.
    ///
    /// A *copy*, not a merge: the user receives the bytes that were verified and the branch
    /// they were on is left exactly as they left it — same HEAD, same index, same history.
    /// Applying the sandbox's diff instead would stage changes nobody staged.
    ///
    /// A file deleted in the sandbox is removed in the primary tree too: a verification that
    /// deleted a file and passed must not leave that file behind on the way out.
    ///
    /// # Errors
    ///
    /// [`HarnessError::Git`] when the sandbox's status cannot be read, [`HarnessError::Io`]
    /// when a transfer fails.
    pub fn merge_to_head(&self) -> Result<Vec<PathBuf>, HarnessError> {
        let mut transferred = Vec::new();
        for relative in self.changed_files()? {
            let source = self.root.join(&relative);
            let destination = self.base_repo.join(&relative);
            if source.is_dir() {
                continue;
            }
            if source.exists() {
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| HarnessError::io(parent, e))?;
                }
                std::fs::copy(&source, &destination)
                    .map_err(|e| HarnessError::io(&destination, e))?;
            } else if destination.exists() {
                std::fs::remove_file(&destination)
                    .map_err(|e| HarnessError::io(&destination, e))?;
            }
            transferred.push(relative);
        }
        tracing::info!(
            sandbox = %self.root.display(),
            files = transferred.len(),
            "verified files transferred to the working tree"
        );
        Ok(transferred)
    }
    fn resolve(&self, path: &Path) -> Result<PathBuf, HarnessError> {
        let escapes = || HarnessError::PathEscapes {
            requested: path.to_path_buf(),
            root: self.root.clone(),
        };
        if path.is_absolute() {
            return Err(escapes());
        }
        // `..` is rejected *before* normalisation rather than after: a path that climbs out
        // and comes back (`sub/../../etc`) normalises back inside the jail and would pass a
        // post-hoc check, which is fine in practice but is exactly the shape an audit of
        // "the jail cannot be escaped" should not have to reason about.
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(escapes());
        }
        let target = self.root.join(path);
        if !target.starts_with(&self.root) {
            return Err(escapes());
        }
        Ok(target)
    }
}
