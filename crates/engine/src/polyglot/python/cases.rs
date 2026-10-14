#[cfg(test)]
mod python_tests {
    use super::super::*;
    use crate::polyglot::parse;

    /// A module with a decorated method, a class docstring, annotated signatures, a splat, a
    /// private helper and an inner definition.
    const SAMPLE: &str = r#"
"""Small helpers."""

from typing import Iterable


def scale(x: float, factor: float) -> float:
    """Multiply by a factor."""
    return x * factor


def total(*values, **options) -> int:
    return sum(values)


class Repository:
    """Reads users out of a store."""

    def __init__(self, url: str) -> None:
        self.url = url

    @staticmethod
    def make(url: str) -> "Repository":
        return Repository(url)

    async def find(self, user_id: str, limit: int = 10) -> str | None:
        """Finds one user."""

        def inner() -> None:
            return None

        return None


def _private():
    pass
"#;

    fn parse_py() -> Vec<UniversalSymbol> {
        parse(Path::new("src/models.py"), SAMPLE, Language::Python).expect("python parses")
    }

    fn named<'a>(symbols: &'a [UniversalSymbol], name: &str) -> &'a UniversalSymbol {
        symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| {
                let found: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
                panic!("no symbol named {name}; found {found:?}")
            })
    }

    #[test]
    fn a_class_is_a_scope_with_its_docstring() {
        let symbols = parse_py();
        let class = named(&symbols, "Repository");
        assert_eq!(class.kind, SymbolKind::Class);
        assert_eq!(class.depth, 2);
        assert_eq!(
            class.docstring.as_deref(),
            Some("Reads users out of a store.")
        );
        assert_eq!(class.path, "models.Repository");
        assert_eq!(class.signature, "class Repository");
        assert!(class.exported);
    }

    #[test]
    fn typed_annotations_and_defaults_are_read() {
        let symbols = parse_py();
        let scale = named(&symbols, "scale");
        assert_eq!(scale.kind, SymbolKind::Function);
        assert_eq!(scale.depth, 3);
        assert_eq!(scale.docstring.as_deref(), Some("Multiply by a factor."));
        assert_eq!(scale.params.len(), 2);
        assert_eq!(scale.params[0].name, "x");
        assert_eq!(scale.params[0].type_name, "float");
        assert_eq!(scale.return_type.as_deref(), Some("float"));

        let find = named(&symbols, "find");
        assert_eq!(find.params[0].name, "self");
        assert_eq!(find.params[0].type_name, "Self");
        assert_eq!(find.params[2].name, "limit");
        assert_eq!(find.return_type.as_deref(), Some("str | None"));
        assert_eq!(find.docstring.as_deref(), Some("Finds one user."));
    }

    #[test]
    fn splat_parameters_keep_their_markers() {
        let symbols = parse_py();
        let total = named(&symbols, "total");
        assert_eq!(total.params[0].name, "*values");
        assert_eq!(total.params[1].name, "**options");
        assert_eq!(total.return_type.as_deref(), Some("int"));
    }

    #[test]
    fn a_method_is_a_member_and_its_decorator_is_written_into_the_signature() {
        let symbols = parse_py();
        let make = named(&symbols, "make");
        assert_eq!(make.kind, SymbolKind::Method);
        assert_eq!(make.depth, 3);
        assert_eq!(
            make.signature,
            "@staticmethod def make(url: str) -> \"Repository\""
        );
        assert_eq!(make.path, "models.Repository.make");
        assert_eq!(make.params[0].name, "url");
        // A decorated definition has no docstring of its own and no comment above it, so the
        // one it does not have is reported as absent rather than invented.
        assert_eq!(make.docstring, None);
    }

    #[test]
    fn an_inner_definition_is_a_leaf() {
        let symbols = parse_py();
        let inner = named(&symbols, "inner");
        assert_eq!(inner.kind, SymbolKind::Function);
        assert_eq!(inner.depth, 4, "a def inside a def is a leaf");
        assert_eq!(inner.path, "models.Repository.find.inner");
        assert_eq!(inner.line, 29);
    }

    #[test]
    fn a_leading_underscore_is_not_exported() {
        let symbols = parse_py();
        assert!(!named(&symbols, "_private").exported);
        assert!(named(&symbols, "total").exported);
    }

    #[test]
    fn the_module_docstring_belongs_to_no_declaration() {
        let symbols = parse_py();
        // `"""Small helpers."""` is the module's own documentation and there is no node for
        // the module itself, so it must not be attached to the first declaration below it.
        assert_eq!(named(&symbols, "Repository").line, 16);
        assert_eq!(
            named(&symbols, "scale").docstring.as_deref(),
            Some("Multiply by a factor.")
        );
    }

    #[test]
    fn a_one_line_docstring_keeps_no_quotes() {
        let source = "\
def one():
    '''Single quoted.'''
    pass
";
        let symbols = parse(Path::new("src/one.py"), source, Language::Python).expect("parses");
        assert_eq!(symbols[0].docstring.as_deref(), Some("Single quoted."));
    }
}
