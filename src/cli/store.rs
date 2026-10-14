//! Database open/save helpers.
//!
//! The backend is `redb`. The records are self-describing (`miter_engine::codec`), so an index
//! written by one build is readable by any build that shares the codec version.

use std::path::Path;

use miter_engine::Engine;

use super::CliError;

/// Saves the whole index, creating the file if necessary.
///
/// # Errors
///
/// [`CliError::Persist`] when the backend cannot write the database.
pub(crate) fn save(engine: &Engine, path: &Path) -> Result<(), CliError> {
    engine.save_redb(path)?;
    Ok(())
}

/// Loads an index, reporting a friendly error when the file is missing.
///
/// # Errors
///
/// [`CliError::MissingIndex`] when the file does not exist, [`CliError::Persist`] when it
/// cannot be read or decoded.
pub(crate) fn load(path: &Path) -> Result<Engine, CliError> {
    if !path.exists() {
        return Err(CliError::MissingIndex(path.to_path_buf()));
    }
    Ok(Engine::open_redb(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loading_a_missing_database_is_a_friendly_error() {
        let path = PathBuf::from("/tmp/miter_never_written_9f3a.redb");
        let error = load(&path).expect_err("missing index is an error");
        assert!(matches!(error, CliError::MissingIndex(_)));
        assert!(error.to_string().contains("miter index"), "{error}");
    }
}
