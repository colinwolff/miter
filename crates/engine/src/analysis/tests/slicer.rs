//! The seam slicer: blueprints, candidate naming and symbol retention.

use std::path::Path;

use crate::analysis::slice::SeamSlicer;

#[test]
fn test_seam_slicer_blueprint_generation() {
    let fixture = r#"
pub struct UserConfig {
    pub name: String,
}

pub fn load_config() -> UserConfig {
    UserConfig { name: "test".to_string() }
}

pub struct DatabasePool {
    pub dsn: String,
}

impl DatabasePool {
    pub fn connect(&self) -> bool {
        true
    }
}

pub fn run_server(pool: &DatabasePool, config: &UserConfig) {
    if pool.connect() {
        println!("User: {}", config.name);
    }
}
"#;

    let slicer = SeamSlicer::with_target_lines(10);
    let blueprints = slicer.slice_file(Path::new("src/server.rs"), fixture);

    assert!(
        !blueprints.is_empty(),
        "must generate extraction blueprints"
    );
    let bp = &blueprints[0];
    assert_eq!(bp.source_file, "src/server.rs");
    assert!(!bp.candidate_file.is_empty());
    assert!(!bp.symbols_to_move.is_empty());
    assert!(bp.suggested_mod_decl.starts_with("mod "));
    assert!(bp.line_range.0 <= bp.line_range.1);
}

#[test]
fn test_seam_slicer_empty_source() {
    let slicer = SeamSlicer::new();
    let blueprints = slicer.slice_file(Path::new("src/empty.rs"), "");
    assert!(blueprints.is_empty());
}

#[test]
fn test_seam_slicer_impl_association_and_exports() {
    let fixture = r#"
pub struct Attention {
    pub heads: usize,
}

impl Attention {
    pub fn new() -> Self {
        Self { heads: 4 }
    }
}

impl Clone for Attention {
    fn clone(&self) -> Self {
        Self { heads: self.heads }
    }
}

pub struct FeedForward {
    pub dim: usize,
}

impl FeedForward {
    pub fn forward(&self) {}
}
"#;

    let slicer = SeamSlicer::with_target_lines(15);
    let blueprints = slicer.slice_file(Path::new("src/model.rs"), fixture);

    assert!(
        blueprints.len() >= 2,
        "multi-struct file must produce multiple candidate slices under target_lines"
    );

    for bp in &blueprints {
        // 1. Verify no synthetic impl names in symbols_to_move
        for sym in &bp.symbols_to_move {
            assert!(
                !sym.starts_with("impl_"),
                "symbols_to_move must not contain synthetic impl names like impl_L..., found: {sym}"
            );
        }

        // 2. Verify suggested_exports contains zero impl entries
        for export in &bp.suggested_exports {
            assert!(
                !export.contains("impl"),
                "suggested_exports must not contain impl entries, found: {export}"
            );
            assert!(
                export.starts_with("pub(crate) "),
                "exports must be formatted as pub(crate) <name>"
            );
        }
    }

    // Verify first cluster is Attention and covers both the struct and its impl blocks
    let attention_bp = blueprints
        .iter()
        .find(|bp| bp.symbols_to_move.contains(&"Attention".to_string()))
        .expect("Attention cluster exists");

    // Line 2 is `pub struct Attention {`, line 16 is `}` of `impl Clone for Attention`
    assert!(
        attention_bp.line_range.0 <= 2 && attention_bp.line_range.1 >= 16,
        "Attention slice line range must include its impl blocks: {:?}",
        attention_bp.line_range
    );
}

#[test]
fn test_seam_slicer_symbol_deduplication() {
    let fixture = r#"
pub struct ServiceWorker {
    pub id: u64,
}

impl ServiceWorker {
    pub fn process(&self) {}
}

impl ServiceWorker {
    pub fn shutdown(&self) {}
}
"#;

    let slicer = SeamSlicer::with_target_lines(50);
    let blueprints = slicer.slice_file(Path::new("src/worker.rs"), fixture);

    assert_eq!(blueprints.len(), 1);
    let bp = &blueprints[0];

    // Verify symbols_to_move has zero duplicate identifiers
    let mut seen = std::collections::HashSet::new();
    for sym in &bp.symbols_to_move {
        assert!(
            seen.insert(sym),
            "symbols_to_move must not contain duplicates: {sym}"
        );
    }
}
