//! Compiler-verifier tests: parsers against captured output, and the verifier against a real
//! fixture crate.
//!
//! The parser tests use the *shape* of real tool output rather than invented strings, and the
//! end-to-end test compiles an actual crate that does not compile with the host's `cargo`.

mod fixtures;
mod parse;
mod verify;
