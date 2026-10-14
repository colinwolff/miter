//! The diagnostic parsers, against captured compiler output.

use super::super::span::offset_of;
use super::fixtures::{cargo_error_line, cargo_warning_line};

#[test]
fn a_cargo_error_yields_its_code_and_its_primary_span_only() {
    let stdout = [
        cargo_warning_line(),
        cargo_error_line(),
        // rustc's own trailing error, which carries no code and no span.
        r#"{"reason":"compiler-message","message":{"level":"error","message":"aborting due to 1 previous error","spans":[]}}"#.to_owned(),
        r#"{"reason":"build-finished","success":false}"#.to_owned(),
    ]
    .join("\n");

    let diagnostics = super::super::parse_cargo_messages(&stdout);
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");

    let first = &diagnostics[0];
    assert_eq!(first.code, "E0308");
    assert_eq!(first.message, "mismatched types");
    assert_eq!(
        first.spans.len(),
        1,
        "a non-primary span survived: {first:?}"
    );
    let span = &first.spans[0];
    assert_eq!(span.file_path, "src/lib.rs");
    assert_eq!((span.byte_start, span.byte_end), (25, 30));
    assert_eq!((span.line_start, span.line_end), (2, 2));
    assert_eq!((span.column_start, span.column_end), (5, 10));
    assert_eq!(span.location(), "src/lib.rs:2:5");
    assert_eq!(span.byte_len(), 5);

    // The uncoded error keeps a diagnostic rather than being dropped for want of a code.
    assert_eq!(diagnostics[1].code, "error");
    assert!(diagnostics[1].spans.is_empty());
}
#[test]
fn a_tsc_error_yields_its_code_and_position() {
    let stderr =
        "src/index.ts(12,7): error TS2322: Type 'number' is not assignable to type 'string'.\n";
    let diagnostics = super::super::parse_tsc_output(stderr);
    assert_eq!(diagnostics.len(), 1);
    let only = &diagnostics[0];
    assert_eq!(only.code, "TS2322");
    assert_eq!(
        only.message,
        "Type 'number' is not assignable to type 'string'."
    );
    assert_eq!(only.spans.len(), 1);
    let span = &only.spans[0];
    assert_eq!(span.file_path, "src/index.ts");
    assert_eq!((span.line_start, span.column_start), (12, 7));
    // tsc reports no byte offsets; the harness resolves them rather than inventing them.
    assert_eq!(span.byte_len(), 0, "a byte offset was fabricated for tsc");
}

#[test]
fn a_wrapped_tsc_message_keeps_its_continuation() {
    let stderr = "\
src/index.ts(12,7): error TS2322: Type 'number' is not assignable
  to type 'string'.
src/index.ts(20,1): error TS1005: '}' expected.
Found 2 errors. Watching for file changes.
";
    let diagnostics = super::super::parse_tsc_output(stderr);
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].message, "Type 'number' is not assignable to type 'string'.",
        "the continuation line was dropped"
    );
    assert!(
        !diagnostics[1].message.contains("Found 2 errors"),
        "the summary was swallowed into a diagnostic: {}",
        diagnostics[1].message
    );
}

#[test]
fn a_malformed_tsc_line_is_not_a_diagnostic() {
    for noise in [
        "error TS5083: Cannot read file 'tsconfig.json'.",
        "just some text\n",
        "src/a.ts(x,y): error TS1005: bad\n",
    ] {
        assert!(
            super::super::parse_tsc_output(noise).is_empty(),
            "`{noise}` parsed as a diagnostic"
        );
    }
}

#[test]
fn offsets_are_counted_in_characters_not_bytes() {
    // Line 1 holds a two-byte codepoint, so "column 6" is not "byte 6": slicing at a byte
    // offset inside a character panics rather than returning nonsense.
    let text = "let θ = 1;\nlet y = 2;\n";
    let offset = offset_of(text, 1, 6).expect("column 6 of line 1");
    assert_eq!(&text[offset..offset + 1], " ");
    // Line 2 starts after line 1's newline, and the newline itself counts: `let θ = 1;` is
    // eleven *bytes* because θ is two, and the twelfth is the newline.
    assert_eq!(offset_of(text, 2, 1), Some(12));
    // A column past the end of the line clamps to the line's end rather than reading the next.
    assert_eq!(offset_of(text, 1, 99), Some(11));
    assert_eq!(offset_of(text, 9, 1), None, "a line past the end resolved");
}

#[test]
fn a_cargo_stream_of_noise_yields_nothing_rather_than_a_guess() {
    let stdout =
        "Compiling miter-fixture v0.1.0\nwarning: build script failed\nnot json at all\n{}\n";
    assert!(super::super::parse_cargo_messages(stdout).is_empty());
}

#[test]
fn a_success_reports_the_last_human_line_and_falls_back_to_the_machine_one() {
    // With `--message-format=json` cargo prints only JSON, so the fallback is what a real run
    // produces; with the flag off it is a sentence. Both have to be a *line cargo wrote*.
    let json_only = "{\"reason\":\"compiler-artifact\",\"success\":true}\n\
                     {\"reason\":\"build-finished\",\"success\":true}\n";
    assert_eq!(
        super::super::run::last_line(json_only),
        r#"{"reason":"build-finished","success":true}"#
    );

    let mixed = "Checking miter v0.1.0\nFinished dev [unoptimized] target(s) in 1.20s\n\
                 {\"reason\":\"build-finished\",\"success\":true}\n";
    assert_eq!(
        super::super::run::last_line(mixed),
        "Finished dev [unoptimized] target(s) in 1.20s"
    );

    assert_eq!(super::super::run::last_line(""), "");
}
