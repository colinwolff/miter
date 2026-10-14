//! Multi-hop blast radius analysis and affected test selector.
//!
//! Traverses the inverted reference graph backwards to identify direct and transitive
//! callers, public API entrypoints, and affected unit test suites.

use miter_storage::schema::ReferenceKind;
use miter_storage::store::WorkspaceStore;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

/// An affected unit test or integration test detected in the blast radius.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffectedTest {
    /// Test function or test module identifier.
    pub symbol: String,
    /// Workspace-relative file path where the test resides.
    pub file_path: String,
    /// Line range `(start_line, end_line)`.
    pub line_range: (usize, usize),
    /// Ready-to-run shell command to execute this test specifically.
    pub test_command: String,
}

/// An entrypoint, handler, or top-level public orchestrator discovered upstream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntrypointHandler {
    /// Symbol name of the entrypoint or handler.
    pub symbol: String,
    /// Workspace-relative file path.
    pub file_path: String,
    /// Line range `(start_line, end_line)`.
    pub line_range: (usize, usize),
    /// Traversal distance in hops from the originating target symbol.
    pub depth: usize,
}

/// An intermediate transitive caller in the call chain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitiveCaller {
    /// Symbol name of the calling function/method.
    pub symbol: String,
    /// Workspace-relative file path.
    pub file_path: String,
    /// Line range `(start_line, end_line)`.
    pub line_range: (usize, usize),
    /// Semantic classification of the call/usage site.
    pub reference_kind: ReferenceKind,
    /// Traversal distance in hops from the originating target symbol.
    pub depth: usize,
}

/// Result of computing multi-hop blast radius and upstream impact.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    /// The target symbol analyzed.
    pub target_symbol: String,
    /// Maximum traversal hops allowed.
    pub max_depth: usize,
    /// Unit and integration tests impacted by changes to the target symbol.
    pub affected_tests: Vec<AffectedTest>,
    /// Public API handlers, CLI entrypoints, or root orchestrators impacted.
    pub entrypoints_and_handlers: Vec<EntrypointHandler>,
    /// Intermediate transitive callers with their hop distance.
    pub transitive_callers: Vec<TransitiveCaller>,
}

/// Checks if a symbol or file path represents a unit or integration test.
#[must_use]
pub fn is_test_symbol(symbol: &str, file_path: &str) -> bool {
    let lower_sym = symbol.to_ascii_lowercase();
    let lower_path = file_path.to_ascii_lowercase();

    lower_sym.starts_with("test_")
        || lower_sym.ends_with("_test")
        || lower_sym == "test"
        || lower_sym.starts_with("it_")
        || lower_path.contains("/tests/")
        || lower_path.contains("/test/")
        || lower_path.ends_with("_test.rs")
        || lower_path.ends_with(".test.ts")
        || lower_path.ends_with(".test.js")
        || lower_path.ends_with(".spec.ts")
        || lower_path.ends_with(".spec.js")
}

/// Constructs a ready-to-run test execution command for the test symbol.
#[must_use]
pub fn format_test_command(symbol: &str, file_path: &str) -> String {
    if file_path.ends_with(".ts") || file_path.ends_with(".js") || file_path.ends_with(".tsx") {
        format!("npm test -- -t \"{symbol}\"")
    } else {
        format!("cargo test {symbol}")
    }
}

/// Checks if a symbol resembles a public handler, CLI command, or entrypoint.
#[must_use]
pub fn is_entrypoint_or_handler(symbol: &str, file_path: &str) -> bool {
    let lower = symbol.to_ascii_lowercase();
    let lower_path = file_path.to_ascii_lowercase();

    lower == "main"
        || lower.starts_with("handle_")
        || lower.ends_with("_handler")
        || lower.starts_with("run_")
        || lower.ends_with("_cmd")
        || lower_path.contains("/cli/")
        || lower_path.contains("/routes/")
        || lower_path.contains("/api/")
}

/// Computes multi-hop reverse blast radius starting from `symbol`.
///
/// Walks `REFERENCES_TABLE` backwards via `enclosing_symbol` up to `max_depth` hops.
/// Visited symbols are tracked to prevent cycles during recursion.
pub fn compute_impact(
    store: &WorkspaceStore,
    symbol: &str,
    max_depth: usize,
) -> Result<ImpactAnalysis, miter_storage::StorageError> {
    let mut analysis = ImpactAnalysis {
        target_symbol: symbol.to_string(),
        max_depth,
        ..Default::default()
    };

    let mut visited_symbols: HashSet<String> = HashSet::new();
    visited_symbols.insert(symbol.to_string());

    let mut visited_tests: HashSet<String> = HashSet::new();
    let mut visited_entrypoints: HashSet<String> = HashSet::new();
    let mut visited_callers: HashSet<String> = HashSet::new();

    // Queue holds (symbol_name, current_depth)
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((symbol.to_string(), 1));

    while let Some((current_sym, depth)) = queue.pop_front() {
        if depth > max_depth {
            continue;
        }

        let refs = store.get_references_for_symbol(&current_sym)?;
        for r in refs {
            let Some(ref enclosing) = r.enclosing_symbol else {
                continue;
            };

            let caller_key = format!("{}:{}:{}", r.file_path, r.line_range.0, enclosing);

            if is_test_symbol(enclosing, &r.file_path) && visited_tests.insert(caller_key.clone()) {
                analysis.affected_tests.push(AffectedTest {
                    symbol: enclosing.clone(),
                    file_path: r.file_path.clone(),
                    line_range: r.line_range,
                    test_command: format_test_command(enclosing, &r.file_path),
                });
            } else if is_entrypoint_or_handler(enclosing, &r.file_path)
                && visited_entrypoints.insert(caller_key.clone())
            {
                analysis.entrypoints_and_handlers.push(EntrypointHandler {
                    symbol: enclosing.clone(),
                    file_path: r.file_path.clone(),
                    line_range: r.line_range,
                    depth,
                });
            }

            if visited_callers.insert(caller_key) {
                analysis.transitive_callers.push(TransitiveCaller {
                    symbol: enclosing.clone(),
                    file_path: r.file_path.clone(),
                    line_range: r.line_range,
                    reference_kind: r.reference_kind,
                    depth,
                });
            }

            if visited_symbols.insert(enclosing.clone()) && depth < max_depth {
                queue.push_back((enclosing.clone(), depth + 1));
            }
        }
    }

    Ok(analysis)
}
