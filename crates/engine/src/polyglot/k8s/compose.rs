//! Docker Compose services.
//!
//! A Compose file's `services:` mapping is the whole declaration surface: each key is a service a
//! reader wants to find, and its `image` (or the presence of a `build`) is what it is. The
//! service's own environment is read by the shared [`super::env`] walk.

use std::path::Path;

use serde_yaml::{Mapping, Value};

use super::super::{Language, SymbolKind, UniversalSymbol, common};
use super::take_line;

/// Records every `services.<name>`, returning `true` when the document declared any.
pub(super) fn push_services(
    map: &Mapping,
    path: &Path,
    module: &str,
    content: &str,
    cursor: &mut usize,
    out: &mut Vec<UniversalSymbol>,
) -> bool {
    let Some(services) = map.get("services").and_then(Value::as_mapping) else {
        return false;
    };

    for (key, spec) in services {
        let Some(name) = key.as_str() else {
            continue;
        };
        let signature = match spec.get("image").and_then(Value::as_str) {
            Some(image) => format!("SERVICE {name} (image: {image})"),
            None => format!("SERVICE {name} (build)"),
        };
        let line = take_line(content, cursor, name, "", *cursor);

        out.push(UniversalSymbol {
            path: common::dotted_path(module, &[], name),
            name: name.to_owned(),
            language: Language::Rust,
            kind: SymbolKind::K8sManifest,
            depth: 2,
            docstring: None,
            signature: super::scrub(signature),
            span: (0, content.len()),
            file: path.to_path_buf(),
            line,
            params: Vec::new(),
            return_type: None,
            exported: true,
        });
    }
    true
}
