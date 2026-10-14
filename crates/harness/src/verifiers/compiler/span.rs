//! Turning a `(line, column)` pair into a byte span.
//!
//! `tsc` reports a position but no offsets, and a span with no offsets cannot be sliced, so
//! the position is measured against the file on disk here.

use std::path::Path;

/// The byte offset a `(line, column)` pair names in `text`, 1-based on both axes.
///
/// tsc reports line and column but no byte offsets, and a span with no offsets cannot be
/// sliced, so this is how a tsc position becomes a byte range. Returns `None` when the
/// position is past the end, which the caller reports as unresolved rather than guessing.
#[must_use]
pub(super) fn offset_of(text: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }
    let mut offset = 0usize;
    for (current_line, candidate) in (1usize..).zip(text.split_inclusive('\n')) {
        if current_line == line {
            // Columns count characters, not bytes: a line holding a two-byte codepoint has more
            // columns than bytes, and slicing mid-character panics rather than returning
            // nonsense.
            let end_of_line = candidate.trim_end_matches('\n').len();
            let target = match column {
                0 => 0,
                _ => candidate
                    .char_indices()
                    .nth(column - 1)
                    .map_or(end_of_line, |(index, _)| index),
            };
            return Some(offset + target);
        }
        offset += candidate.len();
    }
    None
}

/// Widens a parsed tsc position into a span, measured against the file on disk.
///
/// A zero-width span when the file cannot be read: the line and column tsc gave are still
/// true, and a byte range measured against a file nobody read would be a fabrication.
#[must_use]
pub(super) fn resolve_span(root: &Path, span: &crate::DiagnosticSpan) -> crate::DiagnosticSpan {
    let mut resolved = span.clone();
    let Ok(text) = std::fs::read_to_string(root.join(&span.file_path)) else {
        return resolved;
    };
    let Some(byte_start) = offset_of(&text, span.line_start, span.column_start) else {
        return resolved;
    };
    // tsc gives only a start position, so the span runs from there to the end of the line:
    // the end of the line is the boundary tsc did not report, and a narrower one would be a
    // guess about where its own caret stopped.
    let byte_end = offset_of(&text, span.line_end, usize::MAX).unwrap_or(byte_start);
    resolved.byte_start = byte_start;
    resolved.byte_end = byte_end.max(byte_start);
    resolved
}
