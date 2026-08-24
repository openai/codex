//! Applies bounded agent-role overrides to an existing session config.
//!
//! Roles may customize the child or reduce its capabilities, but never replace the parent
//! session's authority. A projected layer keeps existing layer-based consumers in sync.

use crate::config::AgentRoleConfig;
use crate::config::Config;
use crate::config::deserialize_config_toml_with_base;
use anyhow::anyhow;
use codex_agent_roles::parse_agent_role_file_contents;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::SkillsConfig;
use codex_config::loader::resolve_relative_paths_in_config_toml;
use codex_exec_server::read_sensitive_file_to_string;
use codex_features::Feature;
use codex_features::feature_for_key;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::Verbosity;
use codex_protocol::models::BaseInstructionsProvenance;
use codex_protocol::openai_models::ReasoningEffort;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;
use toml::Value as TomlValue;

/// The role name used when a caller omits `agent_type`.
pub const DEFAULT_ROLE_NAME: &str = "default";
const AGENT_TYPE_UNAVAILABLE_ERROR: &str = "agent type is currently not available";

#[derive(Default, Serialize)]
struct AgentRoleOverrides {
    developer_instructions: Option<String>,
    model: Option<String>,
    model_reasoning_effort: Option<ReasoningEffort>,
    model_reasoning_summary: Option<ReasoningSummary>,
    model_verbosity: Option<Verbosity>,
    personality: Option<Personality>,
    service_tier: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    features: BTreeMap<String, bool>,
    skills: Option<SkillsConfig>,
}

/// Applies typed role overrides to the existing parent-derived configuration.
pub(crate) async fn apply_role_to_config(
    config: &mut Config,
    role_name: Option<&str>,
) -> Result<(), String> {
    let role_name = role_name.unwrap_or(DEFAULT_ROLE_NAME);

    let role = resolve_role_config(config, role_name)
        .cloned()
        .ok_or_else(|| format!("unknown agent_type '{role_name}'"))?;

    apply_role_to_config_inner(config, role_name, &role)
        .await
        .map_err(|err| {
            tracing::warn!("failed to apply role to config: {err}");
            AGENT_TYPE_UNAVAILABLE_ERROR.to_string()
        })
}

async fn apply_role_to_config_inner(
    config: &mut Config,
    role_name: &str,
    role: &AgentRoleConfig,
) -> anyhow::Result<()> {
    let is_built_in = !config.agent_roles.contains_key(role_name);
    let Some(config_file) = role.config_file.as_ref() else {
        return Ok(());
    };
    let role_layer_toml = load_role_layer_toml(config, config_file, is_built_in, role_name).await?;
    let role_config = deserialize_config_toml_with_base(role_layer_toml, &config.codex_home)?;
    let mut overrides = AgentRoleOverrides {
        developer_instructions: role_config.developer_instructions,
        model: role_config.model,
        model_reasoning_effort: role_config.model_reasoning_effort,
        model_reasoning_summary: role_config.model_reasoning_summary,
        model_verbosity: role_config.model_verbosity,
        personality: role_config.personality,
        service_tier: role_config.service_tier,
        ..Default::default()
    };

    if let Some(features) = role_config.features {
        for (key, enabled) in features.entries() {
            if !enabled
                && let Some(
                    feature @ (Feature::ShellTool
                    | Feature::Apps
                    | Feature::Personality
                    | Feature::Plugins
                    | Feature::MemoryTool
                    | Feature::RequestPermissionsTool),
                ) = feature_for_key(&key)
            {
                overrides.features.insert(feature.key().to_string(), false);
            }
        }
    }
    if let Some(mut skills) = role_config.skills {
        skills.config.retain(|skill| !skill.enabled);
        skills.bundled = skills.bundled.filter(|bundled| !bundled.enabled);
        skills.include_instructions = skills.include_instructions.filter(|enabled| !enabled);
        skills.max_context_tokens = None;
        if !skills.config.is_empty()
            || skills.bundled.is_some()
            || skills.include_instructions.is_some()
        {
            overrides.skills = Some(skills);
        }
    }

    let role_layer_toml = TomlValue::try_from(&overrides)?;
    if role_layer_toml
        .as_table()
        .is_some_and(toml::map::Map::is_empty)
    {
        return Ok(());
    }
    *config = role_overrides::build_next_config(config, role_layer_toml, &overrides)?;
    Ok(())
}

