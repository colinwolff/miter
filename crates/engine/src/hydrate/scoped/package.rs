//! Hydrating a package: the crate-level public surface.

use crate::polyglot::UniversalSymbol;

use super::render_declarations_block;
use super::{HydrateScope, ScopedHydratedContext};
use crate::hydrate::types::HydrateError;

/// Hydrates crate-level public API surface.
pub(super) fn hydrate_package_scope(
    symbols: &[UniversalSymbol],
    target: &str,
    depth: usize,
    budget: usize,
) -> Result<ScopedHydratedContext, HydrateError> {
    let mut matches: Vec<&UniversalSymbol> = symbols
        .iter()
        .filter(|s| {
            let pkg = package_of(&s.file);
            let matches_pkg = pkg == target || pkg.ends_with(&format!("/{target}"));
            matches_pkg && s.exported
        })
        .collect();

    if matches.is_empty() {
        // Fallback: match module label prefix
        matches = symbols
            .iter()
            .filter(|s| {
                let mod_label = s.module();
                let matches_label =
                    mod_label == target || mod_label.starts_with(&format!("{target}/"));
                matches_label && s.exported
            })
            .collect();
    }

    if matches.is_empty() {
        return Err(HydrateError::NotFound(target.to_owned()));
    }

    // Sort by file, line, depth
    matches.sort_by_key(|s| (&s.file, s.line, s.depth));

    render_declarations_block(
        symbols,
        &matches,
        target,
        HydrateScope::Package,
        depth,
        budget,
    )
}

/// Resolves the package/crate name from a source file path.
fn package_of(path: &std::path::Path) -> String {
    let components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();

    if let Some(pos) = components.iter().position(|c| c == "crates")
        && pos + 1 < components.len()
    {
        return components[pos + 1].clone();
    }

    // Single crate root fallback (e.g. src/...)
    if let Some(first) = components.first().filter(|f| f.as_str() != "src") {
        return first.clone();
    }
    String::new()
}
