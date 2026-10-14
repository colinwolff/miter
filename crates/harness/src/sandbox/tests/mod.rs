//! Sandbox tests: a real git repository, a real worktree, a real jail.
//!
//! These need `git` on `PATH` (a hard dependency of the sandbox itself) and they create real
//! directories, because the property under test — "the user's tree is untouched, and the
//! jail holds" — is a property of the filesystem, not of a mock of it.

mod files;
mod fixtures;
mod lifecycle;
