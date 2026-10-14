//! Hydration tests: the context, its neighbourhood, its refusals and its scoped forms.
//!
//! The corpus and the engine builder are shared by every module below.

mod context;
mod errors;
mod neighbours;
mod scoped;

use super::*;
use crate::polyglot::{Language, SymbolKind};
use crate::store::Engine;
use std::path::Path;

/// Indexes `source` as `path` through the *polyglot* pass and returns the engine.
///
/// The polyglot pass is the one hydration reads: it records the universal symbols a `struct`
/// or an `interface` produces, which the function-shaped pass cannot express.
pub(super) fn engine_with(source: &str, path: &str) -> Engine {
    let mut engine = Engine::new();
    engine
        .ingest_polyglot_file(source, Path::new(path), 0, Language::Rust)
        .expect("valid Rust");
    engine
}

/// A two-module corpus: a struct with a method, a sibling struct, and a free function.
pub(super) const CORPUS: &str = "\
/// A cache of decoded index entries.
pub struct CacheStore {
entries: Vec<f32>,
}

impl CacheStore {
/// Opens one from a directory.
pub fn open(path: &str) -> Self {
    Self { entries: Vec::new() }
}
fn secret_tweak(&mut self) {}
}

/// A buffer of decoded index entries.
pub struct EntryBuffer {
capacity: usize,
}

impl EntryBuffer {
/// Reads an entry.
pub fn read(&self, key: &str) -> Vec<u32> {
    Vec::new()
}
/// Writes an entry.
pub fn write(&self, items: &[u32]) -> String {
    String::new()
}
}

/// Scales a value.
pub fn scale(value: f32) -> f32 {
value
}
";

pub(super) fn corpus() -> Engine {
    engine_with(CORPUS, "crates/engine/src/cache.rs")
}
