//! The verifier backends this workspace ships.
//!
//! One module: [`compiler`] asks *does this tree type-check?* and answers with
//! [`crate::DiagnosticSpan`]s and an error code. That is the whole of it — `miter` verifies by
//! running a compiler and reading what it said.

pub mod compiler;
