//! The verdict a compiler returns.
//!
//! Two outcomes: a compiler either accepted the tree or rejected it, and a rejection carries the
//! *location* of the fault — `src/lib.rs:14:9 — expected u8, found &str` — rather than a rendered
//! frame around it. Collapsing that into a string would mean a caller guessing which bytes are
//! wrong, and a location guessed at is worse than a location reported.

use serde::{Deserialize, Serialize};

/// A located region of source a diagnostic points into.
///
/// Byte offsets *and* line/column pairs are both carried because the two consumers need
/// different ones: a patch writer slices with `byte_start..byte_end`, while a reader of a
/// rendered excerpt needs the line and column to find the region without re-reading the file.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct DiagnosticSpan {
    /// File the span is in, as the tool reported it — relative to the sandbox root when the
    /// tool was run there.
    pub file_path: String,
    /// Inclusive start offset in bytes.
    pub byte_start: usize,
    /// Exclusive end offset in bytes.
    pub byte_end: usize,
    /// First line of the span, one-based, as every compiler counts.
    pub line_start: usize,
    /// Last line of the span, one-based.
    pub line_end: usize,
    /// First column of `line_start`, one-based.
    pub column_start: usize,
    /// Last column of `line_end`, one-based.
    pub column_end: usize,
}

impl DiagnosticSpan {
    /// The number of bytes the span covers.
    ///
    /// Saturating rather than panicking: a tool reporting a reversed range is reporting a
    /// broken span, and a harness that aborts on it cannot report the diagnostic that
    /// described the breakage.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.byte_end.saturating_sub(self.byte_start)
    }

    /// The span's location as `path:line:column`, the form every editor and backtrace uses.
    #[must_use]
    pub fn location(&self) -> String {
        format!(
            "{}:{}:{}",
            self.file_path, self.line_start, self.column_start
        )
    }
}

/// What a verifier concluded.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum VerificationOutcome {
    /// The check passed. `stdout` is kept because a passing `cargo check` still prints the
    /// build-script output a later failure has to be read against.
    Success {
        /// Whatever the tool wrote to stdout, verbatim.
        stdout: String,
        /// How long the verification took, in milliseconds.
        duration_ms: u64,
    },
    /// A compiler rejected the tree, and the rejection is localised.
    DiagnosticFailure {
        /// The tool's own error code (`E0308`, `TS2322`), or `"error"` when it named none.
        error_code: String,
        /// The human-readable message, verbatim.
        message: String,
        /// Every primary span the tool reported, in the order it reported them.
        spans: Vec<DiagnosticSpan>,
        /// The tool's stderr, kept verbatim: a diagnostic that failed to parse is only
        /// debuggable against the text it failed to parse from.
        raw_stderr: String,
    },
}

impl VerificationOutcome {
    /// Whether the verification passed.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, VerificationOutcome::Success { .. })
    }

    /// The error code, when the outcome carries one.
    #[must_use]
    pub fn error_code(&self) -> Option<&str> {
        match self {
            VerificationOutcome::DiagnosticFailure { error_code, .. } => Some(error_code),
            _ => None,
        }
    }

    /// A one-line summary, for a log line or a terminal row.
    ///
    /// Bounded by `SUMMARY_LIMIT` characters: this is a summary, and a summary that
    /// reprints a forty-line compiler message is a log line nobody reads.
    #[must_use]
    pub fn summary(&self) -> String {
        /// Longest summary the harness prints.
        const SUMMARY_LIMIT: usize = 160;
        let raw = match self {
            VerificationOutcome::Success { duration_ms, .. } => format!("ok in {duration_ms}ms"),
            VerificationOutcome::DiagnosticFailure {
                error_code,
                message,
                spans,
                ..
            } => {
                let where_ = spans
                    .first()
                    .map_or_else(String::new, DiagnosticSpan::location);
                format!("{error_code} at {where_}: {message}")
            }
        };
        truncate(&raw, SUMMARY_LIMIT)
    }
}

/// Truncates on a character boundary, with an ellipsis that says it happened.
fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> DiagnosticSpan {
        DiagnosticSpan {
            file_path: "src/lib.rs".into(),
            byte_start: 10,
            byte_end: 18,
            line_start: 3,
            line_end: 3,
            column_start: 5,
            column_end: 13,
        }
    }

    fn every_outcome() -> [VerificationOutcome; 2] {
        [
            VerificationOutcome::Success {
                stdout: "Finished".into(),
                duration_ms: 12,
            },
            VerificationOutcome::DiagnosticFailure {
                error_code: "E0308".into(),
                message: "mismatched types".into(),
                spans: vec![span()],
                raw_stderr: "error[E0308]: mismatched types".into(),
            },
        ]
    }

    #[test]
    fn outcome_round_trips_through_json() {
        for outcome in every_outcome() {
            let text = serde_json::to_string(&outcome).expect("serialise");
            let back: VerificationOutcome =
                serde_json::from_str(&text).expect("deserialise the same document");
            assert_eq!(back, outcome, "round trip changed the outcome");
        }
    }

    #[test]
    fn the_tag_is_the_variant_name() {
        // The CLI and the integration suite both branch on `outcome`, so the tag is part of
        // this crate's interface rather than a detail of how serde happens to lay it out.
        let text = serde_json::to_string(&VerificationOutcome::Success {
            stdout: String::new(),
            duration_ms: 0,
        })
        .expect("serialise");
        assert!(text.contains(r#""outcome":"success""#), "{text}");
    }

    #[test]
    fn a_reversed_span_reports_zero_rather_than_panicking() {
        let mut reversed = span();
        reversed.byte_end = 4;
        assert_eq!(reversed.byte_len(), 0);
    }

    #[test]
    fn summary_is_bounded_and_located() {
        let outcome = VerificationOutcome::DiagnosticFailure {
            error_code: "E0308".into(),
            message: "x".repeat(4_000),
            spans: vec![span()],
            raw_stderr: String::new(),
        };
        let summary = outcome.summary();
        assert!(summary.chars().count() <= 161, "{}", summary.len());
        assert!(summary.starts_with("E0308 at src/lib.rs:3:5"), "{summary}");
    }
}
