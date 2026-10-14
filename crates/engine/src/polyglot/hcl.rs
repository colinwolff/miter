//! OpenTofu / Terraform HCL declarative symbol extractor.
//!
//! Parses `.tf` and `.tofu` files using pure-Rust `hcl-rs`.
//! Extracts compact declaration contracts (~60–120 tokens):
//!
//! ```text
//! RESOURCE <type>.<name>
//! MODULE <name> (source: <source>)
//! VARIABLE <name>
//! ```

use super::{Language, SymbolKind, UniversalSymbol, common};
use hcl::Body;
use std::path::Path;

/// Extracts declarative infra symbols from OpenTofu/Terraform HCL source.
pub fn extract_hcl_symbols(path: &Path, content: &str) -> Vec<UniversalSymbol> {
    let body: Body = match hcl::from_str(content) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };

    let module = common::module_path(path);
    let mut symbols = Vec::new();

    for block in body.into_blocks() {
        let identifier: &str = block.identifier();
        let labels: Vec<&str> = block.labels().iter().map(|l| l.as_str()).collect();

        match identifier {
            "resource" => {
                let resource_type = labels.first().copied().unwrap_or("unknown");
                let resource_name = labels.get(1).copied().unwrap_or("unnamed");
                let name = format!("{resource_type}.{resource_name}");
                let signature = format!("RESOURCE {name}");

                symbols.push(UniversalSymbol {
                    path: common::dotted_path(&module, &[], &name),
                    name,
                    language: Language::Rust,
                    kind: SymbolKind::InfraResource,
                    depth: 2, // level 2: scope / resource entity
                    docstring: None,
                    signature,
                    span: (0, content.len()),
                    file: path.to_path_buf(),
                    line: 1,
                    params: Vec::new(),
                    return_type: None,
                    exported: true,
                });
            }
            "module" => {
                let module_name = labels.first().copied().unwrap_or("unnamed").to_string();
                let source_val = block
                    .body()
                    .attributes()
                    .find(|attr| attr.key() == "source")
                    .and_then(|attr| match attr.expr() {
                        hcl::Expression::String(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("");
                let signature = if source_val.is_empty() {
                    format!("MODULE {module_name}")
                } else {
                    format!("MODULE {module_name} (source: {source_val})")
                };

                symbols.push(UniversalSymbol {
                    path: common::dotted_path(&module, &[], &module_name),
                    name: module_name,
                    language: Language::Rust,
                    kind: SymbolKind::InfraResource,
                    depth: 2,
                    docstring: None,
                    signature,
                    span: (0, content.len()),
                    file: path.to_path_buf(),
                    line: 1,
                    params: Vec::new(),
                    return_type: None,
                    exported: true,
                });
            }
            "variable" => {
                let var_name = labels
                    .first()
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "unnamed".to_string());
                let signature = format!("VARIABLE {var_name}");

                symbols.push(UniversalSymbol {
                    path: common::dotted_path(&module, &[], &var_name),
                    name: var_name,
                    language: Language::Rust,
                    kind: SymbolKind::InfraResource,
                    depth: 2,
                    docstring: None,
                    signature,
                    span: (0, content.len()),
                    file: path.to_path_buf(),
                    line: 1,
                    params: Vec::new(),
                    return_type: None,
                    exported: true,
                });
            }
            _ => {}
        }
    }

    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hcl_resource_extraction() {
        let hcl_src = r#"
            variable "environment" {
              type    = string
              default = "production"
            }

            resource "aws_s3_bucket" "litestream" {
              bucket = "my-backup-bucket"
            }

            module "vpc" {
              source = "terraform-aws-modules/vpc/aws"
            }
        "#;

        let symbols = extract_hcl_symbols(Path::new("infra/main.tf"), hcl_src);
        assert_eq!(symbols.len(), 3);

        let var = &symbols[0];
        assert_eq!(var.name, "environment");
        assert_eq!(var.kind, SymbolKind::InfraResource);
        assert_eq!(var.signature, "VARIABLE environment");

        let res = &symbols[1];
        assert_eq!(res.name, "aws_s3_bucket.litestream");
        assert_eq!(res.kind, SymbolKind::InfraResource);
        assert_eq!(res.signature, "RESOURCE aws_s3_bucket.litestream");

        let module = &symbols[2];
        assert_eq!(module.name, "vpc");
        assert_eq!(module.kind, SymbolKind::InfraResource);
        assert_eq!(
            module.signature,
            "MODULE vpc (source: terraform-aws-modules/vpc/aws)"
        );
    }
}
