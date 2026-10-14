//! Interface skeleton extraction.
//!
//! For Rust and TypeScript/JavaScript this parses with Tree-Sitter and strips
//! function/method implementation bodies (~90% token compression).
//!
//! A *declarative* file — Markdown, SQL, a Dockerfile, a manifest — has no bodies to strip: its
//! implementation **is** its declarations. Its skeleton is therefore the declaration list itself,
//! one line each, which is what `miter skeleton README.md` should print and what makes the command
//! useful on documentation rather than dumping the file.

mod walk;

use std::path::Path;

use walk::collect_function_bodies;

/// Options configuring AST skeleton extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkeletonOptions {
    /// Whether to replace function bodies with `;` (trait-style) or `{ /* ... */ }`.
    pub use_stub_body: bool,
}

impl Default for SkeletonOptions {
    fn default() -> Self {
        Self {
            use_stub_body: true,
        }
    }
}

/// Result of extracting an AST interface skeleton from a source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkeletonResult {
    /// Stripped interface skeleton text.
    pub skeleton_text: String,
    /// Original line count.
    pub original_lines: usize,
    /// Skeleton line count.
    pub skeleton_lines: usize,
    /// Original character count.
    pub original_chars: usize,
    /// Skeleton character count.
    pub skeleton_chars: usize,
}

impl SkeletonResult {
    /// Compression ratio achieved based on characters: `1.0 - (skeleton / original)`.
    #[must_use]
    pub fn compression_ratio(&self) -> f64 {
        if self.original_chars == 0 {
            0.0
        } else {
            1.0 - (self.skeleton_chars as f64 / self.original_chars as f64)
        }
    }
}

/// Detects `miter_ast::Language` from file path extension.
fn detect_language(path: &Path) -> miter_ast::Language {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext.to_ascii_lowercase().as_str() {
            "ts" | "mts" | "cts" => miter_ast::Language::TypeScript,
            "tsx" => miter_ast::Language::Tsx,
            "js" | "jsx" | "mjs" | "cjs" => miter_ast::Language::JavaScript,
            _ => miter_ast::Language::Rust,
        }
    } else {
        miter_ast::Language::Rust
    }
}

/// Strips implementation bodies from `source` for the file at `path`.
#[must_use]
pub fn extract_skeleton(path: &Path, source: &str, options: SkeletonOptions) -> SkeletonResult {
    let original_lines = source.lines().count();
    let original_chars = source.len();

    if crate::polyglot::is_declarative_source(path) {
        return declarative_skeleton(path, source, original_lines, original_chars);
    }

    let lang = detect_language(path);
    let Ok(grammar) = miter_ast::grammar_for(lang) else {
        return empty_result(source, original_lines, original_chars);
    };

    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&grammar).is_err() {
        return empty_result(source, original_lines, original_chars);
    }

    let Some(tree) = parser.parse(source, None) else {
        return empty_result(source, original_lines, original_chars);
    };

    let mut blocks = Vec::new();
    collect_function_bodies(tree.root_node(), lang, &mut blocks);

    if blocks.is_empty() {
        let scrubbed = crate::security::scrub_source_text(source);
        let skeleton_lines = scrubbed.lines().count();
        let skeleton_chars = scrubbed.len();
        return SkeletonResult {
            skeleton_text: scrubbed,
            original_lines,
            skeleton_lines,
            original_chars,
            skeleton_chars,
        };
    }

    // Sort blocks descending by start byte to safely replace backwards without invalidating offsets
    blocks.sort_by_key(|a| std::cmp::Reverse(a.0));
    // Deduplicate any overlapping or nested blocks (keep outer-most)
    let mut non_overlapping = Vec::new();
    for (start, end) in blocks {
        let skip = match non_overlapping.last() {
            Some(&(last_start, _)) => end > last_start,
            None => false,
        };
        if skip {
            continue;
        }
        non_overlapping.push((start, end));
    }

    let stub = if options.use_stub_body {
        "{ /* ... */ }"
    } else {
        ";"
    };

    let mut out = source.to_string();
    for (start, end) in non_overlapping {
        if start <= end && end <= out.len() {
            out.replace_range(start..end, stub);
        }
    }

    // Clean up excessive empty lines
    let mut clean_lines = Vec::new();
    let mut consecutive_blank = 0;
    for line in out.lines() {
        if line.trim().is_empty() {
            consecutive_blank += 1;
            if consecutive_blank <= 1 {
                clean_lines.push("");
            }
        } else {
            consecutive_blank = 0;
            clean_lines.push(line);
        }
    }

    let skeleton_text = crate::security::scrub_source_text(&clean_lines.join("\n"));
    let skeleton_lines = skeleton_text.lines().count();
    let skeleton_chars = skeleton_text.len();

    SkeletonResult {
        skeleton_text,
        original_lines,
        skeleton_lines,
        original_chars,
        skeleton_chars,
    }
}

/// The skeleton of a declarative file: its declarations, one line each.
///
/// A file this crate cannot read yields the scrubbed source rather than an empty string: an empty
/// skeleton would claim the file declares nothing, which is a different statement.
fn declarative_skeleton(
    path: &Path,
    source: &str,
    original_lines: usize,
    original_chars: usize,
) -> SkeletonResult {
    let Some(symbols) = crate::polyglot::declarative_symbols(path, source) else {
        return empty_result(source, original_lines, original_chars);
    };
    if symbols.is_empty() {
        return empty_result(source, original_lines, original_chars);
    }

    let mut out = String::new();
    for symbol in &symbols {
        out.push_str(&format!(
            "{:>5}  {}
",
            symbol.line, symbol.signature
        ));
    }
    let skeleton_text = crate::security::scrub_source_text(&out);
    SkeletonResult {
        skeleton_lines: skeleton_text.lines().count(),
        skeleton_chars: skeleton_text.len(),
        skeleton_text,
        original_lines,
        original_chars,
    }
}

fn empty_result(source: &str, lines: usize, chars: usize) -> SkeletonResult {
    let scrubbed = crate::security::scrub_source_text(source);
    let skeleton_lines = scrubbed.lines().count();
    let skeleton_chars = scrubbed.len();
    SkeletonResult {
        skeleton_text: scrubbed,
        original_lines: lines,
        skeleton_lines,
        original_chars: chars,
        skeleton_chars,
    }
}
