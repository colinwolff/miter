//! The compiler check: `cargo check` and `tsc --noEmit`.
//!
//! ## What is extracted, and why only that
//!
//! A compiler's answer is a *location*. `E0308 mismatched types` tells a repair loop nothing
//! actionable; `src/lib.rs:14:9 — expected u8, found &str` tells it which bytes to look at. So
//! this module parses the machine formats — `--message-format=json` for cargo,
//! `path(line,col): error TSxxxx:` for tsc — and keeps the primary spans, discarding the
//! rendered frame around them.
//!
//! ## Only the primary span survives
//!
//! rustc reports every span a diagnostic touches: the mismatched expression, the function it
//! sits in, the crate it belongs to. Keeping them all would put three locations in a window
//! meant for one, and the repair loop would read the wrong one.
//!
//! ## Warnings are not failures
//!
//! The verdict comes from the **exit status**, with the parsed errors as a failure's payload —
//! not from "did anything parse as an error". A tool that exits 0 with a warning in its output
//! is a build that passed, and reporting it as a failure would make a warning indistinguishable
//! from the fault it would be sent back to repair.

pub mod parse;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::time::Duration;

use crate::DEFAULT_TIMEOUT_MS;
use crate::error::HarnessError;

pub use parse::{Diagnostic, parse_cargo_messages, parse_tsc_output};

/// Which compiler to drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toolchain {
    /// `cargo check` over a Rust workspace.
    Cargo,
    /// `tsc --noEmit` over a TypeScript project.
    Tsc,
}

impl Toolchain {
    /// The verifier's name, as it appears in a report.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Toolchain::Cargo => "cargo-check",
            Toolchain::Tsc => "tsc",
        }
    }

    /// The executable this toolchain runs.
    #[must_use]
    pub fn program(self) -> &'static str {
        match self {
            Toolchain::Cargo => "cargo",
            Toolchain::Tsc => "tsc",
        }
    }

    /// The arguments, given the root to run in.
    ///
    /// `--message-format=json` for cargo because a rendered diagnostic cannot be parsed back
    /// into a span; `--pretty false` for tsc for the same reason — the pretty form uses box
    /// drawing and colour, both of which must be stripped before the text is machine readable
    /// and both of which depend on whether stdout is a terminal.
    fn args(self, root: &Path) -> Result<Vec<String>, HarnessError> {
        match self {
            Toolchain::Cargo => Ok(vec![
                "check".into(),
                "--message-format=json".into(),
                "--all-targets".into(),
                "--color".into(),
                "never".into(),
            ]),
            Toolchain::Tsc => {
                // tsc with no inputs and no project prints its help and exits 1, which is a
                // usage error wearing a compile error's clothes. Naming the project keeps the
                // two apart.
                if !root.join("tsconfig.json").exists() {
                    return Err(HarnessError::Refused(format!(
                        "tsc needs a tsconfig.json in {}: without one it reports a usage \
                         error, which is not a verdict on the tree",
                        root.display()
                    )));
                }
                Ok(vec![
                    "--noEmit".into(),
                    "--pretty".into(),
                    "false".into(),
                    "--project".into(),
                    ".".into(),
                ])
            }
        }
    }
}

/// Runs a compiler over a tree and turns its answer into a [`crate::VerificationOutcome`].
#[derive(Clone, Debug)]
pub struct CompilerVerifier {
    toolchain: Toolchain,
    timeout_ms: u64,
}

impl CompilerVerifier {
    /// A verifier for `toolchain`, with the default [`crate::DEFAULT_TIMEOUT_MS`] budget.
    #[must_use]
    pub fn new(toolchain: Toolchain) -> Self {
        Self {
            toolchain,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// The same verifier with a custom timeout duration.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        self
    }

    /// The same verifier with a different time budget in milliseconds.
    ///
    /// A whole-workspace `cargo check` is minutes rather than seconds, so a caller verifying
    /// the real repository needs to raise this deliberately rather than discover the default
    /// against a fifteen-second wall.
    #[must_use]
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// The toolchain this verifier drives.
    #[must_use]
    pub fn toolchain(&self) -> Toolchain {
        self.toolchain
    }
}

mod run;
mod span;
