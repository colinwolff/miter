//! Tarjan's Strongly Connected Components (SCC) DAG Cycle Detector.
//!
//! Builds a directed file dependency graph `File A -> File B` whenever a symbol in `File A`
//! references a symbol defined in `File B`, and identifies any circular dependencies.

use miter_storage::store::WorkspaceStore;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A strongly connected component (SCC) representing a circular dependency cluster.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyCycle {
    /// Workspace-relative file paths participating in the dependency cycle.
    pub files: Vec<String>,
    /// Size of the cycle (number of files).
    pub size: usize,
}

/// Result of workspace dependency cycle detection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleDetectionReport {
    /// Total number of files evaluated in the dependency graph.
    pub total_files: usize,
    /// Detected dependency cycles with size > 1 or self-loops.
    pub cycles: Vec<DependencyCycle>,
    /// Whether any cyclic dependency risk exists in the workspace.
    pub has_circular_risk: bool,
}

/// Directed graph where nodes are file paths and edges represent imports/references.
#[derive(Clone, Debug, Default)]
pub struct FileDependencyGraph {
    /// Adjacency map: file path -> set of dependent file paths.
    pub adjacency: HashMap<String, HashSet<String>>,
}

impl FileDependencyGraph {
    /// Creates an empty dependency graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a directed dependency edge `from -> to`.
    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) {
        let from_key = from.into();
        let to_key = to.into();
        self.adjacency.entry(to_key.clone()).or_default();
        self.adjacency.entry(from_key).or_default().insert(to_key);
    }

    /// Builds a file dependency graph from workspace symbols and references tables.
    pub fn build_from_workspace(
        store: &WorkspaceStore,
    ) -> Result<Self, miter_storage::StorageError> {
        let mut graph = Self::new();

        // 1. Map symbol name -> defining file path from universal symbols
        let mut symbol_defining_files: HashMap<String, String> = HashMap::new();
        if let Ok(universal_symbols) = crate::persist::load_redb_universal_from(store) {
            for sym in universal_symbols {
                let file_str = sym.file.to_string_lossy().replace('\\', "/");
                symbol_defining_files.insert(sym.name, file_str);
            }
        }

        // 2. Map symbol references -> referencing file path
        let referenced_symbols = store.list_referenced_symbols()?;
        for sym_name in referenced_symbols {
            if let Some(target_file) = symbol_defining_files.get(&sym_name) {
                let refs = store.get_references_for_symbol(&sym_name)?;
                for r in refs {
                    let source_file = r.file_path.replace('\\', "/");
                    if &source_file != target_file {
                        graph.add_edge(source_file, target_file.clone());
                    }
                }
            }
        }

        Ok(graph)
    }

    /// Runs Tarjan's Strongly Connected Components (SCC) algorithm.
    #[must_use]
    pub fn find_cycles(&self) -> CycleDetectionReport {
        let mut index = 0usize;
        let mut indices: HashMap<String, usize> = HashMap::new();
        let mut lowlink: HashMap<String, usize> = HashMap::new();
        let mut on_stack: HashSet<String> = HashSet::new();
        let mut stack: Vec<String> = Vec::new();
        let mut sccs: Vec<Vec<String>> = Vec::new();

        for node in self.adjacency.keys() {
            if !indices.contains_key(node) {
                strongconnect(
                    node,
                    &self.adjacency,
                    &mut index,
                    &mut indices,
                    &mut lowlink,
                    &mut on_stack,
                    &mut stack,
                    &mut sccs,
                );
            }
        }

        let mut cycles = Vec::new();
        for mut scc in sccs {
            let size = scc.len();
            if size > 1 {
                scc.sort();
                cycles.push(DependencyCycle { files: scc, size });
            } else if size == 1 {
                let node = &scc[0];
                if let Some(neighbors) = self.adjacency.get(node)
                    && neighbors.contains(node)
                {
                    cycles.push(DependencyCycle {
                        files: vec![node.clone()],
                        size: 1,
                    });
                }
            }
        }

        cycles.sort_by_key(|c| std::cmp::Reverse(c.size));
        let has_circular_risk = !cycles.is_empty();

        CycleDetectionReport {
            total_files: self.adjacency.len(),
            cycles,
            has_circular_risk,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn strongconnect(
    node: &str,
    adj: &HashMap<String, HashSet<String>>,
    index: &mut usize,
    indices: &mut HashMap<String, usize>,
    lowlink: &mut HashMap<String, usize>,
    on_stack: &mut HashSet<String>,
    stack: &mut Vec<String>,
    sccs: &mut Vec<Vec<String>>,
) {
    indices.insert(node.to_string(), *index);
    lowlink.insert(node.to_string(), *index);
    *index += 1;
    stack.push(node.to_string());
    on_stack.insert(node.to_string());

    if let Some(neighbors) = adj.get(node) {
        for neighbor in neighbors {
            if !indices.contains_key(neighbor) {
                strongconnect(
                    neighbor, adj, index, indices, lowlink, on_stack, stack, sccs,
                );
                let neighbor_low = lowlink[neighbor];
                let current_low = lowlink.get_mut(node).unwrap();
                *current_low = (*current_low).min(neighbor_low);
            } else if on_stack.contains(neighbor) {
                let neighbor_index = indices[neighbor];
                let current_low = lowlink.get_mut(node).unwrap();
                *current_low = (*current_low).min(neighbor_index);
            }
        }
    }

    if lowlink[node] == indices[node] {
        let mut scc = Vec::new();
        while let Some(w) = stack.pop() {
            on_stack.remove(&w);
            let done = w == node;
            scc.push(w);
            if done {
                break;
            }
        }
        sccs.push(scc);
    }
}
