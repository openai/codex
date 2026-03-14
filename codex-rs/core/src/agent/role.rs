//! Applies agent-role configuration layers on top of an existing session config.
//!
//! Roles are selected at spawn time and are loaded with the same config machinery as
//! `config.toml`. This module resolves built-in and user-defined role files, inserts the role as a
//! high-precedence layer, and preserves the caller's current profile/provider unless the role
//! explicitly takes ownership of model selection. It does not decide when to spawn a sub-agent or
//! which role to use; the multi-agent tool handler owns that orchestration.

use crate::config::AgentRoleConfig;
use crate::config::AgentRoleSpawnMode;
use crate::config::Config;
use crate::config::ConfigOverrides;
use crate::config::agent_roles::parse_agent_role_file_contents;
use crate::config::deserialize_config_toml_with_base;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigLayerStackOrdering;
use crate::config_loader::resolve_relative_paths_in_config_toml;
use codex_app_server_protocol::ConfigLayerSource;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;
use toml::Value as TomlValue;

/// The role name used when a caller omits `agent_type`.
pub const DEFAULT_ROLE_NAME: &str = "default";
const AGENT_TYPE_UNAVAILABLE_ERROR: &str = "agent type is currently not available";

pub(crate) fn default_spawn_mode_for_role(
    config: &Config,
    role_name: Option<&str>,
) -> AgentRoleSpawnMode {
    let role_name = role_name.unwrap_or(DEFAULT_ROLE_NAME);
    resolve_role_config(config, role_name)
        .and_then(|role| role.spawn_mode)
        .unwrap_or_default()
}

/// Applies a named role layer to `config` while preserving caller-owned model selection.
///
/// The role layer is inserted at session-flag precedence so it can override persisted config, but
/// the caller's current `profile` and `model_provider` remain sticky runtime choices unless the
/// role explicitly sets `profile`, explicitly sets `model_provider`, or rewrites the active
/// profile's `model_provider` in place. Rebuilding the config without those overrides would make a
/// spawned agent silently fall back to the default provider, which is the bug this preservation
/// logic avoids.
pub(crate) async fn apply_role_to_config(
    config: &mut Config,
    role_name: Option<&str>,
) -> Result<(), String> {
    let role_name = role_name.unwrap_or(DEFAULT_ROLE_NAME);
    let is_built_in = !config.agent_roles.contains_key(role_name);
    let role = resolve_role_config(config, role_name)
        .ok_or_else(|| format!("unknown agent_type '{role_name}'"))?;
    let Some(config_file) = role.config_file.as_ref() else {
        if let Some(model) = &role.model {
            config.model = Some(model.clone());
        }
        return Ok(());
    };

    let (role_config_toml, role_config_base) = if is_built_in {
        let role_config_contents = built_in::config_file_contents(config_file)
            .map(str::to_owned)
            .ok_or_else(|| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
        let role_config_toml: TomlValue = toml::from_str(&role_config_contents)
            .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
        (role_config_toml, config.codex_home.as_path())
    } else {
        let role_config_contents = tokio::fs::read_to_string(config_file)
            .await
            .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
        let role_config_toml = parse_agent_role_file_contents(
            &role_config_contents,
            config_file,
            config_file
                .parent()
                .ok_or_else(|| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?,
            Some(role_name),
        )
        .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?
        .config;
        (
            role_config_toml,
            config_file
                .parent()
                .ok_or_else(|| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?,
        )
    };
    deserialize_config_toml_with_base(role_config_toml.clone(), role_config_base)
        .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
    let mut role_layer_toml =
        resolve_relative_paths_in_config_toml(role_config_toml, role_config_base)
            .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
    if let Some(model) = &role.model
        && let Some(table) = role_layer_toml.as_table_mut()
    {
        table.insert("model".to_string(), TomlValue::String(model.clone()));
    }
    let role_selects_provider = role_layer_toml.get("model_provider").is_some();
    let role_selects_profile = role_layer_toml.get("profile").is_some();
    let role_updates_active_profile_provider = config
        .active_profile
        .as_ref()
        .and_then(|active_profile| {
            role_layer_toml
                .get("profiles")
                .and_then(TomlValue::as_table)
                .and_then(|profiles| profiles.get(active_profile))
                .and_then(TomlValue::as_table)
                .map(|profile| profile.contains_key("model_provider"))
        })
        .unwrap_or(false);
    // A role that does not explicitly take ownership of model selection should inherit the
    // caller's current profile/provider choices across the config reload.
    let preserve_current_profile = !role_selects_provider && !role_selects_profile;
    let preserve_current_provider =
        preserve_current_profile && !role_updates_active_profile_provider;

    let mut layers: Vec<ConfigLayerEntry> = config
        .config_layer_stack
        .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true)
        .into_iter()
        .cloned()
        .collect();
    let layer = ConfigLayerEntry::new(ConfigLayerSource::SessionFlags, role_layer_toml);
    let insertion_index =
        layers.partition_point(|existing_layer| existing_layer.name <= layer.name);
    layers.insert(insertion_index, layer);

    let config_layer_stack = ConfigLayerStack::new(
        layers,
        config.config_layer_stack.requirements().clone(),
        config.config_layer_stack.requirements_toml().clone(),
    )
    .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;

    let merged_toml = config_layer_stack.effective_config();
    let merged_config = deserialize_config_toml_with_base(merged_toml, &config.codex_home)
        .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
    let next_config = Config::load_config_with_layer_stack(
        merged_config,
        ConfigOverrides {
            cwd: Some(config.cwd.clone()),
            model_provider: preserve_current_provider.then(|| config.model_provider_id.clone()),
            config_profile: preserve_current_profile
                .then(|| config.active_profile.clone())
                .flatten(),
            codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
            main_execve_wrapper_exe: config.main_execve_wrapper_exe.clone(),
            js_repl_node_path: config.js_repl_node_path.clone(),
            ..Default::default()
        },
        config.codex_home.clone(),
        config_layer_stack,
    )
    .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
    *config = next_config;

    Ok(())
}

pub(crate) fn resolve_role_config<'a>(
    config: &'a Config,
    role_name: &str,
) -> Option<&'a AgentRoleConfig> {
    config
        .agent_roles
        .get(role_name)
        .or_else(|| built_in::configs().get(role_name))
}

