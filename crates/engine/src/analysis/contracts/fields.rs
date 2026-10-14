//! Canonical names and the field list read off a declaration.

use crate::polyglot::{SymbolKind, UniversalSymbol};

use super::EntityField;

/// Normalizes an identifier into lowercased alphanumeric words.
pub fn normalize_identifier(ident: &str) -> String {
    let mut clean = String::new();
    let mut prev_is_lower = false;
    for ch in ident.chars() {
        if ch == '_' || ch == '-' {
            clean.push('_');
            prev_is_lower = false;
        } else if ch.is_uppercase() {
            if prev_is_lower {
                clean.push('_');
            }
            clean.push(ch.to_ascii_lowercase());
            prev_is_lower = false;
        } else {
            clean.push(ch);
            prev_is_lower = ch.is_alphabetic();
        }
    }
    clean.trim_matches('_').replace("__", "_")
}

/// Normalizes an entity name into canonical singular snake_case.
#[must_use]
/// Derives the canonical entity name for a declaration (e.g. `ResidentLedger` -> `resident_ledger`).
pub fn canonical_entity_name(name: &str) -> String {
    let norm = normalize_identifier(name);
    if norm.len() > 3 && norm.ends_with('s') && !norm.ends_with("ss") {
        norm[..norm.len() - 1].to_string()
    } else {
        norm
    }
}

/// Reads the fields a declaration carries, when it declares an entity.
pub fn extract_fields_from_symbol(sym: &UniversalSymbol) -> Vec<EntityField> {
    let mut fields = Vec::new();
    let sig = sym.signature.trim();

    // 1. SQL Table signature: TABLE <name> (<col1>: <TYPE>, ...)
    if sym.kind == SymbolKind::Table && sig.starts_with("TABLE ") {
        if let Some(open) = sig.find('(')
            && let Some(close) = sig.rfind(')')
        {
            let cols_str = &sig[open + 1..close];
            for part in cols_str.split(',') {
                let trimmed = part.trim();
                if trimmed.starts_with("FK") {
                    continue;
                }
                let col_name = trimmed.split(':').next().unwrap_or("").trim();
                if !col_name.is_empty() {
                    fields.push(EntityField {
                        raw_name: col_name.to_string(),
                        canonical_name: normalize_identifier(col_name),
                        type_hint: trimmed.split(':').nth(1).map(|t| t.trim().to_string()),
                    });
                }
            }
        }
    } else {
        // 2. Struct or Interface fields from signature or params
        for param in &sym.params {
            fields.push(EntityField {
                raw_name: param.name.clone(),
                canonical_name: normalize_identifier(&param.name),
                type_hint: if param.type_name.is_empty() {
                    None
                } else {
                    Some(param.type_name.clone())
                },
            });
        }
    }

    fields
}

/// The fields of a declaration, read from its own record or from the members beneath it.
///
/// A SQL table carries its columns in its signature — one row per column is not a declaration —
/// while a Rust struct's or a TypeScript interface's fields are their own `Property` symbols under
/// the parent's path. Both are read here, so the contract pass sees one shape whichever language
/// wrote the entity.
#[must_use]
pub fn fields_of(symbols: &[UniversalSymbol], sym: &UniversalSymbol) -> Vec<EntityField> {
    let own = extract_fields_from_symbol(sym);
    if !own.is_empty() {
        return own;
    }
    let prefix = format!("{}.", sym.path);
    symbols
        .iter()
        .filter(|child| child.kind == SymbolKind::Property && child.path.starts_with(&prefix))
        .map(|child| EntityField {
            raw_name: child.name.clone(),
            canonical_name: normalize_identifier(&child.name),
            type_hint: child.return_type.clone(),
        })
        .collect()
}
