//! Unit tests for AST interface skeleton extraction.

use std::path::Path;

use super::skeleton::{SkeletonOptions, extract_skeleton};

#[test]
fn test_rust_skeleton_strips_implementation_bodies() {
    let mut fixture = String::new();
    fixture.push_str("/// Configuration struct documentation.\n");
    fixture.push_str("pub struct WorkerConfig {\n    pub id: u64,\n    pub name: String,\n}\n\n");
    fixture.push_str("pub trait WorkerService {\n    fn run(&self) -> bool;\n}\n\n");
    fixture.push_str("impl WorkerService for WorkerConfig {\n");
    fixture.push_str("    /// Executes worker task.\n");
    fixture.push_str("    fn run(&self) -> bool {\n");
    // Generate 400+ lines of implementation body
    for i in 0..420 {
        fixture.push_str(&format!("        let _v{i} = {} + {i};\n", i * 2));
    }
    fixture.push_str("        true\n    }\n}\n\n");
    fixture.push_str("pub fn standalone_helper(x: i32) -> i32 {\n");
    for i in 0..50 {
        fixture.push_str(&format!("    let _temp{i} = x + {i};\n"));
    }
    fixture.push_str("    x\n}\n");

    let original_lines = fixture.lines().count();
    assert!(
        original_lines >= 480,
        "expected 480+ lines, got {original_lines}"
    );

    let result = extract_skeleton(
        Path::new("src/worker.rs"),
        &fixture,
        SkeletonOptions::default(),
    );

    assert!(
        result.skeleton_lines <= 45,
        "expected skeleton to strip down to ~40 lines, got {}",
        result.skeleton_lines
    );
    assert!(
        result.compression_ratio() > 0.85,
        "expected >85% compression, got {:.2}%",
        result.compression_ratio() * 100.0
    );

    // Verify key interface definitions are preserved
    assert!(result.skeleton_text.contains("pub struct WorkerConfig"));
    assert!(result.skeleton_text.contains("pub trait WorkerService"));
    assert!(
        result
            .skeleton_text
            .contains("impl WorkerService for WorkerConfig")
    );
    assert!(
        result
            .skeleton_text
            .contains("fn run(&self) -> bool { /* ... */ }")
    );
    assert!(
        result
            .skeleton_text
            .contains("pub fn standalone_helper(x: i32) -> i32 { /* ... */ }")
    );
    assert!(
        result
            .skeleton_text
            .contains("/// Configuration struct documentation.")
    );
    assert!(result.skeleton_text.contains("/// Executes worker task."));

    // Verify internal variable declarations in function bodies were stripped
    assert!(!result.skeleton_text.contains("let _v0 ="));
    assert!(!result.skeleton_text.contains("let _temp0 ="));
}

#[test]
fn test_typescript_skeleton_strips_implementation_bodies() {
    let mut fixture = String::new();
    fixture.push_str("/**\n * Application service interface.\n */\n");
    fixture.push_str("export interface AppService {\n  start(): Promise<void>;\n}\n\n");
    fixture.push_str("export class ConcreteService implements AppService {\n");
    fixture.push_str("  async start(): Promise<void> {\n");
    for i in 0..420 {
        fixture.push_str(&format!(
            "    const step{i} = await Promise.resolve({i});\n"
        ));
    }
    fixture.push_str("  }\n}\n");

    let original_lines = fixture.lines().count();
    assert!(
        original_lines >= 430,
        "expected 430+ lines, got {original_lines}"
    );

    let result = extract_skeleton(
        Path::new("src/service.ts"),
        &fixture,
        SkeletonOptions::default(),
    );

    assert!(
        result.skeleton_lines <= 30,
        "expected skeleton to strip down to ~20-30 lines, got {}",
        result.skeleton_lines
    );
    assert!(result.skeleton_text.contains("export interface AppService"));
    assert!(
        result
            .skeleton_text
            .contains("export class ConcreteService implements AppService")
    );
    assert!(
        result
            .skeleton_text
            .contains("async start(): Promise<void> { /* ... */ }")
    );
    assert!(!result.skeleton_text.contains("const step0 ="));
}
