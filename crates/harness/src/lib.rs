//! # `miter-harness` — deterministic compiler verification
//!
//! A single execution engine that drives a deterministic, external compiler over a tree and
//! reports what it said. The backend this workspace ships is
//! [`CompilerVerifier`]: `cargo check` for a Rust
//! workspace and `tsc --noEmit` for a TypeScript project.
//!
//! ```text
//!                          (a tree to check)
//!                                 │
//!                                 ▼
//!   ┌────────────────────────────────────────────────────────────┐
//!   │ 1. sandbox — an isolated `git worktree` (sandbox.rs)        │
//!   │    `.miter/worktrees/<task-id>`, never the working tree      │
//!   └───────────────────────────┬────────────────────────────────┘
//!                               ▼
//!   ┌────────────────────────────────────────────────────────────┐
//!   │ 2. verifiers/compiler.rs — `cargo check` / `tsc --noEmit`    │
//!   │    → DiagnosticSpan (bytes + line/col)                       │
//!   └───────────────────────────┬────────────────────────────────┘
//!                               ▼
//!   3. a VerificationOutcome: Success, or a located DiagnosticFailure
//! ```
//!
//! ## What it is for
//!
//! The harness runs a checker that is deterministic and external, and it reports what that
//! checker said. It does not score a tree, does not rank one against another, and holds no
//! opinion about whether a tree is *good*. A verdict this crate invented would be a fixture
//! wearing a metric's name.
//!
//! ## Determinism
//!
//! A compiler has no clock and no seed, and the harness adds none: no sampling, no
//! timeout-dependent branching in a verdict.
//!
//! ## Example
//!
//! ```
//! use miter_harness::{CompilerVerifier, Toolchain};
//!
//! // A verifier for a Rust workspace. Running it is an `async` call:
//! let verifier = CompilerVerifier::new(Toolchain::Cargo);
//! assert_eq!(verifier.name(), "cargo-check");
//! ```
//!
//! #![forbid(unsafe_code)]
//! #![warn(missing_docs)]

pub mod error;
pub mod sandbox;
pub mod verifier;
pub mod verifiers;

pub use error::HarnessError;
pub use sandbox::{DEFAULT_WORKTREE_ROOT, SandboxEnvironment};
pub use verifier::{DiagnosticSpan, VerificationOutcome};
pub use verifiers::compiler::{CompilerVerifier, Toolchain};

/// Default milliseconds a single verifier run may take.
///
/// Fifteen seconds covers cold Cargo builds before incremental caches are warm,
/// while keeping runaway compilers bounded.
pub const DEFAULT_TIMEOUT_MS: u64 = 15_000;
