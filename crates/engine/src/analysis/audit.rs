//! Workspace line-count auditor.
//!
//! Scans workspace source files respecting `.gitignore` and default ignored
//! directories, flagging files exceeding line-count thresholds.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Severity classification of a line count violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuditSeverity {
    /// 500–799 lines: File is growing large and should be monitored.
    Warning,
    /// ≥ 800 lines: Violates the configured modularity ceiling and must be decomposed.
    Critical,
}

impl AuditSeverity {
    /// Human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Warning => "Warning",
            Self::Critical => "Critical",
        }
    }
}

/// A violation flagged during workspace auditing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditViolation {
    /// Path to the violating file.
    pub path: PathBuf,
    /// Total physical line count.
    pub line_count: usize,
    /// Number of top-level declarations identified.
    pub declaration_count: usize,
    /// Violation severity: "Warning" or "Critical".
    pub severity: AuditSeverity,
}

/// Configuration options for workspace auditing.
#[derive(Clone, Debug)]
pub struct AuditConfig {
    /// Maximum allowed lines before a warning or violation is reported (default: 500).
    pub max_lines: usize,
    /// Optional glob patterns or directory substrings to ignore.
    pub ignore_patterns: Vec<String>,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            max_lines: 500,
            ignore_patterns: Vec::new(),
        }
    }
}

impl AuditConfig {
    /// Creates a configuration with the given line ceiling.
    #[must_use]
    pub fn new(max_lines: usize) -> Self {
        Self {
            max_lines,
            ignore_patterns: Vec::new(),
        }
    }

    /// Resolves audit configuration from the workspace, overriding the threshold if explicitly provided.
    #[must_use]
    pub fn from_workspace_with_threshold(root: &Path, explicit_threshold: Option<usize>) -> Self {
        let config = crate::config::MiterConfig::load_from_workspace(root);
        let effective_threshold = explicit_threshold.or(config.audit.threshold).unwrap_or(500);
        Self::new(effective_threshold)
    }

    /// Adds ignore patterns to the configuration.
    #[must_use]
    pub fn with_ignores(mut self, ignores: impl IntoIterator<Item = String>) -> Self {
        self.ignore_patterns.extend(ignores);
        self
    }
}

/// Workspace line-count auditor.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceAuditor {
    config: AuditConfig,
}

impl WorkspaceAuditor {
    /// Creates an auditor with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: AuditConfig::default(),
        }
    }

    /// Creates an auditor with a custom configuration.
    #[must_use]
    pub fn with_config(config: AuditConfig) -> Self {
        Self { config }
    }

    /// Recursively audits the given root directory and emits all violations.
    ///
    /// # Errors
    /// Returns an I/O error if directory traversal fails critically.
    pub fn audit_root(&self, root: &Path) -> Result<Vec<AuditViolation>, std::io::Error> {
        let mut violations = Vec::new();
        let mut ignored_paths = HashSet::new();
        self.load_gitignores(root, &mut ignored_paths);

        let miter_config = crate::config::MiterConfig::load_from_workspace(root);
        let glob_matcher = miter_config.build_audit_matcher().ok();

        let mut queue = vec![root.to_path_buf()];
        while let Some(dir) = queue.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if is_ignored_dir(file_name)
                    || self.is_ignored(&path, root, &ignored_paths, glob_matcher.as_ref())
                {
                    continue;
                }

                let Ok(file_type) = entry.file_type() else {
                    continue;
                };

                if file_type.is_dir() {
                    queue.push(path);
                } else if file_type.is_file()
                    && is_supported_source(&path)
                    && let Some(violation) = self.audit_file(&path)
                {
                    violations.push(violation);
                }
            }
        }

        violations.sort_by_key(|a| std::cmp::Reverse(a.line_count));
        Ok(violations)
    }

    /// Audits an individual file, returning an [`AuditViolation`] if `line_count >= max_lines`.
    pub fn audit_file(&self, path: &Path) -> Option<AuditViolation> {
        if crate::polyglot::env::is_env_file(path) {
            return None;
        }
        let content = fs::read_to_string(path).ok()?;
        let line_count = content.lines().count();
        if line_count < self.config.max_lines {
            return None;
        }

        let declaration_count = count_declarations(path, &content);
        let severity = if line_count >= 800 {
            AuditSeverity::Critical
        } else {
            AuditSeverity::Warning
        };

        Some(AuditViolation {
            path: path.to_path_buf(),
            line_count,
            declaration_count,
            severity,
        })
    }

    fn load_gitignores(&self, root: &Path, ignored: &mut HashSet<PathBuf>) {
        let gitignore_path = root.join(".gitignore");
        if let Ok(content) = fs::read_to_string(gitignore_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                    continue;
                }
                let clean = trimmed.trim_end_matches('/');
                ignored.insert(PathBuf::from(clean));
            }
        }
    }

    fn is_ignored(
        &self,
        path: &Path,
        root: &Path,
        gitignored: &HashSet<PathBuf>,
        glob_matcher: Option<&globset::GlobSet>,
    ) -> bool {
        if let Some(matcher) = glob_matcher
            && crate::config::is_path_excluded(matcher, path, root)
        {
            return true;
        }
        let path_str = path.to_string_lossy();
        self.config
            .ignore_patterns
            .iter()
            .any(|p| path_str.contains(p))
            || gitignored
                .iter()
                .any(|i| path_str.contains(i.to_string_lossy().as_ref()))
    }
}

mod walk;

use walk::{count_declarations, is_ignored_dir, is_supported_source};
