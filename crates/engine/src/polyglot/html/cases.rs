//! HTML extraction.

use std::path::Path;

use super::extract_html_symbols;
use crate::polyglot::SymbolKind;

const PAGE: &str = r#"<!DOCTYPE html>
<html>
<head><title>Demo</title></head>
<body>
  <header id="top"><nav id="menu"></nav></header>
  <main>
    <user-card id="card-1" class="card card--wide"></user-card>
    <section>
      <form id="signup"><input id="email" for="signup"></form>
    </section>
    <script>if (a < b) { document.write('<div>'); }</script>
  </main>
</body>
</html>
"#;

#[test]
fn ids_and_custom_elements_become_declarations() {
    let symbols = extract_html_symbols(Path::new("templates/index.html"), PAGE);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "header",
            "top",
            "nav",
            "menu",
            "main",
            "user-card",
            "card-1",
            "section",
            "form",
            "signup",
            "email"
        ]
    );

    let by_name = |name: &str| symbols.iter().find(|s| s.name == name).expect("declared");
    assert_eq!(by_name("top").kind, SymbolKind::Property);
    assert_eq!(by_name("top").signature, "ID top <header>");
    assert_eq!(by_name("top").line, 5);
    assert_eq!(by_name("user-card").kind, SymbolKind::Class);
    assert_eq!(by_name("user-card").signature, "ELEMENT <user-card>");
    assert_eq!(by_name("main").kind, SymbolKind::Module);
    assert_eq!(by_name("main").signature, "LANDMARK <main>");
    assert_eq!(by_name("email").language, crate::polyglot::Language::Html);
}

#[test]
fn a_landmark_is_declared_once_however_often_it_appears() {
    let source = "<section></section>\n<section id=\"two\"></section>\n<section></section>\n";
    let symbols = extract_html_symbols(Path::new("a.html"), source);
    let sections = symbols.iter().filter(|s| s.name == "section").count();
    assert_eq!(sections, 1, "one landmark, not one per tag");
    assert_eq!(
        symbols.iter().find(|s| s.name == "section").unwrap().line,
        1
    );
}

#[test]
fn a_script_body_is_not_markup() {
    let source = "<main>\n<script>const s = '<div id=\"ghost\">';</script>\n</main>\n";
    let names: Vec<String> = extract_html_symbols(Path::new("a.html"), source)
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert!(!names.contains(&"ghost".to_owned()), "{names:?}");
}

#[test]
fn attribute_values_survive_quoting_styles() {
    let source = "<div id='single' data-x=bare></div>\n";
    let symbols = extract_html_symbols(Path::new("a.html"), source);
    assert_eq!(symbols[0].name, "single");
}

#[test]
fn comment_prose_and_comparisons_are_not_tags() {
    let source = "\
<!-- <fake-comment-tag id=\"ghost\"> -->
<p>x <- y and count < 250</p>
<my-widget id=\"real\"></my-widget>
";
    let symbols = extract_html_symbols(Path::new("a.html"), source);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    for ghost in ["fake-comment-tag", "ghost", "-"] {
        assert!(
            !names.contains(&ghost),
            "{ghost} leaked from markup: {names:?}"
        );
    }
    let widget = symbols
        .iter()
        .find(|s| s.name == "my-widget")
        .expect("the real custom element");
    assert_eq!(widget.kind, SymbolKind::Class);
    assert_eq!(widget.signature, "ELEMENT <my-widget>");
    assert_eq!(widget.line, 3);
    let real = symbols
        .iter()
        .find(|s| s.name == "real")
        .expect("the real id");
    assert_eq!(real.line, 3);
}
