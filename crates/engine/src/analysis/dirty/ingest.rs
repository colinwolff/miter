//! The incremental pass: re-parse only what changed.

use std::path::Path;
use std::time::SystemTime;

use super::detect_dirty_files;

/// Performs an incremental re-index of dirty and deleted files.
pub fn ingest_dirty(
    engine: &mut crate::Engine,
    root: &Path,
    store: &miter_storage::WorkspaceStore,
) -> Result<crate::store::IncrementalStats, crate::EngineError> {
    let indexed = store
        .list_indexed_files()
        .map_err(|e| crate::EngineError::Persist(crate::PersistError::Redb(e.to_string())))?;
    let scan = detect_dirty_files(root, &indexed);

    let mut stats = crate::store::IncrementalStats {
        files_scanned: indexed.len() + scan.modified_files.len(),
        files_skipped: indexed
            .len()
            .saturating_sub(scan.modified_files.len() + scan.deleted_files.len()),
        ..Default::default()
    };

    // Load existing records from store if engine has not been populated
    if engine.is_empty() {
        if let Ok(records) = crate::persist::load_redb_from(store) {
            for sym in records {
                let rel = crate::store::relative_to(root, &sym.path);
                if !scan.deleted_files.contains(&rel) && !scan.modified_files.contains(&sym.path) {
                    engine.insert(sym);
                }
            }
        }
        if let Ok(records) = crate::persist::load_redb_universal_from(store) {
            let survivors: Vec<_> = records
                .into_iter()
                .filter(|sym| {
                    let rel = crate::store::relative_to(root, &sym.file);
                    !scan.deleted_files.contains(&rel) && !scan.modified_files.contains(&sym.file)
                })
                .collect();
            engine.add_universal_symbols(survivors);
        }
    }

    // 1. Prune deleted files
    for rel in &scan.deleted_files {
        let _ = store.remove_file_hash(rel);
        let _ = store.remove_indexed_file(rel);
        let _ = store.clear_references_for_file(rel);
    }

    // 2. Index modified files
    let indexer = engine.indexer().clone();
    let mut universal_symbols = Vec::new();

    for path in scan.modified_files {
        let rel = crate::store::relative_to(root, &path);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                stats.failures.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let digest = miter_storage::sha256(&bytes);
        let text = match String::from_utf8(bytes) {
            Ok(t) => t,
            Err(e) => {
                stats
                    .failures
                    .push(format!("{}: not UTF-8: {e}", path.display()));
                continue;
            }
        };

        let parsed = match crate::polyglot::Language::from_path(&path) {
            Some(lang) if lang.is_tree_sitter() => {
                crate::polyglot::parse(&path, &text, lang).map_err(|e| e.to_string())
            }
            Some(_) => Ok(crate::polyglot::declarative_symbols(&path, &text).unwrap_or_default()),
            None => Ok(Vec::new()),
        };

        match parsed {
            Ok(symbols) => {
                for sym in &symbols {
                    if engine.insert(sym.function_symbol(&indexer, 0)) {
                        stats.symbols += 1;
                    }
                }
                stats.files_indexed += 1;
                universal_symbols.extend(symbols);
                let _ = store.set_file_hash(&rel, &digest);
                let mtime_info = path
                    .metadata()
                    .ok()
                    .and_then(|meta| meta.modified().ok().map(|m| (meta, m)))
                    .and_then(|(meta, mtime)| {
                        mtime
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .ok()
                            .map(|dur| (dur.as_secs(), dur.subsec_nanos(), meta.len()))
                    });
                if let Some((secs, nanos, len)) = mtime_info {
                    let rec = miter_storage::IndexedFileRecord::new(secs, nanos, digest, len);
                    let _ = store.set_indexed_file(&rel, &rec);
                }
                let refs = crate::analysis::extract_references(Path::new(&rel), &text);
                let _ = store.clear_references_for_file(&rel);
                let _ = store.insert_references(&rel, refs);
            }
            Err(err) => stats.failures.push(format!("{}: {err}", path.display())),
        }
    }

    engine.add_universal_symbols(universal_symbols);
    if stats.files_indexed > 0 || !scan.deleted_files.is_empty() {
        let _ = crate::persist::save_redb_into(store, engine);
    }
    Ok(stats)
}
