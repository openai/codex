use crate::config::Config;
use crate::config::ConfigOverrides;
use crate::config::deserialize_config_toml_with_base;
use crate::config::find_codex_home;
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

const BUILT_IN_AGENTS_CONFIG: &str = include_str!("builtins_agents_config.toml");
const BUILT_IN_EXPLORER_CONFIG: &str = include_str!("builtins/explorer.toml");

const AGENTS_CONFIG_FILENAME: &str = "agents_config.toml";
const AGENTS_CONFIG_SCHEMA_VERSION: u32 = 1;
const DEFAULT_ROLE_NAME: &str = "default";
const AGENT_TYPE_UNAVAILABLE_ERROR: &str = "agent type is currently not available";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentsConfigToml {
    version: Option<u32>,
    #[serde(default)]
    agents: BTreeMap<String, AgentDeclarationToml>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentDeclarationToml {
    /// Human-facing role documentation used in spawn tool guidance.
    description: Option<String>,
    /// Path to a role-specific config layer.
    config_file: Option<PathBuf>,
}

/// Applies a role config layer to a mutable config and preserves unspecified keys.
pub(crate) async fn apply_role_to_config(
    config: &mut Config,
    role_name: Option<&str>,
) -> Result<(), String> {
    let role_name = role_name.unwrap_or(DEFAULT_ROLE_NAME);
    let built_in_agents_config = built_in::configs();
    let user_agents_config =
        user_defined::config(config.codex_home.as_path()).unwrap_or_else(|err| {
            tracing::warn!(
                agent_type = role_name,
                error = %err,
                "failed to load user-defined agents config; falling back to built-in roles"
            );
            AgentsConfigToml::default()
        });

    let agent_config = if let Some(role) = user_agents_config.agents.get(role_name) {
        if let Some(config_file) = &role.config_file {
            let content = tokio::fs::read_to_string(config_file)
                .await
                .map_err(|err| {
                    tracing::warn!("failed to read user-defined role config_file: {err:?}");
                    AGENT_TYPE_UNAVAILABLE_ERROR.to_string()
                })?;
            let parsed: TomlValue = toml::from_str(&content).map_err(|err| {
                tracing::warn!("failed to read user-defined role config_file: {err:?}");
                AGENT_TYPE_UNAVAILABLE_ERROR.to_string()
            })?;
            Some(parsed)
        } else {
            None
        }
    } else if let Some(role) = built_in_agents_config.agents.get(role_name) {
        if let Some(config_file) = &role.config_file {
            let content = built_in::config_file(config_file).ok_or_else(|| {
                tracing::warn!("failed to read user-defined role config_file.");
                AGENT_TYPE_UNAVAILABLE_ERROR.to_string()
            })?;
            let parsed: TomlValue = toml::from_str(content).map_err(|err| {
                tracing::warn!("failed to read user-defined role config_file: {err:?}");
                AGENT_TYPE_UNAVAILABLE_ERROR.to_string()
            })?;
            Some(parsed)
        } else {
            None
        }
    } else {
        return Err(format!("unknown agent_type '{role_name}'"));
    };

    let Some(agent_config) = agent_config else {
        return Ok(());
    };

    let original = config.clone();
    let original_stack = &original.config_layer_stack;
    let mut layers = original
        .config_layer_stack
        .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();

    let role_layer = ConfigLayerEntry::new(ConfigLayerSource::SessionFlags, agent_config);
    let role_layer_precedence = role_layer.name.precedence();
    let role_layer_index =
        layers.partition_point(|layer| layer.name.precedence() <= role_layer_precedence);
    layers.insert(role_layer_index, role_layer);
    let layered_stack = ConfigLayerStack::new(
        layers,
        original_stack.requirements().clone(),
        original_stack.requirements_toml().clone(),
    )
    .map_err(|err| {
        tracing::warn!(
            agent_type = role_name,
            error = %err,
            "failed to build layered config stack for role"
        );
        AGENT_TYPE_UNAVAILABLE_ERROR.to_string()
    })?;
    let layered_config =
        deserialize_config_toml_with_base(layered_stack.effective_config(), &original.codex_home)
            .map_err(|err| {
            tracing::warn!(
                agent_type = role_name,
                error = %err,
                "failed to deserialize layered config for role"
            );
            AGENT_TYPE_UNAVAILABLE_ERROR.to_string()
        })?;

    *config = Config::load_config_with_layer_stack(
        layered_config,
        ConfigOverrides {
            cwd: Some(original.cwd.clone()),
            codex_linux_sandbox_exe: original.codex_linux_sandbox_exe.clone(),
            ..Default::default()
        },
        original.codex_home.clone(),
        layered_stack,
    )
    .map_err(|err| {
        tracing::warn!(
            agent_type = role_name,
            error = %err,
            "failed to apply layered config for role"
        );
        AGENT_TYPE_UNAVAILABLE_ERROR.to_string()
    })?;
    Ok(())
}

pub(crate) mod spawn_tool_spec {
    use super::*;

    /// Builds the spawn-agent tool description text from built-in and configured roles.
    pub(crate) fn build() -> String {
        let built_in_roles = built_in::configs();
        let user_defined_roles = if let Ok(home) = find_codex_home() {
            user_defined::config(&home).unwrap_or_default()
        } else {
            Default::default()
        };

        build_from_configs(built_in_roles, &user_defined_roles)
    }

    fn build_from_configs(
        built_in_roles: &AgentsConfigToml,
        user_defined_roles: &AgentsConfigToml,
    ) -> String {
        let mut seen = BTreeSet::new();
        let mut formatted_roles = Vec::new();
        for (name, declaration) in &user_defined_roles.agents {
            if seen.insert(name.as_str()) {
                formatted_roles.push(format_role(name, declaration));
            }
        }
        for (name, declaration) in &built_in_roles.agents {
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

    fn format_role(name: &str, declaration: &AgentDeclarationToml) -> String {
        if let Some(description) = &declaration.description {
            format!("{name}: {{\n{description}\n}}")
        } else {
            format!("{name}: no description")
        }
    }

    #[cfg(test)]
    pub(super) fn build_for_test(
        built_in_roles: &AgentsConfigToml,
        user_defined_roles: &AgentsConfigToml,
    ) -> String {
        build_from_configs(built_in_roles, user_defined_roles)
    }
}

mod built_in {
    use super::*;

    /// Returns the cached built-in role declarations parsed from
    /// `builtins_agents_config.toml`.
    ///
    /// `panic` are safe because of [`tests::built_in_config`] test.
    pub(super) fn configs() -> &'static AgentsConfigToml {
        static CONFIG: LazyLock<AgentsConfigToml> = LazyLock::new(|| {
            let parsed =
                parse_agents_config(BUILT_IN_AGENTS_CONFIG, "embedded built-in agents config")
                    .unwrap_or_else(|err| panic!("invalid embedded built-in agents config: {err}"));
            validate_config(&parsed)
                .unwrap_or_else(|err| panic!("invalid built-in role declarations: {err}"));
            parsed
        });
        &CONFIG
    }

    /// Validates metadata rules for built-in role declarations.
    fn validate_config(agents_config: &AgentsConfigToml) -> Result<(), String> {
        if !agents_config.agents.contains_key(DEFAULT_ROLE_NAME) {
            return Err(format!(
                "built-ins must include the '{DEFAULT_ROLE_NAME}' role"
            ));
        }

        let unknown_embedded_config_files = agents_config
            .agents
            .iter()
            .filter_map(|(name, role)| {
                role.config_file
                    .as_deref()
                    .filter(|cf| config_file(cf).is_none())
                    .map(|_| name.clone())
            })
            .collect::<Vec<_>>();

        if !unknown_embedded_config_files.is_empty() {
            return Err(format!(
                "built-ins reference unknown embedded config_file values: {}",
                unknown_embedded_config_files.join(", ")
            ));
        }

        Ok(())
    }

    /// Resolves a built-in role `config_file` path to embedded content.
    pub(super) fn config_file(path: &Path) -> Option<&'static str> {
        match path.to_str()? {
            "explorer.toml" => Some(BUILT_IN_EXPLORER_CONFIG),
            _ => None,
        }
    }
}

mod user_defined {
    use super::*;

    /// Loads and parses `agents_config.toml` from `codex_home`.
    pub(super) fn config(codex_home: &Path) -> Result<AgentsConfigToml, String> {
        let config_path = codex_home.join(AGENTS_CONFIG_FILENAME);
        let contents = match std::fs::read_to_string(&config_path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(AgentsConfigToml::default());
            }
            Err(err) => {
                return Err(format!("failed to read '{}': {err}", config_path.display()));
            }
        };

        let mut parsed = parse_agents_config(&contents, &config_path.display().to_string())?;
        let config_dir = config_path.parent().ok_or_else(|| {
            format!(
                "failed to resolve parent directory for '{}'",
                config_path.display()
            )
        })?;
        for role in parsed.agents.values_mut() {
            if let Some(config_file) = role.config_file.as_mut()
                && config_file.is_relative()
            {
                *config_file = config_dir.join(&*config_file);
            }
        }
        Ok(parsed)
    }
}

