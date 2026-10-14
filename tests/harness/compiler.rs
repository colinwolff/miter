//! The compiler check, end to end, against a real `cargo check`.

use std::sync::Arc;

use miter_harness::{CompilerVerifier, SandboxEnvironment, Toolchain, VerificationOutcome};

use crate::{BROKEN_SOURCE, FIXED_SOURCE, cargo_fixture, scratch};

/// Generous, because the first `cargo check` in a fresh target directory builds the standard
/// library's proc-macro surface and takes tens of seconds on a cold machine.
const PATIENCE_MS: u64 = 300_000;

#[tokio::test]
async fn an_intentional_type_error_is_caught_with_its_file_and_span() {
    let root = cargo_fixture("compiler-typed", BROKEN_SOURCE);
    let verifier = Arc::new(CompilerVerifier::new(Toolchain::Cargo).with_timeout_ms(PATIENCE_MS));

    let outcome = verifier.verify(&root).await.expect("cargo ran");
    match outcome {
        VerificationOutcome::DiagnosticFailure {
            error_code,
            message,
            spans,
            ..
        } => {
            assert_eq!(error_code, "E0308", "{message}");
            assert!(message.contains("mismatched types"), "{message}");
            let span = spans.first().expect("a located failure");
            assert_eq!(span.file_path, "src/lib.rs", "{span:?}");
            assert_eq!(span.line_start, 2, "{span:?}");
            // The byte range is real, not a placeholder: it slices the offending token out of
            // the file the compiler read.
            let source = std::fs::read_to_string(root.join("src/lib.rs")).expect("read the source");
            assert_eq!(
                &source[span.byte_start..span.byte_end],
                "\"forty-two\"",
                "the span does not cover the expression rustc rejected"
            );
        }
        other => panic!("expected a located diagnostic failure, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_syntax_error_is_caught_too() {
    // A parse error and a type error are different codes and different spans, and a harness that
    // only understood one of them would report the other as an unreadable failure.
    let root = cargo_fixture(
        "compiler-syntax",
        "pub fn answer() -> u8 {\n    let = 42;\n}\n",
    );
    let verifier = Arc::new(CompilerVerifier::new(Toolchain::Cargo).with_timeout_ms(PATIENCE_MS));
    let outcome = verifier.verify(&root).await.expect("cargo ran");

    let VerificationOutcome::DiagnosticFailure { spans, .. } = outcome else {
        panic!("a syntax error was not reported as a diagnostic: {outcome:?}");
    };
    let span = spans.first().expect("a located failure");
    assert_eq!(span.file_path, "src/lib.rs", "{span:?}");
    assert_eq!(span.line_start, 2, "{span:?}");

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_fixed_crate_passes_and_says_how_long_it_took() {
    let root = cargo_fixture("compiler-fixed", FIXED_SOURCE);
    let verifier = Arc::new(CompilerVerifier::new(Toolchain::Cargo).with_timeout_ms(PATIENCE_MS));
    let outcome = verifier.verify(&root).await.expect("cargo ran");

    match outcome {
        VerificationOutcome::Success {
            stdout,
            duration_ms,
        } => {
            // A duration of zero would mean nothing was measured; a build took *some* time.
            assert!(duration_ms > 0, "a build reported zero milliseconds");
            assert!(
                !stdout.is_empty(),
                "a passing build reported nothing about itself"
            );
            // `cargo check --message-format=json` prints *only* JSON, so the tail is the last
            // of those; what is asserted here is that the harness reported the run at all
            // rather than an empty string it invented.
            assert!(stdout.contains("build-finished"), "{stdout}");
        }
        other => panic!("a fixed crate was rejected: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_sandboxed_verification_leaves_the_caller_tree_untouched() {
    // The isolation property, exercised through the public sandbox: a repository whose tree does
    // not compile is verified inside a worktree, and the caller's working tree is still there
    // afterwards, broken in exactly the way it was.
    let root = cargo_fixture("compiler-sandbox", BROKEN_SOURCE);
    let git = |args: &[&str]| {
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output();
    };
    git(&["init", "--quiet", "--initial-branch", "main"]);
    git(&["config", "user.email", "harness@example.invalid"]);
    git(&["config", "user.name", "Harness Fixture"]);
    git(&["config", "commit.gpgsign", "false"]);
    git(&["add", "-A"]);
    git(&["commit", "--quiet", "-m", "broken fixture"]);

    let verifier = CompilerVerifier::new(Toolchain::Cargo).with_timeout_ms(PATIENCE_MS);
    let mut sandbox = SandboxEnvironment::create(&root, "e2e-sandbox").expect("sandbox");
    let outcome = verifier.verify(sandbox.root()).await.expect("cargo ran");
    assert!(
        !outcome.is_success(),
        "the broken tree must be rejected: {outcome:?}"
    );

    // A rejected tree merges nothing — the caller's files are not what the harness rejected.
    let after = std::fs::read_to_string(root.join("src/lib.rs")).expect("read the source");
    assert_eq!(
        after, BROKEN_SOURCE,
        "the sandbox changed the caller's tree"
    );
    sandbox.cleanup().expect("cleanup");
    // …and the worktree is gone rather than left standing.
    assert!(
        !root.join(".miter/worktrees/e2e-sandbox").exists(),
        "the sandbox outlived the check"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_directory_that_is_not_a_crate_is_reported_rather_than_passed() {
    // `cargo check` in an empty directory is a usage error. The harness must not turn a
    // non-answer into a `Success`, which is the one thing a verification harness must not do.
    let root = scratch("compiler-not-a-crate");
    let verifier = Arc::new(CompilerVerifier::new(Toolchain::Cargo).with_timeout_ms(PATIENCE_MS));
    let outcome = verifier.verify(&root).await;

    match outcome {
        // Either cargo named the problem (`failed to find a Cargo.toml`) or it is surfaced as a
        // harness error. What must never happen is `Success` over a tree nobody compiled.
        Ok(VerificationOutcome::DiagnosticFailure { .. }) | Err(_) => {}
        Ok(other) => panic!("an empty directory passed verification: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}
