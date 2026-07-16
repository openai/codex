use super::RewriteProfile;
use super::external_agent_config_dir;
use super::invalid_data_error;
use super::json_u64;
use super::rewrite_hook_command_for_source;
use super::write_hook_migration;
use codex_hooks::HOOK_EVENT_NAMES;
use codex_hooks::HOOK_EVENT_NAMES_WITH_MATCHERS;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

pub fn hooks_migration_description_cla(
    source_external_agent_dir: &Path,
    target_hooks: &Path,
    rewrite_profile: RewriteProfile,
) -> io::Result<Option<String>> {
    if hook_migration_event_names_cla(source_external_agent_dir, target_hooks, rewrite_profile)?
        .is_empty()
    {
        return Ok(None);
    }

    Ok(Some(format!(
        "Migrate hooks from {} to {}",
        source_external_agent_dir.display(),
        target_hooks.display()
    )))
}

pub fn hook_migration_event_names_cla(
    source_external_agent_dir: &Path,
    target_hooks: &Path,
    rewrite_profile: RewriteProfile,
) -> io::Result<Vec<String>> {
    let migration = hook_migration_cla(
        source_external_agent_dir,
        target_hooks.parent(),
        rewrite_profile,
    )?;
    Ok(migration.keys().cloned().collect())
}

pub fn import_hooks_cla(
    source_external_agent_dir: &Path,
    target_hooks: &Path,
    rewrite_profile: RewriteProfile,
) -> io::Result<bool> {
    let Some(parent) = target_hooks.parent() else {
        return Err(invalid_data_error("hooks target path has no parent"));
    };
    let migration = hook_migration_cla(source_external_agent_dir, Some(parent), rewrite_profile)?;
    if migration.is_empty() {
        return Ok(false);
    }

    write_hook_migration(source_external_agent_dir, target_hooks, migration)
}

pub(super) fn hook_migration_cla(
    source_external_agent_dir: &Path,
    target_config_dir: Option<&Path>,
    rewrite_profile: RewriteProfile,
) -> io::Result<serde_json::Map<String, JsonValue>> {
    let mut settings_files = Vec::new();
    let mut disable_all_hooks = None;
    for settings_name in ["settings.json", "settings.local.json"] {
        let settings_file = source_external_agent_dir.join(settings_name);
        if !settings_file.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&settings_file)?;
        let settings: JsonValue = serde_json::from_str(&raw)
            .map_err(|err| invalid_data_error(format!("invalid hooks settings: {err}")))?;
        if let Some(disabled) = settings.get("disableAllHooks").and_then(JsonValue::as_bool) {
            disable_all_hooks = Some(disabled);
        }
        settings_files.push(settings);
    }

    if disable_all_hooks.unwrap_or(false) {
        return Ok(serde_json::Map::new());
    }

    let mut migration = serde_json::Map::new();
    for settings in settings_files {
        append_convertible_hook_groups_cla(
            &settings,
            &mut migration,
            target_config_dir,
            rewrite_profile,
        );
    }

    Ok(migration)
}

pub(super) fn append_convertible_hook_groups_cla(
    settings: &JsonValue,
    hooks_payload: &mut serde_json::Map<String, JsonValue>,
    target_config_dir: Option<&Path>,
    rewrite_profile: RewriteProfile,
) {
    let Some(hooks_config) = settings.get("hooks").and_then(JsonValue::as_object) else {
        return;
    };

    for event_name in HOOK_EVENT_NAMES {
        let Some(groups) = hooks_config.get(event_name).and_then(JsonValue::as_array) else {
            continue;
        };
        for group in groups {
            let Some(group_object) = group.as_object() else {
                continue;
            };
            if group_object.contains_key("if")
                || group_object
                    .keys()
                    .any(|key| !matches!(key.as_str(), "matcher" | "hooks"))
            {
                continue;
            }
            let mut hook_commands = Vec::new();
            if let Some(hooks) = group_object.get("hooks").and_then(JsonValue::as_array) {
                for hook in hooks {
                    let Some(hook_object) = hook.as_object() else {
                        continue;
                    };
                    let hook_type = hook_object
                        .get("type")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("command");
                    if hook_type != "command" {
                        continue;
                    }
                    if hook_object.keys().any(|key| {
                        !matches!(
                            key.as_str(),
                            "type"
                                | "command"
                                | "timeout"
                                | "timeoutSec"
                                | "statusMessage"
                                | "async"
                        )
                    }) {
                        continue;
                    }
                    if hook_object
                        .get("async")
                        .and_then(JsonValue::as_bool)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    if ["asyncRewake", "shell", "once"]
                        .into_iter()
                        .any(|field| hook_object.contains_key(field))
                    {
                        continue;
                    }
                    let Some(command) = hook_object
                        .get("command")
                        .and_then(JsonValue::as_str)
                        .map(str::trim)
                        .filter(|command| !command.is_empty())
                    else {
                        continue;
                    };

                    let mut command_payload = serde_json::Map::new();
                    command_payload
                        .insert("type".to_string(), JsonValue::String("command".to_string()));
                    command_payload.insert(
                        "command".to_string(),
                        JsonValue::String(rewrite_hook_command_cla(command, target_config_dir)),
                    );
                    if let Some(timeout) = hook_object
                        .get("timeout")
                        .or_else(|| hook_object.get("timeoutSec"))
                        .and_then(json_u64)
                    {
                        command_payload.insert(
                            "timeout".to_string(),
                            JsonValue::Number(serde_json::Number::from(timeout)),
                        );
                    }
                    if let Some(status_message) =
                        hook_object.get("statusMessage").and_then(JsonValue::as_str)
                    {
                        command_payload.insert(
                            "statusMessage".to_string(),
                            JsonValue::String(rewrite_profile.rewrite(status_message)),
                        );
                    }
                    hook_commands.push(JsonValue::Object(command_payload));
                }
            }
            if hook_commands.is_empty() {
                continue;
            }

            let mut group_payload = serde_json::Map::new();
            if HOOK_EVENT_NAMES_WITH_MATCHERS.contains(&event_name)
                && let Some(matcher) = group_object.get("matcher").and_then(JsonValue::as_str)
            {
                group_payload.insert(
                    "matcher".to_string(),
                    JsonValue::String(matcher.to_string()),
                );
            }
            group_payload.insert("hooks".to_string(), JsonValue::Array(hook_commands));
            if let Some(groups) = hooks_payload
                .entry(event_name.to_string())
                .or_insert_with(|| JsonValue::Array(Vec::new()))
                .as_array_mut()
            {
                groups.push(JsonValue::Object(group_payload));
            }
        }
    }
}

pub(super) fn rewrite_hook_command_cla(command: &str, target_config_dir: Option<&Path>) -> String {
    let source_external_agent_dir = PathBuf::from(external_agent_config_dir());
    rewrite_hook_command_for_source(command, target_config_dir, &source_external_agent_dir)
}
