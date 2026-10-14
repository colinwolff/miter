//! The verification suite: `miter verify` end to end.
//!
//! One binary. The unit suites inside `crates/harness` test the diagnostic parsers and the
//! sandbox against fixtures; this suite tests the thing a user runs — a real crate that does
//! not compile, and the compiler check that reports it.
//!
//! ```text
//! cargo test --test harness compiler
//! cargo nextest run -E 'test(miter_harness::compiler)'
//! ```

mod compiler;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// A directory of this suite's own, unique per process and per call.
///
/// Every fixture here is a real tree a real `cargo` or a real solver reads, so the tests write
/// to the filesystem throughout. One shared directory would mean parallel tests deleting each
/// other's crates.
pub fn scratch(label: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let ordinal = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "miter-harness-suite-{label}-{}-{ordinal}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create the fixture directory");
    path
}

/// Writes a one-crate Cargo project that depends on nothing, and returns its root.
///
/// `[workspace]` is declared so the crate is its own workspace root: without it, a fixture
/// under a temporary directory that happens to sit inside another workspace inherits that
/// workspace's lockfile and features, and the test measures the wrong build.
pub fn cargo_fixture(label: &str, source: &str) -> PathBuf {
    let root = scratch(label);
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"miter-harness-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
         [workspace]\n",
    )
    .expect("write the manifest");
    std::fs::write(root.join("src/lib.rs"), source).expect("write the source");
    root
}

/// A crate whose `answer` does not type-check: a `&str` where the signature promises `u8`.
pub const BROKEN_SOURCE: &str = "pub fn answer() -> u8 {\n    \"forty-two\"\n}\n";

/// The same crate, fixed.
pub const FIXED_SOURCE: &str = "pub fn answer() -> u8 {\n    42\n}\n";
