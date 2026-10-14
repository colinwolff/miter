//! Round-trip and refusal tests for both record shapes.

mod symbol_tests {
    use crate::codec::*;
    use miter_ast::{AstIndexer, FunctionSymbol, Language};

    fn sample() -> FunctionSymbol {
        let indexer = AstIndexer::new();
        let mut symbols = indexer
            .index_source(
                "fn scale(x: f32, factor: f32) -> f32 { x * factor }",
                Language::Rust,
                std::path::Path::new("src/lib.rs"),
                2,
            )
            .expect("parse");
        symbols.remove(0)
    }

    #[test]
    fn round_trip_preserves_every_field() {
        let symbol = sample();
        let bytes = encode_symbol(&symbol);
        assert!(bytes.starts_with(&MAGIC));
        let restored = decode_symbol(&bytes).expect("decode");
        assert_eq!(restored.id, symbol.id);
        assert_eq!(restored.name, symbol.name);
        assert_eq!(restored.path, symbol.path);
        assert_eq!(restored.language, symbol.language);
        assert_eq!(restored.params, symbol.params);
        assert_eq!(restored.return_type, symbol.return_type);
        assert_eq!(restored.dir_depth, symbol.dir_depth);
        assert_eq!(restored.scope_depth, symbol.scope_depth);
        assert_eq!(restored.line, symbol.line);
        assert_eq!(restored.start_byte, symbol.start_byte);
        assert_eq!(restored.end_byte, symbol.end_byte);
    }

    #[test]
    fn records_are_self_describing_and_bounded() {
        let bytes = encode_symbol(&sample());
        // 4 magic + 1 version + 1 language + 8 id + 5*4 numeric + 4 param count + text.
        assert!(
            bytes.len() > 32,
            "record must carry the header and its text"
        );
        assert!(
            decode_symbol(&bytes[..8]).is_err(),
            "truncation is detected"
        );
        let mut corrupted = bytes.clone();
        corrupted[0] = b'X';
        assert!(decode_symbol(&corrupted).is_err(), "bad magic is detected");
        let mut reversioned = bytes;
        reversioned[5] = 99;
        assert!(
            decode_symbol(&reversioned).is_err(),
            "bad version is detected"
        );
    }
}

mod universal_tests {
    use crate::codec::universal::{
        universal_kind_code, universal_language_code, universal_language_from_code,
    };
    use crate::codec::*;
    use crate::polyglot::{Language, SymbolKind, UniversalSymbol, parse};
    use std::path::Path;

    /// A struct with a docstring, two parameters and a return type — every optional field present.
    fn rich() -> UniversalSymbol {
        let source = "\
/// Scales a value.
/// Handles overflow by saturating.
pub struct Scaler {
    factor: f32,
}

impl Scaler {
    /// Applies the factor.
    pub fn apply(&self, value: f32, bias: f32) -> Option<f32> {
        Some(value * self.factor + bias)
    }
}
";
        let symbols = parse(
            Path::new("crates/engine/src/scaler.rs"),
            source,
            Language::Rust,
        )
        .expect("parses");
        symbols
            .into_iter()
            .find(|s| s.kind == SymbolKind::Struct)
            .expect("the struct")
    }

    #[test]
    fn a_universal_record_round_trips_through_every_field() {
        let symbol = rich();
        let decoded = decode_universal(&encode_universal(&symbol)).expect("decodes");
        assert_eq!(decoded, symbol, "every field survives the round trip");
    }

    #[test]
    fn a_record_without_the_optional_fields_still_round_trips() {
        let symbols = parse(
            Path::new("src/lib.rs"),
            "pub struct Bare;\n",
            Language::Rust,
        )
        .expect("parses");
        let symbol = symbols
            .into_iter()
            .find(|s| s.kind == SymbolKind::Struct)
            .expect("struct");
        assert_eq!(symbol.docstring, None);
        assert_eq!(symbol.return_type, None);
        assert!(symbol.params.is_empty());

        let decoded = decode_universal(&encode_universal(&symbol)).expect("decodes");
        assert_eq!(decoded, symbol);
    }

    #[test]
    fn a_method_keeps_its_parameters_and_return_type() {
        let symbols = parse(
            Path::new("crates/engine/src/scaler.rs"),
            "pub struct S;\nimpl S {\n    pub fn apply(&self, value: f32, bias: f32) -> Option<f32> { None }\n}\n",
            Language::Rust,
        )
        .expect("parses");
        let method = symbols
            .into_iter()
            .find(|s| s.kind == SymbolKind::Method)
            .expect("the method");
        assert_eq!(method.params.len(), 3, "including the receiver");
        assert_eq!(method.return_type.as_deref(), Some("Option<f32>"));

        let decoded = decode_universal(&encode_universal(&method)).expect("decodes");
        assert_eq!(decoded, method);
    }

    #[test]
    fn every_kind_and_language_survives_its_code() {
        for (kind, code) in [
            (SymbolKind::Module, 0u8),
            (SymbolKind::Struct, 1),
            (SymbolKind::Enum, 2),
            (SymbolKind::Trait, 3),
            (SymbolKind::Class, 4),
            (SymbolKind::Interface, 5),
            (SymbolKind::TypeAlias, 6),
            (SymbolKind::Function, 7),
            (SymbolKind::Method, 8),
            (SymbolKind::Property, 9),
            (SymbolKind::Table, 10),
            (SymbolKind::InfraResource, 11),
            (SymbolKind::DockerStage, 12),
            (SymbolKind::K8sManifest, 13),
            (SymbolKind::Heading, 14),
        ] {
            assert_eq!(universal_kind_code(kind), code, "{kind:?}");
            assert_eq!(universal_kind_from_code(code), Some(kind), "{code}");
        }
        assert_eq!(
            universal_kind_from_code(15),
            None,
            "an unknown code is refused"
        );

        for language in Language::ALL {
            let code = universal_language_code(language);
            assert_eq!(
                universal_language_from_code(code),
                Some(language),
                "{language:?}"
            );
        }
        assert_eq!(universal_language_from_code(13), None);
    }

    #[test]
    fn a_foreign_or_damaged_record_is_refused_rather_than_misread() {
        let bytes = encode_universal(&rich());

        assert!(decode_universal(b"NOTMITER-record").is_err(), "wrong magic");

        let mut wrong_version = bytes.clone();
        wrong_version[5] = UNIVERSAL_VERSION + 1;
        assert!(decode_universal(&wrong_version).is_err(), "wrong version");

        for cut in [1usize, 8, 20, 40] {
            assert!(
                decode_universal(&bytes[..bytes.len().saturating_sub(cut)]).is_err(),
                "truncated by {cut}"
            );
        }

        // A record whose id no longer matches its own `file:line:name` is damaged, and is
        // refused rather than read against a key that disagrees with the fields beside it. The
        // layout is magic(0..5), version(5), language(6), kind(7), exported(8), id(9..17).
        let mut tampered = bytes.clone();
        tampered[9..17].copy_from_slice(&0xDEAD_BEEFu64.to_le_bytes());
        let error = decode_universal(&tampered).expect_err("a mismatched id is refused");
        assert!(error.to_string().contains("identity"), "{error}");

        // A kind byte this build does not know is refused rather than defaulted: defaulting it
        // would silently turn a `struct` into a function.
        let mut unknown_kind = bytes.clone();
        unknown_kind[7] = 200;
        assert!(
            decode_universal(&unknown_kind).is_err(),
            "an unknown kind is refused"
        );
    }
}
