//! Every failure the harness itself can raise.
//!
//! The split that matters: a *harness* error means the check did not run (the worktree would
//! not open, `tsc` is not installed, the solver timed out before answering). A check that ran
//! and returned a verdict is never an error — it is a [`crate::VerificationOutcome`].
//! Conflating the two would make `?` swallow the very thing the caller exists to see.
//!
//! Every variant is a fact a caller can branch on. There is no catch-all: a harness that
//! answers "something went wrong" to a caller deciding whether to retry has thrown away the
//! decision it was asked for.

use std::path::PathBuf;

use thiserror::Error;

/// Failures raised by the harness rather than by the check it ran.
#[derive(Debug, Error)]
pub enum HarnessError {
    /// A file or directory could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The path the operation was on.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: std::io::Error,
    },

    /// The base directory is not a git repository, so no worktree can be cut from it.
    #[error("{path} is not inside a git repository")]
    NotARepository {
        /// The directory that was inspected.
        path: PathBuf,
    },

    /// A `git` invocation returned a non-zero status.
    #[error("git {subcommand} failed with {status}: {stderr}")]
    Git {
        /// The subcommand that failed (`worktree add`, …).
        subcommand: String,
        /// Its exit status.
        status: String,
        /// Its stderr, trimmed.
        stderr: String,
    },

    /// A patch named a path that resolves outside the sandbox.
    ///
    /// Refused rather than clamped: a proposal that writes above its jail is not a proposal
    /// this harness will reinterpret as writing somewhere else.
    #[error("{requested} resolves outside the sandbox at {root}")]
    PathEscapes {
        /// The path as the proposal spelled it.
        requested: PathBuf,
        /// The sandbox root it must stay under.
        root: PathBuf,
    },

    /// The verifier's executable is not on `PATH`.
    #[error("{tool} is not installed: the harness cannot run it")]
    ToolMissing {
        /// The executable that was looked for.
        tool: String,
    },

    /// The verifier's process could not be started or ended abnormally.
    #[error("could not run {tool}: {detail}")]
    Spawn {
        /// The executable that was run.
        tool: String,
        /// What went wrong.
        detail: String,
    },

    /// The verifier ran out of its time budget.
    #[error("{tool} exceeded its {limit_ms}ms budget")]
    Timeout {
        /// The executable that was run.
        tool: String,
        /// The budget it exceeded, in milliseconds.
        limit_ms: u64,
    },

    /// The verifier's output did not parse.
    ///
    /// Carries the raw text, because a parse failure is only diagnosable against the bytes
    /// that failed to parse.
    #[error("could not read {tool}'s output: {detail}")]
    Unparsable {
        /// The tool whose output could not be read.
        tool: String,
        /// What was expected, and what arrived instead.
        detail: String,
    },

    /// A caller-supplied condition that the harness refuses to proceed past.
    #[error("{0}")]
    Refused(String),
}

impl HarnessError {
    /// Wraps an I/O failure with the path it happened on.
    ///
    /// A bare [`std::io::Error`] does not say *what* could not be read, and a harness that
    /// reports "permission denied" without the path has thrown away the diagnosis.
    #[must_use]
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        HarnessError::Io {
            path: path.into(),
            source,
        }
    }
}
