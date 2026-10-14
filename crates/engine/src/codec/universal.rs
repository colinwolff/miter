//! The universal record: a declaration that can name a `struct`, an `interface` or a `def`.

use std::path::PathBuf;

use super::wire::{Reader, Writer};
use super::{DecodeError, MAGIC};

/// The encoding version of a [`crate::polyglot::UniversalSymbol`] record.
///
/// Separate from [`super::VERSION`] because the two records are *different records*, not two revisions of
/// one: the function-shaped record predates the universal one and keeps its own version so an
/// older database's symbols keep decoding.
pub const UNIVERSAL_VERSION: u8 = 1;

/// Encodes a universal symbol into a self-contained little-endian record.
///
/// The layout is the [`super::encode_symbol`] one — magic, version, then fixed fields and length-prefixed
/// text — because the alternative was a second record format in the same table family, and a
/// reader that has to know two shapes is a reader that can guess wrong.
///
/// Only the fields that describe the declaration are written: its identity and its level are
/// stored once, so nothing is derived twice and the record stays small.
pub fn encode_universal(symbol: &crate::polyglot::UniversalSymbol) -> Vec<u8> {
    let mut writer = Writer::with_capacity(512);
    writer.bytes(&MAGIC);
    writer.u8(UNIVERSAL_VERSION);
    writer.u8(universal_language_code(symbol.language));
    writer.u8(universal_kind_code(symbol.kind));
    writer.u8(u8::from(symbol.exported));
    writer.u64(symbol.id());
    writer.u32(symbol.depth as u32);
    writer.u32(symbol.line as u32);
    writer.u32(symbol.span.0 as u32);
    writer.u32(symbol.span.1 as u32);
    writer.text(&symbol.file.to_string_lossy());
    writer.text(&symbol.name);
    writer.text(&symbol.path);
    writer.text(&crate::security::scrub_source_text(&symbol.signature));
    match &symbol.docstring {
        Some(docstring) => {
            writer.u8(1);
            writer.text(docstring);
        }
        None => writer.u8(0),
    }
    match &symbol.return_type {
        Some(return_type) => {
            writer.u8(1);
            writer.text(return_type);
        }
        None => writer.u8(0),
    }
    writer.u32(symbol.params.len() as u32);
    for param in &symbol.params {
        writer.text(&param.name);
        writer.text(&param.type_name);
    }
    writer.finish()
}

/// Decodes a record produced by [`encode_universal`].
///
/// # Errors
///
/// [`DecodeError`] for a wrong magic or version, a truncated record, a code this build does not
/// know, or malformed UTF-8. An unknown *code* is refused rather than defaulted: defaulting a
/// kind would silently turn a `struct` into a function, which is the misreading this whole module
/// exists to prevent.
pub fn decode_universal(bytes: &[u8]) -> Result<crate::polyglot::UniversalSymbol, DecodeError> {
    let mut reader = Reader::new(bytes);
    if reader.bytes(5)? != MAGIC {
        return Err(reader.fail("not a miter record"));
    }
    let version = reader.u8()?;
    if version != UNIVERSAL_VERSION {
        return Err(reader.fail("unsupported universal record version"));
    }
    let language = universal_language_from_code(reader.u8()?)
        .ok_or_else(|| reader.fail("unknown language code"))?;
    let kind = universal_kind_from_code(reader.u8()?)
        .ok_or_else(|| reader.fail("unknown symbol kind code"))?;
    let exported = reader.u8()? != 0;
    let id = reader.u64()?;
    let depth = reader.u32()? as usize;
    let line = reader.u32()? as usize;
    let span = (reader.u32()? as usize, reader.u32()? as usize);
    let file = PathBuf::from(reader.text()?);
    let name = reader.text()?;
    let path = reader.text()?;
    let signature = reader.text()?;
    let docstring = match reader.u8()? {
        1 => Some(reader.text()?),
        _ => None,
    };
    let return_type = match reader.u8()? {
        1 => Some(reader.text()?),
        _ => None,
    };
    let count = reader.u32()? as usize;
    let mut params = Vec::with_capacity(count.min(64));
    for _ in 0..count {
        let name = reader.text()?;
        let type_name = reader.text()?;
        params.push(miter_ast::Param { name, type_name });
    }

    // The id is derived from `file:line:name`, so it is *recomputed* rather than trusted: a record
    // whose stored id disagrees with its own identity is a damaged record, and rebuilding it keeps
    // a hydration from resolving against a key that no longer matches the fields beside it.
    let symbol = crate::polyglot::UniversalSymbol {
        name,
        language,
        kind,
        path,
        depth,
        docstring,
        signature,
        span,
        file,
        line,
        params,
        return_type,
        exported,
    };
    if symbol.id() != id {
        return Err(DecodeError {
            message: format!("stored id {id} does not match the record's own identity"),
            offset: 9,
        });
    }
    Ok(symbol)
}

