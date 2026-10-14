//! AST-Aware Secret Protection & Egress Sanitization.
//!
//! Preserves symbol and variable names for semantic queries while scrubbing
//! raw secret values and credentials to `"[REDACTED_SECRET]"`.

pub mod entropy;
pub mod redact;

pub use entropy::{is_high_entropy_secret, shannon_entropy};
pub use redact::{REDACTED_SECRET, is_secret_literal, is_sensitive_identifier};

/// Scans source text line-by-line and replaces sensitive variable assignment literals or
/// high-entropy/vendor secret values with `"[REDACTED_SECRET]"`.
#[must_use]
pub fn scrub_source_text(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut lines = source.lines().peekable();

    while let Some(line) = lines.next() {
        let scrubbed_line = scrub_line(line);
        out.push_str(&scrubbed_line);
        if lines.peek().is_some() || source.ends_with('\n') {
            out.push('\n');
        }
    }

    out
}

/// Scrubs a single line of source code or configuration.
fn scrub_line(line: &str) -> String {
    // 1. Check for .env line format: KEY=VALUE or export KEY="VALUE"
    if let Some(scrubbed_env) = try_scrub_env_line(line) {
        return scrubbed_env;
    }

    // 2. Check for code assignment: let/const/var/static IDENT = "LITERAL" or ident: "LITERAL"
    if let Some(scrubbed_assign) = try_scrub_assignment(line) {
        return scrubbed_assign;
    }

    // 3. Check for standalone secret literals within string quotes on this line
    scrub_string_literals_in_line(line)
}

/// Tries to scrub an environment variable line (`KEY=VALUE` or `export KEY="VALUE"`).
fn try_scrub_env_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with('#') || !trimmed.contains('=') {
        return None;
    }

    let without_export = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    let eq_idx = without_export.find('=')?;
    let key = without_export[..eq_idx].trim();
    let val = without_export[eq_idx + 1..].trim();

    // Key must be an identifier (alphanumeric + underscore)
    if key.is_empty() || !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }

    if val.is_empty() {
        return None;
    }

    let val_unquoted = val.trim_matches(|c| c == '"' || c == '\'');
    if is_sensitive_identifier(key)
        || is_secret_literal(val_unquoted)
        || is_high_entropy_secret(val_unquoted)
    {
        let prefix = line.find('=').map_or("", |i| &line[..=i]);
        let quote = if val.starts_with('"') && val.ends_with('"') {
            "\""
        } else if val.starts_with('\'') && val.ends_with('\'') {
            "'"
        } else {
            "\""
        };
        return Some(format!("{prefix}{quote}{REDACTED_SECRET}{quote}"));
    }

    None
}

/// Tries to scrub assignment lines: `IDENT = "VALUE"` or `IDENT: "VALUE"`.
fn try_scrub_assignment(line: &str) -> Option<String> {
    let delim = match (line.find('='), line.find(':')) {
        (Some(idx), _) => {
            let prev = line[..idx].chars().last();
            let next = line[idx + 1..].chars().next();
            if matches!(prev, Some('!' | '=' | '<' | '>')) || matches!(next, Some('=' | '>')) {
                return None;
            }
            '='
        }
        (None, Some(c_idx)) if !line[c_idx + 1..].starts_with(':') => ':',
        _ => return None,
    };

    let idx = line.find(delim)?;
    let lhs = &line[..idx];
    let rhs = &line[idx + 1..];

    let ident = extract_identifier_from_lhs(lhs);
    let sensitive_ident = ident.as_deref().is_some_and(is_sensitive_identifier);

    // Look for string literals in rhs
    if let Some((quote, start, end)) = find_string_literal(rhs) {
        let literal_content = &rhs[start + 1..end];
        if sensitive_ident || is_secret_literal(literal_content) {
            let mut out = String::new();
            out.push_str(lhs);
            out.push(delim);
            out.push_str(&rhs[..start]);
            out.push(quote);
            out.push_str(REDACTED_SECRET);
            out.push(quote);
            out.push_str(&rhs[end + 1..]);
            return Some(out);
        }
    }

    None
}

/// Finds the first string literal in a slice: returns (quote_char, start_idx, end_idx).
fn find_string_literal(s: &str) -> Option<(char, usize, usize)> {
    for quote in ['"', '\'', '`'] {
        if let Some(start) = s.find(quote) {
            let remainder = &s[start + 1..];
            if let Some(relative_end) = remainder.find(quote) {
                let end = start + 1 + relative_end;
                return Some((quote, start, end));
            }
        }
    }
    None
}

/// Extracts the last identifier token from an assignment LHS (e.g., `let secret_key`).
fn extract_identifier_from_lhs(lhs: &str) -> Option<String> {
    // Remove type annotations if present (e.g. `pub const STRIPE_KEY: &str`)
    let cleaned = if let Some(col_idx) = lhs.find(':') {
        &lhs[..col_idx]
    } else {
        lhs
    };

    let tokens: Vec<&str> = cleaned
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .collect();

    // Filter out language keywords
    let keywords = [
        "let", "const", "var", "static", "pub", "crate", "mut", "final", "val",
    ];
    tokens
        .into_iter()
        .rev()
        .find(|&t| !keywords.contains(&t))
        .map(str::to_owned)
}

/// Replaces any string literal on `line` that is classified as a secret literal.
fn scrub_string_literals_in_line(line: &str) -> String {
    let mut result = String::new();
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut i = 0;

    while i < chars.len() {
        let (byte_idx, ch) = chars[i];
        if ch == '"' || ch == '\'' || ch == '`' {
            let quote = ch;
            let start_byte = byte_idx;
            let mut end_byte = None;
            let mut j = i + 1;
            while j < chars.len() {
                let (b_idx, c) = chars[j];
                if c == quote && chars[j - 1].1 != '\\' {
                    end_byte = Some(b_idx);
                    break;
                }
                j += 1;
            }

            if let Some(end) = end_byte {
                let inner = &line[start_byte + 1..end];
                if is_secret_literal(inner) {
                    result.push(quote);
                    result.push_str(REDACTED_SECRET);
                    result.push(quote);
                } else {
                    result.push_str(&line[start_byte..=end]);
                }
                i = j + 1;
                continue;
            }
        }
        result.push(ch);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrub_env_line() {
        let line = "STRIPE_SECRET_KEY=sk_live_1234567890abcdef";
        assert_eq!(
            scrub_source_text(line),
            "STRIPE_SECRET_KEY=\"[REDACTED_SECRET]\""
        );

        let export_line = "export DB_PASSWORD=\"supersecret\"";
        assert_eq!(
            scrub_source_text(export_line),
            "export DB_PASSWORD=\"[REDACTED_SECRET]\""
        );
    }

    #[test]
    fn test_scrub_code_assignment() {
        let rust_line = "pub const STRIPE_KEY: &str = \"sk_live_51Nz987654321abcdef\";";
        assert_eq!(
            scrub_source_text(rust_line),
            "pub const STRIPE_KEY: &str = \"[REDACTED_SECRET]\";"
        );
        let ts_line = "const defaultHeader = \"ghp_1234567890abcdefghijklmnopqrstuvwxyz\";";
        assert_eq!(
            scrub_source_text(ts_line),
            "const defaultHeader = \"[REDACTED_SECRET]\";"
        );
    }

    #[test]
    fn test_benign_code_preservation() {
        let code = "pub struct SyntaxToken { pub kind: String }";
        assert_eq!(scrub_source_text(code), code);
        let const_tok = "const MAX_TOKENS: usize = 4096;";
        assert_eq!(scrub_source_text(const_tok), const_tok);
    }
}
