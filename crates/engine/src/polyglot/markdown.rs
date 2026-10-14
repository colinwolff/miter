//! Markdown heading extraction: a document's own outline.
//!
//! An ATX heading (`#` through `######`) is a declaration. Its *level* is its `#` count, capped
//! at the hierarchy model's deepest level, and its span runs from the heading line to the line
//! before the next heading of the same or a shallower level — which is what makes
//! `miter skeleton README.md` print the table of contents and `miter lookup <heading>` resolve a
//! section rather than a line.
//!
//! Setext headings (`===`/`---` underlines) are deliberately *not* read: they carry no `#` count,
//! so a document mixing the two would produce a hierarchy the source does not state.

use std::path::Path;

use super::{Language, SymbolKind, UniversalSymbol, common};

/// The deepest level a heading maps to, matching the hierarchy model's floor.
const MAX_HEADING_LEVEL: usize = 4;

/// One ATX heading, as read from the source.
struct Heading {
    /// One-based line the heading sits on.
    line: usize,
    /// Byte offset the heading starts at.
    start: usize,
    /// `#` count, `1..=6`.
    hashes: usize,
    /// The heading text, with the hashes and any closing hashes removed.
    text: String,
}

/// Extracts ATX headings from Markdown source.
#[must_use]
pub fn extract_markdown_symbols(path: &Path, content: &str) -> Vec<UniversalSymbol> {
    let headings = read_headings(content);
    let module = common::module_path(path);
    let mut symbols = Vec::with_capacity(headings.len());

    for (index, heading) in headings.iter().enumerate() {
        // The section ends where the next heading at this level or shallower begins, and the last
        // one runs to the end of the file.
        let end = headings[index + 1..]
            .iter()
            .find(|next| next.hashes <= heading.hashes)
            .map_or(content.len(), |next| next.start);

        symbols.push(UniversalSymbol {
            path: common::dotted_path(&module, &[], &heading.text),
            name: heading.text.clone(),
            language: Language::Rust,
            kind: SymbolKind::Heading,
            depth: heading.hashes.min(MAX_HEADING_LEVEL),
            docstring: None,
            signature: format!("{} {}", "#".repeat(heading.hashes), heading.text),
            span: (heading.start, end),
            file: path.to_path_buf(),
            line: heading.line,
            params: Vec::new(),
            return_type: None,
            exported: true,
        });
    }

    symbols
}

/// Reads every ATX heading in `content`, in source order.
fn read_headings(content: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut offset = 0usize;
    let mut fence: Option<(char, usize)> = None;

    for (index, line) in content.lines().enumerate() {
        let line_start = offset;
        offset += line.len() + 1; // the newline `lines()` dropped

        // A fenced code block holds literal text, so a `#` inside one is a shell comment rather
        // than a heading — which is exactly what a README's usage examples are full of.
        if let Some((marker, run)) = fence_marker(line) {
            match fence {
                // A closing fence repeats the opening marker, at least as many times.
                Some((open, length)) if open == marker && run >= length => fence = None,
                Some(_) => {}
                None => fence = Some((marker, run)),
            }
            continue;
        }
        if fence.is_some() {
            continue;
        }

        // Indented code blocks and blockquotes are not headings, and a `#` with no space after it
        // (`#hashtag`) is not one either — the marker needs its separating space to be a heading.
        let Some((hashes, rest)) = split_hashes(line) else {
            continue;
        };
        let text = strip_closing_hashes(rest);
        if text.is_empty() {
            continue;
        }
        headings.push(Heading {
            line: index + 1,
            start: line_start,
            hashes,
            text,
        });
    }

    headings
}

/// The fence a line opens or closes: its marker character and its run length.
///
/// A fence is three or more backticks or tildes at the start of a line. An opening and a closing
/// fence pair up only when they use the same marker, and the closer may be the longer run.
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let marker = line.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let run = line.chars().take_while(|c| *c == marker).count();
    (run >= 3).then_some((marker, run))
}

/// The `#` count and the text after the marker, or `None` when the line is not an ATX heading.
fn split_hashes(line: &str) -> Option<(usize, &str)> {
    if line.starts_with(' ') || line.starts_with('\t') || line.starts_with('>') {
        return None;
    }
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    if !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    Some((hashes, rest.trim()))
}

/// Drops the optional run of closing hashes CommonMark allows (`## Title ##`).
fn strip_closing_hashes(text: &str) -> String {
    text.trim_end_matches('#').trim_end().to_owned()
}

#[cfg(test)]
mod cases {
    use super::*;

    const DOC: &str = "\
# Title

Intro.

## Install

Steps.

### Linux

More.

## Usage

Done.

# Appendix
";

    #[test]
    fn headings_are_read_with_their_levels_and_lines() {
        let symbols = extract_markdown_symbols(Path::new("docs/guide.md"), DOC);
        let outline: Vec<(&str, usize, usize)> = symbols
            .iter()
            .map(|s| (s.name.as_str(), s.depth, s.line))
            .collect();
        assert_eq!(
            outline,
            vec![
                ("Title", 1, 1),
                ("Install", 2, 5),
                ("Linux", 3, 9),
                ("Usage", 2, 13),
                ("Appendix", 1, 17),
            ]
        );
        assert!(symbols.iter().all(|s| s.kind == SymbolKind::Heading));
        assert_eq!(symbols[0].signature, "# Title");
        assert_eq!(symbols[2].signature, "### Linux");
    }

    #[test]
    fn a_section_ends_where_the_next_peer_begins() {
        let symbols = extract_markdown_symbols(Path::new("a.md"), DOC);
        // `## Install` runs until `## Usage`, swallowing its `### Linux` subsection.
        let install = &symbols[1];
        let usage = &symbols[3];
        assert!(
            install.span.1 <= usage.span.0,
            "{:?} {:?}",
            install.span,
            usage.span
        );
        // The last heading runs to the end of the file.
        assert_eq!(symbols[4].span.1, DOC.len());
    }

    #[test]
    fn non_headings_are_ignored() {
        let source = "#real hashtag\n    # indented code\n> # quoted\n#\n####### seven\n# Kept\n";
        let names: Vec<String> = extract_markdown_symbols(Path::new("a.md"), source)
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, vec!["Kept"]);
    }

    #[test]
    fn a_hash_inside_a_fenced_block_is_not_a_heading() {
        let source = "# Title

```bash
# not a heading
miter index .
```

## After

~~~
## also not
~~~
";
        let names: Vec<String> = extract_markdown_symbols(Path::new("README.md"), source)
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, vec!["Title", "After"]);
    }

    #[test]
    fn closing_hashes_are_not_part_of_the_name() {
        let symbols = extract_markdown_symbols(Path::new("a.md"), "## Title ##\n");
        assert_eq!(symbols[0].name, "Title");
        assert_eq!(symbols[0].signature, "## Title");
    }
}
