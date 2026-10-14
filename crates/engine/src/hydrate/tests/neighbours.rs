//! The neighbourhood: ordering, the band, and the token budget.

use super::resolve::name_affinity;
use super::*;

#[test]
fn neighbours_are_ordered_by_depth_then_affinity_and_stay_inside_the_band() {
    let engine = corpus();
    let context =
        hydrate_symbol_context(&engine, "CacheStore", DEFAULT_MAX_TOKENS).expect("hydrates");

    assert!(
        !context.neighbours.is_empty(),
        "the band must admit the corpus's own siblings"
    );

    // Every neighbour is inside the band and is not the anchor.
    for neighbour in &context.neighbours {
        assert!(
            neighbour.level_delta <= HYDRATION_LEVEL_BAND,
            "{} is {} levels away and is outside the band",
            neighbour.name,
            neighbour.level_delta
        );
        assert_ne!(
            neighbour.name, "CacheStore",
            "a symbol is not its own neighbour"
        );
    }

    // Depth closeness is the leading key, so the list never gets *less* structurally close as
    // it goes down.
    let depths: Vec<usize> = context
        .neighbours
        .iter()
        .map(|neighbour| neighbour.depth.abs_diff(context.depth))
        .collect();
    for window in depths.windows(2) {
        assert!(
            window[0] <= window[1],
            "ordered by depth closeness: {depths:?}"
        );
    }

    // Within one depth, a name sharing a word with the anchor outranks one that does not.
    let same_depth: Vec<&str> = context
        .neighbours
        .iter()
        .filter(|n| n.depth.abs_diff(context.depth) == depths[0])
        .map(|n| n.name.as_str())
        .collect();
    for (index, name) in same_depth.iter().enumerate() {
        let affinity = name_affinity(&context.name, name);
        for earlier in &same_depth[..index] {
            assert!(
                affinity <= name_affinity(&context.name, earlier),
                "`{name}` (affinity {affinity}) outranks `{earlier}` ahead of it"
            );
        }
    }
}

#[test]
fn the_band_admits_siblings_and_excludes_a_distant_level() {
    let engine = corpus();
    let context =
        hydrate_symbol_context(&engine, "CacheStore", DEFAULT_MAX_TOKENS).expect("hydrates");
    let names: Vec<&str> = context
        .neighbours
        .iter()
        .map(|neighbour| neighbour.name.as_str())
        .collect();

    // The sibling struct is the canonical neighbour: same level, same file.
    assert!(
        names.contains(&"EntryBuffer"),
        "a sibling struct is in the band: {names:?}"
    );
    // Every admitted neighbour is within one level of the anchor — the band is narrow enough
    // to exclude the crate-root scaffolding.
    for neighbour in &context.neighbours {
        assert!(
            neighbour.depth.abs_diff(context.depth) <= 1,
            "{} is at level {}, anchor at {}",
            neighbour.name,
            neighbour.depth,
            context.depth
        );
    }
}

#[test]
fn a_zero_level_band_admits_only_the_anchor_level() {
    let engine = corpus();
    let context = hydrate_symbol_context_with(&engine, "CacheStore", DEFAULT_MAX_TOKENS, 0)
        .expect("hydrates");
    // A zero band admits exactly the declarations at the anchor's own level — its siblings — and
    // nothing from another level. It is *not* "only the anchor": the corpus declares two structs,
    // and a struct is a struct however it is named.
    for neighbour in &context.neighbours {
        assert_eq!(
            neighbour.level_delta, 0,
            "{} is not at the anchor's level",
            neighbour.name
        );
        assert_eq!(
            neighbour.depth, context.depth,
            "{} is at another level and does not belong in a zero band",
            neighbour.name
        );
    }
    assert!(
        !context.neighbours.iter().any(|n| n.name == "scale"),
        "the free function is a level away and must be excluded"
    );
}

#[test]
fn the_payload_stays_within_the_budget_and_sheds_whole_lines() {
    let engine = corpus();
    let generous =
        hydrate_symbol_context(&engine, "CacheStore", DEFAULT_MAX_TOKENS).expect("hydrates");
    assert!(generous.tokens <= DEFAULT_MAX_TOKENS);

    // The full payload, line by line. A budgeted render may drop a line but must never rewrite
    // one — a half-rendered neighbour is exactly the failure this property rules out.
    let full: Vec<String> = generous
        .render_within(usize::MAX)
        .lines()
        .map(str::to_owned)
        .collect();

    for budget in [64usize, 80, 96, 128] {
        let text = generous.render_within(budget);
        let tokens = estimate_tokens(&text);
        if tokens > budget {
            // Only admissible when the header alone exceeds it, which hydration refuses.
            continue;
        }
        assert!(text.starts_with("Symbol: CacheStore"), "{budget}: {text}");
        assert!(text.ends_with('\n'), "a payload ends on a whole line");
        for line in text.lines() {
            assert!(
                full.iter().any(|kept| kept == line),
                "a budgeted render rewrote a line ({budget}): {line:?}"
            );
        }
    }
}
