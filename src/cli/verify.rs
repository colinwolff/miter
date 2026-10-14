//! CLI subcommand for `miter verify`.
//!
//! Isolated compiler and type verification on a workspace or crate. Drives
//! [`CompilerVerifier`] over the target tree and prints the
//! located diagnostics it reports. With `--sandbox` the check runs inside an ephemeral `git`
//! worktree, so a verified tree is verified without the working tree ever holding an
//! intermediate state.

use std::path::{Path, PathBuf};

use miter_harness::{CompilerVerifier, SandboxEnvironment, Toolchain, VerificationOutcome};

use crate::cli::CliError;

/// Options for `miter verify`.
#[derive(Clone, Debug)]
pub struct VerifyOptions {
    /// Directory or workspace root to verify.
    pub path: PathBuf,
    /// Whether to run in an ephemeral git worktree sandbox.
    pub sandbox: bool,
    /// Unique task ID if sandboxed.
    pub task_id: Option<String>,
    /// Verification timeout budget in milliseconds.
    pub timeout_ms: u64,
    /// Whether to emit machine-readable JSON.
    pub json: bool,
}

/// Runs `miter verify`.
///
/// # Errors
///
/// [`CliError::Usage`] when the harness itself fails (a sandbox that will not open, a compiler
/// that is not installed) and [`CliError::Rejected`] when the compiler rejects the tree.
pub fn run_verify(options: VerifyOptions) -> Result<(), CliError> {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let workspace_root = miter_storage::resolve_workspace_root(&current_dir);
    let target_dir = if options.path.is_absolute() {
        options.path.clone()
    } else {
        workspace_root.join(&options.path)
    };

    let verifier = CompilerVerifier::new(Toolchain::Cargo).with_timeout_ms(options.timeout_ms);

    block_on(async move {
        // With `--sandbox` the verifier is pointed at a fresh worktree rather than the working
        // tree; without it, at the target directly.
        let mut sandbox = if options.sandbox {
            let task_id = options
                .task_id
                .clone()
                .unwrap_or_else(|| format!("cli-{}", std::process::id()));
            Some(
                SandboxEnvironment::create(&target_dir, &task_id)
                    .map_err(|error| CliError::Usage(error.to_string()))?,
            )
        } else {
            None
        };

        let verdict_root = sandbox.as_ref().map_or_else(
            || target_dir.clone(),
            |sandbox| sandbox.root().to_path_buf(),
        );

        let outcome = verifier
            .verify(&verdict_root)
            .await
            .map_err(|error| CliError::Usage(error.to_string()))?;

        // A passing sandbox transfers its changed files into the working tree; a failing one
        // transfers nothing. Either way the worktree is retired on the way out.
        if outcome.is_success()
            && let Some(sandbox) = sandbox.as_ref()
        {
            let merged = sandbox
                .merge_to_head()
                .map_err(|error| CliError::Usage(error.to_string()))?;
            if !options.json {
                println!("  merged {} verified file(s)", merged.len());
            }
        }
        if let Some(sandbox) = sandbox.as_mut() {
            sandbox
                .cleanup()
                .map_err(|error| CliError::Usage(error.to_string()))?;
        }

        if options.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome).unwrap_or_default()
            );
        } else {
            print_outcome(&outcome, &verdict_root);
        }

        if outcome.is_success() {
            Ok(())
        } else {
            Err(CliError::Rejected(outcome.summary()))
        }
    })
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_or_else(
            |error| panic!("current-thread runtime error: {error}"),
            |rt| rt.block_on(future),
        )
}

fn print_outcome(outcome: &VerificationOutcome, root: &Path) {
    match outcome {
        VerificationOutcome::Success { duration_ms, .. } => {
            println!("VERIFICATION PASSED: No compiler or type errors ({duration_ms}ms)");
        }
        VerificationOutcome::DiagnosticFailure {
            error_code,
            message,
            spans,
            ..
        } => {
            println!("VERIFICATION FAILED: [{error_code}] {message}");
            for span in spans {
                println!("  --> {}", span.location());
                if let Ok(content) = std::fs::read_to_string(root.join(&span.file_path))
                    && let Some(line) = content.lines().nth(span.line_start.saturating_sub(1))
                {
                    println!("      | {line}");
                }
            }
        }
    }
}
