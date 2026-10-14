//! Rust ingestion: extraction correctness, provenance and depth.
//!
//! The assertions cover the contract: *parse functions, inputs and return types*, with the
//! provenance (file, line, byte span) a caller needs to find them again.

mod rust;
mod tree;

use miter_ast::{AstIndexer, FunctionNode, Language, Param};

pub(super) const SAMPLE: &str = r#"
pub fn scale(x: f32, factor: f32) -> f32 {
    x * factor
}

impl Widget {
    pub fn new(width: u32) -> Self {
        Self { width }
    }

    pub fn area(&self, scale: f32) -> Option<f32> {
        if scale > 0.0 { Some(1.0) } else { None }
    }

    fn consume(self) {}
}

trait Draw {
    fn draw(&self);
}

mod inner {
    pub fn nested(values: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let helper = |x: u8| x + 1;
        fn deeper(flag: bool) -> bool { !flag }
        let _ = helper(1);
        deeper(true);
        Ok(values.to_vec())
    }
}
"#;

pub(super) fn extract(source: &str) -> Vec<FunctionNode> {
    AstIndexer::new()
        .extract(source, Language::Rust)
        .expect("rust source parses")
}

pub(super) fn by_name<'a>(nodes: &'a [FunctionNode], name: &str) -> &'a FunctionNode {
    nodes
        .iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("no function named {name} in {:?}", names(nodes)))
}

pub(super) fn names(nodes: &[FunctionNode]) -> Vec<String> {
    nodes.iter().map(|node| node.name.clone()).collect()
}
