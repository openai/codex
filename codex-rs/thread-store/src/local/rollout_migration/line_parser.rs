//! Makes historical rollout JSON parse like today's protocol types.
//!
//! Legacy rollouts span a bunch of old wire shapes: renamed fields, old sandbox policy layouts,
//! string timestamps, legacy command cwd paths, retired events, and payloads that only deserialize
//! correctly after going through `serde_json::Value`.
//!
//! This module only applies narrowly scoped compatibility rewrites for shapes we know existed in
//! persisted rollouts. It does not decide turn boundaries, rollback behavior, or what gets written
//! into paginated history.

use chrono::DateTime;
use codex_rollout::RolloutLine;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use serde_json::Map;
use serde_json::Value;

/// Parse one legacy rollout line after applying narrowly scoped compatibility
/// rewrites for historical wire shapes that current protocol types no longer
/// accept.
pub(super) fn parse_legacy_rollout_line(bytes: &[u8]) -> Result<Option<RolloutLine>, String> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }

    // Deserializing through Value is intentional. Some historical numeric
    // payloads fail serde_json's streaming enum path but deserialize correctly
    // once their tagged object shape has been materialized.
    let value = serde_json::from_slice::<Value>(bytes).map_err(|error| error.to_string())?;
    parse_legacy_rollout_value(value)
}

pub(super) fn parse_legacy_rollout_value(mut value: Value) -> Result<Option<RolloutLine>, String> {
    if should_skip_retired_record(&value) {
        return Ok(None);
    }
    normalize_legacy_turn_context(&mut value);
    normalize_legacy_sandbox_policy(&mut value);
    normalize_legacy_rate_limit_resets(&mut value);
    normalize_legacy_review_entry(&mut value);
    normalize_legacy_command_cwd(&mut value)?;
    serde_json::from_value(value)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn should_skip_retired_record(value: &Value) -> bool {
    matches!(
        event_type(value),
        Some("guardian_assessment" | "thread_name_updated" | "undo_completed")
    ) || (rollout_type(value) == Some("response_item")
        && value
            .get("payload")
            .and_then(Value::as_object)
            .and_then(|payload| payload.get("type"))
            .and_then(Value::as_str)
            == Some("ghost_snapshot"))
}

fn normalize_legacy_turn_context(value: &mut Value) {
    if rollout_type(value) != Some("turn_context") {
        return;
    }
    let Some(payload) = payload_object_mut(value) else {
        return;
    };
    let Some(collaboration_mode) = payload
        .get_mut("collaboration_mode")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if collaboration_mode.contains_key("settings") {
        return;
    }

    let mut settings = Map::new();
    for key in ["model", "reasoning_effort", "developer_instructions"] {
        if let Some(value) = collaboration_mode.get(key) {
            settings.insert(key.to_string(), value.clone());
        }
    }
    if !settings.is_empty() {
        collaboration_mode.insert("settings".to_string(), Value::Object(settings));
    }
}

fn normalize_legacy_review_entry(value: &mut Value) {
    if event_type(value) != Some("entered_review_mode") {
        return;
    }
    let Some(payload) = payload_object_mut(value) else {
        return;
    };
    if payload.contains_key("target") {
        return;
    }
    let Some(prompt) = payload.get("prompt").and_then(Value::as_str) else {
        return;
    };
    payload.insert(
        "target".to_string(),
        serde_json::json!({
            "type": "custom",
            "instructions": prompt,
        }),
    );
}

fn normalize_legacy_rate_limit_resets(value: &mut Value) {
    if event_type(value) != Some("token_count") {
        return;
    }
    let Some(rate_limits) = payload_object_mut(value)
        .and_then(|payload| payload.get_mut("rate_limits"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    for window_name in ["primary", "secondary"] {
        let Some(window) = rate_limits
            .get_mut(window_name)
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        let Some(resets_at) = window.get("resets_at").and_then(Value::as_str) else {
            continue;
        };
        let Ok(resets_at) = DateTime::parse_from_rfc3339(resets_at) else {
            continue;
        };
        window.insert("resets_at".to_string(), Value::from(resets_at.timestamp()));
    }
}

fn normalize_legacy_sandbox_policy(value: &mut Value) {
    if rollout_type(value) != Some("turn_context") {
        return;
    }
    let Some(sandbox_policy) = payload_object_mut(value)
        .and_then(|payload| payload.get_mut("sandbox_policy"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if sandbox_policy.contains_key("type") {
        return;
    }
    if let Some(mode) = sandbox_policy.get("mode").cloned() {
        sandbox_policy.insert("type".to_string(), mode);
    }
}

fn normalize_legacy_command_cwd(value: &mut Value) -> Result<(), String> {
    if !matches!(
        event_type(value),
        Some("exec_command_begin" | "exec_command_end")
    ) {
        return Ok(());
    }
    let Some(payload) = payload_object_mut(value) else {
        return Ok(());
    };
    let Some(cwd) = payload.get("cwd").and_then(Value::as_str) else {
        return Ok(());
    };
    if cwd.starts_with("file:") {
        return Ok(());
    }
    let cwd = PathUri::try_from(LegacyAppPathString::from_string(cwd))
        .map_err(|error| format!("invalid legacy command cwd: {error}"))?;
    payload.insert(
        "cwd".to_string(),
        serde_json::to_value(cwd).map_err(|error| error.to_string())?,
    );
    Ok(())
}

fn rollout_type(value: &Value) -> Option<&str> {
    value.get("type").and_then(Value::as_str)
}

fn event_type(value: &Value) -> Option<&str> {
    if rollout_type(value) != Some("event_msg") {
        return None;
    }
    value
        .get("payload")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get("type"))
        .and_then(Value::as_str)
}

fn payload_object_mut(value: &mut Value) -> Option<&mut Map<String, Value>> {
    value.get_mut("payload").and_then(Value::as_object_mut)
}

#[cfg(test)]
#[path = "line_parser_tests.rs"]
mod tests;
