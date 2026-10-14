//! CSS and SCSS: the names a stylesheet declares.
//!
//! A stylesheet declares three kinds of name — a custom property (`--brand`), a selector
//! (`.card`, `#main`) and an at-rule (`@keyframes fade-in`, `@layer utilities`) — and each is a
//! token a template or another rule refers to by name. Declaring them as symbols is what makes
//! `miter references --brand` able to trace a design token across a project.
//!
//! Each name is declared **once**, at its first occurrence: a class used in twelve rules is one
//! class, and twelve declarations that only a line number tells apart is noise, not an index.

use std::collections::BTreeMap;
use std::path::Path;

use super::{Language, SymbolKind, UniversalSymbol, common};

/// Extracts the names a stylesheet declares.
#[must_use]
pub fn extract_css_symbols(path: &Path, source: &str) -> Vec<UniversalSymbol> {
    let module = common::module_path(path);
    let mut declared: BTreeMap<String, (usize, SymbolKind, String)> = BTreeMap::new();

    for (offset, name, value) in custom_properties(source) {
        let signature = format!(
            "{name}: {}",
            crate::security::scrub_source_text(value.trim())
        );
        declared.entry(name.to_owned()).or_insert((
            common::line_at(source, offset),
            SymbolKind::Property,
            signature,
        ));
    }

    for (offset, keyword, name) in at_rules(source) {
        let kind = if keyword == "keyframes" {
            SymbolKind::Function
        } else {
            SymbolKind::Module
        };
        declared.entry(name.to_owned()).or_insert((
            common::line_at(source, offset),
            kind,
            format!("@{keyword} {name}"),
        ));
    }

    for (offset, token) in selectors(source) {
        let kind = if token.starts_with('#') {
            SymbolKind::Property
        } else {
            SymbolKind::Class
        };
        declared.entry(token.to_owned()).or_insert((
            common::line_at(source, offset),
            kind,
            token.to_owned(),
        ));
    }

    let mut symbols: Vec<UniversalSymbol> = declared
        .into_iter()
        .map(|(name, (line, kind, signature))| UniversalSymbol {
            path: common::dotted_path(&module, &[], &name),
            name,
            language: Language::Css,
            kind,
            depth: if kind == SymbolKind::Module { 1 } else { 3 },
            docstring: None,
            signature,
            span: (0, source.len()),
            file: path.to_path_buf(),
            line,
            params: Vec::new(),
            return_type: None,
            exported: true,
        })
        .collect();
    symbols.sort_by(|left, right| (left.line, &left.name).cmp(&(right.line, &right.name)));
    symbols
}

/// Every `--name: value` declaration, in source order.
fn custom_properties(source: &str) -> Vec<(usize, &str, &str)> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while let Some(offset) = source[index..].find("--") {
        let start = index + offset;
        let name_len = ident_len(&source[start..]);
        let after = start + name_len;
        if name_len > 2
            && source[after..].trim_start().starts_with(':')
            && !source[start..after].contains(char::is_whitespace)
        {
            let colon = after + source[after..].find(':').unwrap_or(0);
            let value_end = source[colon + 1..]
                .find([';', '}', '\n'])
                .map_or(source.len(), |at| colon + 1 + at);
            out.push((start, &source[start..after], &source[colon + 1..value_end]));
            index = value_end;
        } else {
            index = after;
        }
    }
    out
}

/// Every `@keyframes <name>` and `@layer <name>`, in source order.
fn at_rules(source: &str) -> Vec<(usize, &str, &str)> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while let Some(offset) = source[index..].find('@') {
        let start = index + offset;
        let keyword_len = source[start + 1..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .unwrap_or(source.len() - start - 1);
        let keyword = &source[start + 1..start + 1 + keyword_len];
        let mut cursor = start + 1 + keyword_len;
        while source[cursor..].starts_with(char::is_whitespace) {
            cursor += 1;
        }
        let name_len = ident_len(&source[cursor..]);
        let name = &source[cursor..cursor + name_len];
        if matches!(keyword, "keyframes" | "layer") && !name.is_empty() {
            out.push((start, keyword, name));
        }
        index = start + 1;
    }
    out
}

/// Every class and id token a selector list names, in source order.
fn selectors(source: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut index = 0usize;
    while let Some(offset) = source[index..].find('{') {
        let brace = index + offset;
        let prelude_start = source[..brace]
            .rfind(['}', ';', '{'])
            .map_or(0, |at| at + 1);
        let prelude = &source[prelude_start..brace];
        // An at-rule's prelude (`@media (max-width: 40em)`) names no selector.
        if !prelude.trim_start().starts_with('@') {
            for (at, token) in tokens(prelude) {
                out.push((prelude_start + at, token));
            }
        }
        index = brace + 1;
    }
    out
}

/// The `.class` and `#id` tokens of a selector prelude.
fn tokens(prelude: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let bytes = prelude.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'.' || bytes[index] == b'#' {
            let len = ident_len(&prelude[index + 1..]);
            if len > 0 {
                out.push((index, &prelude[index..index + 1 + len]));
                index += 1 + len;
                continue;
            }
        }
        index += 1;
    }
    out
}

/// The length of the identifier-like run at the start of `text`.
fn ident_len(text: &str) -> usize {
    text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(text.len())
}

#[cfg(test)]
mod cases;
