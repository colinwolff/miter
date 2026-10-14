//! The function-shaped record: one declaration's identity, provenance and parameters.

use std::path::PathBuf;

use miter_ast::{FunctionSymbol, Language};

use super::wire::{Reader, Writer};
use super::{DecodeError, MAGIC, VERSION};

/// Encodes a symbol into a self-contained little-endian record.
pub fn encode_symbol(symbol: &FunctionSymbol) -> Vec<u8> {
    let mut writer = Writer::with_capacity(512);
    writer.bytes(&MAGIC);
    writer.u8(VERSION);
    writer.u8(language_code(symbol.language));
    writer.u64(symbol.id);
    writer.u32(symbol.dir_depth as u32);
    writer.u32(symbol.scope_depth as u32);
    writer.u32(symbol.line as u32);
    writer.u32(symbol.start_byte as u32);
    writer.u32(symbol.end_byte as u32);
    writer.text(&symbol.path.to_string_lossy());
    writer.text(&symbol.name);
    writer.u32(symbol.params.len() as u32);
    for param in &symbol.params {
        writer.text(&param.name);
        writer.text(&param.type_name);
    }
    match &symbol.return_type {
        Some(return_type) => {
            writer.u8(1);
            writer.text(return_type);
        }
        None => writer.u8(0),
    }
    writer.finish()
}

/// Decodes a record produced by [`encode_symbol`].
///
/// # Errors
///
/// [`DecodeError`] for a wrong magic/version, a truncated record, or malformed
/// UTF-8 payloads.
pub fn decode_symbol(bytes: &[u8]) -> Result<FunctionSymbol, DecodeError> {
    let mut reader = Reader::new(bytes);
    if reader.bytes(5)? != MAGIC {
        return Err(reader.fail("bad magic prefix"));
    }
    let version = reader.u8()?;
    if version != VERSION {
        return Err(reader.fail(&format!("unsupported record version {version}")));
    }
    let language =
        language_from_code(reader.u8()?).ok_or_else(|| reader.fail("unknown language code"))?;
    let id = reader.u64()?;
    let dir_depth = reader.u32()? as usize;
    let scope_depth = reader.u32()? as usize;
    let line = reader.u32()? as usize;
    let start_byte = reader.u32()? as usize;
    let end_byte = reader.u32()? as usize;
    let path = PathBuf::from(reader.text()?);
    let name = reader.text()?;

    let param_count = reader.u32()? as usize;
    let mut params = Vec::with_capacity(param_count.min(64));
    for _ in 0..param_count {
        let name = reader.text()?;
        let type_name = reader.text()?;
        params.push(miter_ast::Param { name, type_name });
    }
    let return_type = if reader.u8()? == 1 {
        Some(reader.text()?)
    } else {
        None
    };

    Ok(FunctionSymbol {
        id,
        name,
        path,
        language,
        params,
        return_type,
        dir_depth,
        scope_depth,
        line,
        start_byte,
        end_byte,
    })
}

/// The codec's one-byte language tag.
///
/// The two additions are `2` and `3` were unused, so a record written before this
/// release decodes unchanged, and a record written now is refused by an older build's
/// `language_from_code` (which returns `None` for an unknown tag rather than guessing).
const fn language_code(language: Language) -> u8 {
    match language {
        Language::Rust => 0,
        Language::TypeScript => 1,
        Language::Tsx => 2,
        Language::JavaScript => 3,
        Language::Python => 4,
    }
}

const fn language_from_code(code: u8) -> Option<Language> {
    match code {
        0 => Some(Language::Rust),
        1 => Some(Language::TypeScript),
        2 => Some(Language::Tsx),
        3 => Some(Language::JavaScript),
        4 => Some(Language::Python),
        _ => None,
    }
}
