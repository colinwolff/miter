//! Argument validation: a `tools/call` params object in, typed arguments out.

use super::{
    AuditArgs, ImpactArgs, LookupArgs, ReferencesArgs, SkeletonArgs, SliceArgs, VerifyArgs,
};

/// Validates and parses arguments for `miter_lookup`.
///
/// # Errors
/// Returns an error message if the `target` (or `symbol`) parameter is missing or invalid.
pub fn parse_lookup_args(params: &Option<serde_json::Value>) -> Result<LookupArgs, String> {
    let Some(obj) = params.as_ref().and_then(serde_json::Value::as_object) else {
        return Err("Missing required argument: 'target' (or 'symbol')".to_owned());
    };

    let target = obj
        .get("target")
        .or_else(|| obj.get("symbol"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing required parameter 'target'".to_owned())?
        .to_owned();

    let scope = obj
        .get("scope")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .unwrap_or("symbol")
        .to_owned();

    let depth = obj
        .get("depth")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(1);

    let context = obj
        .get("context")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);

    let max_tokens = obj
        .get("max_tokens")
        .and_then(serde_json::Value::as_u64)
        .map(|v| (v as usize).clamp(64, 2048))
        .unwrap_or(miter_engine::DEFAULT_MAX_TOKENS);

    Ok(LookupArgs {
        target,
        scope,
        depth,
        context,
        max_tokens,
    })
}

/// Validates and parses arguments for `miter_verify`.
///
/// # Errors
/// Returns an error message if argument types are invalid.
pub fn parse_verify_args(params: &Option<serde_json::Value>) -> Result<VerifyArgs, String> {
    let Some(obj) = params.as_ref().and_then(serde_json::Value::as_object) else {
        return Ok(VerifyArgs {
            path: None,
            sandbox: false,
            task_id: None,
            timeout_ms: None,
        });
    };

    let path = obj
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);

    let sandbox = obj
        .get("sandbox")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let task_id = obj
        .get("task_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);

    let timeout_ms = obj.get("timeout_ms").and_then(serde_json::Value::as_u64);

    Ok(VerifyArgs {
        path,
        sandbox,
        task_id,
        timeout_ms,
    })
}

/// Validates and parses arguments for `miter_audit`.
pub fn parse_audit_args(params: &Option<serde_json::Value>) -> Result<AuditArgs, String> {
    let Some(obj) = params.as_ref().and_then(serde_json::Value::as_object) else {
        return Ok(AuditArgs {
            threshold: None,
            path: None,
        });
    };

    let threshold = obj
        .get("threshold")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as usize);

    let path = obj
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);

    Ok(AuditArgs { threshold, path })
}

/// Validates and parses arguments for `miter_slice`.
pub fn parse_slice_args(params: &Option<serde_json::Value>) -> Result<SliceArgs, String> {
    let Some(obj) = params.as_ref().and_then(serde_json::Value::as_object) else {
        return Err("Missing required argument: 'path'".to_owned());
    };

    let path = obj
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing required parameter 'path'".to_owned())?
        .to_owned();

    let target_lines = obj
        .get("target_lines")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(250);

    Ok(SliceArgs { path, target_lines })
}

/// Validates and parses arguments for `miter_skeleton`.
pub fn parse_skeleton_args(params: &Option<serde_json::Value>) -> Result<SkeletonArgs, String> {
    let Some(obj) = params.as_ref().and_then(serde_json::Value::as_object) else {
        return Err("Missing required argument: 'path'".to_owned());
    };

    let path = obj
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing required parameter 'path'".to_owned())?
        .to_owned();

    Ok(SkeletonArgs { path })
}

/// Validates and parses arguments for `miter_references`.
pub fn parse_references_args(params: &Option<serde_json::Value>) -> Result<ReferencesArgs, String> {
    let Some(obj) = params.as_ref().and_then(serde_json::Value::as_object) else {
        return Err("Missing required argument: 'symbol'".to_owned());
    };

    let symbol = obj
        .get("symbol")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing required parameter 'symbol'".to_owned())?
        .to_owned();

    let limit = obj
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(50);

    Ok(ReferencesArgs { symbol, limit })
}

/// Validates and parses arguments for `miter_impact`.
pub fn parse_impact_args(params: &Option<serde_json::Value>) -> Result<ImpactArgs, String> {
    let Some(obj) = params.as_ref().and_then(serde_json::Value::as_object) else {
        return Err("Missing required argument: 'symbol'".to_owned());
    };

    let symbol = obj
        .get("symbol")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Missing required parameter 'symbol'".to_owned())?
        .to_owned();

    let max_depth = obj
        .get("max_depth")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(4);

    Ok(ImpactArgs { symbol, max_depth })
}
