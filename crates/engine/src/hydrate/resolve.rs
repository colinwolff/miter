//! Resolution: which declaration a name means, and which declarations are its neighbours.
//!
//! [`neighbours_of`] admits the declarations within [`super::types::HYDRATION_LEVEL_BAND`] levels
//! of the anchor, and orders them by level closeness, then name affinity, then position — so two
//! runs over the same index always produce the same list.

use std::path::PathBuf;

use crate::polyglot::UniversalSymbol;

use super::types::{HydratedNeighbour, MAX_MEMBERS, MAX_NEIGHBOURS};

/// Resolves `symbol_name` to one declaration, and reports the others it matched.
///
/// An exact qualified-path match wins over a bare name, because `store.Store` is an answer and
/// `Store` is a question. Ties break on the id so two runs resolve the same declaration.
pub fn resolve<'a>(
    symbols: &'a [UniversalSymbol],
    symbol_name: &str,
) -> Option<(&'a UniversalSymbol, Vec<PathBuf>)> {
    let matches: Vec<&UniversalSymbol> = symbols
        .iter()
        .filter(|symbol| symbol.name == symbol_name || symbol.path == symbol_name)
        .collect();
    if matches.is_empty() {
        return None;
    }
    let exact = matches
        .iter()
        .copied()
        .find(|symbol| symbol.path == symbol_name)
        .unwrap_or(matches[0]);
    let others = matches
        .iter()
        .filter(|symbol| symbol.id() != exact.id())
        .map(|symbol| symbol.file.clone())
        .collect();
    Some((exact, others))
}

/// The exported members declared inside `target`.
///
/// Only exported ones: a private leaf is an implementation detail, and listing it is the payload
/// this module exists to avoid. Members are ordered by line, which is the order a reader of the
/// source would meet them in. A member is one level below its container, so its `level_delta` is
/// its distance from the anchor by construction.
pub(super) fn members_of(
    symbols: &[UniversalSymbol],
    target: &UniversalSymbol,
) -> Vec<HydratedNeighbour> {
    let prefix = format!("{}.", target.path);
    let mut members: Vec<&UniversalSymbol> = symbols
        .iter()
        .filter(|symbol| symbol.exported && symbol.path.starts_with(&prefix))
        .collect();
    members.sort_by_key(|symbol| (symbol.line, symbol.id()));
    members
        .into_iter()
        .take(MAX_MEMBERS)
        .map(|symbol| HydratedNeighbour {
            name: symbol.name.clone(),
            file: symbol.file.clone(),
            line: symbol.line,
            depth: symbol.depth,
            level_delta: symbol.depth.abs_diff(target.depth),
        })
        .collect()
}

/// The declarations within the anchor's level band, nearest first.
///
/// `level_band` is how many hierarchy steps either way are admitted, so a neighbour qualifies
/// when `|neighbour level − anchor level| <= level_band`. Ordering, in keys of decreasing
/// importance:
///
/// 1. **level closeness** — a neighbour one level away is a closer relation than one two away;
/// 2. **name affinity** — inside one level a dozen declarations share the same delta, and sorting
///    on the delta alone would rank them by the id hash, which is *arbitrary*. This is what keeps
///    the peers a reader wants ahead of unrelated declarations at the same level;
/// 3. **line, then name, then file** — so two runs agree on an ordering of otherwise-equal
///    neighbours instead of on whatever order the scan happened to produce.
///
/// Scaffolding names are not dropped — they are real declarations and the index would be lying
/// about what it holds — but they score zero affinity and so sort behind anything substantive.
pub(super) fn neighbours_of(
    symbols: &[UniversalSymbol],
    target: &UniversalSymbol,
    level_band: usize,
) -> Vec<HydratedNeighbour> {
    let target_id = target.id();
    let mut admitted: Vec<HydratedNeighbour> = symbols
        .iter()
        .filter(|symbol| symbol.id() != target_id)
        .filter_map(|symbol| {
            let level_delta = symbol.depth.abs_diff(target.depth);
            if level_delta > level_band {
                return None;
            }
            Some(HydratedNeighbour {
                name: symbol.name.clone(),
                file: symbol.file.clone(),
                line: symbol.line,
                depth: symbol.depth,
                level_delta,
            })
        })
        .collect();

    admitted.sort_by(|left, right| {
        left.level_delta
            .cmp(&right.level_delta)
            .then_with(|| {
                name_affinity(&target.name, &right.name)
                    .cmp(&name_affinity(&target.name, &left.name))
            })
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.file.cmp(&right.file))
    });
    admitted.truncate(MAX_NEIGHBOURS);
    admitted
}

/// The scaffolding names that describe a file rather than a thing declared in it.
///
/// A `mod` or a `tests` is a real declaration and stays in the band — but it is not what a reader
/// means by "what is near this symbol", so it ranks last rather than being dropped. Dropping it
/// would make the neighbour list lie about what the index actually holds.
pub(super) fn is_scaffolding(name: &str) -> bool {
    matches!(
        name,
        "mod" | "tests" | "test" | "use" | "lib" | "main" | "prelude" | "common"
    )
}

/// How strongly `neighbour`'s name relates to `anchor`'s, larger being closer a reading of them.
///
/// A word, not a substring: `Index` shares nothing with `scale`, and a substring rule would have
/// claimed otherwise. Words shorter than three characters are ignored because every declaration has
/// them — a one-letter split fragment is not a word.
pub(super) fn name_affinity(anchor: &str, neighbour: &str) -> u32 {
    if is_scaffolding(neighbour) {
        return 0;
    }
    let words = |text: &str| -> Vec<String> {
        text.split(|character: char| !character.is_alphanumeric())
            .filter(|word| word.chars().count() > 2)
            .map(str::to_lowercase)
            .collect()
    };
    let anchor_words = words(anchor);
    let shared = words(neighbour)
        .iter()
        .filter(|word| anchor_words.contains(word))
        .count();
    // The offset keeps a non-zero affinity above every zero, so a related name outranks both an
    // unrelated one and the scaffolding names however the ids happen to fall.
    1 + shared as u32
}
