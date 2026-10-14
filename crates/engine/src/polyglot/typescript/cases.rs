#[cfg(test)]
mod typescript_tests {
    use super::super::*;
    use crate::polyglot::parse;

    /// The four declaration shapes a TypeScript project always has: an interface with two
    /// kinds of member, an exported class with a method, a property and a function-valued
    /// field, an exported `const` arrow function, a namespace, a type alias and a default
    /// export.
    const SAMPLE: &str = r#"
/** A user of the system. */
export interface User {
    id: string;
    /** The display name. */
    label(): string;
}

/**
 * Fetches a user.
 * Cached per id.
 */
export class Repository {
    /** The base url. */
    readonly url: string;

    /** Fetches one. */
    async find(id: string, retries: number): Promise<User | null> {
        return null;
    }

    format = (user: User): string => user.id;
}

/** Doubles a number. */
export const double = (value: number): number => value * 2;

namespace Internals {
    export function nested(flag: boolean): boolean {
        return !flag;
    }
}

type Handler = (event: string) => void;

export default function main(): void {}
"#;

    fn parse_ts() -> Vec<UniversalSymbol> {
        parse(Path::new("src/api.ts"), SAMPLE, Language::TypeScript).expect("typescript parses")
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
    fn classes_interfaces_and_their_members_are_extracted() {
        let symbols = parse_ts();
        let interface = named(&symbols, "User");
        assert_eq!(interface.kind, SymbolKind::Interface);
        assert_eq!(interface.depth, 2);
        assert_eq!(
            interface.docstring.as_deref(),
            Some("A user of the system.")
        );
        assert!(interface.exported);
        assert_eq!(interface.path, "api.User");

        let class = named(&symbols, "Repository");
        assert_eq!(class.kind, SymbolKind::Class);
        assert_eq!(class.depth, 2);
        assert_eq!(
            class.docstring.as_deref(),
            Some("Fetches a user.\nCached per id.")
        );
        assert!(class.exported);
    }

    #[test]
    fn members_are_level_three_inside_their_type() {
        let symbols = parse_ts();
        // A method of an interface…
        let label = named(&symbols, "label");
        assert_eq!(label.kind, SymbolKind::Method);
        assert_eq!(label.depth, 3);
        assert_eq!(label.docstring.as_deref(), Some("The display name."));

        // …and of a class, with its parameters and return type.
        let find = named(&symbols, "find");
        assert_eq!(find.kind, SymbolKind::Method);
        assert_eq!(find.depth, 3);
        assert_eq!(find.params.len(), 2);
        assert_eq!(find.params[0].name, "id");
        assert_eq!(find.params[0].type_name, "string");
        assert_eq!(find.return_type.as_deref(), Some("Promise<User | null>"));
        assert_eq!(find.docstring.as_deref(), Some("Fetches one."));
        assert!(!find.exported, "a method is not exported on its own");
    }

    #[test]
    fn a_property_is_a_member_and_a_function_valued_field_is_a_method() {
        let symbols = parse_ts();
        let url = named(&symbols, "url");
        assert_eq!(url.kind, SymbolKind::Property);
        assert_eq!(url.depth, 3);
        assert_eq!(url.docstring.as_deref(), Some("The base url."));

        // `format = (user) => …` is an arrow function bound to a class field: a method.
        let format = named(&symbols, "format");
        assert_eq!(format.kind, SymbolKind::Method);
        assert_eq!(format.depth, 3);
        assert_eq!(format.params[0].name, "user");
        assert_eq!(format.return_type.as_deref(), Some("string"));
    }

    #[test]
    fn an_exported_arrow_function_is_named_by_its_binding() {
        let symbols = parse_ts();
        let double = named(&symbols, "double");
        assert_eq!(double.kind, SymbolKind::Function);
        assert_eq!(double.depth, 3);
        assert!(double.exported);
        assert_eq!(double.docstring.as_deref(), Some("Doubles a number."));
        assert_eq!(double.params[0].name, "value");
        assert_eq!(double.return_type.as_deref(), Some("number"));
        // The signature stops at the arrow's body, so it is the header rather than the whole
        // expression.
        assert_eq!(double.signature, "double = (value: number): number =>");
    }

    #[test]
    fn a_namespace_is_a_module_and_a_function_inside_it_is_a_member() {
        let symbols = parse_ts();
        let namespace = named(&symbols, "Internals");
        assert_eq!(namespace.kind, SymbolKind::Module);
        assert_eq!(namespace.depth, 1);

        let nested = named(&symbols, "nested");
        assert_eq!(nested.kind, SymbolKind::Function);
        assert_eq!(nested.depth, 3);
        assert_eq!(nested.path, "api.Internals.nested");
    }

    #[test]
    fn a_type_alias_and_a_default_export_are_read() {
        let symbols = parse_ts();
        let alias = named(&symbols, "Handler");
        assert_eq!(alias.kind, SymbolKind::TypeAlias);
        assert_eq!(alias.depth, 2);

        let main = named(&symbols, "main");
        assert_eq!(main.kind, SymbolKind::Function);
        assert_eq!(main.depth, 3);
        assert!(main.exported, "`export default function` is an export");
    }

    #[test]
    fn a_plain_line_comment_is_not_a_docstring() {
        let source = "\
// Not documentation.
export function plain(): void {}
";
        let symbols =
            parse(Path::new("src/plain.ts"), source, Language::TypeScript).expect("parses");
        assert_eq!(symbols[0].docstring, None);
        assert_eq!(symbols[0].signature, "function plain(): void");
    }

    #[test]
    fn a_tsx_component_is_read_with_the_tsx_grammar() {
        let source = r#"
/** Renders a label. */
export function Widget({ label }: { label: string }) {
    return <span className="widget">{label}</span>;
}
"#;
        let symbols =
            parse(Path::new("src/Widget.tsx"), source, Language::Tsx).expect("tsx parses");
        let widget = &symbols[0];
        assert_eq!(widget.name, "Widget");
        assert_eq!(widget.language, Language::Tsx);
        assert_eq!(widget.kind, SymbolKind::Function);
        assert_eq!(widget.docstring.as_deref(), Some("Renders a label."));
        assert_eq!(widget.params.len(), 1);
        assert_eq!(
            widget.language.index_language(),
            Some(miter_ast::Language::Tsx)
        );
    }

    #[test]
    fn a_nested_callback_body_is_a_leaf() {
        let source = "\
export function outer(): void {
    const inner = (n: number): number => n + 1;
}
";
        let symbols =
            parse(Path::new("src/nest.ts"), source, Language::TypeScript).expect("parses");
        let inner = named(&symbols, "inner");
        assert_eq!(inner.kind, SymbolKind::Function);
        assert_eq!(inner.depth, 4, "a function inside a function is a leaf");
    }
}
