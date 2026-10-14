//! Hydrating a module: every exported declaration in one file.

use crate::polyglot::UniversalSymbol;

use super::render_declarations_block;
use super::{HydrateScope, ScopedHydratedContext};
use crate::hydrate::types::HydrateError;

/// Hydrates all exported public items in a module.
pub(super) fn hydrate_module_scope(
    symbols: &[UniversalSymbol],
    target: &str,
    depth: usize,
    budget: usize,
) -> Result<ScopedHydratedContext, HydrateError> {
    let mut matches: Vec<&UniversalSymbol> = symbols
        .iter()
        .filter(|s| {
            let mod_path = crate::polyglot::common::module_path(&s.file);
            let mod_label = s.module();
            let matches_path = mod_path == target
                || mod_path.ends_with(&format!(".{target}"))
                || s.file.to_string_lossy().contains(target);
            let matches_label = mod_label == target || mod_label.ends_with(&format!("/{target}"));
            let matches_prefix = s.path.starts_with(target);

            (matches_path || matches_label || matches_prefix) && (s.exported || s.depth <= 2)
        })
        .collect();

    if matches.is_empty() {
        return Err(HydrateError::NotFound(target.to_owned()));
    }

    // Sort by file, line, depth
    matches.sort_by_key(|s| (&s.file, s.line, s.depth));

    render_declarations_block(
        symbols,
        &matches,
        target,
        HydrateScope::Module,
        depth,
        budget,
    )
}
