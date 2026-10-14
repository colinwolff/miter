//! References in the declarative formats: markup, stylesheets and SQL.
//!
//! A code reference names a *symbol* (`store::Engine`); a markup reference names a *token* — a CSS
//! class from an HTML `class` attribute, a custom property from a `var()` call, a table from a
//! `FROM` clause. They share [`ReferenceKind::Usage`] because none of them calls or types anything:
//! each one uses a name that was declared somewhere else, which is exactly what
//! `miter references <name>` answers.

use miter_storage::schema::{ReferenceEntry, ReferenceKind};

/// References in an HTML document: class names, fragment ids and `for=` targets.
#[must_use]
pub(super) fn html(file_path: &str, source: &str) -> Vec<(String, ReferenceEntry)> {
    let mut out = Vec::new();
    for (offset, name, value) in crate::polyglot::html::attributes_of(source) {
        match name.to_ascii_lowercase().as_str() {
            // A class is declared in a stylesheet, where it is spelled `.card`.
            "class" => {
                let mut cursor = offset;
                for token in value.split_whitespace() {
                    out.push(entry(file_path, format!(".{token}"), cursor, source));
                    cursor += token.len() + 1;
                }
            }
            // `href="#main"` and `for="signup"` both name an id, which HTML declares bare.
            "href" => {
                if let Some(fragment) = value.strip_prefix('#') {
                    out.push(entry(file_path, fragment.to_owned(), offset + 1, source));
                }
            }
            "for" => out.push(entry(file_path, value.to_owned(), offset, source)),
            _ => {}
        }
    }
    out
}

/// References in a stylesheet: every `var(--token)` call.
///
/// The scan runs over the comment-masked text so a `var(--ghost)` written inside a comment is
/// not a reference, while the offsets (and so the line numbers) stay exact because masking keeps
/// the source length.
#[must_use]
pub(super) fn css(file_path: &str, source: &str) -> Vec<(String, ReferenceEntry)> {
    let masked = crate::polyglot::css::mask_comments(source);
    let mut out = Vec::new();
    let mut index = 0usize;
    while let Some(found) = masked[index..].find("var(") {
        let call = index + found;
        let open = call + "var(".len();
        let close = masked[open..]
            .find(')')
            .map_or(masked.len(), |at| open + at);
        let argument = masked[open..close].trim();
        // `var(--brand, #3b82f6)` — the fallback after the comma is a value, not a reference.
        let token = argument.split(',').next().unwrap_or("").trim();
        if token.starts_with("--") {
            out.push(entry(file_path, token.to_owned(), open, source));
        }
        index = close.max(open);
    }
    out
}

/// References in SQL: every table a `FROM`, `JOIN` or `REFERENCES` clause names.
#[must_use]
pub(super) fn sql(file_path: &str, source: &str) -> Vec<(String, ReferenceEntry)> {
    const KEYWORDS: [&str; 3] = ["FROM", "JOIN", "REFERENCES"];
    let mut out = Vec::new();
    for (offset, word) in words(source) {
        let upper = word.to_ascii_uppercase();
        if !KEYWORDS.contains(&upper.as_str()) {
            continue;
        }
        if let Some((at, table)) = next_identifier(source, offset + word.len()) {
            out.push(entry(file_path, table, at, source));
        }
    }
    out
}

/// References in a Dockerfile: every `COPY --from=<stage>`.
///
/// A multi-stage build's second stage copies from the first by *name*, so the name is a reference
/// the index has to hold for `miter references builder` to answer.
#[must_use]
pub(super) fn docker(file_path: &str, source: &str) -> Vec<(String, ReferenceEntry)> {
    let Ok(dockerfile) = dockerfile_parser::Dockerfile::parse(source) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for instruction in dockerfile.instructions {
        let dockerfile_parser::Instruction::Copy(copy) = instruction else {
            continue;
        };
        for flag in &copy.flags {
            if flag.name.content.eq_ignore_ascii_case("from") {
                out.push(entry(
                    file_path,
                    flag.value.content.clone(),
                    flag.value.span.start,
                    source,
                ));
            }
        }
    }
    out
}

/// Builds one `Usage` entry for `name` at `offset`.
fn entry(file_path: &str, name: String, offset: usize, source: &str) -> (String, ReferenceEntry) {
    let line = crate::polyglot::common::line_at(source, offset);
    let length = name.len();
    (
        name,
        ReferenceEntry {
            file_path: file_path.to_owned(),
            byte_range: (offset, offset + length),
            line_range: (line, line),
            enclosing_symbol: None,
            reference_kind: ReferenceKind::Usage,
        },
    )
}

/// The identifier-like run after `offset`, skipping whitespace.
fn next_identifier(source: &str, offset: usize) -> Option<(usize, String)> {
    let rest = source.get(offset..)?;
    let start = offset + rest.len() - rest.trim_start().len();
    let text = &source[start..];
    let length = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(text.len());
    if length == 0 {
        return None;
    }
    Some((start, text[..length].to_owned()))
}

/// Every identifier-like word in `source`, skipping strings and comments.
fn words(source: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < source.len() {
        let rest = &source[index..];
        let Some(character) = rest.chars().next() else {
            break;
        };
        if matches!(character, '\'' | '"' | '`') {
            index += 1 + rest[1..].find(character).unwrap_or(rest.len() - 1);
            continue;
        }
        if rest.starts_with("--") {
            index = rest.find('\n').map_or(source.len(), |at| index + at + 1);
            continue;
        }
        if rest.starts_with("/*") {
            index = rest.find("*/").map_or(source.len(), |at| index + at + 2);
            continue;
        }
        if character.is_ascii_alphanumeric() || character == '_' {
            let length = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            out.push((index, &source[index..index + length]));
            index += length;
            continue;
        }
        index += character.len_utf8();
    }
    out
}

#[cfg(test)]
mod cases;
