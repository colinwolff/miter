//! `ARG` and `ENV`: the variables that parameterise a build.
//!
//! A variable's *key* survives verbatim — a reader searching for `DATABASE_URL` wants to find the
//! declaration — but its *value* is configuration, and configuration is where a credential lives.
//! Every value therefore goes through [`redacted_value`] before it is recorded.

use std::path::Path;

use dockerfile_parser::{ArgInstruction, EnvInstruction};

use super::{UniversalSymbol, line_of, member};
use crate::polyglot::redact::redacted_value;

/// Records an `ARG` declaration.
pub(super) fn push_arg(
    path: &Path,
    content: &str,
    module: &str,
    stage: &Option<String>,
    arg: &ArgInstruction,
    out: &mut Vec<UniversalSymbol>,
) {
    let key = arg.name.content.clone();
    let value = arg.value.as_ref().map(|v| v.content.as_str()).unwrap_or("");
    out.push(declaration(
        path,
        content,
        module,
        stage,
        "arg",
        &key,
        &format!("ARG {key}"),
        value,
        arg.span.start,
        arg.span.end,
    ));
}

/// Records every `ENV` pair a single instruction declares.
pub(super) fn push_env(
    path: &Path,
    content: &str,
    module: &str,
    stage: &Option<String>,
    env: &EnvInstruction,
    out: &mut Vec<UniversalSymbol>,
) {
    for var in &env.vars {
        let key = var.key.content.clone();
        out.push(declaration(
            path,
            content,
            module,
            stage,
            "env",
            &key,
            &format!("ENV {key}"),
            &var.value.to_string(),
            var.span.start,
            var.span.end,
        ));
    }
}

/// Assembles one variable declaration, redacting its value.
#[allow(clippy::too_many_arguments)]
fn declaration(
    path: &Path,
    content: &str,
    module: &str,
    stage: &Option<String>,
    kind: &str,
    key: &str,
    prefix: &str,
    value: &str,
    start: usize,
    end: usize,
) -> UniversalSymbol {
    let redacted = redacted_value(key, value);
    let signature = if redacted.is_empty() {
        prefix.to_owned()
    } else {
        format!("{prefix}={redacted}")
    };
    // The declaration is named after the key, not the stage, so `miter lookup DATABASE_URL`
    // resolves the variable rather than the build that declares it.
    let mut symbol = member(
        path,
        module,
        stage,
        kind,
        signature,
        line_of(content, start),
        (start, end),
    );
    symbol.path = crate::polyglot::common::dotted_path(module, &[], key);
    symbol.name = key.to_owned();
    symbol
}

#[cfg(test)]
mod cases {
    use super::*;
    use crate::polyglot::SymbolKind;

    const SOURCE: &str = "\
FROM rust:1.80 AS builder
ARG CARGO_PROFILE=release
ARG TOKEN=ghp_1234567890abcdef
ENV DATABASE_URL=postgres://admin:secret@localhost/db
ENV PORT=8080 LOG_LEVEL=debug
";

    #[test]
    fn args_and_envs_are_declared_by_key() {
        let symbols =
            crate::polyglot::docker::extract_docker_symbols(Path::new("Dockerfile"), SOURCE);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "builder",
                "CARGO_PROFILE",
                "TOKEN",
                "DATABASE_URL",
                "PORT",
                "LOG_LEVEL"
            ]
        );
        assert!(symbols.iter().all(|s| s.kind == SymbolKind::DockerStage));
        assert_eq!(symbols[1].line, 2);
        assert_eq!(symbols[1].signature, "ARG CARGO_PROFILE=release");
    }

    #[test]
    fn secret_values_never_reach_the_signature() {
        let symbols =
            crate::polyglot::docker::extract_docker_symbols(Path::new("Dockerfile"), SOURCE);
        let by_name = |name: &str| symbols.iter().find(|s| s.name == name).expect("declared");

        let token = by_name("TOKEN");
        assert_eq!(token.signature, "ARG TOKEN=[REDACTED_SECRET]");
        assert!(!token.signature.contains("ghp_"));

        // A URL carrying credentials is caught by the entropy check.
        let db = by_name("DATABASE_URL");
        assert!(!db.signature.contains("secret"), "{}", db.signature);
        assert!(
            db.signature.starts_with("ENV DATABASE_URL="),
            "{}",
            db.signature
        );

        // Ordinary configuration survives, so the index is still useful.
        assert_eq!(
            by_name("CARGO_PROFILE").signature,
            "ARG CARGO_PROFILE=release"
        );
        assert_eq!(by_name("PORT").signature, "ENV PORT=8080");
        assert_eq!(by_name("LOG_LEVEL").signature, "ENV LOG_LEVEL=debug");
    }
}
