//! Markup, stylesheet and SQL reference extraction.

use miter_storage::schema::ReferenceKind;

use super::{css, html, sql};

#[test]
fn html_classes_fragments_and_labels_are_references() {
    let source =
        "<a class=\"card card--wide\" href=\"#main\">x</a>\n<label for=\"email\">e</label>\n";
    let refs = html("templates/index.html", source);
    let names: Vec<&str> = refs.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, vec![".card", ".card--wide", "main", "email"]);
    assert!(
        refs.iter()
            .all(|(_, e)| e.reference_kind == ReferenceKind::Usage)
    );
    assert_eq!(refs[0].1.line_range, (1, 1));
    assert_eq!(refs[0].1.file_path, "templates/index.html");
}

#[test]
fn a_css_var_call_is_a_reference_to_the_token() {
    let source = ".card {\n  color: var(--brand);\n  gap: var(--space-sm, 4px);\n}\n";
    let refs = css("static/app.css", source);
    let names: Vec<&str> = refs.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, vec!["--brand", "--space-sm"]);
    assert_eq!(refs[0].1.line_range, (2, 2));
}

#[test]
fn sql_table_clauses_are_references() {
    let source = "CREATE TABLE posts (\n  author_id INTEGER REFERENCES users(id)\n);\nSELECT * FROM posts JOIN users ON users.id = posts.author_id;\n";
    let refs = sql("migrations/001.sql", source);
    let names: Vec<&str> = refs.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, vec!["users", "posts", "users"]);
    assert_eq!(refs[0].1.line_range, (2, 2));
}

#[test]
fn a_quoted_string_is_not_a_clause() {
    let source = "INSERT INTO t VALUES ('FROM users');\n";
    assert!(sql("a.sql", source).is_empty());
}
