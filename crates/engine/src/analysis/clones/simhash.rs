//! Token collection and the 128-bit SimHash built from it.

use tree_sitter::Node;

/// Extracts structural control-flow tokens from an AST node recursively.
pub fn collect_structural_tokens(node: Node<'_>, depth: usize, tokens: &mut Vec<(String, usize)>) {
    let kind = node.kind();
    // Record structural control flow and statement constructs
    if matches!(
        kind,
        "if_expression"
            | "else_clause"
            | "match_expression"
            | "match_arm"
            | "for_expression"
            | "while_expression"
            | "loop_expression"
            | "try_expression"
            | "return_expression"
            | "break_expression"
            | "continue_expression"
            | "binary_expression"
            | "unary_expression"
            | "call_expression"
            | "field_expression"
            | "let_declaration"
            | "assignment_expression"
            | "compound_assignment_expr"
            | "statement_block"
            | "block"
            | "closure_expression"
    ) {
        tokens.push((kind.to_string(), depth));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_structural_tokens(child, depth + 1, tokens);
    }
}

/// Deterministic FNV-1a 64-bit hash.
#[inline]
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Computes a 128-bit SimHash bitvector from structural tokens.
#[must_use]
pub fn compute_simhash(tokens: &[(String, usize)]) -> (u64, u64) {
    if tokens.is_empty() {
        return (0, 0);
    }

    let mut v = [0i32; 128];

    for (token, depth) in tokens {
        let h1 = fnv1a64(format!("{token}:{depth}").as_bytes());
        let h2 = fnv1a64(format!("{depth}#{token}").as_bytes());

        for i in 0..64 {
            if (h1 >> i) & 1 == 1 {
                v[i] += 1;
            } else {
                v[i] -= 1;
            }
            if (h2 >> i) & 1 == 1 {
                v[64 + i] += 1;
            } else {
                v[64 + i] -= 1;
            }
        }
    }

    let mut high = 0u64;
    let mut low = 0u64;

    for i in 0..64 {
        if v[i] > 0 {
            low |= 1u64 << i;
        }
        if v[64 + i] > 0 {
            high |= 1u64 << i;
        }
    }

    (high, low)
}
