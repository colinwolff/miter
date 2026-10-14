//! Reading a compiler's answer off the wire.
//!
//! Both parsers are pure functions over text, and neither shells out. The interesting
//! behaviour of this module — *which* span is kept, what a line without a primary span
//! becomes, how a malformed line is treated — is decided by a function, so it is tested by
//! calling it with a string rather than by arranging for a compiler to fail in a specific way.
//!
//! A parser that fails *softly* is a trap here, so the rules are stated once and honoured by
//! both: an unparsable line is skipped, a diagnostic with no usable span is kept with an empty
//! span list (the message still names the file), and nothing is invented to fill a gap.

use serde::Deserialize;

use crate::verifier::DiagnosticSpan;

/// One compiler diagnostic, reduced to what a repair loop can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// The tool's error code (`E0308`, `TS2322`), or `"error"` when it named none.
    pub code: String,
    /// The message, without the rendered frame around it.
    pub message: String,
    /// The primary spans, in the order the tool reported them.
    pub spans: Vec<DiagnosticSpan>,
}

/// The code a diagnostic gets when the tool named none.
const UNCODED: &str = "error";

/// The `--message-format=json` envelope cargo emits, one JSON document per line.
#[derive(Debug, Deserialize)]
struct CargoMessage {
    /// Why cargo emitted this line: `compiler-message`, `build-finished`, …
    reason: String,
    /// The compiler's own diagnostic.
    message: CargoDiagnostic,
}

/// The diagnostic inside a `compiler-message` envelope.
#[derive(Debug, Deserialize)]
struct CargoDiagnostic {
    /// `error`, `warning`, `note`, …
    level: String,
    /// The diagnostic's text.
    message: String,
    /// Present on some diagnostics (`E0308`), absent on others (`aborting due to …`).
    #[serde(default)]
    code: Option<CargoCode>,
    /// Every span the diagnostic touches, primary and not.
    #[serde(default)]
    spans: Vec<CargoSpan>,
}

/// The `{"code": "E0308", "explanation": …}` object inside a diagnostic.
#[derive(Debug, Deserialize)]
struct CargoCode {
    /// The bare code.
    code: String,
}

/// One span inside a cargo diagnostic.
#[derive(Debug, Deserialize)]
struct CargoSpan {
    /// The file, relative to the workspace root cargo ran in.
    file_name: String,
    /// Inclusive byte start.
    byte_start: usize,
    /// Exclusive byte end.
    byte_end: usize,
    /// One-based line range.
    line_start: usize,
    /// One-based line range.
    line_end: usize,
    /// One-based column range.
    column_start: usize,
    /// One-based column range.
    column_end: usize,
    /// Whether this is the span the diagnostic is *about*.
    #[serde(default)]
    is_primary: bool,
}

impl From<CargoSpan> for DiagnosticSpan {
    fn from(span: CargoSpan) -> Self {
        DiagnosticSpan {
            file_path: span.file_name,
            byte_start: span.byte_start,
            byte_end: span.byte_end,
            line_start: span.line_start,
            line_end: span.line_end,
            column_start: span.column_start,
            column_end: span.column_end,
        }
    }
}
/// Parses cargo's JSON diagnostic stream into errors, in the order cargo reported them.
///
/// Only `level == "error"` is returned, and only primary spans are kept. Both are the same
/// decision seen from two sides: the loop gets the faults, at the places the faults are.
///
/// The `build-finished` line cargo emits last is skipped by the `reason` check rather than by
/// its shape — a parser that identifies lines by their contents re-learns that lesson every
/// time cargo adds a new `reason`.
#[must_use]
pub fn parse_cargo_messages(stdout: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        // A line that is not JSON is not a diagnostic this harness can read. Skipping it is the
        // documented behaviour, and it is safe because the *exit status* — not this function —
        // decides whether the build passed.
        let Ok(envelope) = serde_json::from_str::<CargoMessage>(line) else {
            continue;
        };
        if envelope.reason != "compiler-message" || envelope.message.level != "error" {
            continue;
        }
        let spans = envelope
            .message
            .spans
            .into_iter()
            .filter(|span| span.is_primary)
            .map(DiagnosticSpan::from)
            .collect();
        diagnostics.push(Diagnostic {
            code: envelope
                .message
                .code
                .map_or_else(|| UNCODED.to_owned(), |code| code.code),
            message: envelope.message.message,
            spans,
        });
    }
    diagnostics
}

/// Parses `tsc --pretty false` output.
///
/// The format is one diagnostic per line:
///
/// ```text
/// src/index.ts(12,7): error TS2322: Type 'number' is not assignable to type 'string'.
/// ```
///
/// The byte offsets are **not** in the text at all — tsc reports line and column only — so what
/// this function returns is a *position*, and the caller resolves it against the file through
/// `offset_of`. Zero offsets mean "unresolved", and the feedback renderer reads such a
/// span by line instead of by byte.
///
/// An indented continuation line (tsc wraps long messages) has no leading `path(line,col)`, so
/// it is appended to the message above rather than dropped: a truncated message is a worse
/// failure than a long one.
#[must_use]
pub fn parse_tsc_output(stderr: &str) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for line in stderr.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match parse_tsc_line(trimmed) {
            Some(parsed) => diagnostics.push(parsed),
            // The trailing `Found 2 errors. Watching…` summary, and anything else with no
            // position: appended to the message above rather than dropped.
            None => {
                if let Some(last) = diagnostics.last_mut()
                    && !is_summary_line(trimmed)
                {
                    last.message.push(' ');
                    last.message.push_str(trimmed);
                }
            }
        }
    }
    diagnostics
}

/// The trailing `Found N errors. Watching…` summary line, recognised so it is not swallowed as
/// a continuation of the last real diagnostic.
fn is_summary_line(text: &str) -> bool {
    text.starts_with("Found ") && text.contains(" error")
}

/// One `tsc` diagnostic, if `text` is one.
fn parse_tsc_line(text: &str) -> Option<Diagnostic> {
    if is_summary_line(text) {
        return None;
    }
    let (position, rest) = text.split_once(": error ")?;
    let (code, message) = match rest.split_once(": ") {
        Some((code, message)) => (code.trim().to_owned(), message.trim().to_owned()),
        // `error TS1005: 'x'` always carries the colon, but a truncated line may not; the
        // diagnostic is still worth reporting, just without a message of its own.
        None => (UNCODED.to_owned(), rest.trim().to_owned()),
    };
    let (file, line, column) = parse_tsc_position(position)?;
    Some(Diagnostic {
        code,
        message,
        spans: vec![DiagnosticSpan {
            file_path: file,
            byte_start: 0,
            byte_end: 0,
            line_start: line,
            line_end: line,
            column_start: column,
            // tsc reports a start position only, so the span ends where it begins until the
            // caller widens it against the file.
            column_end: column,
        }],
    })
}

/// `src/index.ts(12,7)` → (`src/index.ts`, 12, 7).
fn parse_tsc_position(position: &str) -> Option<(String, usize, usize)> {
    let open = position.rfind('(')?;
    if !position.ends_with(')') {
        return None;
    }
    let file = position[..open].trim();
    let (line, column) = position[open + 1..position.len() - 1].split_once(',')?;
    Some((
        file.to_owned(),
        line.trim().parse().ok()?,
        column.trim().parse().ok()?,
    ))
}
