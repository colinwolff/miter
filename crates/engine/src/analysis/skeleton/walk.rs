//! The body walk: which byte ranges of a file are implementation rather than interface.

/// Recursively discovers callable implementation block bodies.
pub(super) fn collect_function_bodies(
    node: tree_sitter::Node<'_>,
    lang: miter_ast::Language,
    blocks: &mut Vec<(usize, usize)>,
) {
    let kind = node.kind();
    match lang {
        miter_ast::Language::Rust => {
            let body_opt = if matches!(kind, "function_item" | "closure_expression") {
                node.child_by_field_name("body")
            } else {
                None
            };
            if let Some(body) = body_opt {
                blocks.push((body.start_byte(), body.end_byte()));
                return;
            }
        }
        miter_ast::Language::TypeScript
        | miter_ast::Language::Tsx
        | miter_ast::Language::JavaScript => {
            let is_ts_callable = matches!(
                kind,
                "function_declaration"
                    | "generator_function_declaration"
                    | "method_definition"
                    | "arrow_function"
                    | "function_expression"
            );
            if is_ts_callable {
                let body_match = node
                    .child_by_field_name("body")
                    .filter(|b| b.kind() == "statement_block");
                if let Some(body) = body_match {
                    blocks.push((body.start_byte(), body.end_byte()));
                    return;
                }
            }
        }
        _ => {}
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_function_bodies(child, lang, blocks);
        }
    }
}
