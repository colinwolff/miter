//! A minimal, allocation-light JSON writer.
//!
//! The workspace deliberately avoids `serde` (see the README's dependency
//! justification: the persistence format is a hand-rolled versioned record), and the
//! CLI output is a small fixed shape, so a short writer with correct string escaping is
//! all the tool interface needs. Output is pretty-printed with two-space indentation and
//! deterministic key order, which is what makes it diffable in agent transcripts.

/// A JSON value that can be rendered.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    /// `null`
    Null,
    /// `true` / `false`
    Bool(bool),
    /// An unsigned integer, rendered exactly.
    Unsigned(u64),
    /// A pre-formatted number literal (the caller decides the precision, which keeps
    /// `f32` scores short and stable).
    Number(String),
    /// A string, escaped on render.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object; the order of the pairs is preserved.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// An array from any iterator of values.
    pub fn array<I: IntoIterator<Item = Json>>(values: I) -> Self {
        Json::Array(values.into_iter().collect())
    }

    /// An object from any iterator of key/value pairs.
    pub fn object<I: IntoIterator<Item = (&'static str, Json)>>(entries: I) -> Self {
        Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }

    /// A string value.
    pub fn string(value: impl Into<String>) -> Self {
        Json::String(value.into())
    }

    /// A floating point value rendered with a fixed number of decimals.
    pub fn fixed(value: f64, decimals: usize) -> Self {
        Json::Number(format!("{value:.decimals$}"))
    }

    /// A `f32` rendered with a fixed number of decimals.
    pub fn fixed_f32(value: f32, decimals: usize) -> Self {
        Json::fixed(f64::from(value), decimals)
    }

    /// `null` for `None`, otherwise the contained value.
    pub fn optional(value: Option<Json>) -> Self {
        value.unwrap_or(Json::Null)
    }

    /// Renders the value as pretty-printed JSON.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(256);
        self.render_into(&mut out, 0);
        out
    }

    /// Renders the value compactly (no whitespace between tokens).
    pub fn render_compact(&self) -> String {
        let mut out = String::with_capacity(128);
        self.render_compact_into(&mut out);
        out
    }
}

/// Appends `value` as a quoted, escaped JSON string.
///
/// Escapes the two mandatory characters (`"` and `\`), the short control escapes, and
/// every other C0 control character as `\u00XX` (as RFC 8259 requires). Non-ASCII
/// characters are emitted verbatim as UTF-8, which is valid JSON.
pub fn escape_into(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            control if control < '\u{20}' => {
                out.push_str(&format!("\\u{:04x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// A quoted, escaped JSON string.
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    escape_into(&mut out, value);
    out
}

impl Json {
    fn render_into(&self, out: &mut String, depth: usize) {
        match self {
            Json::Null | Json::Bool(_) | Json::Unsigned(_) | Json::Number(_) => {
                self.render_compact_into(out);
            }
            Json::String(value) => escape_into(out, value),
            Json::Array(values) => {
                if values.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    newline_indent(out, depth + 1);
                    value.render_into(out, depth + 1);
                }
                newline_indent(out, depth);
                out.push(']');
            }
            Json::Object(entries) => {
                if entries.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    newline_indent(out, depth + 1);
                    escape_into(out, key);
                    out.push_str(": ");
                    value.render_into(out, depth + 1);
                }
                newline_indent(out, depth);
                out.push('}');
            }
        }
    }

    fn render_compact_into(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Json::Unsigned(value) => out.push_str(&value.to_string()),
            Json::Number(literal) => out.push_str(literal),
            Json::String(value) => escape_into(out, value),
            Json::Array(values) => {
                out.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    value.render_compact_into(out);
                }
                out.push(']');
            }
            Json::Object(entries) => {
                out.push('{');
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    escape_into(out, key);
                    out.push(':');
                    value.render_compact_into(out);
                }
                out.push('}');
            }
        }
    }
}

/// Pushes a newline followed by two spaces per nesting level.
fn newline_indent(out: &mut String, depth: usize) {
    out.push('\n');
    for _ in 0..depth {
        out.push_str("  ");
    }
}

#[cfg(test)]
mod cases;
