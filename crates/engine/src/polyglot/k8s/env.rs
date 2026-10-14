//! The `env:` walk, shared by Kubernetes and Docker Compose.
//!
//! Both nest configuration under a path the other does not use — Kubernetes under
//! `spec.template.spec.containers[].env` and a `ConfigMap`/`Secret` under `data`, Compose under
//! `services.<name>.environment` — so the walk looks for the *key* rather than a fixed path:
//! wherever a mapping carries an `env`, `environment`, `data` or `stringData` entry, its items
//! are declarations.
//!
//! A literal value is configuration and is redacted; a `secretKeyRef`/`configMapKeyRef` names a
//! secret rather than holding one, so the reference is recorded verbatim.

use std::path::Path;

use serde_yaml::{Mapping, Value};

use super::super::{Language, SymbolKind, UniversalSymbol, common};
use super::take_line;
use crate::polyglot::redact::redacted_value;

/// Records every environment variable declared anywhere under `value`.
pub(super) fn push_env(
    value: &Value,
    path: &Path,
    module: &str,
    content: &str,
    cursor: &mut usize,
    out: &mut Vec<UniversalSymbol>,
) {
    match value {
        Value::Mapping(map) => {
            for (key, nested) in map {
                match key.as_str() {
                    // `data`/`stringData` are a `ConfigMap`'s and a `Secret`'s own payload: the
                    // same question ("which keys does this manifest configure?") with the same
                    // answer, so they are read by the same walk.
                    Some("env" | "environment" | "data" | "stringData") => {
                        push_entries(nested, path, module, content, cursor, out);
                    }
                    _ => push_env(nested, path, module, content, cursor, out),
                }
            }
        }
        Value::Sequence(items) => {
            for item in items {
                push_env(item, path, module, content, cursor, out);
            }
        }
        _ => {}
    }
}

/// Records the variables of one `env`/`environment` value.
///
/// The long form is a sequence of `{name, value|valueFrom}` mappings; Compose also allows the
/// short form, a mapping of `KEY: VALUE`. Both are read.
fn push_entries(
    value: &Value,
    path: &Path,
    module: &str,
    content: &str,
    cursor: &mut usize,
    out: &mut Vec<UniversalSymbol>,
) {
    match value {
        Value::Sequence(items) => {
            for item in items {
                if let Some(map) = item.as_mapping() {
                    push_named(map, path, module, content, cursor, out);
                }
            }
        }
        Value::Mapping(map) => {
            for (key, entry) in map {
                if let Some(name) = key.as_str() {
                    let raw = entry.as_str().unwrap_or("");
                    // The short form spells the key itself, so that is what the line is found by.
                    let value = redacted_value(name, raw);
                    out.push(symbol(path, module, content, cursor, name, name, &value));
                }
            }
        }
        _ => {}
    }
}

/// Records one `{name, value}` or `{name, valueFrom}` entry.
fn push_named(
    map: &Mapping,
    path: &Path,
    module: &str,
    content: &str,
    cursor: &mut usize,
    out: &mut Vec<UniversalSymbol>,
) {
    let Some(name) = map.get("name").and_then(Value::as_str) else {
        return;
    };
    let rendered = match map.get("value") {
        Some(literal) => redacted_value(name, literal.as_str().unwrap_or("")),
        None => map.get("valueFrom").map(reference_of).unwrap_or_default(),
    };
    out.push(symbol(
        path, module, content, cursor, "name", name, &rendered,
    ));
}

/// `secretKeyRef(name.key)` for the reference an entry points at.
fn reference_of(source: &Value) -> String {
    for kind in ["secretKeyRef", "configMapKeyRef"] {
        if let Some(reference) = source.get(kind) {
            let target = reference.get("name").and_then(Value::as_str).unwrap_or("?");
            let key = reference.get("key").and_then(Value::as_str).unwrap_or("?");
            return format!("{kind}({target}.{key})");
        }
    }
    String::new()
}

/// Builds one environment-variable declaration.
///
/// `line_key` is the spelling the source uses for this entry's key: `name` for the long form
/// (`- name: LOG_LEVEL`), and the key itself for Compose's short form (`QUEUE: default`).
fn symbol(
    path: &Path,
    module: &str,
    content: &str,
    cursor: &mut usize,
    line_key: &str,
    name: &str,
    value: &str,
) -> UniversalSymbol {
    let line = take_line(content, cursor, line_key, name, *cursor);
    let signature = super::scrub(if value.is_empty() {
        format!("ENV {name}")
    } else {
        format!("ENV {name}={value}")
    });
    UniversalSymbol {
        path: common::dotted_path(module, &[], name),
        name: name.to_owned(),
        language: Language::Rust,
        kind: SymbolKind::Property,
        depth: 3,
        docstring: None,
        signature,
        span: (0, content.len()),
        file: path.to_path_buf(),
        line,
        params: Vec::new(),
        return_type: None,
        exported: true,
    }
}
