//! Comment masking that preserves byte offsets and line numbers.
//!
//! A stylesheet scanner that reads the selector list from the text before a `{` must not read
//! the prose of a comment as a selector: `/* docs/style.css */` above a rule would otherwise
//! declare a phantom `.css` class, and a SCSS `// theme.config.css` a phantom `.config`. Masking
//! replaces every non-newline byte inside a comment with a space, so the masked text is exactly
//! as long as the source and every byte offset — and therefore every one-based line number —
//! still points at the same character. Strings and `url(...)` tokens are left intact: their dots
//! are values, not selectors, and the selector reader skips them explicitly.

/// The source with `/* … */` and `// …` comments blanked to spaces, newlines preserved.
pub(crate) fn mask_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'"' || byte == b'\'' {
            index = skip_quoted(bytes, index);
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index = blank_block(&mut out, bytes, index);
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index = blank_line(&mut out, bytes, index);
        } else if url_open_at(bytes, index) {
            index = skip_url(bytes, index);
        } else {
            index += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| source.to_owned())
}

/// The index just past the string literal that opens at `open`.
pub(super) fn skip_quoted(bytes: &[u8], open: usize) -> usize {
    let quote = bytes[open];
    let mut index = open + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            byte if byte == quote => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// Blanks `/* … */`, keeping newlines, and returns the index just past `*/`.
fn blank_block(out: &mut [u8], bytes: &[u8], open: usize) -> usize {
    let mut index = open;
    while index < bytes.len() {
        if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
            out[index] = b' ';
            out[index + 1] = b' ';
            return index + 2;
        }
        if bytes[index] != b'\n' {
            out[index] = b' ';
        }
        index += 1;
    }
    bytes.len()
}

/// Blanks `// …` up to (but not including) the line break.
fn blank_line(out: &mut [u8], bytes: &[u8], open: usize) -> usize {
    let mut index = open;
    while index < bytes.len() && bytes[index] != b'\n' {
        out[index] = b' ';
        index += 1;
    }
    index
}

/// `true` when a `url(` function opens at `index` (and not inside a longer identifier).
pub(super) fn url_open_at(bytes: &[u8], index: usize) -> bool {
    let boundary = index == 0 || !is_ident_byte(bytes[index - 1]);
    boundary
        && bytes.len() >= index + 4
        && bytes[index..index + 3].eq_ignore_ascii_case(b"url")
        && bytes[index + 3] == b'('
}

/// The index just past the `)` that closes the `url(...)` opening at `open`.
pub(super) fn skip_url(bytes: &[u8], open: usize) -> usize {
    match bytes[open..].iter().position(|byte| *byte == b')') {
        Some(at) => open + at + 1,
        None => bytes.len(),
    }
}

/// `true` for a byte that can sit inside an identifier.
fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}
