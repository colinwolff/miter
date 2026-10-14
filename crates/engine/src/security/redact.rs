//! Sensitive identifier matching and known secret literal prefix detection.

use super::entropy::is_high_entropy_secret;

/// Replacement mask used for redacted secret values.
pub const REDACTED_SECRET: &str = "[REDACTED_SECRET]";

/// Sensitive keyword stems to match against identifier names.
const SENSITIVE_KEYWORDS: &[&str] = &[
    "secret",
    "token",
    "password",
    "passwd",
    "api_key",
    "apikey",
    "private_key",
    "priv_key",
    "credential",
    "auth_key",
    "client_secret",
    "signing_key",
    "encryption_key",
    "bearer",
    "salt",
];

/// Substrings that indicate an identifier is an AST, lexer, or framework term rather than a credential.
const FALSE_POSITIVE_TERMS: &[&str] = &[
    "tokenstream",
    "syntaxtoken",
    "tokenizer",
    "tokenkind",
    "cancel_token",
    "cancellation_token",
    "cancellationtoken",
    "next_token",
    "token_stream",
    "syntax_token",
    "token_kind",
];

/// Checks if an identifier name is considered sensitive (e.g., `STRIPE_SECRET_KEY`, `db_password`).
///
/// Preserves benign AST/lexer terminology like `SyntaxToken`, `Tokenizer`, `cancel_token`.
#[must_use]
pub fn is_sensitive_identifier(ident: &str) -> bool {
    let lowered = ident.to_ascii_lowercase();

    // Check false positive exceptions first
    for fp in FALSE_POSITIVE_TERMS {
        if lowered.contains(fp) {
            return false;
        }
    }

    // Match against sensitive keywords
    for kw in SENSITIVE_KEYWORDS {
        if lowered.contains(kw) {
            return true;
        }
    }

    false
}

/// Known API prefixes for credentials.
const KNOWN_SECRET_PREFIXES: &[&str] = &[
    "sk_live_",
    "rk_live_",
    "sk_test_",
    "whsec_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "xoxr-",
    "sk-ant-",
    "sk-proj-",
    "AIzaSy",
    "-----BEGIN ",
];

/// Checks whether `literal` is a known secret format, prefix, or high-entropy credential.
#[must_use]
pub fn is_secret_literal(literal: &str) -> bool {
    let trimmed = literal
        .trim()
        .trim_matches(|c| c == '\'' || c == '"' || c == '`');
    if trimmed.is_empty() {
        return false;
    }

    // 1. Known vendor prefix check
    for prefix in KNOWN_SECRET_PREFIXES {
        if trimmed.starts_with(prefix) {
            return true;
        }
    }

    // 2. AWS Access Key ID: AKIA followed by 16 alphanumeric uppercase chars
    if trimmed.starts_with("AKIA") && trimmed.len() >= 20 {
        let suffix = &trimmed[4..20];
        if suffix
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            return true;
        }
    }

    // 3. JWTs: three base64url segments starting with eyJ
    if trimmed.starts_with("eyJ") {
        let parts: Vec<&str> = trimmed.split('.').collect();
        if parts.len() == 3
            && parts[0].starts_with("eyJ")
            && parts.iter().all(|p| !p.is_empty() && is_base64url(p))
        {
            return true;
        }
    }

    // 4. Connection strings with embedded passwords
    // (postgres|postgresql|mysql|mongodb|redis|amqp)://[^:\s]+:[^@\s]+@
    if is_database_connection_with_secret(trimmed) {
        return true;
    }

    // 5. High-entropy string detector
    if is_high_entropy_secret(trimmed) {
        return true;
    }

    false
}

/// Checks if string has database scheme and embedded password in userinfo.
fn is_database_connection_with_secret(s: &str) -> bool {
    let schemes = [
        "postgres://",
        "postgresql://",
        "mysql://",
        "mongodb://",
        "mongodb+srv://",
        "redis://",
        "rediss://",
        "amqp://",
        "amqps://",
    ];

    let has_scheme = schemes.iter().any(|&scheme| s.starts_with(scheme));
    if !has_scheme {
        return false;
    }

    if let Some(at_idx) = s.find('@') {
        let userinfo_start = s.find("://").map_or(0, |i| i + 3);
        if userinfo_start < at_idx {
            let userinfo = &s[userinfo_start..at_idx];
            // user:password format
            if let Some(colon_idx) = userinfo.find(':') {
                let pass = &userinfo[colon_idx + 1..];
                return !pass.is_empty();
            }
        }
    }

    false
}

fn is_base64url(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitive_identifier() {
        assert!(is_sensitive_identifier("STRIPE_SECRET_KEY"));
        assert!(is_sensitive_identifier("api_key"));
        assert!(is_sensitive_identifier("db_password"));
        assert!(is_sensitive_identifier("jwt_secret"));
        assert!(is_sensitive_identifier("auth_token"));

        // False positives
        assert!(!is_sensitive_identifier("SyntaxToken"));
        assert!(!is_sensitive_identifier("TokenStream"));
        assert!(!is_sensitive_identifier("Tokenizer"));
        assert!(!is_sensitive_identifier("cancel_token"));
        assert!(!is_sensitive_identifier("TokenKind"));
    }

    #[test]
    fn test_secret_literals() {
        assert!(is_secret_literal("sk_live_1234567890"));
        assert!(is_secret_literal("ghp_1234567890abcdef"));
        assert!(is_secret_literal("AKIAIOSFODNN7EXAMPLE"));
        assert!(is_secret_literal("xoxb-12345-67890"));
        assert!(is_secret_literal("-----BEGIN RSA PRIVATE KEY-----"));
        assert!(is_secret_literal(
            "postgres://admin:secret123@localhost:5432/db"
        ));
        assert!(!is_secret_literal("postgres://localhost:5432/db"));
        assert!(!is_secret_literal("http://example.com/api"));
    }
}
