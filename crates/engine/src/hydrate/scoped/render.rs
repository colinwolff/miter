//! Rendering a declaration list into one budgeted interface block.

use crate::polyglot::UniversalSymbol;

use super::{HydrateScope, ScopedHydratedContext};
use crate::hydrate::render::{estimate_tokens, first_sentence, render_signature};
use crate::hydrate::resolve::members_of;
use crate::hydrate::types::HydrateError;

/// Formats a list of declarations into a clean interface block bounded by `budget` tokens.
pub(super) fn render_declarations_block(
    all_symbols: &[UniversalSymbol],
    matches: &[&UniversalSymbol],
    target: &str,
    scope: HydrateScope,
    depth: usize,
    budget: usize,
) -> Result<ScopedHydratedContext, HydrateError> {
    let scope_label = match scope {
        HydrateScope::Module => "Module Interface",
        HydrateScope::Package => "Package Public Surface",
        HydrateScope::Symbol => "Symbol",
    };

    let header = format!("// Scope: {scope_label} [{target}]\n");
    let header_tokens = estimate_tokens(&header);
    if header_tokens > budget {
        return Err(HydrateError::EmptyBudget(budget));
    }

    let mut out = header;
    let mut included = 0;
    let total_matches = matches.len();

    for (i, sym) in matches.iter().enumerate() {
        let mut decl_block = format!("// {}:{}\n", sym.file.display(), sym.line);
        if let Some(doc) = &sym.docstring {
            decl_block.push_str(&format!("/// {}\n", first_sentence(doc)));
        }
        decl_block.push_str(&format!("{}\n", render_signature(sym)));

        if depth > 1 {
            // Include public members of this declaration
            let mems = members_of(all_symbols, sym);
            for m in mems.iter().take(4) {
                decl_block.push_str(&format!("  // member: {} at line {}\n", m.name, m.line));
            }
        }
        decl_block.push('\n');
        decl_block = crate::security::scrub_source_text(&decl_block);

        let remaining_after_this = total_matches - (i + 1);
        let truncation_msg = if remaining_after_this > 0 {
            format!(
                "// ... [truncated: {remaining_after_this} additional declarations omitted. Use scope='module' on submodules]\n"
            )
        } else {
            String::new()
        };

        let candidate_text = format!("{}{}{}", out, decl_block, truncation_msg);
        if estimate_tokens(&candidate_text) <= budget {
            out.push_str(&decl_block);
            included += 1;
        } else {
            // Can't fit this declaration. Append truncation notice and break.
            let omitted = total_matches - included;
            let note = format!(
                "// ... [truncated: {omitted} additional declarations omitted. Use scope='module' on submodules]\n"
            );
            let with_note = format!("{}{}", out, note);
            if estimate_tokens(&with_note) <= budget {
                out = with_note;
            }
            break;
        }
    }

    let tokens = estimate_tokens(&out);
    Ok(ScopedHydratedContext {
        target: target.to_owned(),
        scope,
        text: out,
        tokens,
        match_count: total_matches,
        included_count: included,
    })
}
