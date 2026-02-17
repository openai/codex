use crate::config::AgentRoleConfig;
use crate::config::Config;
use crate::config::ConfigOverrides;
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
    deserialize_config_toml_with_base(role_config_toml.clone(), role_config_base)
        .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;
    let role_layer_toml = resolve_relative_paths_in_config_toml(role_config_toml, role_config_base)
        .map_err(|_| AGENT_TYPE_UNAVAILABLE_ERROR.to_string())?;

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

    /// Returns the cached built-in role declarations defined in this module.
    pub(super) fn configs() -> &'static BTreeMap<String, AgentRoleConfig> {
        static CONFIG: LazyLock<BTreeMap<String, AgentRoleConfig>> = LazyLock::new(|| {
            BTreeMap::from([
                (
                    DEFAULT_ROLE_NAME.to_string(),
                    AgentRoleConfig {
                        description: Some("Default agent.".to_string()),
                        config_file: None,
                    }
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
    use super::*;
    use crate::config::ConfigBuilder;
    use crate::config_loader::ConfigLayerStackOrdering;
    use codex_protocol::openai_models::ReasoningEffort;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tempfile::TempDir;

    async fn test_config_with_cli_overrides(
        cli_overrides: Vec<(String, TomlValue)>,
    ) -> (TempDir, Config) {
        let home = TempDir::new().expect("create temp dir");
        let home_path = home.path().to_path_buf();
        let config = ConfigBuilder::default()
            .codex_home(home_path.clone())
            .cli_overrides(cli_overrides)
            .fallback_cwd(Some(home_path))
            .build()
            .await
            .expect("load test config");
        (home, config)
    }

    async fn write_role_config(home: &TempDir, name: &str, contents: &str) -> PathBuf {
        let role_path = home.path().join(name);
        tokio::fs::write(&role_path, contents)
            .await
            .expect("write role config");
        role_path
    }

    fn session_flags_layer_count(config: &Config) -> usize {
        config
            .config_layer_stack
            .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true)
            .into_iter()
            .filter(|layer| layer.name == ConfigLayerSource::SessionFlags)
            .count()
    }

    #[tokio::test]
    async fn apply_role_defaults_to_default_and_leaves_config_unchanged() {
        let (_home, mut config) = test_config_with_cli_overrides(Vec::new()).await;
        let before = config.clone();

        apply_role_to_config(&mut config, None)
            .await
            .expect("default role should apply");

        assert_eq!(before, config);
    }

    #[tokio::test]
    async fn apply_role_returns_error_for_unknown_role() {
        let (_home, mut config) = test_config_with_cli_overrides(Vec::new()).await;

        let err = apply_role_to_config(&mut config, Some("missing-role"))
            .await
            .expect_err("unknown role should fail");

        assert_eq!(err, "unknown agent_type 'missing-role'");
    }

    #[tokio::test]
    async fn apply_explorer_role_sets_model_and_adds_session_flags_layer() {
        let (_home, mut config) = test_config_with_cli_overrides(Vec::new()).await;
        let before_layers = session_flags_layer_count(&config);

        apply_role_to_config(&mut config, Some("explorer"))
            .await
            .expect("explorer role should apply");

        assert_eq!(config.model.as_deref(), Some("gpt-5.1-codex-mini"));
        assert_eq!(config.model_reasoning_effort, Some(ReasoningEffort::Medium));
        assert_eq!(session_flags_layer_count(&config), before_layers + 1);
    }

    #[tokio::test]
    async fn apply_role_returns_unavailable_for_missing_user_role_file() {
        let (_home, mut config) = test_config_with_cli_overrides(Vec::new()).await;
        config.agent_roles.insert(
            "custom".to_string(),
            AgentRoleConfig {
                description: None,
                config_file: Some(PathBuf::from("/path/does/not/exist.toml")),
            },
        );

        let err = apply_role_to_config(&mut config, Some("custom"))
            .await
            .expect_err("missing role file should fail");

        assert_eq!(err, AGENT_TYPE_UNAVAILABLE_ERROR);
    }

    #[tokio::test]
    async fn apply_role_returns_unavailable_for_invalid_user_role_toml() {
        let (home, mut config) = test_config_with_cli_overrides(Vec::new()).await;
        let role_path = write_role_config(&home, "invalid-role.toml", "model = [").await;
        config.agent_roles.insert(
            "custom".to_string(),
            AgentRoleConfig {
                description: None,
                config_file: Some(role_path),
            },
        );

        let err = apply_role_to_config(&mut config, Some("custom"))
            .await
            .expect_err("invalid role file should fail");

        assert_eq!(err, AGENT_TYPE_UNAVAILABLE_ERROR);
    }

    #[tokio::test]
    async fn apply_role_preserves_unspecified_keys() {
        let (home, mut config) = test_config_with_cli_overrides(vec![(
            "model".to_string(),
            TomlValue::String("base-model".to_string()),
        )])
        .await;
        let role_path = write_role_config(
            &home,
            "effort-only.toml",
            "model_reasoning_effort = \"high\"",
        )
        .await;
        config.agent_roles.insert(
            "custom".to_string(),
            AgentRoleConfig {
                description: None,
                config_file: Some(role_path),
            },
        );

        apply_role_to_config(&mut config, Some("custom"))
            .await
            .expect("custom role should apply");

        assert_eq!(config.model.as_deref(), Some("base-model"));
        assert_eq!(config.model_reasoning_effort, Some(ReasoningEffort::High));
    }

    #[tokio::test]
    #[cfg(not(windows))]
    async fn apply_role_does_not_materialize_default_sandbox_workspace_write_fields() {
        use codex_protocol::protocol::SandboxPolicy;
        let (home, mut config) = test_config_with_cli_overrides(vec![
            (
                "sandbox_mode".to_string(),
                TomlValue::String("workspace-write".to_string()),
            ),
            (
                "sandbox_workspace_write.network_access".to_string(),
                TomlValue::Boolean(true),
            ),
        ])
        .await;
        let role_path = write_role_config(
            &home,
            "sandbox-role.toml",
            r#"[sandbox_workspace_write]
writable_roots = ["./sandbox-root"]
"#,
        )
        .await;
        config.agent_roles.insert(
            "custom".to_string(),
            AgentRoleConfig {
                description: None,
                config_file: Some(role_path),
            },
        );

        apply_role_to_config(&mut config, Some("custom"))
            .await
            .expect("custom role should apply");

        let role_layer = config
            .config_layer_stack
            .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true)
            .into_iter()
            .rfind(|layer| layer.name == ConfigLayerSource::SessionFlags)
            .expect("expected a session flags layer");
        let sandbox_workspace_write = role_layer
            .config
            .get("sandbox_workspace_write")
            .and_then(TomlValue::as_table)
            .expect("role layer should include sandbox_workspace_write");
        assert_eq!(
            sandbox_workspace_write.contains_key("network_access"),
            false
        );
        assert_eq!(
            sandbox_workspace_write.contains_key("exclude_tmpdir_env_var"),
            false
        );
        assert_eq!(
            sandbox_workspace_write.contains_key("exclude_slash_tmp"),
            false
        );

        match &*config.permissions.sandbox_policy {
            SandboxPolicy::WorkspaceWrite { network_access, .. } => {
                assert_eq!(*network_access, true);
            }
            other => panic!("expected workspace-write sandbox policy, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn apply_role_takes_precedence_over_existing_session_flags_for_same_key() {
        let (home, mut config) = test_config_with_cli_overrides(vec![(
            "model".to_string(),
            TomlValue::String("cli-model".to_string()),
        )])
        .await;
        let before_layers = session_flags_layer_count(&config);
        let role_path = write_role_config(&home, "model-role.toml", "model = \"role-model\"").await;
        config.agent_roles.insert(
            "custom".to_string(),
            AgentRoleConfig {
                description: None,
                config_file: Some(role_path),
            },
        );

        apply_role_to_config(&mut config, Some("custom"))
            .await
            .expect("custom role should apply");

        assert_eq!(config.model.as_deref(), Some("role-model"));
        assert_eq!(session_flags_layer_count(&config), before_layers + 1);
    }

    #[test]
    fn spawn_tool_spec_build_deduplicates_user_defined_built_in_roles() {
        let user_defined_roles = BTreeMap::from([
            (
                "explorer".to_string(),
                AgentRoleConfig {
                    description: Some("user override".to_string()),
                    config_file: None,
                },
            ),
            ("researcher".to_string(), AgentRoleConfig::default()),
        ]);

        let spec = spawn_tool_spec::build(&user_defined_roles);

        assert!(spec.contains("researcher: no description"));
        assert!(spec.contains("explorer: {\nuser override\n}"));
        assert!(spec.contains("default: {\nDefault agent.\n}"));
        assert!(!spec.contains("Explorers are fast and authoritative."));
    }

    #[test]
    fn spawn_tool_spec_lists_user_defined_roles_before_built_ins() {
        let user_defined_roles = BTreeMap::from([(
            "aaa".to_string(),
            AgentRoleConfig {
                description: Some("first".to_string()),
                config_file: None,
            },
        )]);

        let spec = spawn_tool_spec::build(&user_defined_roles);
        let user_index = spec.find("aaa: {\nfirst\n}").expect("find user role");
        let built_in_index = spec
            .find("default: {\nDefault agent.\n}")
            .expect("find built-in role");

        assert!(user_index < built_in_index);
    }

    #[test]
    fn built_in_config_file_contents_resolves_explorer_only() {
        assert_eq!(
            built_in::config_file_contents(Path::new("explorer.toml")),
            Some(BUILT_IN_EXPLORER_CONFIG)
        );
        assert_eq!(
            built_in::config_file_contents(Path::new("missing.toml")),
            None
        );
    }
}
