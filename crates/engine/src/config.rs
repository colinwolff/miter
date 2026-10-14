//! Workspace configuration model and pattern matchers (`miter.toml` / `.miter/config.toml`).

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Tool-specific configuration for indexing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexerConfig {
    /// Ignore patterns specific to the AST and polyglot indexer.
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Tool-specific configuration for file-length auditing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditToolConfig {
    /// Maximum line count threshold before triggering violations (default: 500).
    pub threshold: Option<usize>,
    /// Ignore patterns specific to the auditor.
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for AuditToolConfig {
    fn default() -> Self {
        Self {
            threshold: Some(500),
            exclude: Vec::new(),
        }
    }
}

impl AuditToolConfig {
    /// Resolves the effective threshold with a fallback of 500 lines.
    #[must_use]
    pub fn effective_threshold(&self) -> usize {
        self.threshold.unwrap_or(500)
    }
}

/// Top-level workspace configuration for `miter`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiterConfig {
    /// Global exclude patterns applied to all tools (indexing, auditing, etc.).
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Indexer-specific configuration.
    #[serde(default)]
    pub indexer: IndexerConfig,
    /// Audit-specific configuration.
    #[serde(default)]
    pub audit: AuditToolConfig,
}

/// Baseline certificate, private key, and credential file exclusion patterns.
pub const DEFAULT_SECURITY_EXCLUDES: &[&str] = &[
    "**/*.pem",
    "**/*.key",
    "**/*.p12",
    "**/*.pfx",
    "**/*.crt",
    "**/*id_rsa*",
    "**/*id_ed25519*",
    "**/*id_ecdsa*",
    "**/credentials.json",
    "**/service-account*.json",
    "**/.npmrc",
    "**/.pypirc",
];

impl MiterConfig {
    /// Loads configuration from the workspace root.
    ///
    /// Precedence:
    /// 1. `<workspace_root>/.miter/config.toml`
    /// 2. `<workspace_root>/miter.toml`
    /// 3. Default fallback
    #[must_use]
    pub fn load_from_workspace(workspace_root: &Path) -> Self {
        let dot_miter = workspace_root.join(".miter").join("config.toml");
        if dot_miter.is_file()
            && let Ok(content) = std::fs::read_to_string(&dot_miter)
            && let Ok(config) = toml::from_str(&content)
        {
            return config;
        }

        let root_config = workspace_root.join("miter.toml");
        if root_config.is_file()
            && let Ok(content) = std::fs::read_to_string(&root_config)
            && let Ok(config) = toml::from_str(&content)
        {
            return config;
        }

        Self::default()
    }

    /// Builds a [`GlobSet`] matcher combining baseline security exclusions, global exclusions,
    /// and indexer-specific exclusions.
    ///
    /// # Errors
    /// Returns a [`globset::Error`] if any pattern is invalid.
    pub fn build_indexer_matcher(&self) -> Result<GlobSet, globset::Error> {
        let security_patterns = DEFAULT_SECURITY_EXCLUDES.iter().map(|&s| s.to_string());
        let patterns = security_patterns
            .chain(self.exclude.iter().cloned())
            .chain(self.indexer.exclude.iter().cloned());
        build_matcher(patterns.collect::<Vec<_>>().iter())
    }

    /// Builds a [`GlobSet`] matcher combining baseline security exclusions, global exclusions,
    /// and audit-specific exclusions.
    ///
    /// # Errors
    /// Returns a [`globset::Error`] if any pattern is invalid.
    pub fn build_audit_matcher(&self) -> Result<GlobSet, globset::Error> {
        let security_patterns = DEFAULT_SECURITY_EXCLUDES.iter().map(|&s| s.to_string());
        let patterns = security_patterns
            .chain(self.exclude.iter().cloned())
            .chain(self.audit.exclude.iter().cloned());
        build_matcher(patterns.collect::<Vec<_>>().iter())
    }
}

/// Builds a [`GlobSet`] from an iterator of pattern strings.
fn build_matcher<'a>(
    patterns: impl IntoIterator<Item = &'a String>,
) -> Result<GlobSet, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let trimmed = pattern.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Normalize pattern: leading slashes stripped so relative matching works uniformly
        let clean = trimmed.strip_prefix('/').unwrap_or(trimmed);
        let glob = GlobBuilder::new(clean).literal_separator(true).build()?;
        builder.add(glob);
    }
    builder.build()
}

/// Tests if `path` (relative or absolute) matches the `GlobSet` when normalized relative to `root`.
/// Also tests whether directory prefixes match directory globs (e.g. `migrations/**`).
pub fn is_path_excluded(matcher: &GlobSet, path: &Path, workspace_root: &Path) -> bool {
    if matcher.is_empty() {
        return false;
    }
    let relative = path.strip_prefix(workspace_root).unwrap_or(path);
    if relative.as_os_str().is_empty() {
        return false;
    }
    let rel_str = relative.to_string_lossy();
    let normalized = rel_str.replace('\\', "/");
    let trimmed = normalized.trim_matches('/');

    // 1. Direct path match
    if matcher.is_match(&normalized) || matcher.is_match(trimmed) {
        return true;
    }

    // 2. Trailing slash (directory) match
    let dir_form = format!("{trimmed}/");
    if matcher.is_match(&dir_form) {
        return true;
    }

    // 3. Child within an excluded directory: check if any ancestor directory matches
    // e.g. path is "migrations/001.sql" and pattern is "migrations/**" or "migrations"
    let mut current = Path::new(trimmed);
    while let Some(parent) = current.parent() {
        if parent.as_os_str().is_empty() {
            break;
        }
        let parent_str = parent.to_string_lossy().replace('\\', "/");
        if matcher.is_match(&parent_str)
            || matcher.is_match(format!("{parent_str}/"))
            || matcher.is_match(format!("{parent_str}/**"))
        {
            return true;
        }
        current = parent;
    }

    // 4. If current is a directory itself, check if patterns like "dir/**" would match its contents
    let wildcard_form = format!("{trimmed}/**");
    if matcher.is_match(&wildcard_form) {
        return true;
    }

    false
}
