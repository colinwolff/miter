//! Redaction for the values a declarative extractor records.
//!
//! A Dockerfile's `ENV`/`ARG` and a manifest's `env:` block are *configuration*, and a
//! configuration value is exactly where a credential lives. Every value an extractor puts into a
//! signature passes through [`redacted_value`] first, so a secret can never reach
//! `.miter/miter.redb` or an MCP response.

use crate::security::{
    REDACTED_SECRET, is_high_entropy_secret, is_secret_literal, is_sensitive_identifier,
    scrub_source_text,
};

/// The value to record for `key`.
///
/// A sensitive key name (`STRIPE_SECRET_KEY`), a vendor-prefixed literal (`sk_live_…`, `ghp_…`,
/// `AKIA…`) or a high-entropy value is replaced by [`REDACTED_SECRET`] outright. Anything else is
/// still passed through [`scrub_source_text`], so a credential embedded in an otherwise ordinary
/// value is scrubbed too.
#[must_use]
pub fn redacted_value(key: &str, value: &str) -> String {
    let trimmed = value.trim().trim_matches(['"', '\'', '`']);
    if trimmed.is_empty() {
        return String::new();
    }
    if is_sensitive_identifier(key) || is_secret_literal(trimmed) || is_high_entropy_secret(trimmed)
    {
        return REDACTED_SECRET.to_owned();
    }
    scrub_source_text(trimmed)
}

#[cfg(test)]
mod cases {
    use super::*;

    #[test]
    fn a_sensitive_key_is_redacted_whatever_its_value_looks_like() {
        assert_eq!(redacted_value("STRIPE_SECRET_KEY", "abc"), REDACTED_SECRET);
        assert_eq!(redacted_value("db_password", "hunter2"), REDACTED_SECRET);
    }

    #[test]
    fn a_vendor_literal_is_redacted_under_an_ordinary_key() {
        assert_eq!(
            redacted_value("TOKEN", "ghp_1234567890abcdef"),
            REDACTED_SECRET
        );
        assert_eq!(
            redacted_value("KEY", "AKIAIOSFODNN7EXAMPLE"),
            REDACTED_SECRET
        );
    }

    #[test]
    fn an_ordinary_value_survives() {
        assert_eq!(redacted_value("PORT", "8080"), "8080");
        assert_eq!(redacted_value("LOG_LEVEL", "debug"), "debug");
        assert_eq!(redacted_value("EMPTY", "  "), "");
    }
}
