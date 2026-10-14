//! Rendering: the payload a [`HydratedContext`] becomes, and the pieces it is built from.
//!
//! `render_within` is the entry point the rest of the module calls: it sheds whole lines until the
//! payload fits the budget, so what a caller receives is always a complete set of lines rather than
//! a truncated one. `estimate_tokens` is the measurement that loop is bounded by, and it is public
//! because the CLI and the MCP surface both budget against it.

use crate::polyglot::{SymbolKind, UniversalSymbol};

use super::types::{CHARS_PER_TOKEN, HYDRATION_LEVEL_BAND, HydratedContext, LEVEL_NAMES};

impl HydratedContext {
    /// Re-renders the payload under a new token ceiling.
    ///
    /// Two concessions are made, in this order, and both are whole lines: the farthest neighbour
    /// goes first, because the neighbourhood is the part a reader asked for, and then the last
    /// member, because an inventory of methods is the part they can most easily live without. The
    /// loop stops when the payload fits or when there is nothing left to drop, so what remains is
    /// always a complete set of lines rather than a truncated one.
    pub fn render_within(&self, max_tokens: usize) -> String {
        let mut kept = self.neighbours.len();
        let mut members = self.members.len();
        loop {
            let text = self.render(kept, members);
            if estimate_tokens(&text) <= max_tokens {
                return text;
            }
            if kept > 0 {
                kept -= 1;
            } else if members > 0 {
                members -= 1;
            } else {
                return text;
            }
        }
    }

    /// The payload with the `kept` nearest neighbours and the first `members` public members.
    pub(super) fn render(&self, kept: usize, members: usize) -> String {
        let mut out = String::with_capacity(512);
        out.push_str(&format!(
            "Symbol: {}\nPath: {}:{}\nKind: {} | Depth: Level {} ({})\n",
            self.name,
            self.file.display(),
            self.line,
            self.kind.name(),
            self.depth,
            LEVEL_NAMES.get(self.depth).copied().unwrap_or("Beyond"),
        ));

        out.push_str(&format!("Signature: {}\n", self.signature));

        if let Some(docstring) = &self.docstring {
            out.push_str(&format!("Doc: {docstring}\n"));
        }
        if !self.params.is_empty() {
            let rendered = self
                .params
                .iter()
                .map(|param| format!("{}: {}", param.name, param.type_name))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("Parameters: {rendered}\n"));
        }
        if let Some(return_type) = &self.return_type {
            out.push_str(&format!("Returns: {return_type}\n"));
        }
        let shown_members = members.min(self.members.len());
        if shown_members > 0 {
            out.push_str("Public Members:\n");
            for member in self.members.iter().take(shown_members) {
                out.push_str(&format!(
                    "  - {} ({}:{})\n",
                    member.name,
                    member.file.display(),
                    member.line,
                ));
            }
            if shown_members < self.members.len() {
                out.push_str(&format!(
                    "  … and {} more ({} public members in total)\n",
                    self.members.len() - shown_members,
                    self.members.len()
                ));
            }
        }

        let shown = kept.min(self.neighbours.len());
        if shown > 0 {
            out.push_str(&format!(
                "Neighbours (within {HYDRATION_LEVEL_BAND} level of Level {}):\n",
                self.depth
            ));
            for neighbour in self.neighbours.iter().take(shown) {
                out.push_str(&format!(
                    "  - {} (level {}, Δlevel {}, {})\n",
                    neighbour.name,
                    neighbour.depth,
                    neighbour.level_delta,
                    neighbour.file.display(),
                ));
            }
        }

        if !self.ambiguous_with.is_empty() {
            let others = self
                .ambiguous_with
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("Ambiguous with: {others}\n"));
        }
        out
    }
}

/// Estimates the token count of `text`.
///
/// The larger of two estimators, so the number is an upper bound on the usual 4-characters-per-token
/// rate rather than a guess that can be beaten: code is punctuation-dense, and a whitespace count
/// alone under-reports `foo(&self, x: u32) -> u64` by a factor of three.
pub fn estimate_tokens(text: &str) -> usize {
    let characters = text.chars().count().div_ceil(CHARS_PER_TOKEN);
    let fields = text.split_whitespace().count();
    characters.max(fields)
}

/// The first sentence of `docstring`, which is the part that says what the thing *is*.
///
/// A doc comment's later paragraphs are examples and rationale; the claim is the first sentence,
/// and the budget is better spent on neighbours than on a worked example.
pub(super) fn first_sentence(docstring: &str) -> String {
    let collapsed = docstring.split_whitespace().collect::<Vec<_>>().join(" ");
    match collapsed.find(". ") {
        Some(end) => collapsed[..end + 1].to_owned(),
        None => collapsed,
    }
}

/// The signature line, with a body ellipsis for a type declaration.
///
/// The parse collapses a declaration onto its header, which for a `struct` stops at the name. A
/// reader needs to know it opens a body, so the ellipsis is added here rather than at the parse,
/// where it would be a claim about the syntax tree.
pub(super) fn render_signature(symbol: &UniversalSymbol) -> String {
    let opens_a_body = matches!(
        symbol.kind,
        SymbolKind::Struct
            | SymbolKind::Enum
            | SymbolKind::Trait
            | SymbolKind::Class
            | SymbolKind::Interface
            | SymbolKind::Module
    );
    if opens_a_body {
        format!("{} {{ ... }}", symbol.signature)
    } else {
        symbol.signature.clone()
    }
}
