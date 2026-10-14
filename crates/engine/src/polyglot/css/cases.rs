//! CSS extraction.

use std::path::Path;

use super::extract_css_symbols;
use crate::polyglot::{Language, SymbolKind};

const SHEET: &str = "\
:root {
  --brand: #3b82f6;
  --space-sm: 4px;
}

.card, .card--wide {
  color: var(--brand);
}

#main .card {
  padding: var(--space-sm);
}

@keyframes fade-in {
  from { opacity: 0; }
}

@layer utilities {
  .sr-only { position: absolute; }
}

@media (max-width: 40em) {
  .card { color: black; }
}
";

#[test]
fn custom_properties_selectors_and_at_rules_are_declared() {
    let symbols = extract_css_symbols(Path::new("static/app.css"), SHEET);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "--brand",
            "--space-sm",
            ".card",
            ".card--wide",
            "#main",
            "fade-in",
            "utilities",
            ".sr-only"
        ]
    );
    assert!(symbols.iter().all(|s| s.language == Language::Css));
}

#[test]
fn a_name_is_declared_once_at_its_first_occurrence() {
    let symbols = extract_css_symbols(Path::new("a.css"), SHEET);
    let cards: Vec<&str> = symbols
        .iter()
        .filter(|s| s.name == ".card")
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(cards.len(), 1, "one class, however many rules use it");
    assert_eq!(symbols.iter().find(|s| s.name == ".card").unwrap().line, 6);
}

#[test]
fn kinds_distinguish_a_token_from_a_selector_and_a_layer() {
    let symbols = extract_css_symbols(Path::new("a.css"), SHEET);
    let by_name = |name: &str| symbols.iter().find(|s| s.name == name).expect("declared");
    assert_eq!(by_name("--brand").kind, SymbolKind::Property);
    assert_eq!(by_name("--brand").signature, "--brand: #3b82f6");
    assert_eq!(by_name(".card").kind, SymbolKind::Class);
    assert_eq!(by_name("#main").kind, SymbolKind::Property);
    assert_eq!(by_name("fade-in").kind, SymbolKind::Function);
    assert_eq!(by_name("fade-in").signature, "@keyframes fade-in");
    assert_eq!(by_name("utilities").kind, SymbolKind::Module);
    assert_eq!(by_name("utilities").signature, "@layer utilities");
}

#[test]
fn an_at_rule_prelude_names_no_selector() {
    let source = "@media (max-width: 40em) {\n  body { margin: 0; }\n}\n";
    let symbols = extract_css_symbols(Path::new("a.css"), source);
    assert!(symbols.is_empty(), "{symbols:?}");
}

#[test]
fn scss_files_are_stylesheets_too() {
    let symbols = extract_css_symbols(
        Path::new("theme.scss"),
        "$gap: 1rem;\n.grid { gap: $gap; }\n",
    );
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, ".grid");
}
