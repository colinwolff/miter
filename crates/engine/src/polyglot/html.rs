//! HTML: the addressable names of a document.
//!
//! A document declares by *attribute*: an `id` is a name a stylesheet, a script or an anchor can
//! point at, and a custom element (`<user-card>`) is a component. Both become symbols, so
//! `miter lookup user-card` finds the component and `miter references '#main'` finds what points
//! at the id.
//!
//! The scanner is a tag reader rather than a parser: markup that does not close its tags still
//! yields the names it declares, and a `script` or `style` body is skipped because its `<` is
//! code rather than markup.

use std::collections::BTreeMap;
use std::path::Path;

use super::{Language, SymbolKind, UniversalSymbol, common};

/// Tags whose presence is a semantic landmark of the page.
const LANDMARKS: &[&str] = &["header", "nav", "main", "section", "form", "dialog"];

/// Extracts the addressable names of an HTML document.
#[must_use]
pub fn extract_html_symbols(path: &Path, source: &str) -> Vec<UniversalSymbol> {
    let module = common::module_path(path);
    let mut symbols = Vec::new();
    // A landmark is declared once, at its first occurrence: a page with four `<section>`s has one
    // landmark, not four declarations that only a line number tells apart.
    let mut landmarks: BTreeMap<&str, (usize, usize)> = BTreeMap::new();

    for tag in tags(source) {
        if let Some((id, offset)) = tag.attribute("id")
            && !id.is_empty()
        {
            symbols.push(declaration(
                path,
                &module,
                id,
                SymbolKind::Property,
                format!("ID {id} <{}>", tag.name),
                tag.line,
                offset,
            ));
        }
        if is_custom_element(tag.name) {
            symbols.push(declaration(
                path,
                &module,
                tag.name,
                SymbolKind::Class,
                format!("ELEMENT <{}>", tag.name),
                tag.line,
                tag.start,
            ));
        }
        if LANDMARKS.contains(&tag.name) {
            landmarks.entry(tag.name).or_insert((tag.line, tag.start));
        }
    }

    for (name, (line, start)) in landmarks {
        symbols.push(declaration(
            path,
            &module,
            name,
            SymbolKind::Module,
            format!("LANDMARK <{name}>"),
            line,
            start,
        ));
    }
    // Source order, so the outline reads the way the document does.
    symbols.sort_by(|left, right| (left.span.0, &left.name).cmp(&(right.span.0, &right.name)));
    symbols
}

/// Builds one declaration.
fn declaration(
    path: &Path,
    module: &str,
    name: &str,
    kind: SymbolKind,
    signature: String,
    line: usize,
    start: usize,
) -> UniversalSymbol {
    UniversalSymbol {
        path: common::dotted_path(module, &[], name),
        name: name.to_owned(),
        language: Language::Html,
        kind,
        depth: if kind == SymbolKind::Module { 1 } else { 3 },
        docstring: None,
        signature,
        span: (start, start),
        file: path.to_path_buf(),
        line,
        params: Vec::new(),
        return_type: None,
        exported: true,
    }
}

/// One start tag, as the scanner read it.
struct Tag<'a> {
    /// Lowercased tag name.
    name: &'a str,
    /// Attributes in source order, as `(byte offset, name, value)`.
    attrs: Vec<(usize, &'a str, &'a str)>,
    /// One-based line the tag opens on.
    line: usize,
    /// Byte offset the tag opens at.
    start: usize,
}

impl Tag<'_> {
    /// The value of `key` and the offset it sits at, when the tag carries it.
    fn attribute(&self, key: &str) -> Option<(&str, usize)> {
        self.attrs
            .iter()
            .find(|(_, name, _)| name.eq_ignore_ascii_case(key))
            .map(|(offset, _, value)| (*value, *offset))
    }
}

/// Reads every start tag in `source`.
fn tags(source: &str) -> Vec<Tag<'_>> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while let Some(offset) = source[index..].find('<') {
        let start = index + offset;
        let rest = &source[start + 1..];
        // A comment is skipped whole: `<` and `>` inside its prose are text, not markup.
        if rest.starts_with("!--") {
            index = source[start..]
                .find("-->")
                .map_or(source.len(), |end| start + end + 3);
            continue;
        }
        // A declaration (`<!DOCTYPE …>`) has no tag name and no attributes.
        if rest.starts_with('!') {
            index = start + 1 + rest.find('>').map_or(rest.len(), |end| end + 1);
            continue;
        }
        // A start tag opens only when `<` is followed by an ASCII letter; `<-`, `<=`,
        // `< 250` and a closing `</…` are text, not tags.
        if !rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
            index = start + 1;
            continue;
        }
        let name_len = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':'))
            .unwrap_or(rest.len());
        let name = &rest[..name_len];
        let body_len = rest.find('>').unwrap_or(rest.len());
        out.push(Tag {
            name,
            attrs: attributes(&rest[name_len..body_len], start + 1 + name_len),
            line: common::line_at(source, start),
            start,
        });
        index = start + 1 + body_len + 1;
        // A `script` or `style` body is code: its `<` is an operator, not a tag.
        if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("style") {
            let lowered = source[start..].to_ascii_lowercase();
            if let Some(at) = lowered.find(&format!("</{}", name.to_ascii_lowercase())) {
                index = start + at;
            }
        }
    }
    out
}

/// `true` for a custom element name: a lowercase start, a mandatory hyphen and ASCII tag
/// characters only, as the HTML spec defines them.
///
/// The guard is what stops a bare `<` in text — `<-`, `<=`, `< 250` — from ever registering as a
/// component, because none of those carry a lowercase letter before the hyphen.
fn is_custom_element(name: &str) -> bool {
    matches!(name.as_bytes().first(), Some(byte) if byte.is_ascii_lowercase())
        && name.contains('-')
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

/// Reads the `name="value"` pairs of one tag body, with the offset each value starts at.
///
/// `base` is where the body begins in the whole document, so the offsets are document-relative.
fn attributes(body: &str, base: usize) -> Vec<(usize, &str, &str)> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while let Some(offset) = body[index..].find('=') {
        let eq = index + offset;
        let name_end = body[..eq].trim_end().len();
        let name_start = body[..name_end]
            .rfind(|c: char| c.is_whitespace())
            .map_or(0, |at| at + 1);
        let mut cursor = eq + 1;
        while body[cursor..].starts_with(char::is_whitespace) {
            cursor += 1;
        }
        let quote = body[cursor..]
            .chars()
            .next()
            .filter(|c| *c == '"' || *c == '\'');
        let value_start = if quote.is_some() { cursor + 1 } else { cursor };
        let value_end = match quote {
            Some(mark) => body[value_start..]
                .find(mark)
                .map_or(body.len(), |at| value_start + at),
            None => body[value_start..]
                .find(char::is_whitespace)
                .map_or(body.len(), |at| value_start + at),
        };
        let name = &body[name_start..name_end];
        if !name.is_empty() {
            out.push((base + value_start, name, &body[value_start..value_end]));
        }
        index = (value_end + 1).min(body.len());
        if index >= body.len() {
            break;
        }
    }
    out
}

/// Every attribute of every start tag, as `(byte offset, attribute name, value)`.
///
/// The tag reader lives here; what an attribute *means* as a reference is decided by the caller.
#[must_use]
pub fn attributes_of(source: &str) -> Vec<(usize, &str, &str)> {
    tags(source).into_iter().flat_map(|tag| tag.attrs).collect()
}

#[cfg(test)]
mod cases;