pub(crate) mod spawn_tool_spec {
    use super::*;

    /// Builds the spawn-agent tool description text from built-in and configured roles.
    pub(crate) fn build(user_defined_agent_roles: &BTreeMap<String, AgentRoleConfig>) -> String {
        let built_in_roles = built_in::configs();
        build_from_configs(built_in_roles, user_defined_agent_roles)
    }

    // This function is not inlined for testing purpose.
    fn build_from_configs(
        built_in_roles: &BTreeMap<String, AgentRoleConfig>,
        user_defined_roles: &BTreeMap<String, AgentRoleConfig>,
    ) -> String {
        let mut seen = BTreeSet::new();
        let mut formatted_roles = Vec::new();
        for (name, declaration) in user_defined_roles {
            if seen.insert(name.as_str()) {
                formatted_roles.push(format_role(name, declaration));
            }
        }
        for (name, declaration) in built_in_roles {
            if seen.insert(name.as_str()) {
                formatted_roles.push(format_role(name, declaration));
            }
        }

        format!(
            "Optional type name for the new agent. If omitted, `{DEFAULT_ROLE_NAME}` is used.\nAvailable roles:\n{}",
            formatted_roles.join("\n"),
        )
    }

    fn format_role(name: &str, declaration: &AgentRoleConfig) -> String {
        let mut body = Vec::new();
        if let Some(description) = &declaration.description {
            body.push(description.clone());
        }
        let locked_settings_note = declaration
            .config_file
            .as_ref()
            .and_then(|config_file| {
                built_in::config_file_contents(config_file)
                    .map(str::to_owned)
                    .or_else(|| std::fs::read_to_string(config_file).ok())
            })
            .and_then(|contents| toml::from_str::<TomlValue>(&contents).ok())
            .and_then(|role_toml| {
                let model = role_toml.get("model").and_then(TomlValue::as_str);
                let reasoning_effort = role_toml
                    .get("model_reasoning_effort")
                    .and_then(TomlValue::as_str);

                match (model, reasoning_effort) {
                    (Some(model), Some(reasoning_effort)) => Some(format!(
                        "- This role's model is set to `{model}` and its reasoning effort is set to `{reasoning_effort}`. These settings cannot be changed."
                    )),
                    (Some(model), None) => Some(format!(
                        "- This role's model is set to `{model}` and cannot be changed."
                    )),
                    (None, Some(reasoning_effort)) => Some(format!(
                        "- This role's reasoning effort is set to `{reasoning_effort}` and cannot be changed."
                    )),
                    (None, None) => None,
                }
            });
        if let Some(locked_settings_note) = locked_settings_note {
            body.push(locked_settings_note);
        }
        if let Some(spawn_mode) = declaration.spawn_mode {
            let default_spawn_mode = match spawn_mode {
                AgentRoleSpawnMode::Spawn => "spawn",
                AgentRoleSpawnMode::Fork => "fork",
            };
            body.push(format!("Default spawn mode: {default_spawn_mode}"));
        }
        if let Some(model) = &declaration.model {
            body.push(format!("Model override: {model}"));
        }
        if body.is_empty() {
            format!("{name}: no description")
        } else {
            format!("{name}: {{\n{}\n}}", body.join("\n"))
        }
    }
}

