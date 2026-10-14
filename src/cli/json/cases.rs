#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn escapes_the_mandatory_characters() {
        assert_eq!(escape("quote\"backslash\\"), "\"quote\\\"backslash\\\\\"");
        assert_eq!(escape("line\nbreak\ttab\r"), "\"line\\nbreak\\ttab\\r\"");
        assert_eq!(escape("bell\u{7}"), "\"bell\\u0007\"");
        assert_eq!(escape("\u{8}\u{c}"), "\"\\b\\f\"");
        assert_eq!(escape("café 🚀"), "\"café 🚀\"", "non-ASCII stays UTF-8");
    }

    #[test]
    fn renders_scalars_and_containers() {
        assert_eq!(Json::Null.render_compact(), "null");
        assert_eq!(Json::Bool(true).render_compact(), "true");
        assert_eq!(
            Json::Unsigned(u64::MAX).render_compact(),
            u64::MAX.to_string()
        );
        assert_eq!(Json::fixed(0.9619, 3).render_compact(), "0.962");
        assert_eq!(Json::fixed_f32(-0.5, 2).render_compact(), "-0.50");
        assert_eq!(Json::optional(None).render_compact(), "null");
        assert_eq!(Json::array([]).render_compact(), "[]");
        assert_eq!(Json::object([]).render_compact(), "{}");
    }

    #[test]
    fn pretty_output_is_indented_and_ordered() {
        let value = Json::Object(vec![
            (
                "query".to_string(),
                Json::Object(vec![
                    ("param".to_string(), Json::string("&Token")),
                    ("return".to_string(), Json::string("Result<User>")),
                ]),
            ),
            ("matches".to_string(), Json::Array(vec![])),
        ]);
        assert_eq!(
            value.render(),
            "{\n  \"query\": {\n    \"param\": \"&Token\",\n    \"return\": \"Result<User>\"\n  },\n  \"matches\": []\n}"
        );
        assert_eq!(
            value.render_compact(),
            "{\"query\":{\"param\":\"&Token\",\"return\":\"Result<User>\"},\"matches\":[]}"
        );
    }

    #[test]
    fn nested_arrays_are_rendered_with_indentation() {
        let value = Json::array([
            Json::Unsigned(1),
            Json::array([Json::string("a")]),
            Json::object([("k", Json::Bool(false))]),
        ]);
        assert_eq!(
            value.render(),
            "[\n  1,\n  [\n    \"a\"\n  ],\n  {\n    \"k\": false\n  }\n]"
        );
    }
}
