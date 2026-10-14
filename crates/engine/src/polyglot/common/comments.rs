//! Comment and docstring harvesting.

/// `true` for the node kinds a grammar uses for a comment.
pub(super) fn is_comment_kind(kind: &str) -> bool {
    matches!(kind, "comment" | "line_comment" | "block_comment")
}

/// `true` for a Rust `///` or `/** */` doc comment (and not `//!`/`/*!`).
///
/// The inner forms document the *containing* item, so attaching them to the next
/// declaration would credit it with a doc comment that was never about it.
pub(crate) fn is_rust_doc(raw: &str) -> bool {
    let trimmed = raw.trim_start();
    if trimmed.starts_with("//!") || trimmed.starts_with("/*!") {
        return false;
    }
    trimmed.starts_with("///") || trimmed.starts_with("/**")
}

/// `true` for a JSDoc block: `/** … */`.
///
/// A `//` line comment is not a doc comment, and neither is `/* … */`: TypeScript's tooling
/// (and its editor hover) read JSDoc only from the double-star form, so a universal visitor
/// that accepted the others would report documentation the language does not.
pub(crate) fn is_jsdoc(raw: &str) -> bool {
    raw.trim_start().starts_with("/**")
}

/// Removes `///`, `//`, `/**`, `*/` and a leading `*` from every line of a comment block.
pub(super) fn strip_comment_markers(block: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in block.lines() {
        let mut text = line.trim().to_string();
        for marker in ["///", "//!", "/**", "/*!", "*/", "//", "/*"] {
            if let Some(rest) = text.strip_prefix(marker) {
                text = rest.trim_start().to_string();
                break;
            }
        }
        // A one-line block comment (`/** Doc. */`) closes on the same line it opens.
        if let Some(rest) = text.strip_suffix("*/") {
            text = rest.trim_end().to_string();
        }
        if text.starts_with('*') {
            text = text.trim_start_matches('*').trim_start().to_string();
        }
        lines.push(text.trim_end().to_string());
    }
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}
