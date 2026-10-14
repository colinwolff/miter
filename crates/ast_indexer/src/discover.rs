//! Source-tree discovery.
//!
//! Uses only `std::fs` (no `walkdir` dependency, in line with AGENTS.md
//! *"no external crates added without explicit approval"*). Traversal is
//! iterative, sorted, and skips VCS/build directories so that the ingested tree
//! is reproducible.

use std::path::{Path, PathBuf};

use crate::IndexError;
use crate::extract::Language;

/// A discovered source file, annotated with its depth below the scan root.
///
/// `dir_depth` is the number of directory levels below `root`, so a file
/// directly inside `root` has depth `0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// Absolute or root-relative path exactly as discovered.
    pub path: PathBuf,
    /// Language inferred from the file extension.
    pub language: Language,
    /// Directory nesting below the scan root.
    pub dir_depth: usize,
}

/// Directory names that are never descended into.
pub const IGNORED_DIRECTORIES: &[&str] = &["target", "node_modules", ".git", ".hg", ".svn"];

/// Recursively collects the source files below `root`.
///
/// Files whose extension maps to a [`Language`] that is not compiled into this
/// build are skipped, hidden directories are skipped, and the result is sorted
/// by path so that indexing is deterministic.
///
/// # Errors
///
/// Propagates I/O failures of [`std::fs::read_dir`] or of the entry metadata.
pub fn discover_sources(root: &Path) -> Result<Vec<SourceFile>, IndexError> {
    let mut sources = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0usize)];

    while let Some((directory, depth)) = pending.pop() {
        let entries = std::fs::read_dir(&directory)?;
        let mut files = Vec::new();
        let mut directories = Vec::new();

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                if is_traversable(&path) {
                    directories.push(path);
                }
            } else if file_type.is_file() {
                files.push(path);
            }
        }

        for path in files {
            if let Some(language) = Language::from_path(&path)
                && language.is_enabled()
            {
                sources.push(SourceFile {
                    path,
                    language,
                    dir_depth: depth,
                });
            }
        }
        for path in directories {
            pending.push((path, depth + 1));
        }
    }

    sources.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(sources)
}

/// `true` when the directory is neither hidden nor in [`IGNORED_DIRECTORIES`].
fn is_traversable(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    !name.starts_with('.') && !IGNORED_DIRECTORIES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_and_build_directories_are_skipped() {
        assert!(!is_traversable(Path::new("/tmp/.git")));
        assert!(!is_traversable(Path::new("/tmp/target")));
        assert!(!is_traversable(Path::new("/tmp/node_modules")));
        assert!(is_traversable(Path::new("/tmp/crates")));
    }

    #[test]
    fn language_detection_by_extension() {
        assert_eq!(
            Language::from_path(Path::new("a/b/parse.rs")),
            Some(Language::Rust)
        );
        assert_eq!(
            Language::from_path(Path::new("a/b/parse.ts")),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::from_path(Path::new("a/b/parse.mts")),
            Some(Language::TypeScript)
        );
        // the rest of the polyglot surface.
        assert_eq!(
            Language::from_path(Path::new("a/b/parse.tsx")),
            Some(Language::Tsx)
        );
        assert_eq!(
            Language::from_path(Path::new("a/b/parse.js")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_path(Path::new("a/b/parse.jsx")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_path(Path::new("a/b/parse.py")),
            Some(Language::Python)
        );
        // A `.d.ts` file is TypeScript, not a file with the extension `ts` in a
        // directory called `d` — the name is checked before the extension.
        assert_eq!(
            Language::from_path(Path::new("a/b/parse.d.ts")),
            Some(Language::TypeScript)
        );
        assert_eq!(Language::from_path(Path::new("a/b/README.md")), None);
        assert_eq!(Language::from_path(Path::new("a/b/no_extension")), None);
    }

    #[test]
    fn the_three_typescript_members_are_one_family() {
        for language in [Language::TypeScript, Language::Tsx, Language::JavaScript] {
            assert_eq!(language.family(), "typescript", "{}", language.name());
            assert!(language.is_enabled());
        }
        assert_eq!(Language::Rust.family(), "rust");
        assert_eq!(Language::Python.family(), "python");
        assert!(Language::Python.is_enabled());
    }
}
