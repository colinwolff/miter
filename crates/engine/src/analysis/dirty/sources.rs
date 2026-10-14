//! Which files an incremental pass considers.

use std::path::Path;

use crate::polyglot::Language;

/// Checks whether `file` is a format this workspace indexes.
///
/// The question is asked of [`Language::from_path`] rather than of a local extension list, so a
/// format added to the polyglot table is picked up here without a second edit that can be
/// forgotten.
pub(super) fn is_supported_source(path: &Path) -> bool {
    Language::from_path(path).is_some()
}
