//! Candidate naming, root retention and comment elevation.

use std::path::Path;

use crate::analysis::slice::SeamSlicer;

#[test]
fn test_seam_slicer_smart_candidate_naming() {
    let fixture = r#"
fn config_path() -> String {
    "/tmp/config".to_string()
}

pub struct EngineState {
    pub is_ready: bool,
}

impl EngineState {
    pub fn new() -> Self {
        Self { is_ready: true }
    }
}
"#;

    let slicer = SeamSlicer::with_target_lines(50);
    let blueprints = slicer.slice_file(Path::new("src/cli/state.rs"), fixture);

    assert_eq!(blueprints.len(), 1);
    let bp = &blueprints[0];

    // Priority 1: Type sovereignty picks EngineState over config_path
    assert_eq!(bp.suggested_mod_decl, "mod engine_state;");
    assert!(
        bp.candidate_file.ends_with("engine_state.rs"),
        "expected candidate_file ending with engine_state.rs, got: {}",
        bp.candidate_file
    );
}

#[test]
fn test_seam_slicer_root_orchestrator_retention() {
    let fixture = r#"
fn helper_a() -> i32 {
    1
}

fn helper_b() -> i32 {
    2
}

pub fn run_pipeline() {
    let a = helper_a();
    let b = helper_b();
    println!("{}", a + b);
}
"#;

    let slicer = SeamSlicer::with_target_lines(5);
    let blueprints = slicer.slice_file(Path::new("src/cli/state.rs"), fixture);

    // run_pipeline is a root orchestrator calling helper_a and helper_b.
    // It must be retained in the parent file and excluded from extraction slices.
    for bp in &blueprints {
        assert!(
            !bp.symbols_to_move.contains(&"run_pipeline".to_string()),
            "run_pipeline must be retained in the parent file and not extracted"
        );
    }

    // Helpers must still be extracted
    let has_helper_slice = blueprints
        .iter()
        .any(|bp| bp.symbols_to_move.contains(&"helper_a".to_string()));
    assert!(has_helper_slice, "helpers must be extracted into a slice");
}

#[test]
fn test_seam_slicer_bidirectional_elevations_and_comments() {
    let fixture = r#"
/// Helper token function in parent module.
fn token_of(raw: &str) -> String {
    raw.trim().to_string()
}

/// Extracted Frame struct with doc comments.
pub struct Frame {
    pub name: String,
}

impl Frame {
    /// Constructs a Frame using parent's private helper.
    pub fn new(name: &str) -> Self {
        Self { name: token_of(name) }
    }
}
"#;

    let slicer = SeamSlicer::with_target_lines(10);
    let blueprints = slicer.slice_file(Path::new("src/roles.rs"), fixture);

    assert!(!blueprints.is_empty(), "must produce blueprint for Frame");
    let frame_bp = blueprints
        .iter()
        .find(|bp| bp.symbols_to_move.contains(&"Frame".to_string()))
        .expect("Frame blueprint found");

    // Frame calls token_of (unexported parent function).
    // token_of must be present in parent_elevations so roles.rs elevates it to pub(crate)!
    assert!(
        frame_bp
            .parent_elevations
            .contains(&"pub(crate) token_of".to_string()),
        "parent_elevations must include token_of, found: {:?}",
        frame_bp.parent_elevations
    );

    // Frame must also import token_of from super
    assert!(
        frame_bp
            .suggested_imports
            .contains(&"super::token_of".to_string()),
        "suggested_imports must include super::token_of, found: {:?}",
        frame_bp.suggested_imports
    );

    // Verify doc comments are included in line budgeting (starts at line 7 `/// Extracted Frame`)
    assert!(
        frame_bp.line_range.0 <= 7,
        "line_range.0 must start at or before doc comment line 7: {:?}",
        frame_bp.line_range
    );
}
