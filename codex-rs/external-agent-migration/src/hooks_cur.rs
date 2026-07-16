use super::RewriteProfile;
use super::invalid_data_error;
use super::json_u64;
use super::rewrite_hook_command_for_source;
use super::write_hook_migration;
use codex_hooks::HOOK_EVENT_NAMES_WITH_MATCHERS;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::Path;

pub fn hook_migration_event_names_cur(
    source_external_agent_dir: &Path,
    source_hooks: &Path,
    target_hooks: &Path,
    rewrite_profile: RewriteProfile,
) -> io::Result<Vec<String>> {
    let migration = hook_migration_cur(
        source_external_agent_dir,
        source_hooks,
        target_hooks.parent(),
        rewrite_profile,
    )?;
    Ok(migration.keys().cloned().collect())
}

pub fn import_hooks_cur(
    source_external_agent_dir: &Path,
    source_hooks: &Path,
    target_hooks: &Path,
    rewrite_profile: RewriteProfile,
) -> io::Result<bool> {
    let migration = hook_migration_cur(
        source_external_agent_dir,
        source_hooks,
        target_hooks.parent(),
        rewrite_profile,
    )?;
    write_hook_migration(source_external_agent_dir, target_hooks, migration)
}

fn hook_migration_cur(
    source_external_agent_dir: &Path,
    source_hooks: &Path,
    target_config_dir: Option<&Path>,
    rewrite_profile: RewriteProfile,
) -> io::Result<serde_json::Map<String, JsonValue>> {
    if !source_hooks.is_file() {
        return Ok(serde_json::Map::new());
    }
    let raw = fs::read_to_string(source_hooks)?;
    let settings: JsonValue = serde_json::from_str(&raw)
        .map_err(|err| invalid_data_error(format!("invalid hooks config: {err}")))?;
    let Some(source_hooks) = settings.get("hooks").and_then(JsonValue::as_object) else {
        return Ok(serde_json::Map::new());
    };

    let mut migration = serde_json::Map::new();
    for (source_event_name, handlers) in source_hooks {
        let Some(event_name) = compatible_hook_event_name(source_event_name) else {
            continue;
        };
        let Some(handlers) = handlers.as_array() else {
            continue;
        };
        for handler in handlers {
            let Some(handler) = handler.as_object() else {
                continue;
            };
            // Codex does not currently support a per-hook failure policy, so accept
            // Cursor's `failClosed` field without copying it into the migrated handler.
            if handler.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "command"
                        | "failClosed"
                        | "matcher"
                        | "statusMessage"
                        | "timeout"
                        | "timeoutSec"
                        | "type"
                )
            }) {
                continue;
            }
            let Some(command) = handler
                .get("command")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .filter(|command| !command.is_empty())
            else {
                continue;
            };
            if handler
                .get("type")
                .and_then(JsonValue::as_str)
                .is_some_and(|handler_type| handler_type != "command")
            {
                continue;
            }

            let mut command_payload = serde_json::Map::new();
            command_payload.insert("type".to_string(), JsonValue::String("command".to_string()));
            command_payload.insert(
                "command".to_string(),
                JsonValue::String(rewrite_hook_command_for_source(
                    command,
                    target_config_dir,
                    source_external_agent_dir,
                )),
            );
            if let Some(timeout) = handler
                .get("timeout")
                .or_else(|| handler.get("timeoutSec"))
                .and_then(json_u64)
            {
                command_payload.insert(
                    "timeout".to_string(),
                    JsonValue::Number(serde_json::Number::from(timeout)),
                );
            }
            if let Some(status_message) = handler.get("statusMessage").and_then(JsonValue::as_str) {
                command_payload.insert(
                    "statusMessage".to_string(),
                    JsonValue::String(rewrite_profile.rewrite(status_message)),
                );
            }

            let mut group_payload = serde_json::Map::new();
            if HOOK_EVENT_NAMES_WITH_MATCHERS.contains(&event_name)
                && let Some(matcher) = handler.get("matcher").and_then(JsonValue::as_str)
            {
                group_payload.insert(
                    "matcher".to_string(),
                    JsonValue::String(matcher.to_string()),
                );
            }
            group_payload.insert(
                "hooks".to_string(),
                JsonValue::Array(vec![JsonValue::Object(command_payload)]),
            );
            let groups = migration
                .entry(event_name.to_string())
                .or_insert_with(|| JsonValue::Array(Vec::new()));
            if let Some(groups) = groups.as_array_mut() {
                groups.push(JsonValue::Object(group_payload));
            }
        }
    }
    Ok(migration)
}

fn compatible_hook_event_name(event_name: &str) -> Option<&'static str> {
    match event_name {
        "preToolUse" => Some("PreToolUse"),
        "postToolUse" => Some("PostToolUse"),
        "preCompact" => Some("PreCompact"),
        "postCompact" => Some("PostCompact"),
        "sessionStart" => Some("SessionStart"),
        "subagentStart" => Some("SubagentStart"),
        "subagentStop" => Some("SubagentStop"),
        "beforeSubmitPrompt" => Some("UserPromptSubmit"),
        "stop" => Some("Stop"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "hooks_cur_tests.rs"]
mod tests;
