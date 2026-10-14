//! The verifier end to end, against a real `cargo check`.

use super::super::{CompilerVerifier, Toolchain};
use super::fixtures::broken_crate;
use crate::error::HarnessError;
use crate::verifier::VerificationOutcome;

#[test]
fn the_verifier_names_its_toolchain() {
    assert_eq!(
        CompilerVerifier::new(Toolchain::Cargo).name(),
        "cargo-check"
    );
    assert_eq!(CompilerVerifier::new(Toolchain::Tsc).name(), "tsc");
    assert_eq!(
        CompilerVerifier::new(Toolchain::Cargo)
            .with_timeout_ms(1)
            .toolchain(),
        Toolchain::Cargo
    );
}

#[tokio::test]
async fn a_real_type_error_is_caught_with_its_file_and_span() {
    let root = broken_crate();
    let verifier = CompilerVerifier::new(Toolchain::Cargo).with_timeout_ms(180_000);
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
            assert!(span.byte_end > span.byte_start, "{span:?}");
        }
        other => panic!("expected a located diagnostic failure, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_crate_that_compiles_is_a_success_not_a_diagnostic() {
    let root = broken_crate();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn answer() -> u8 {\n    42\n}\n",
    )
    .expect("write the fixed source");

    let verifier = CompilerVerifier::new(Toolchain::Cargo).with_timeout_ms(180_000);
    let outcome = verifier.verify(&root).await.expect("cargo ran");
    assert!(outcome.is_success(), "{outcome:?}");

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_missing_compiler_is_a_typed_error_rather_than_a_spawn_failure() {
    // The absence of a tool is a fact about the host, and it is reported as
    // `ToolMissing` rather than as whatever the spawn attempt happened to return.
    let missing = Toolchain::Tsc;
    let error = super::super::run::ensure_on_path("miter-harness-no-such-compiler")
        .expect_err("a compiler that does not exist cannot be on PATH");
    assert!(
        matches!(error, HarnessError::ToolMissing { ref tool } if tool == "miter-harness-no-such-compiler"),
        "{error:?}"
    );
    // …and a tool that *is* installed is found, so the check cannot pass vacuously.
    super::super::run::ensure_on_path("git").expect("git is on PATH");
    let _ = missing;
}

#[tokio::test]
async fn a_project_without_a_tsconfig_is_refused_rather_than_misreported() {
    let bare =
        std::env::temp_dir().join(format!("miter-harness-notsconfig-{}", std::process::id()));
    std::fs::create_dir_all(&bare).expect("create the bare directory");
    let verifier = CompilerVerifier::new(Toolchain::Tsc);
    let error = verifier
        .verify(&bare)
        .await
        .expect_err("tsc without a project is a usage error, not a verdict");
    assert!(
        matches!(error, HarnessError::Refused(ref message) if message.contains("tsconfig.json")),
        "{error:?}"
    );
    let _ = std::fs::remove_dir_all(&bare);
}
