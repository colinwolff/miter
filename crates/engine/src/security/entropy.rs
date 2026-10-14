//! High-entropy string detector for credential detection.
//!
//! Uses Shannon entropy over character distributions to identify potential
//! secrets and API keys embedded in code or configuration.

use std::collections::HashMap;

/// Minimum length for entropy-based secret detection.
pub const MIN_ENTROPY_LENGTH: usize = 32;

/// Shannon entropy threshold (bits/char) indicating high randomness typical of keys.
pub const ENTROPY_THRESHOLD: f64 = 4.5;

/// Computes Shannon entropy in bits per character for a given string slice.
#[must_use]
pub fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut freq = HashMap::new();
    let mut count = 0usize;
    for ch in s.chars() {
        *freq.entry(ch).or_insert(0usize) += 1;
        count += 1;
    }
    let total = count as f64;
    freq.values().fold(0.0, |acc, &cnt| {
        let p = (cnt as f64) / total;
        acc - p * p.log2()
    })
}

/// Returns `true` if `literal` is a high-entropy string typical of an API key or token.
///
/// Criteria:
/// 1. Length >= 32 characters
/// 2. No whitespace characters
/// 3. Not a file path or URL without credentials
/// 4. Shannon entropy > 4.5 bits/char
/// 5. Mixed character set (at least two of: lowercase, uppercase, digits, punctuation)
#[must_use]
pub fn is_high_entropy_secret(literal: &str) -> bool {
    let trimmed = literal.trim();
    if trimmed.len() < MIN_ENTROPY_LENGTH {
        return false;
    }

    // Ignore if contains spaces
    if trimmed.chars().any(char::is_whitespace) {
        return false;
    }

    // Ignore typical paths or filenames (without query/hash)
    if is_benign_path_or_url(trimmed) {
        return false;
    }

    let entropy = shannon_entropy(trimmed);
    if entropy <= ENTROPY_THRESHOLD {
        return false;
    }

    // Character diversity check: mixed alphanumeric/symbol
    let mut has_lower = false;
    let mut has_upper = false;
    let mut has_digit = false;
    let mut has_special = false;

    for ch in trimmed.chars() {
        if ch.is_ascii_lowercase() {
            has_lower = true;
        } else if ch.is_ascii_uppercase() {
            has_upper = true;
        } else if ch.is_ascii_digit() {
            has_digit = true;
        } else if ch.is_ascii_punctuation() {
            has_special = true;
        }
    }

    let diversity = usize::from(has_lower)
        + usize::from(has_upper)
        + usize::from(has_digit)
        + usize::from(has_special);

    diversity >= 2
}

/// Checks whether a string looks like a benign path, URL, or standard package identifier.
fn is_benign_path_or_url(s: &str) -> bool {
    // Normal URLs without embedded userinfo
    if (s.starts_with("http://") || s.starts_with("https://")) && !s.contains('@') {
        return true;
    }
    // File paths without secret characters
    if (s.starts_with('/') || s.starts_with("./") || s.starts_with("../"))
        && !s.contains('@')
        && !s.contains(':')
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shannon_entropy() {
        assert_eq!(shannon_entropy(""), 0.0);
        assert_eq!(shannon_entropy("aaaa"), 0.0);
        // 2 characters equal frequency -> 1 bit/char
        assert!((shannon_entropy("abab") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_high_entropy_secret_detection() {
        // High entropy 32+ char token (> 4.5 bits/char)
        let token = "xK9#mQ2$vL8*pZ5!jR1@wN4%yT7^bC0&eH3~";
        assert!(is_high_entropy_secret(token));

        // Short string
        assert!(!is_high_entropy_secret("secret123"));

        // Low entropy repetitive string
        let repetitive = "abcdefghijklmnopqrstuvwxyzabcdef";
        assert!(repetitive.len() >= 32);
        // Alphabet has higher entropy if all unique, but lets test repeating:
        let low_ent = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        assert!(!is_high_entropy_secret(low_ent));

        // Standard URL
        assert!(!is_high_entropy_secret(
            "https://example.com/api/v1/users/list_all_resources_today"
        ));
    }
}
