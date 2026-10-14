//! Stage, port and entrypoint extraction.

use std::path::Path;

use super::extract_docker_symbols;
use crate::polyglot::SymbolKind;

#[test]
fn a_multi_stage_build_yields_one_scope_per_stage() {
    let source = "\
FROM rust:1.80-alpine AS builder
WORKDIR /app
RUN cargo build --release

FROM alpine:3.20 AS runner
EXPOSE 8080/tcp
CMD [\"/usr/local/bin/server\"]
";

    let symbols = extract_docker_symbols(Path::new("Dockerfile"), source);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["builder", "runner", "expose_runner", "cmd_runner"]
    );
    assert!(symbols.iter().all(|s| s.kind == SymbolKind::DockerStage));

    let builder = &symbols[0];
    assert_eq!(builder.depth, 2, "a stage is a scope");
    assert_eq!(builder.line, 1);
    assert_eq!(builder.signature, "STAGE builder FROM rust:1.80-alpine");

    let runner = &symbols[1];
    assert_eq!(runner.line, 5);
    assert_eq!(runner.signature, "STAGE runner FROM alpine:3.20");

    let expose = &symbols[2];
    assert_eq!(expose.depth, 3, "a member sits inside its stage");
    assert_eq!(expose.line, 6);
    assert_eq!(expose.signature, "EXPOSE 8080/tcp");

    let cmd = &symbols[3];
    assert_eq!(cmd.signature, "CMD [\"/usr/local/bin/server\"]");
}

#[test]
fn an_unnamed_stage_is_numbered_and_its_members_still_resolve() {
    let source = "FROM alpine\nEXPOSE 80\n";
    let symbols = extract_docker_symbols(Path::new("Dockerfile"), source);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["stage_1", "expose_stage_1"]);
}

#[test]
fn an_instruction_before_any_from_belongs_to_the_default_stage() {
    let source = "# a comment\nEXPOSE 9000\nFROM alpine AS app\n";
    let symbols = extract_docker_symbols(Path::new("Dockerfile"), source);
    assert_eq!(symbols[0].name, "expose_default");
    assert_eq!(symbols[0].line, 2);
}

#[test]
fn unparsable_source_yields_nothing_rather_than_a_guess() {
    let symbols = extract_docker_symbols(Path::new("Dockerfile"), "FROM\n\u{0}not a dockerfile");
    assert!(symbols.is_empty());
}
