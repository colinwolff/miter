//! Symbolic encoding: AST provenance plus the hierarchy depth a declaration sits at.

use std::path::PathBuf;

use crate::extract::{Language, Param};

/// FNV-1a 64-bit hash of bytes.
#[inline]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Deterministic FNV-1a 64-bit seed from text.
#[inline]
pub fn seed_from_text(text: &str) -> u64 {
    fnv1a64(text.as_bytes()) | 1
}

/// A fully encoded function: provenance plus its hierarchy depth.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionSymbol {
    /// Stable identifier derived from `path:line:name`.
    pub id: u64,
    /// Bare function name.
    pub name: String,
    /// File the function was read from.
    pub path: PathBuf,
    /// Language of the file.
    pub language: Language,
    /// Declared inputs.
    pub params: Vec<Param>,
    /// Declared return type, if annotated.
    pub return_type: Option<String>,
    /// Directory depth below the scan root.
    pub dir_depth: usize,
    /// Nesting depth inside the file.
    pub scope_depth: usize,
    /// One-based declaration line.
    pub line: usize,
    /// Byte range of the declaration.
    pub start_byte: usize,
    /// Byte offset one past the declaration.
    pub end_byte: usize,
}

impl FunctionSymbol {
    /// Total nesting depth: the directory level plus the nesting inside the file.
    pub fn depth(&self) -> usize {
        self.dir_depth + self.scope_depth
    }

    /// `path::name` — unique as long as a file declares a name once.
    pub fn qualified_name(&self) -> String {
        format!("{}::{}", self.path.display(), self.name)
    }

    /// One-line signature, e.g. `parse_source(&str, &Language) -> Vec<FunctionNode>`.
    ///
    /// Rust receivers are rendered as written (`&self`, not `&self: Self`), because the
    /// type there is implied.
    pub fn signature(&self) -> String {
        let mut text = String::with_capacity(self.name.len() + 32);
        text.push_str(&self.name);
        text.push('(');
        for (index, param) in self.params.iter().enumerate() {
            if index > 0 {
                text.push_str(", ");
            }
            text.push_str(&param.name);
            if !param.type_name.is_empty() && !param.name.contains("self") {
                text.push_str(": ");
                text.push_str(&param.type_name);
            }
        }
        text.push(')');
        if let Some(return_type) = &self.return_type {
            text.push_str(" -> ");
            text.push_str(return_type);
        }
        text
    }

    /// Searchable text for the symbol: name, signature and file stem.
    pub fn search_text(&self) -> String {
        let stem = self
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        format!("{} {} {}", self.name, self.signature(), stem)
    }
}

mod indexer;

pub use indexer::{AstIndexer, IndexReport};