mod built_in {
    use super::*;

    /// Returns the cached built-in role declarations defined in this module.
    pub(super) fn configs() -> &'static BTreeMap<String, AgentRoleConfig> {
        static CONFIG: LazyLock<BTreeMap<String, AgentRoleConfig>> = LazyLock::new(|| {
            BTreeMap::from([
                (
                    DEFAULT_ROLE_NAME.to_string(),
                    AgentRoleConfig {
                        description: Some("Default agent.".to_string()),
                        config_file: None,
                        model: None,
                        spawn_mode: None,
                        nickname_candidates: None,
                    },
                ),
                (
                    "explorer".to_string(),
                    AgentRoleConfig {
                        description: Some(r#"Use `explorer` for specific codebase questions.
Explorers are fast and authoritative.
They must be used to ask specific, well-scoped questions on the codebase.
Rules:
- In order to avoid redundant work, you should avoid exploring the same problem that explorers have already covered. Typically, you should trust the explorer results without additional verification. You are still allowed to inspect the code yourself to gain the needed context!
- You are encouraged to spawn up multiple explorers in parallel when you have multiple distinct questions to ask about the codebase that can be answered independently. This allows you to get more information faster without waiting for one question to finish before asking the next. While waiting for the explorer results, you can continue working on other local tasks that do not depend on those results. This parallelism is a key advantage of delegation, so use it whenever you have multiple questions to ask.
- Reuse existing explorers for related questions."#
                            .to_string()),
                        model: None,
                        config_file: Some("explorer.toml".to_string().parse().unwrap_or_default()),
                        spawn_mode: None,
                        nickname_candidates: None,
                    },
                ),
                (
                    "fast-worker".to_string(),
                    AgentRoleConfig {
                        description: Some(r#"Use `fast-worker` for tightly constrained problems.
Typical tasks:
- Make a small, localized code change
- Execute a narrowly scoped command sequence
- Handle an isolated fix from a self-contained prompt
Rules:
- Keep scope tight and avoid broad repo exploration.
- Treat the prompt as self-contained and do not assume shared context.
- Prefer direct execution over extended analysis."#
                            .to_string()),
                        config_file: None,
                        model: None,
                        spawn_mode: None,
                        nickname_candidates: None,
                    },
                ),
                (
                    "worker".to_string(),
                    AgentRoleConfig {
                        description: Some(r#"Use for execution and production work.
Typical tasks:
- Implement part of a feature
- Fix tests or bugs
- Split large refactors into independent chunks
Rules:
- Explicitly assign **ownership** of the task (files / responsibility). When the subtask involves code changes, you should clearly specify which files or modules the worker is responsible for. This helps avoid merge conflicts and ensures accountability. For example, you can say "Worker 1 is responsible for updating the authentication module, while Worker 2 will handle the database layer." By defining clear ownership, you can delegate more effectively and reduce coordination overhead.
- Always tell workers they are **not alone in the codebase**, and they should not revert the edits made by others, and they should adjust their implementation to accommodate the changes made by others. This is important because there may be multiple workers making changes in parallel, and they need to be aware of each other's work to avoid conflicts and ensure a cohesive final product."#
                            .to_string()),
                        config_file: None,
                        model: None,
                        spawn_mode: None,
                        nickname_candidates: None,
                    },
                ),
                (
                    "awaiter".to_string(),
                    AgentRoleConfig {
                        description: Some(r#"Use an `awaiter` agent EVERY TIME you must run a command that might take some time.
This includes, but is not limited to:
* testing
* monitoring of a long-running process
* explicit ask to wait for something

Rules:
- When you wait for the `awaiter` to be done, use the largest possible timeout.
- Only use `awaiter` for commands that may take time."#
                            .to_string()),
                        config_file: Some("awaiter.toml".to_string().parse().unwrap_or_default()),
                        model: None,
                        spawn_mode: None,
                        nickname_candidates: None,
                    },
                ),
            ])
        });
        &CONFIG
    }

    /// Resolves a built-in role `config_file` path to embedded content.
    pub(super) fn config_file_contents(path: &Path) -> Option<&'static str> {
        const EXPLORER: &str = include_str!("builtins/explorer.toml");
        const AWAITER: &str = include_str!("builtins/awaiter.toml");
        match path.to_str()? {
            "explorer.toml" => Some(EXPLORER),
            "awaiter.toml" => Some(AWAITER),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "role_tests.rs"]
mod tests;