async fn load_role_layer_toml(
    config: &Config,
    config_file: &Path,
    is_built_in: bool,
    role_name: &str,
) -> anyhow::Result<TomlValue> {
    let (role_config_toml, role_config_base) = if is_built_in {
        let role_config_contents = built_in::config_file_contents(config_file)
            .map(str::to_owned)
            .ok_or(anyhow!("No corresponding config content"))?;
        let role_config_toml: TomlValue = toml::from_str(&role_config_contents)?;
        (role_config_toml, config.codex_home.as_path())
    } else {
        let role_config_contents = read_sensitive_file_to_string(config_file).await?;
        let role_config_base = config_file
            .parent()
            .ok_or(anyhow!("No corresponding config content"))?;
        let role_config_toml = parse_agent_role_file_contents(
            &role_config_contents,
            config_file,
            role_config_base,
            Some(role_name),
        )?
        .config;
        (role_config_toml, role_config_base)
    };

    deserialize_config_toml_with_base(role_config_toml.clone(), role_config_base)?;
    Ok(resolve_relative_paths_in_config_toml(
        role_config_toml,
        role_config_base,
    )?)
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

mod role_overrides {
    use super::*;

    pub(super) fn build_next_config(
        config: &Config,
        role_layer_toml: TomlValue,
        overrides: &AgentRoleOverrides,
    ) -> anyhow::Result<Config> {
        let mut next_config = config.clone();
        next_config.config_layer_stack = build_config_layer_stack(config, &role_layer_toml)?;
        if let Some(model) = &overrides.model {
            next_config.model = Some(model.clone());
        }
        if let Some(instructions) = &overrides.developer_instructions {
            next_config.developer_instructions = Some(instructions.clone());
        }
        if let Some(effort) = overrides.model_reasoning_effort.clone() {
            next_config.model_reasoning_effort = Some(effort);
        }
        if let Some(summary) = overrides.model_reasoning_summary {
            next_config.model_reasoning_summary = Some(summary);
        }
        if let Some(verbosity) = overrides.model_verbosity {
            next_config.model_verbosity = Some(verbosity);
        }
        if let Some(personality) = overrides.personality {
            next_config.personality = Some(personality);
        }
        if let Some(service_tier) = &overrides.service_tier {
            next_config.service_tier = match ServiceTier::from_request_value(service_tier) {
                Some(ServiceTier::Fast) => next_config
                    .features
                    .enabled(Feature::FastMode)
                    .then(|| ServiceTier::Fast.request_value().to_string()),
                Some(ServiceTier::Flex) => Some(ServiceTier::Flex.request_value().to_string()),
                None => Some(service_tier.clone()),
            };
        }
        for key in overrides.features.keys() {
            if let Some(feature) = feature_for_key(key) {
                next_config.features.disable(feature)?;
            }
        }
        if overrides
            .skills
            .as_ref()
            .is_some_and(|skills| skills.include_instructions == Some(false))
        {
            next_config.include_skill_instructions = false;
        }
        let personality_changed = config.personality != next_config.personality
            || config.features.enabled(Feature::Personality)
                != next_config.features.enabled(Feature::Personality);
        if personality_changed
            && matches!(
                config.base_instructions_provenance,
                Some(BaseInstructionsProvenance::Model { .. })
            )
        {
            next_config.base_instructions = None;
            next_config.base_instructions_provenance = None;
        }
        Ok(next_config)
    }

    fn build_config_layer_stack(
        config: &Config,
        role_layer_toml: &TomlValue,
    ) -> anyhow::Result<ConfigLayerStack> {
        let mut layers: Vec<_> = config
            .config_layer_stack
            .all_layers_low_to_high()
            .cloned()
            .collect();
        let role_layer =
            ConfigLayerEntry::new(ConfigLayerSource::SessionFlags, role_layer_toml.clone());
        let insertion_index = layers.partition_point(|layer| layer.name <= role_layer.name);
        layers.insert(insertion_index, role_layer);
        Ok(ConfigLayerStack::new(
            layers,
            config.config_layer_stack.requirements().clone(),
            config.config_layer_stack.requirements_toml().clone(),
        )?
        .with_user_and_project_exec_policy_rules_ignored(
            config
                .config_layer_stack
                .ignore_user_and_project_exec_policy_rules(),
        ))
    }
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

        format!("Available roles:\n{}", formatted_roles.join("\n"))
    }

    fn format_role(name: &str, declaration: &AgentRoleConfig) -> String {
        if let Some(description) = &declaration.description {
            let locked_settings_note = declaration
                .config_file
                .as_ref()
                .and_then(|config_file| {
                    built_in::config_file_contents(config_file)
                        .map(str::to_owned)
                        .or_else(|| std::fs::read_to_string(config_file).ok())
                })
                .and_then(|contents| toml::from_str::<TomlValue>(&contents).ok())
                .map(|role_toml| {
                    let model = role_toml
                        .get("model")
                        .and_then(TomlValue::as_str);
                    let reasoning_effort = role_toml
                        .get("model_reasoning_effort")
                        .and_then(TomlValue::as_str);
                    let service_tier = role_toml
                        .get("service_tier")
                        .and_then(TomlValue::as_str);

                    let model_and_reasoning_note = match (model, reasoning_effort) {
                        (Some(model), Some(reasoning_effort)) => format!(
                            "\n- This role's model is set to `{model}` and its reasoning effort is set to `{reasoning_effort}`. These settings cannot be changed."
                        ),
                        (Some(model), None) => {
                            format!(
                                "\n- This role's model is set to `{model}` and cannot be changed."
                            )
                        }
                        (None, Some(reasoning_effort)) => {
                            format!(
                                "\n- This role's reasoning effort is set to `{reasoning_effort}` and cannot be changed."
                            )
                        }
                        (None, None) => String::new(),
                    };
                    let service_tier_note = service_tier
                        .map(|service_tier| {
                            format!(
                                "\n- This role's service tier is set to `{service_tier}`. If it is supported by the resolved model, it takes precedence over a valid spawn request service tier."
                            )
                        })
                        .unwrap_or_default();
                    format!("{model_and_reasoning_note}{service_tier_note}")
                })
                .unwrap_or_default();
            format!("{name}: {{\n{description}{locked_settings_note}\n}}")
        } else {
            format!("{name}: no description")
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
                        nickname_candidates: None,
                    }
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
- Reuse existing explorers for related questions."#.to_string()),
                        config_file: Some("explorer.toml".to_string().parse().unwrap_or_default()),
                        nickname_candidates: None,
                    }
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
- Always tell workers they are **not alone in the codebase**, and they should not revert the edits made by others, and they should adjust their implementation to accommodate the changes made by others. This is important because there may be multiple workers making changes in parallel, and they need to be aware of each other's work to avoid conflicts and ensure a cohesive final product."#.to_string()),
                        config_file: None,
                        nickname_candidates: None,
                    }
                ),
                // Awaiter is temp removed
//                 (
//                     "awaiter".to_string(),
//                     AgentRoleConfig {
//                         description: Some(r#"Use an `awaiter` agent EVERY TIME you must run a command that will take some very long time.
// This includes, but not only:
// * testing
// * monitoring of a long running process
// * explicit ask to wait for something
//
// Rules:
// - When an awaiter is running, you can work on something else. If you need to wait for its completion, use the largest possible timeout.
// - Be patient with the `awaiter`.
// - Do not use an awaiter for every compilation/test if it won't take time. Only use if for long running commands.
// - Close the awaiter when you're done with it."#.to_string()),
//                         config_file: Some("awaiter.toml".to_string().parse().unwrap_or_default()),
//                     }
//                 )
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
