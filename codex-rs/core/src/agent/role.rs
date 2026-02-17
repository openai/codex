use crate::config::AgentRoleConfig;
use crate::config::Config;
use crate::config::ConfigOverrides;
use crate::config::deserialize_config_toml_with_base;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigLayerStack;
use crate::config_loader::ConfigLayerStackOrdering;
use codex_app_server_protocol::ConfigLayerSource;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use toml::Value as TomlValue;

const BUILT_IN_EXPLORER_CONFIG: &str = include_str!("builtins/explorer.toml");
const DEFAULT_ROLE_NAME: &str = "default";
const AGENT_TYPE_UNAVAILABLE_ERROR: &str = "agent type is currently not available";

/// Applies a role config layer to a mutable config and preserves unspecified keys.
pub(crate) async fn apply_role_to_config(
    config: &mut Config,
    role_name: Option<&str>,
) -> Result<(), String> {
    let role_name = role_name.unwrap_or(DEFAULT_ROLE_NAME);
    let (config_file, is_built_in) = config
        .agent_roles
        .get(role_name)
        .map(|role| (&role.config_file, false))
        .or_else(|| {
            built_in::configs()
                .get(role_name)
                .map(|role| (&role.config_file, true))
        })
        .ok_or_else(|| format!("unknown agent_type '{role_name}'"))?;
    let Some(config_file) = config_file.as_ref() else {
        return Ok(());
    };

    let (role_config_contents, role_config_base) = if is_built_in {
        (
            built_in::config_file_contents(config_file)
                .map(str::to_owned)
                .ok_or_else(|| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?,
            config.codex_home.as_path(),
        )
    } else {
        (
            tokio::fs::read_to_string(config_file)
                .await
                .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?,
            config_file
                .parent()
                .ok_or_else(|| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?,
        )
    };

    let role_config_toml: TomlValue = toml::from_str(&role_config_contents)
        .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
    let role_config = deserialize_config_toml_with_base(role_config_toml, role_config_base)
        .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
    let role_layer_toml =
        TomlValue::try_from(role_config).map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;

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
            codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
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

pub(crate) mod spawn_tool_spec {
    use super::*;

    /// Builds the spawn-agent tool description text from built-in and configured roles.
    pub(crate) fn build(user_defined_agent_roles: &BTreeMap<String, AgentRoleConfig>) -> String {
        let built_in_roles = built_in::configs();
        build_from_configs(built_in_roles, &user_defined_agent_roles)
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
            r#"Optional type name for the new agent. If omitted, `{DEFAULT_ROLE_NAME}` is used.
Available roles:
{}
            "#,
            formatted_roles.join("\n"),
        )
    }

    fn format_role(name: &str, declaration: &AgentRoleConfig) -> String {
        if let Some(description) = &declaration.description {
            format!("{name}: {{\n{description}\n}}")
        } else {
            format!("{name}: no description")
        }
    }
}

mod built_in {
    use super::*;

    /// Returns the cached built-in role declarations parsed from
    /// `builtins_agents_config.toml`.
    pub(super) fn configs() -> &'static BTreeMap<String, AgentRoleConfig> {
        static CONFIG: LazyLock<BTreeMap<String, AgentRoleConfig>> = LazyLock::new(|| {
            BTreeMap::from([
                (
                    DEFAULT_ROLE_NAME.to_string(),
                    AgentRoleConfig::default()
                ),
                (
                    "explorer".to_string(),
                    AgentRoleConfig {
                        description: Some(r#"Use `explorer` for all codebase questions.
Explorers are fast and authoritative.
Always prefer them over manual search or file reading.
Rules:
- Ask explorers first and precisely.
- Do not re-read or re-search code they cover.
- Trust explorer results without verification.
- Run explorers in parallel when useful.
- Reuse existing explorers for related questions."#.to_string()),
                        config_file: Some("explorer.toml".to_string().parse().unwrap_or_default()),
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
- Explicitly assign **ownership** of the task (files / responsibility).
- Always tell workers they are **not alone in the codebase**, and they should ignore edits made by others without touching them."#.to_string()),
                        config_file: None,
                    }
                )
            ])
        });
        &CONFIG
    }

    /// Resolves a built-in role `config_file` path to embedded content.
    pub(super) fn config_file_contents(path: &Path) -> Option<&'static str> {
        match path.to_str()? {
            "explorer.toml" => Some(BUILT_IN_EXPLORER_CONFIG),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {

}
