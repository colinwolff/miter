//! Dockerfile declarative symbol extractor.
//!
//! A Dockerfile is a build graph with names: a stage (`FROM … AS <name>`), the variables that
//! parameterise it (`ARG`/`ENV`) and the ports it publishes (`EXPOSE`). Each is a declaration a
//! reader wants to find by name, so each becomes a symbol at the level it sits at — a stage is a
//! scope, everything inside it a member.
//!
//! `ARG` and `ENV` values are configuration, and configuration is where a credential lives, so
//! every value is redacted before it is recorded (see `vars`).

#[cfg(test)]
mod tests;
mod vars;

use std::path::Path;

use dockerfile_parser::{Dockerfile, Instruction};

use super::{Language, SymbolKind, UniversalSymbol, common};

/// Extracts declarative container build symbols from Dockerfile source.
#[must_use]
pub fn extract_docker_symbols(path: &Path, content: &str) -> Vec<UniversalSymbol> {
    let Ok(dockerfile) = Dockerfile::parse(content) else {
        return Vec::new();
    };

    let module = common::module_path(path);
    let mut symbols = Vec::new();
    let mut current_stage: Option<String> = None;
    let mut stage_index = 0usize;

    for instruction in dockerfile.instructions {
        match instruction {
            Instruction::From(from) => {
                stage_index += 1;
                let image = from.image.content.clone();
                let stage_name = from
                    .alias
                    .map(|alias| alias.content.clone())
                    .unwrap_or_else(|| format!("stage_{stage_index}"));
                let start = from.span.start;

                symbols.push(UniversalSymbol {
                    path: common::dotted_path(&module, &[], &stage_name),
                    signature: scrub(format!("STAGE {stage_name} FROM {image}")),
                    name: stage_name.clone(),
                    language: Language::Rust,
                    kind: SymbolKind::DockerStage,
                    depth: 2,
                    docstring: None,
                    span: (start, from.span.end),
                    file: path.to_path_buf(),
                    line: line_of(content, start),
                    params: Vec::new(),
                    return_type: None,
                    exported: true,
                });
                current_stage = Some(stage_name);
            }
            Instruction::Arg(arg) => {
                vars::push_arg(path, content, &module, &current_stage, &arg, &mut symbols);
            }
            Instruction::Env(env) => {
                vars::push_env(path, content, &module, &current_stage, &env, &mut symbols);
            }
            Instruction::Misc(misc) if misc.instruction.as_ref().eq_ignore_ascii_case("EXPOSE") => {
                let start = misc.span.start;
                let signature = format!("EXPOSE {}", misc.arguments.to_string().trim());
                symbols.push(member(
                    path,
                    &module,
                    &current_stage,
                    "expose",
                    signature,
                    line_of(content, start),
                    (start, misc.span.end),
                ));
            }
            Instruction::Entrypoint(entry) => {
                let start = entry.span.start;
                let signature = format!("ENTRYPOINT {}", format_expr(&entry.expr));
                symbols.push(member(
                    path,
                    &module,
                    &current_stage,
                    "entrypoint",
                    signature,
                    line_of(content, start),
                    (start, entry.span.end),
                ));
            }
            Instruction::Cmd(cmd) => {
                let start = cmd.span.start;
                let signature = format!("CMD {}", format_expr(&cmd.expr));
                symbols.push(member(
                    path,
                    &module,
                    &current_stage,
                    "cmd",
                    signature,
                    line_of(content, start),
                    (start, cmd.span.end),
                ));
            }
            _ => {}
        }
    }

    symbols
}

/// Builds a member declaration belonging to `stage` (or the file's unnamed default stage).
///
/// The name is `kind_stage`, so two stages publishing the same port produce two distinguishable
/// declarations rather than one overwriting the other.
pub(super) fn member(
    path: &Path,
    module: &str,
    stage: &Option<String>,
    kind: &str,
    signature: String,
    line: usize,
    span: (usize, usize),
) -> UniversalSymbol {
    let prefix = stage.clone().unwrap_or_else(|| "default".to_owned());
    let name = format!("{kind}_{prefix}");
    UniversalSymbol {
        path: common::dotted_path(module, &[prefix], &name),
        name,
        language: Language::Rust,
        kind: SymbolKind::DockerStage,
        depth: 3,
        docstring: None,
        signature: scrub(signature),
        span,
        file: path.to_path_buf(),
        line,
        params: Vec::new(),
        return_type: None,
        exported: true,
    }
}

/// A signature with every embedded secret scrubbed.
///
/// The extractors redact the values they *know* are sensitive; this is the belt-and-braces pass
/// that catches a credential sitting in a value that looked ordinary.
pub(super) fn scrub(signature: String) -> String {
    crate::security::scrub_source_text(&signature)
}

/// The one-based line a byte offset sits on.
///
/// The parser reports byte spans into the source it was given, so the line is counted rather than
/// asked for — which is also what keeps a multi-line `ENV a=1 \\\n b=2` on the line it starts on.
pub(super) fn line_of(content: &str, offset: usize) -> usize {
    content[..offset.min(content.len())].matches('\n').count() + 1
}

/// Renders a `CMD`/`ENTRYPOINT` expression the way the source spells it.
fn format_expr(expr: &dockerfile_parser::ShellOrExecExpr) -> String {
    match expr {
        dockerfile_parser::ShellOrExecExpr::Shell(shell) => shell.to_string(),
        dockerfile_parser::ShellOrExecExpr::Exec(array) => {
            let items: Vec<String> = array
                .as_str_vec()
                .iter()
                .map(|item| format!("\"{item}\""))
                .collect();
            format!("[{}]", items.join(", "))
        }
    }
}