fn parse_agents_config(contents: &str, source: &str) -> Result<AgentsConfigToml, String> {
    let parsed: AgentsConfigToml =
        toml::from_str(contents).map_err(|err| format!("failed to parse '{source}': {err}"))?;
    if let Some(version) = parsed.version
        && version != AGENTS_CONFIG_SCHEMA_VERSION
    {
        return Err(format!(
            "'{source}' has unsupported version {version}; expected {AGENTS_CONFIG_SCHEMA_VERSION}"
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_config;
    use codex_protocol::openai_models::ReasoningEffort;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn built_in_config() {
        // Validate the loading of the built-in configs without panics.
        let _ = built_in::configs();
    }

    /// Writes `agents_config.toml` into the temporary directory.
    fn write_agents_config(dir: &TempDir, body: &str) {
        std::fs::write(dir.path().join(AGENTS_CONFIG_FILENAME), body).expect("write config");
    }

    /// Writes a test role config file under `dir` for use by role tests.
    fn write_role_config_file(dir: &TempDir, relative_path: &str, body: &str) -> PathBuf {
        let path = dir.path().join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create role config parent");
        }
        std::fs::write(&path, body).expect("write role config");
        path
    }

    /// Loads the built-in explorer role and applies its configuration layer.
    #[tokio::test]
    async fn apply_role_to_config_uses_builtin_explorer_config_layer() {
        let mut config = test_config();

        apply_role_to_config(&mut config, Some("explorer"))
            .await
            .expect("apply explorer role");

        assert_eq!(config.model, Some("gpt-5.1-codex-mini".to_string()));
        assert_eq!(config.model_reasoning_effort, Some(ReasoningEffort::Medium));
    }

    #[tokio::test]
    async fn apply_role_to_config_falls_back_to_builtins_when_user_config_is_invalid() {
        let dir = TempDir::new().expect("tempdir");
        write_agents_config(
            &dir,
            r#"
[agents.explorer
description = "broken"
"#,
        );

        let mut config = test_config();
        config.codex_home = dir.path().to_path_buf();

        apply_role_to_config(&mut config, Some("explorer"))
            .await
            .expect("apply explorer role");

        assert_eq!(config.model, Some("gpt-5.1-codex-mini".to_string()));
        assert_eq!(config.model_reasoning_effort, Some(ReasoningEffort::Medium));
    }

    /// Applies a custom user role config loaded from disk.
    #[tokio::test]
    async fn apply_role_to_config_supports_custom_role_config_file() {
        let dir = TempDir::new().expect("tempdir");
        let planner_path = write_role_config_file(
            &dir,
            "agents/planner.toml",
            r#"
model = "gpt-5.1-codex"
sandbox_mode = "read-only"
"#,
        );
        write_agents_config(
            &dir,
            &format!(
                "[agents.planner]\ndescription = \"Planning-focused role.\"\nconfig_file = {planner_path:?}\n"
            ),
        );

        let mut config = test_config();
        config.codex_home = dir.path().to_path_buf();
        apply_role_to_config(&mut config, Some("planner"))
            .await
            .expect("apply planner role");

        assert_eq!(config.model, Some("gpt-5.1-codex".to_string()));
        assert_eq!(
            config.permissions.sandbox_policy.get(),
            &crate::protocol::SandboxPolicy::new_read_only_policy()
        );
    }

    /// Resolves relative config_file paths from the agents_config.toml directory.
    #[tokio::test]
    async fn apply_role_to_config_supports_relative_custom_role_config_file() {
        let dir = TempDir::new().expect("tempdir");
        write_role_config_file(
            &dir,
            "agents/planner.toml",
            r#"
model = "gpt-5.1-codex"
sandbox_mode = "read-only"
"#,
        );
        write_agents_config(
            &dir,
            r#"
[agents.planner]
description = "Planning-focused role."
config_file = "agents/planner.toml"
"#,
        );

        let mut config = test_config();
        config.codex_home = dir.path().to_path_buf();
        apply_role_to_config(&mut config, Some("planner"))
            .await
            .expect("apply planner role");

        assert_eq!(config.model, Some("gpt-5.1-codex".to_string()));
        assert_eq!(
            config.permissions.sandbox_policy.get(),
            &crate::protocol::SandboxPolicy::new_read_only_policy()
        );
    }

    #[tokio::test]
    async fn apply_role_to_config_reports_unknown_agent_type() {
        let mut config = test_config();

        let err = apply_role_to_config(&mut config, Some("missing"))
            .await
            .expect_err("missing role should fail");

        assert_eq!(err, "unknown agent_type 'missing'");
    }

    #[tokio::test]
    async fn apply_role_to_config_reports_unavailable_agent_type() {
        let dir = TempDir::new().expect("tempdir");
        write_agents_config(
            &dir,
            r#"
[agents.planner]
config_file = "agents/does-not-exist.toml"
"#,
        );

        let mut config = test_config();
        config.codex_home = dir.path().to_path_buf();
        let err = apply_role_to_config(&mut config, Some("planner"))
            .await
            .expect_err("missing config file should fail");

        assert_eq!(err, AGENT_TYPE_UNAVAILABLE_ERROR);
    }

    /// Lets a user config file override a built-in role config file.
    #[tokio::test]
    async fn apply_role_to_config_lets_user_override_builtin_config_file() {
        let dir = TempDir::new().expect("tempdir");
        let custom_explorer_path = write_role_config_file(
            &dir,
            "agents/custom_explorer.toml",
            r#"
model = "gpt-5.1-codex"
model_reasoning_effort = "high"
"#,
        );
        write_agents_config(
            &dir,
            &format!("[agents.explorer]\nconfig_file = {custom_explorer_path:?}\n"),
        );

        let mut config = test_config();
        config.codex_home = dir.path().to_path_buf();
        apply_role_to_config(&mut config, Some("explorer"))
            .await
            .expect("apply explorer role");

        assert_eq!(config.model, Some("gpt-5.1-codex".to_string()));
        assert_eq!(config.model_reasoning_effort, Some(ReasoningEffort::High));
    }

    /// Applies MCP server settings from a role config file.
    #[tokio::test]
    async fn apply_role_to_config_applies_mcp_servers_config_file_layer() {
        let dir = TempDir::new().expect("tempdir");
        let tester_path = write_role_config_file(
            &dir,
            "agents/tester.toml",
            r#"
[mcp_servers.docs]
command = "echo"
enabled_tools = ["search"]
"#,
        );
        write_agents_config(
            &dir,
            &format!("[agents.tester]\nconfig_file = {tester_path:?}\n"),
        );

        let mut config = test_config();
        config.codex_home = dir.path().to_path_buf();
        apply_role_to_config(&mut config, Some("tester"))
            .await
            .expect("apply tester role");

        let mcp_servers = config.mcp_servers.get();
        assert_eq!(
            mcp_servers
                .get("docs")
                .and_then(|server| server.enabled_tools.clone()),
            Some(vec!["search".to_string()])
        );
    }

    /// Inserts a role SessionFlags layer in precedence order when legacy managed
    /// layers are already present.
    #[tokio::test]
    async fn apply_role_to_config_keeps_layer_ordering_with_legacy_managed_layers() {
        let mut config = test_config();
        let dir = TempDir::new().expect("tempdir");
        let managed_path = dir.path().join("managed_config.toml");
        std::fs::write(&managed_path, "").expect("write managed config");
        let managed_file = AbsolutePathBuf::try_from(managed_path).expect("managed file");
        config.config_layer_stack = ConfigLayerStack::new(
            vec![ConfigLayerEntry::new(
                ConfigLayerSource::LegacyManagedConfigTomlFromFile { file: managed_file },
                TomlValue::Table(toml::map::Map::new()),
            )],
            config.config_layer_stack.requirements().clone(),
            config.config_layer_stack.requirements_toml().clone(),
        )
        .expect("build initial stack");

        apply_role_to_config(&mut config, Some("explorer"))
            .await
            .expect("apply explorer role");

        let layers = config
            .config_layer_stack
            .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true);
        assert!(matches!(
            layers.first().map(|layer| &layer.name),
            Some(ConfigLayerSource::SessionFlags)
        ));
        assert!(matches!(
            layers.last().map(|layer| &layer.name),
            Some(ConfigLayerSource::LegacyManagedConfigTomlFromFile { .. })
        ));
    }

    #[test]
    fn spawn_tool_spec_build_dedups_and_prefers_user_defined_roles() {
        let built_in_roles = parse_agents_config(
            r#"
[agents.default]
description = "Built-in default."

[agents.explorer]
description = "Built-in explorer."
"#,
            "built-in test roles",
        )
        .expect("parse built-in roles");
        let user_defined_roles = parse_agents_config(
            r#"
[agents.explorer]
description = "User explorer."
"#,
            "user-defined test roles",
        )
        .expect("parse user roles");

        let spec = spawn_tool_spec::build_for_test(&built_in_roles, &user_defined_roles);

        assert_eq!(spec.matches("explorer:").count(), 1);
        assert!(spec.contains("explorer: {\nUser explorer.\n}"));
        assert!(!spec.contains("Built-in explorer."));
    }

    #[test]
    fn spawn_tool_spec_build_lists_user_defined_roles_first() {
        let built_in_roles = parse_agents_config(
            r#"
[agents.default]
description = "Built-in default."

[agents.worker]
description = "Built-in worker."
"#,
            "built-in test roles",
        )
        .expect("parse built-in roles");
        let user_defined_roles = parse_agents_config(
            r#"
[agents.planner]
description = "User planner."
"#,
            "user-defined test roles",
        )
        .expect("parse user roles");

        let spec = spawn_tool_spec::build_for_test(&built_in_roles, &user_defined_roles);

        let planner_pos = spec.find("planner:").expect("planner role is present");
        let default_pos = spec.find("default:").expect("default role is present");
        assert!(planner_pos < default_pos);
    }

    #[test]
    fn spawn_tool_spec_build_formats_missing_description() {
        let built_in_roles = parse_agents_config(
            r#"
[agents.default]
description = "Built-in default."
"#,
            "built-in test roles",
        )
        .expect("parse built-in roles");
        let user_defined_roles = parse_agents_config(
            r#"
[agents.planner]
"#,
            "user-defined test roles",
        )
        .expect("parse user roles");

        let spec = spawn_tool_spec::build_for_test(&built_in_roles, &user_defined_roles);

        assert!(spec.contains("planner: no description"));
    }
}
