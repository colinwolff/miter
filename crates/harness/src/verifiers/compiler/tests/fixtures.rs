//! Fixtures: captured tool output, and a crate that does not compile.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
/// A `cargo check --message-format=json` line for one rustc error, with the three spans rustc
/// really emits for a type error: the expression, the enclosing function, and the crate.
pub(super) fn cargo_error_line() -> String {
    serde_json::json!({
        "reason": "compiler-message",
        "package_id": "miter-fixture 0.1.0 (path+file:///tmp/miter-fixture)",
        "target": {"kind": ["lib"], "name": "miter_fixture"},
        "message": {
            "rendered": "error[E0308]: mismatched types\n --> src/lib.rs:2:5\n",
            "level": "error",
            "code": {"code": "E0308", "explanation": "https://doc.rust-lang.org/…"},
            "message": "mismatched types",
            "spans": [
                {
                    "file_name": "src/lib.rs",
                    "byte_start": 25, "byte_end": 30,
                    "line_start": 2, "line_end": 2,
                    "column_start": 5, "column_end": 10,
                    "is_primary": true
                },
                {
                    "file_name": "src/lib.rs",
                    "byte_start": 0, "byte_end": 30,
                    "line_start": 2, "line_end": 2,
                    "column_start": 1, "column_end": 30,
                    "is_primary": false
                },
                {
                    "file_name": "src/lib.rs",
                    "byte_start": 0, "byte_end": 30,
                    "line_start": 1, "line_end": 2,
                    "column_start": 1, "column_end": 30,
                    "is_primary": false
                }
            ]
        }
    })
    .to_string()
}

/// A cargo warning line: same envelope, a different level, and it must never become a verdict.
pub(super) fn cargo_warning_line() -> String {
    serde_json::json!({
        "reason": "compiler-message",
        "message": {
            "rendered": "warning: unused variable: `x`\n",
            "level": "warning",
            "code": {"code": "unused_variables"},
            "message": "unused variable: `x`",
            "spans": [{
                "file_name": "src/lib.rs", "byte_start": 9, "byte_end": 10,
                "line_start": 1, "line_end": 1,
                "column_start": 10, "column_end": 11, "is_primary": true
            }]
        }
    })
    .to_string()
}
/// A crate that does not compile, with no dependency on the network.
pub(super) fn broken_crate() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "miter-harness-broken-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).expect("create the fixture crate");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"miter-harness-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
         [workspace]\n",
    )
    .expect("write the manifest");
    // A type error: a `&str` where the signature promises `u8`.
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn answer() -> u8 {\n    \"forty-two\"\n}\n",
    )
    .expect("write the broken source");
    root
}
