#[cfg(test)]
mod tests {
    use super::super::comments::strip_comment_markers;
    use super::super::*;
    use crate::polyglot::SymbolKind;

    #[test]
    fn a_module_is_read_off_the_path() {
        assert_eq!(
            module_label(Path::new("crates/engine/src/store.rs")),
            "engine"
        );
        assert_eq!(
            module_label(Path::new("crates/server/ui-src/app.tsx")),
            "server/ui-src"
        );
        assert_eq!(module_label(Path::new("src/auth.rs")), "");
        assert_eq!(
            module_path(Path::new("crates/engine/src/polyglot/python.rs")),
            "engine.polyglot.python"
        );
        assert_eq!(module_path(Path::new("src/auth.rs")), "auth");
    }

    #[test]
    fn a_dotted_path_is_module_then_chain_then_name() {
        assert_eq!(
            dotted_path("engine.store", &["Store".into()], "fetch"),
            "engine.store.Store.fetch"
        );
        assert_eq!(dotted_path("", &[], "free"), "free");
    }

    #[test]
    fn the_level_rule_matches_the_objectives_examples() {
        // A method inside a class or an impl is a member, not a leaf.
        assert_eq!(level(base_level(SymbolKind::Method), 0), 3);
        assert_eq!(level(base_level(SymbolKind::Struct), 0), 2);
        // A function inside a function is a leaf.
        assert_eq!(level(base_level(SymbolKind::Function), 1), 4);
        assert_eq!(level(base_level(SymbolKind::Method), 1), 4);
        // The cap holds however deep the nesting runs.
        assert_eq!(level(base_level(SymbolKind::Function), 9), MAX_LEVEL);
    }

    #[test]
    fn comment_markers_are_stripped_and_inner_docs_are_not() {
        assert!(is_rust_doc("/// A doc line."));
        assert!(is_rust_doc("/** Block. */"));
        assert!(!is_rust_doc("//! Module docs."));
        assert!(!is_rust_doc("// plain"));
        assert_eq!(
            strip_comment_markers("/// One.\n/// Two."),
            "One.\nTwo.".to_string()
        );
        assert_eq!(
            strip_comment_markers("/**\n * One.\n */"),
            "One.".to_string()
        );
    }
}
