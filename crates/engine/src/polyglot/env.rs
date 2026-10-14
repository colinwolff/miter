//! Lightweight key extractor for .env and environment variable files.
//!
//! Indexes variable/configuration keys as `SymbolKind::Property` with exact line numbers
//! while never persisting raw RHS values (always redacted).

use super::{Language, SymbolKind, UniversalSymbol, common};
use crate::security::REDACTED_SECRET;
use std::path::Path;

/// Checks if a file path is an environment variable configuration file (`.env` or `.env.*`).
#[must_use]
pub fn is_env_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    file_name == ".env" || file_name.starts_with(".env.")
}

/// Extracts environment variable key symbols from `.env` content.
///
/// Preserves key names, module hierarchy, and exact line numbers.
/// The signature always redacts the value to `"[REDACTED_SECRET]"`.
#[must_use]
pub fn extract_env_symbols(path: &Path, content: &str) -> Vec<UniversalSymbol> {
    let module = common::module_path(path);
    let mut symbols = Vec::new();
    let mut current_offset = 0usize;

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;
        let line_start = current_offset;
        let line_end = current_offset + line.len();
        current_offset = line_end + 1; // account for newline

        let trimmed = line.trim();
        if trimmed.starts_with('#') || !trimmed.contains('=') {
            continue;
        }

        let without_export = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let Some(eq_idx) = without_export.find('=') else {
            continue;
        };

        let key = without_export[..eq_idx].trim();
        if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }

        let signature = format!("{key}=\"{REDACTED_SECRET}\"");

        symbols.push(UniversalSymbol {
            path: common::dotted_path(&module, &[], key),
            name: key.to_string(),
            language: Language::Rust,
            kind: SymbolKind::Property,
            depth: 2, // level 2: scope/property
            docstring: None,
            signature,
            span: (line_start, line_end),
            file: path.to_path_buf(),
            line: line_num,
            params: Vec::new(),
            return_type: None,
            exported: true,
        });
    }

    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_env_symbols() {
        let env_content = "
# Comment line
DATABASE_URL=postgres://admin:secret@localhost:5432/mydb
export STRIPE_SECRET_KEY=\"sk_live_12345\"
PORT=8080
INVALID LINE WITHOUT EQUALS
";
        let path = Path::new(".env");
        let symbols = extract_env_symbols(path, env_content);
        assert_eq!(symbols.len(), 3);

        assert_eq!(symbols[0].name, "DATABASE_URL");
        assert_eq!(symbols[0].line, 3);
        assert_eq!(symbols[0].signature, "DATABASE_URL=\"[REDACTED_SECRET]\"");

        assert_eq!(symbols[1].name, "STRIPE_SECRET_KEY");
        assert_eq!(symbols[1].line, 4);
        assert_eq!(
            symbols[1].signature,
            "STRIPE_SECRET_KEY=\"[REDACTED_SECRET]\""
        );

        assert_eq!(symbols[2].name, "PORT");
        assert_eq!(symbols[2].line, 5);
    }
}
