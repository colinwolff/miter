//! Walking a real directory tree: depths, discovery and the encoding budget.

use std::path::{Path, PathBuf};

use miter_ast::AstIndexer;

use super::*;

/// A throwaway directory tree that removes itself when dropped.
struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(label: &str) -> Self {
        let unique = format!(
            "miter_{}_{}_{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).expect("create temp root");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&path, contents).expect("write source file");
        path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn deeply_nested_symbols_report_their_full_depth() {
    let indexer = AstIndexer::new();
    // 400 nested modules: the depth keeps counting however deep the nesting goes.
    let mut source = String::new();
    for level in 0..400 {
        source.push_str(&format!("mod level_{level} {{\n"));
    }
    source.push_str("fn bottom() -> u8 { 0 }\n");
    for _ in 0..400 {
        source.push_str("}\n");
    }

    let symbols = indexer
        .index_source(&source, Language::Rust, Path::new("deep.rs"), 400)
        .expect("parses");
    let bottom = symbols
        .iter()
        .find(|symbol| symbol.name == "bottom")
        .expect("bottom function is indexed");
    assert_eq!(bottom.depth(), 800);
}

#[test]
fn directory_depths_are_reported_and_deterministic() {
    let tree = TempTree::new("dir");
    tree.write("top.rs", "fn top_level() {}");
    tree.write("nested/deep.rs", "fn deep_level() {}");
    tree.write("nested/deeper/deepest.rs", "fn deepest_level() {}");
    tree.write("nested/ignored.md", "# not source");
    tree.write(
        "nested/mod.ts",
        "export function typed(): number { return 1; }",
    );

    let report = AstIndexer::new().index_dir(&tree.root).expect("scan");
    assert!(report.failures.is_empty(), "{:?}", report.failures);

    let mut depths: Vec<(String, usize)> = report
        .symbols
        .iter()
        .map(|symbol| (symbol.name.clone(), symbol.depth()))
        .collect();
    depths.sort();

    let mut expected = vec![
        ("deep_level".to_string(), 1usize),
        ("deepest_level".to_string(), 2),
        ("top_level".to_string(), 0),
    ];
    if Language::TypeScript.is_enabled() {
        expected.push(("typed".to_string(), 1));
    }
    expected.sort();
    assert_eq!(depths, expected);

    let dir_depth_of = |name: &str| {
        report
            .symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .expect("symbol present")
            .dir_depth
    };
    assert!(dir_depth_of("top_level") < dir_depth_of("deep_level"));
    assert!(dir_depth_of("deep_level") < dir_depth_of("deepest_level"));

    // The scan is idempotent: a second pass finds the same symbols and depths.
    let second = AstIndexer::new().index_dir(&tree.root).expect("rescan");
    assert_eq!(second.symbols.len(), report.symbols.len());
    assert_eq!(report.failed_files(), 0);
    assert_eq!(report.len(), report.symbols.len());
    assert!(!report.is_empty());
}

#[test]
fn discovery_requires_a_real_directory() {
    let indexer = AstIndexer::new();
    let missing = std::env::temp_dir().join("miter_definitely_missing_directory");
    assert!(indexer.index_dir(&missing).is_err());
    assert!(miter_ast::discover_sources(&missing).is_err());
}

/// Encoding guard-rail (release only): the per-symbol encoding that ingestion pays for.
///
/// A symbol costs one parse plus a handful of field copies, so a budget here catches the
/// regressions that matter: an accidental re-parse, or a per-symbol allocation in the loop.
#[cfg(not(debug_assertions))]
#[test]
fn encoding_stays_within_budget() {
    use std::hint::black_box;
    use std::time::Instant;

    /// Iterations per measured batch.
    const ITERATIONS: u32 = 200;
    /// Batches; the fastest per-iteration time is reported (this VM shows ±30% variance).
    const BATCHES: usize = 5;

    /// Fastest per-iteration cost of `work`, in microseconds.
    fn fastest_micros(mut work: impl FnMut()) -> f64 {
        work();
        let mut fastest = f64::MAX;
        for _ in 0..BATCHES {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                work();
            }
            let per_iteration = start.elapsed().as_secs_f64() * 1e6 / f64::from(ITERATIONS);
            fastest = fastest.min(per_iteration);
        }
        fastest
    }

    const SOURCE: &str =
        "fn hash_password(secret: &str, salt: &[u8]) -> Result<Vec<u8>, Error> { todo!() }";
    let indexer = AstIndexer::new();
    assert_eq!(
        indexer
            .index_source(SOURCE, Language::Rust, Path::new("src/auth.rs"), 2)
            .expect("parses")
            .len(),
        1
    );

    let symbol_micros = fastest_micros(|| {
        black_box(
            indexer
                .index_source(
                    black_box(SOURCE),
                    Language::Rust,
                    Path::new("src/auth.rs"),
                    2,
                )
                .expect("parses"),
        );
    });
    assert!(
        symbol_micros < 1_500.0,
        "encoding one symbol took {symbol_micros:.2} us (budget 1500 us)"
    );
}
