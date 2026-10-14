//! Progress reporting for an ingestion: the trait an indexer reports through, and the bar
//! [`indicatif`] draws for it.
//!
//! The trait lives here rather than in `engine` because this crate is what knows the *unit* of the
//! work — a source file, and the symbols a file yields.
//!
//! There is exactly one way to report: hold an [`indicatif::ProgressBar`] and pass it. This crate
//! contributes the *style* ([`indexing_style`]) and the bridge ([`IndexingProgress`] implemented
//! for `ProgressBar`), so the bar a caller draws and the events an ingestion emits cannot describe
//! different things. A caller with nothing to draw passes [`SilentProgress`], which is what
//! `Engine::ingest_dir` does.
//!
//! ```no_run
//! use miter_ast::{indexing_style, IndexingProgress};
//! use indicatif::ProgressBar;
//!
//! let bar = ProgressBar::new(3).with_style(indexing_style());
//! bar.file(std::path::Path::new("src/lib.rs"));
//! bar.finish_and_clear();
//! ```
//!
//! The bar is drawn on stderr, so a program's own output on stdout stays clean for a pipe, and the
//! bridge is `Sync`: an ingestion parses files on a Rayon pool, and every worker reports through
//! the same bar (which is why the `rayon` feature is enabled for `indicatif`).

use std::path::Path;

use indicatif::{ProgressBar, ProgressStyle};

/// The template of the ingestion bar: `Indexing 7/23 files | src/lib.rs · index_source`.
pub const INDEXING_TEMPLATE: &str = "{prefix}Indexing {pos}/{len} files | {msg}";

/// What an ingestion reports while it runs.
///
/// Every method has a default that does nothing, so a reporter only implements what it shows —
/// and `Sync` because the parsing half of an ingestion runs on a thread pool.
pub trait IndexingProgress: Sync {
    /// A file is about to be parsed.
    fn file(&self, path: &Path) {
        let _ = path;
    }

    /// A symbol has been stored, with the file it came from.
    fn symbol(&self, name: &str, path: &Path) {
        let _ = (name, path);
    }

    /// The ingestion is over, whether or not every file could be read.
    fn finish(&self) {}
}

/// The reporter that reports nothing: what an ingestion runs with when nobody is watching.
#[derive(Debug, Default, Clone, Copy)]
pub struct SilentProgress;

impl IndexingProgress for SilentProgress {}

impl IndexingProgress for ProgressBar {
    fn file(&self, path: &Path) {
        self.set_message(path.display().to_string());
        self.inc(1);
    }

    fn symbol(&self, name: &str, path: &Path) {
        self.set_message(format!("{} · {name}", path.display()));
    }

    fn finish(&self) {
        self.finish_and_clear();
    }
}

/// The style the indexing bar is drawn with.
///
/// Exported so a caller builds its own `ProgressBar` — and with it its own draw target, which is
/// the decision that belongs to whichever binary owns the terminal — while the *look* of the bar
/// stays in one place.
pub fn indexing_style() -> ProgressStyle {
    ProgressStyle::with_template(INDEXING_TEMPLATE)
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=> ")
}