/// The `Language` a universal record's byte stands for.
pub(super) const fn universal_language_code(language: crate::polyglot::Language) -> u8 {
    match language {
        crate::polyglot::Language::Rust => 0,
        crate::polyglot::Language::TypeScript => 1,
        crate::polyglot::Language::Tsx => 2,
        crate::polyglot::Language::Python => 3,
        crate::polyglot::Language::JavaScript => 4,
        crate::polyglot::Language::Sql => 5,
        crate::polyglot::Language::Html => 6,
        crate::polyglot::Language::Css => 7,
        crate::polyglot::Language::Markdown => 8,
        crate::polyglot::Language::Dockerfile => 9,
        crate::polyglot::Language::Yaml => 10,
        crate::polyglot::Language::Hcl => 11,
        crate::polyglot::Language::Env => 12,
    }
}

/// The `Language` a universal record's byte denotes, if this build knows it.
pub(super) const fn universal_language_from_code(code: u8) -> Option<crate::polyglot::Language> {
    match code {
        0 => Some(crate::polyglot::Language::Rust),
        1 => Some(crate::polyglot::Language::TypeScript),
        2 => Some(crate::polyglot::Language::Tsx),
        3 => Some(crate::polyglot::Language::Python),
        4 => Some(crate::polyglot::Language::JavaScript),
        5 => Some(crate::polyglot::Language::Sql),
        6 => Some(crate::polyglot::Language::Html),
        7 => Some(crate::polyglot::Language::Css),
        8 => Some(crate::polyglot::Language::Markdown),
        9 => Some(crate::polyglot::Language::Dockerfile),
        10 => Some(crate::polyglot::Language::Yaml),
        11 => Some(crate::polyglot::Language::Hcl),
        12 => Some(crate::polyglot::Language::Env),
        _ => None,
    }
}

/// The `SymbolKind` a universal record's byte stands for.
///
/// Numbered from [`crate::polyglot::SymbolKind::ALL`]'s own order, which is the report order, so
/// adding a kind is a decision about the encoding and not an accident of the match arms.
pub(super) const fn universal_kind_code(kind: crate::polyglot::SymbolKind) -> u8 {
    match kind {
        crate::polyglot::SymbolKind::Module => 0,
        crate::polyglot::SymbolKind::Struct => 1,
        crate::polyglot::SymbolKind::Enum => 2,
        crate::polyglot::SymbolKind::Trait => 3,
        crate::polyglot::SymbolKind::Class => 4,
        crate::polyglot::SymbolKind::Interface => 5,
        crate::polyglot::SymbolKind::TypeAlias => 6,
        crate::polyglot::SymbolKind::Function => 7,
        crate::polyglot::SymbolKind::Method => 8,
        crate::polyglot::SymbolKind::Property => 9,
        crate::polyglot::SymbolKind::Table => 10,
        crate::polyglot::SymbolKind::InfraResource => 11,
        crate::polyglot::SymbolKind::DockerStage => 12,
        crate::polyglot::SymbolKind::K8sManifest => 13,
        crate::polyglot::SymbolKind::Heading => 14,
    }
}

/// The `SymbolKind` a universal record's byte denotes, if this build knows it.
pub const fn universal_kind_from_code(code: u8) -> Option<crate::polyglot::SymbolKind> {
    use crate::polyglot::SymbolKind;
    match code {
        0 => Some(SymbolKind::Module),
        1 => Some(SymbolKind::Struct),
        2 => Some(SymbolKind::Enum),
        3 => Some(SymbolKind::Trait),
        4 => Some(SymbolKind::Class),
        5 => Some(SymbolKind::Interface),
        6 => Some(SymbolKind::TypeAlias),
        7 => Some(SymbolKind::Function),
        8 => Some(SymbolKind::Method),
        9 => Some(SymbolKind::Property),
        10 => Some(SymbolKind::Table),
        11 => Some(SymbolKind::InfraResource),
        12 => Some(SymbolKind::DockerStage),
        13 => Some(SymbolKind::K8sManifest),
        14 => Some(SymbolKind::Heading),
        _ => None,
    }
}
