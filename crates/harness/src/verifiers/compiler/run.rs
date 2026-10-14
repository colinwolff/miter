//! Running the compiler and reading its verdict.
//!
//! The subprocess surface: check the tool is on `PATH`, spawn it under a timeout, and keep
//! the exit status as the verdict.

use std::path::Path;
use std::time::{Duration, Instant};

use tokio::process::Command;

use crate::error::HarnessError;
use crate::verifier::VerificationOutcome;

use super::span::resolve_span;
use super::{CompilerVerifier, Toolchain, parse_cargo_messages, parse_tsc_output};

impl CompilerVerifier {
    /// The verifier's name, as it appears in a report and in a log line.
    #[must_use]
    pub fn name(&self) -> &str {
        self.toolchain.as_str()
    }

    /// Runs the compiler over `workspace_root` and reports what it said.
    ///
    /// # Errors
    ///
    /// [`HarnessError`] for a failure of the *harness* — an unreadable root, a process that
    /// would not start, a compiler that is not installed. A check that ran and rejected the
    /// tree is **not** an error here: it is [`VerificationOutcome::DiagnosticFailure`].
    pub async fn verify(&self, workspace_root: &Path) -> Result<VerificationOutcome, HarnessError> {
        let tool = self.toolchain.program();
        let root = workspace_root.to_path_buf();
        let args = self.toolchain.args(&root)?;
        ensure_on_path(tool)?;

        let started = Instant::now();
        let output =
            run_with_timeout(tool, &args, &root, Duration::from_millis(self.timeout_ms)).await?;
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        // The exit status decides. A tool that exited 0 has verified the tree whatever its
        // warnings say, and one that exited non-zero has not, whatever its output parses to.
        if output.status.success() {
            return Ok(VerificationOutcome::Success {
                stdout: last_line(&stdout),
                duration_ms,
            });
        }

        let diagnostics = match self.toolchain {
            Toolchain::Cargo => parse_cargo_messages(&stdout),
            // tsc reports positions, not byte ranges, so the spans are measured against the
            // files it was reading. A file that cannot be read keeps a zero-width span rather
            // than an invented one.
            Toolchain::Tsc => parse_tsc_output(&stderr)
                .into_iter()
                .map(|mut diagnostic| {
                    diagnostic.spans = diagnostic
                        .spans
                        .iter()
                        .map(|span| resolve_span(&root, span))
                        .collect();
                    diagnostic
                })
                .collect(),
        };
        let first = diagnostics
            .into_iter()
            .next()
            .ok_or_else(|| HarnessError::Unparsable {
                tool: tool.into(),
                detail: format!(
                    "it exited {} but printed no diagnostic this harness could read",
                    output.status
                ),
            })?;

        Ok(VerificationOutcome::DiagnosticFailure {
            error_code: first.code,
            message: first.message,
            spans: first.spans,
            raw_stderr: stderr,
        })
    }
}

/// Fails with [`HarnessError::ToolMissing`] when `tool` is not on `PATH`.
///
/// Checked before spawning rather than inferred from a spawn failure: "the compiler is not
/// installed" and "the compiler refused to start" are different facts, and only the first is
/// something a user fixes by installing something.
pub(super) fn ensure_on_path(tool: &str) -> Result<(), HarnessError> {
    let Some(paths) = std::env::var_os("PATH") else {
        return Err(HarnessError::ToolMissing { tool: tool.into() });
    };
    if std::env::split_paths(&paths).any(|dir| is_executable(&dir.join(tool))) {
        Ok(())
    } else {
        Err(HarnessError::ToolMissing { tool: tool.into() })
    }
}

/// Whether `path` is a file this process could execute.
pub(super) fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Runs `tool` in `root`, killing it if it outruns `limit`.
///
/// `tokio::process` is what makes the timeout real: `kill_on_drop(true)` plus a `timeout`
/// around the wait means a runaway compiler dies with the future rather than outliving the
/// loop that spawned it.
async fn run_with_timeout(
    tool: &str,
    args: &[String],
    root: &Path,
    limit: Duration,
) -> Result<std::process::Output, HarnessError> {
    // `kill_on_drop(true)` is what makes the timeout real: the process dies with the future
    // rather than outliving the loop that spawned it. `Command::output` borrows the command,
    // so the future is boxed to satisfy `timeout`'s `Future` bound.
    let output = Command::new(tool)
        .args(args)
        .current_dir(root)
        .kill_on_drop(true)
        .output();
    match tokio::time::timeout(limit, output).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(HarnessError::Spawn {
            tool: tool.into(),
            detail: error.to_string(),
        }),
        Err(_) => Err(HarnessError::Timeout {
            tool: tool.into(),
            limit_ms: u64::try_from(limit.as_millis()).unwrap_or(u64::MAX),
        }),
    }
}

/// The last human-readable line of a cargo run: `Finished dev [unoptimized] target(s) in 1.2s`.
///
/// A `Success` carries a *tail*, not the whole stream: `cargo check --message-format=json`
/// prints a JSON document per crate, and shipping all of it as a success message would put a
/// few hundred kilobytes of build-script chatter into every report. The JSON lines are skipped
/// rather than rendered, because a success message made of machine syntax is a success message
/// nobody reads — and if the run printed *only* JSON, the last of those is still the truth.
pub(super) fn last_line(stdout: &str) -> String {
    let last = |filter: &dyn Fn(&str) -> bool| {
        stdout
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty() && filter(line.trim()))
            .unwrap_or_default()
            .to_owned()
    };
    let readable = last(&|line| !line.starts_with('{'));
    if readable.is_empty() {
        last(&|_| true)
    } else {
        readable
    }
}
