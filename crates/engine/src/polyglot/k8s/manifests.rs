//! Kubernetes resources and Litestream replication configuration.

use std::path::Path;

use serde_yaml::Value;

use super::super::{Language, SymbolKind, UniversalSymbol, common};
use super::{find_line, take_line};

/// Records a Kubernetes resource, returning `true` when the document was one.
pub(super) fn push_kubernetes(
    map: &serde_yaml::Mapping,
    path: &Path,
    module: &str,
    content: &str,
    doc_index: usize,
    cursor: &mut usize,
    out: &mut Vec<UniversalSymbol>,
) -> bool {
    let api_version = map.get("apiVersion").and_then(Value::as_str);
    let Some(kind) = map.get("kind").and_then(Value::as_str) else {
        return false;
    };
    let Some(api_version) = api_version else {
        return false;
    };
    let name = map
        .get("metadata")
        .and_then(Value::as_mapping)
        .and_then(|metadata| metadata.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("unnamed");

    let full_name = format!("{kind}/{name}");
    let line = take_line(content, cursor, "kind", kind, doc_index + 1);
    out.push(UniversalSymbol {
        path: common::dotted_path(module, &[], &full_name),
        name: full_name,
        language: Language::Rust,
        kind: SymbolKind::K8sManifest,
        depth: 2,
        docstring: None,
        signature: super::scrub(format!("K8S {kind}/{name} (api: {api_version})")),
        span: (0, content.len()),
        file: path.to_path_buf(),
        line,
        params: Vec::new(),
        return_type: None,
        exported: true,
    });
    true
}

/// Records Litestream replication targets, returning `true` when the document declared any.
pub(super) fn push_litestream(
    map: &serde_yaml::Mapping,
    path: &Path,
    module: &str,
    content: &str,
    cursor: &mut usize,
    out: &mut Vec<UniversalSymbol>,
) -> bool {
    let Some(databases) = map.get("dbs").and_then(Value::as_sequence) else {
        return false;
    };

    for (index, entry) in databases.iter().enumerate() {
        let database = entry
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let replica = entry
            .get("replicas")
            .and_then(Value::as_sequence)
            .and_then(|replicas| replicas.iter().find_map(replica_target))
            .unwrap_or_else(|| "none".to_owned());

        let stem = Path::new(database)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("db");
        let name = format!("litestream_{stem}_{index}");
        let line = find_line(content, *cursor, "path", database);

        out.push(UniversalSymbol {
            path: common::dotted_path(module, &[], &name),
            name,
            language: Language::Rust,
            kind: SymbolKind::K8sManifest,
            depth: 2,
            docstring: None,
            signature: super::scrub(format!("LITESTREAM db={database} replica={replica}")),
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

/// The bucket or URL a replica writes to.
fn replica_target(replica: &Value) -> Option<String> {
    replica
        .get("bucket")
        .or_else(|| replica.get("url"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}
