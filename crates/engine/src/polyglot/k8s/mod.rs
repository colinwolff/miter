//! Kubernetes, Docker Compose and Litestream manifest extraction.
//!
//! A YAML file declares things by name: a Kubernetes resource (`kind` + `metadata.name`), a Compose
//! service (`services.<name>`) and the environment each one is configured with. All three are
//! declarations a reader wants to find, so all three become symbols.
//!
//! Environment values are configuration, and configuration is where a credential lives, so every
//! literal value is redacted before it is recorded (see [`env`]). A `secretKeyRef` names a secret
//! rather than holding one, so the *reference* is recorded verbatim — losing it would hide which
//! secret a workload depends on.

mod compose;
mod env;
mod manifests;
#[cfg(test)]
mod tests;

use std::path::Path;

use serde::Deserialize;
use serde_yaml::Value;

use super::UniversalSymbol;
use super::common;

/// Extracts declarative manifest symbols from YAML source.
///
/// Every document in the stream is offered to each reader in turn; the first that recognises it
/// owns the document, so a Compose file is never also read as a Kubernetes manifest.
#[must_use]
pub fn extract_yaml_symbols(path: &Path, content: &str) -> Vec<UniversalSymbol> {
    let module = common::module_path(path);
    let starts = document_starts(content);
    let mut symbols = Vec::new();

    for (doc_index, document) in serde_yaml::Deserializer::from_str(content).enumerate() {
        let start = starts.get(doc_index).copied().unwrap_or(1);
        // One cursor per reader: the declaration list and the environment list are independent,
        // and sharing a cursor would make whichever ran second report the first one's last line.
        let mut cursor = start;
        let mut env_cursor = start;
        let Ok(value) = Value::deserialize(document) else {
            continue;
        };
        let Some(map) = value.as_mapping() else {
            continue;
        };

        let recognised =
            manifests::push_kubernetes(
                map,
                path,
                &module,
                content,
                doc_index,
                &mut cursor,
                &mut symbols,
            ) || manifests::push_litestream(map, path, &module, content, &mut cursor, &mut symbols)
                || compose::push_services(map, path, &module, content, &mut cursor, &mut symbols);

        if recognised {
            env::push_env(
                &value,
                path,
                &module,
                content,
                &mut env_cursor,
                &mut symbols,
            );
        }
    }

    symbols
}

/// A signature with every embedded secret scrubbed.
///
/// The extractors redact the values they *know* are sensitive; this is the belt-and-braces pass
/// that catches a credential sitting in a value that looked ordinary.
pub(super) fn scrub(signature: String) -> String {
    crate::security::scrub_source_text(&signature)
}

/// The one-based line of the next `key: value` occurrence at or after `from`.
///
/// `serde_yaml` reads a document but not the *position* of a field inside it, so the line is
/// recovered from the text: the first line at or after the previous declaration that spells
/// `key: value`. The cursor only ever moves forward, so documents stay in source order.
fn find_line(content: &str, from: usize, key: &str, value: &str) -> usize {
    let needle = format!("{key}:");
    for (index, line) in content.lines().enumerate().skip(from.saturating_sub(1)) {
        // A sequence entry spells its key after the dash (`- name: LOG_LEVEL`), so the dash is
        // stepped over before the key is matched.
        let trimmed = line.trim_start().trim_start_matches('-').trim_start();
        if trimmed.starts_with(&needle) && (value.is_empty() || line.contains(value)) {
            return index + 1;
        }
    }
    from
}

/// The one-based line each YAML document starts on.
///
/// `serde_yaml` yields documents in order but not where they began, so the separators are counted
/// here. A leading `---` opens the *first* document rather than starting a second one.
fn document_starts(content: &str) -> Vec<usize> {
    let lines: Vec<&str> = content.lines().collect();
    let leading = lines.first().is_some_and(|line| line.trim_end() == "---");
    let mut starts = vec![1usize];
    let mut seen = 0usize;
    for (position, line) in lines.iter().enumerate() {
        if line.trim_end() != "---" {
            continue;
        }
        seen += 1;
        if !(leading && seen == 1) {
            starts.push(position + 2);
        }
    }
    starts
}

/// Advances `cursor` to the line of `key: value` and returns it.
pub(super) fn take_line(
    content: &str,
    cursor: &mut usize,
    key: &str,
    value: &str,
    fallback: usize,
) -> usize {
    let found = find_line(content, (*cursor).max(fallback), key, value);
    *cursor = found;
    found
}
