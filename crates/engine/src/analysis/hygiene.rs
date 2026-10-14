//! Dead symbol and visibility reaper.
//!
//! Performs set-difference analysis between declared symbols and workspace references
//! to flag dead code and identify over-broad visibilities that can be downgraded to private.

use crate::polyglot::UniversalSymbol;
use miter_storage::store::WorkspaceStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A dead symbol flagged with zero references across the workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeadSymbol {
    /// Bare symbol name.
    pub name: String,
    /// Defining file path.
    pub file_path: String,
    /// 1-indexed line number.
    pub line: usize,
    /// Language or kind label.
    pub kind: String,
}

/// A symbol marked `pub` or `pub(crate)` whose references occur solely within its defining file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisibilityCandidate {
    /// Bare symbol name.
    pub name: String,
    /// Defining file path.
    pub file_path: String,
    /// 1-indexed line number.
    pub line: usize,
    /// Total references count within the defining file.
    pub local_references_count: usize,
    /// Suggested visibility action (e.g., "downgrade to private").
    pub suggestion: String,
}

/// Workspace hygiene report containing dead symbols and visibility downgrade candidates.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HygieneReport {
    /// Symbols with 0 references across the workspace.
    pub dead_symbols: Vec<DeadSymbol>,
    /// Public symbols whose references never leave their defining file.
    pub visibility_downgrades: Vec<VisibilityCandidate>,
}

/// Checks if a symbol name is exempt from dead-code flagging.
#[must_use]
pub fn is_exempt_symbol(sym: &UniversalSymbol) -> bool {
    let name = sym.name.as_str();
    name == "main"
        || name == "run"
        || name.starts_with("test_")
        || name.ends_with("_test")
        || name == "default"
        || name == "new"
        || name.starts_with("handle_")
        || name.starts_with("on_")
        || sym.file.to_string_lossy().contains("/tests/")
        || sym.file.to_string_lossy().contains("/benches/")
}

/// Analyzes workspace symbols and references to detect dead code and visibility downgrades.
pub fn analyze_hygiene(
    store: &WorkspaceStore,
) -> Result<HygieneReport, miter_storage::StorageError> {
    let mut report = HygieneReport::default();
    let symbols = crate::persist::load_redb_universal_from(store).unwrap_or_default();

    // Map symbol name -> list of defining symbols
    let mut symbols_by_name: HashMap<String, Vec<&UniversalSymbol>> = HashMap::new();
    for sym in &symbols {
        symbols_by_name
            .entry(sym.name.clone())
            .or_default()
            .push(sym);
    }

    let referenced_symbols = store.list_referenced_symbols()?;

    for (name, decls) in &symbols_by_name {
        let refs = if referenced_symbols.contains(name) {
            store.get_references_for_symbol(name)?
        } else {
            Vec::new()
        };

        if refs.is_empty() {
            for sym in decls {
                if !is_exempt_symbol(sym) && sym.depth >= 2 {
                    report.dead_symbols.push(DeadSymbol {
                        name: sym.name.clone(),
                        file_path: sym.file.to_string_lossy().replace('\\', "/"),
                        line: sym.line,
                        kind: sym.kind.name().to_string(),
                    });
                }
            }
        } else {
            // Check visibility downgrades for exported/public symbols
            for sym in decls {
                if !sym.exported || is_exempt_symbol(sym) {
                    continue;
                }
                let sym_file = sym.file.to_string_lossy().replace('\\', "/");
                let mut local_refs = 0;
                let mut external_refs = 0;

                for r in &refs {
                    let ref_file = r.file_path.replace('\\', "/");
                    if ref_file == sym_file {
                        local_refs += 1;
                    } else {
                        external_refs += 1;
                    }
                }

                if local_refs > 0 && external_refs == 0 {
                    report.visibility_downgrades.push(VisibilityCandidate {
                        name: sym.name.clone(),
                        file_path: sym_file,
                        line: sym.line,
                        local_references_count: local_refs,
                        suggestion:
                            "100% of references occur within same file; downgrade to private"
                                .to_string(),
                    });
                }
            }
        }
    }

    report
        .dead_symbols
        .sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.line.cmp(&b.line)));
    report
        .visibility_downgrades
        .sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.line.cmp(&b.line)));

    Ok(report)
}
