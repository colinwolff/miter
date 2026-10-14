//! Directory filtering and declaration counting for the auditor.

use std::path::Path;

pub(super) fn is_ignored_dir(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "target" | "node_modules" | "dist" | "build" | "vendor" | "bin"
        )
}

pub(super) fn is_supported_source(path: &Path) -> bool {
    // The ceiling is a *code* ceiling: a README or a `.env` key file is not a monolith to
    // decompose, so documentation is deliberately not counted.
    crate::polyglot::Language::from_path(path)
        .is_some_and(crate::polyglot::Language::is_line_counted)
}

pub(super) fn count_declarations(path: &Path, content: &str) -> usize {
    if let Ok(Some(symbols)) = crate::polyglot::parse_detected(path, content) {
        symbols.iter().filter(|s| s.depth <= 3).count()
    } else {
        content
            .lines()
            .map(str::trim)
            .filter(|t| {
                t.starts_with("fn ")
                    || t.starts_with("pub fn ")
                    || t.starts_with("struct ")
                    || t.starts_with("pub struct ")
                    || t.starts_with("enum ")
                    || t.starts_with("pub enum ")
                    || t.starts_with("export function ")
                    || t.starts_with("export class ")
                    || t.starts_with("CREATE TABLE ")
            })
            .count()
    }
}
