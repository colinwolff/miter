//! The `miter_verify` tool: run the compiler and slice its diagnostics.

use std::path::Path;

use miter_harness::{CompilerVerifier, SandboxEnvironment, Toolchain, VerificationOutcome};

use crate::protocol::CallToolResult;
use crate::tools::VerifyArgs;

use super::VERIFICATION_PASSED_MESSAGE;

/// Executes the `miter_verify` tool.
///
/// Runs `CompilerVerifier` on the target workspace, optionally within a sandboxed worktree.
/// Returns sliced diagnostic spans for errors with zero noisy build scrollback.
pub async fn handle_verify(workspace_root: &Path, args: VerifyArgs) -> CallToolResult {
    let timeout_ms = args.timeout_ms.unwrap_or(miter_harness::DEFAULT_TIMEOUT_MS);

    let target_dir = match &args.path {
        Some(p) => workspace_root.join(p),
        None => workspace_root.to_path_buf(),
    };

    let verifier = CompilerVerifier::new(Toolchain::Cargo).with_timeout_ms(timeout_ms);

    let mut sandbox = if args.sandbox {
        let task_id = args
            .task_id
            .clone()
            .unwrap_or_else(|| format!("mcp-{}", std::process::id()));
        match SandboxEnvironment::create(&target_dir, &task_id) {
            Ok(sandbox) => Some(sandbox),
            Err(error) => {
                return CallToolResult::error(format!("Harness verification failure: {error}"));
            }
        }
    } else {
        None
    };

    let verdict_root = sandbox.as_ref().map_or_else(
        || target_dir.clone(),
        |sandbox| sandbox.root().to_path_buf(),
    );

    let outcome = match verifier.verify(&verdict_root).await {
        Ok(outcome) => outcome,
        Err(error) => {
            if let Some(sandbox) = sandbox.as_mut() {
                let _ = sandbox.cleanup();
            }
            return CallToolResult::error(format!("Harness verification failure: {error}"));
        }
    };

    if outcome.is_success()
        && let Some(sandbox) = sandbox.as_ref()
    {
        let _ = sandbox.merge_to_head();
    }
    if let Some(sandbox) = sandbox.as_mut() {
        let _ = sandbox.cleanup();
    }

    format_verification_outcome(&outcome, &verdict_root)
}

/// Formats the verification outcome into a clean, concise result.
fn format_verification_outcome(outcome: &VerificationOutcome, root: &Path) -> CallToolResult {
    match outcome {
        VerificationOutcome::Success { .. } => CallToolResult::text(VERIFICATION_PASSED_MESSAGE),
        VerificationOutcome::DiagnosticFailure {
            error_code,
            message,
            spans,
            ..
        } => {
            let mut out = format!("VERIFICATION FAILED: [{error_code}] {message}\n");
            if let Some(span) = spans.first() {
                out.push_str(&format!("Location: {}\n", span.location()));
                if let Some(excerpt) = read_span_excerpt(root, span) {
                    out.push_str(&format!("Excerpt:\n{excerpt}\n"));
                }
            }
            for span in spans.iter().skip(1) {
                out.push_str(&format!("Also at: {}\n", span.location()));
            }
            CallToolResult::error(out.trim_end())
        }
    }
}

/// Slices the source code excerpt corresponding to a DiagnosticSpan.
fn read_span_excerpt(root: &Path, span: &miter_harness::DiagnosticSpan) -> Option<String> {
    let file_path = root.join(&span.file_path);
    let content = std::fs::read_to_string(&file_path).ok()?;

    if span.byte_len() > 0 && span.byte_start < content.len() {
        let start = span.byte_start;
        let end = span.byte_end.min(content.len()).max(start);
        content.get(start..end).map(str::to_owned)
    } else {
        content
            .lines()
            .nth(span.line_start.saturating_sub(1))
            .map(str::to_owned)
    }
}
