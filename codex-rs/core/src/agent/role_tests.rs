use super::*;
use crate::SkillsManager;
use crate::config::CONFIG_TOML_FILE;
use crate::config::ConfigBuilder;
use crate::config_loader::ConfigLayerStackOrdering;
use crate::plugins::PluginsManager;
use crate::skills_load_input_from_config;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Verbosity;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
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
        .get_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ true,
        )
        .into_iter()
        .filter(|layer| layer.name == ConfigLayerSource::SessionFlags)
        .count()
}

#[tokio::test]
async fn apply_role_defaults_to_default_and_leaves_config_unchanged() {
    let (_home, mut config) = test_config_with_cli_overrides(Vec::new()).await;
    let before = config.clone();

    apply_role_to_config(&mut config, /*role_name*/ None)
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
#[ignore = "No role requiring it for now"]
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
            model: None,
            config_file: Some(PathBuf::from("/path/does/not/exist.toml")),
            spawn_mode: None,
            nickname_candidates: None,
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
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    let err = apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect_err("invalid role file should fail");

    assert_eq!(err, AGENT_TYPE_UNAVAILABLE_ERROR);
}

#[tokio::test]
async fn apply_role_ignores_agent_metadata_fields_in_user_role_file() {
    let (home, mut config) = test_config_with_cli_overrides(Vec::new()).await;
    let role_path = write_role_config(
        &home,
        "metadata-role.toml",
        r#"
name = "archivist"
description = "Role metadata"
nickname_candidates = ["Hypatia"]
developer_instructions = "Stay focused"
model = "role-model"
"#,
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    assert_eq!(config.model.as_deref(), Some("role-model"));
}

#[tokio::test]
async fn apply_role_preserves_unspecified_keys() {
    let (home, mut config) = test_config_with_cli_overrides(vec![(
        "model".to_string(),
        TomlValue::String("base-model".to_string()),
    )])
    .await;
    config.codex_linux_sandbox_exe = Some(PathBuf::from("/tmp/codex-linux-sandbox"));
    config.main_execve_wrapper_exe = Some(PathBuf::from("/tmp/codex-execve-wrapper"));
    let role_path = write_role_config(
        &home,
        "effort-only.toml",
        "developer_instructions = \"Stay focused\"\nmodel_reasoning_effort = \"high\"",
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    assert_eq!(config.model.as_deref(), Some("base-model"));
    assert_eq!(config.model_reasoning_effort, Some(ReasoningEffort::High));
    assert_eq!(
        config.codex_linux_sandbox_exe,
        Some(PathBuf::from("/tmp/codex-linux-sandbox"))
    );
    assert_eq!(
        config.main_execve_wrapper_exe,
        Some(PathBuf::from("/tmp/codex-execve-wrapper"))
    );
}

#[tokio::test]
async fn apply_role_preserves_active_profile_and_model_provider() {
    let home = TempDir::new().expect("create temp dir");
    tokio::fs::write(
        home.path().join(CONFIG_TOML_FILE),
        r#"
[model_providers.test-provider]
name = "Test Provider"
base_url = "https://example.com/v1"
env_key = "TEST_PROVIDER_API_KEY"
wire_api = "responses"

[profiles.test-profile]
model_provider = "test-provider"
"#,
    )
    .await
    .expect("write config.toml");
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            config_profile: Some("test-profile".to_string()),
            ..Default::default()
        })
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await
        .expect("load config");
    let role_path = write_role_config(
        &home,
        "empty-role.toml",
        "developer_instructions = \"Stay focused\"",
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    assert_eq!(config.active_profile.as_deref(), Some("test-profile"));
    assert_eq!(config.model_provider_id, "test-provider");
    assert_eq!(config.model_provider.name, "Test Provider");
}

#[tokio::test]
async fn apply_role_top_level_profile_settings_override_preserved_profile() {
    let home = TempDir::new().expect("create temp dir");
    tokio::fs::write(
        home.path().join(CONFIG_TOML_FILE),
        r#"
[profiles.base-profile]
model = "profile-model"
model_reasoning_effort = "low"
model_reasoning_summary = "concise"
model_verbosity = "low"
"#,
    )
    .await
    .expect("write config.toml");
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            config_profile: Some("base-profile".to_string()),
            ..Default::default()
        })
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await
        .expect("load config");
    let role_path = write_role_config(
        &home,
        "top-level-profile-settings-role.toml",
        r#"developer_instructions = "Stay focused"
model = "role-model"
model_reasoning_effort = "high"
model_reasoning_summary = "detailed"
model_verbosity = "high"
"#,
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    assert_eq!(config.active_profile.as_deref(), Some("base-profile"));
    assert_eq!(config.model.as_deref(), Some("role-model"));
    assert_eq!(config.model_reasoning_effort, Some(ReasoningEffort::High));
    assert_eq!(
        config.model_reasoning_summary,
        Some(ReasoningSummary::Detailed)
    );
    assert_eq!(config.model_verbosity, Some(Verbosity::High));
}

#[tokio::test]
async fn apply_role_uses_role_profile_instead_of_current_profile() {
    let home = TempDir::new().expect("create temp dir");
    tokio::fs::write(
        home.path().join(CONFIG_TOML_FILE),
        r#"
[model_providers.base-provider]
name = "Base Provider"
base_url = "https://base.example.com/v1"
env_key = "BASE_PROVIDER_API_KEY"
wire_api = "responses"

[model_providers.role-provider]
name = "Role Provider"
base_url = "https://role.example.com/v1"
env_key = "ROLE_PROVIDER_API_KEY"
wire_api = "responses"

[profiles.base-profile]
model_provider = "base-provider"

[profiles.role-profile]
model_provider = "role-provider"
"#,
    )
    .await
    .expect("write config.toml");
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            config_profile: Some("base-profile".to_string()),
            ..Default::default()
        })
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await
        .expect("load config");
    let role_path = write_role_config(
        &home,
        "profile-role.toml",
        "developer_instructions = \"Stay focused\"\nprofile = \"role-profile\"",
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    assert_eq!(config.active_profile.as_deref(), Some("role-profile"));
    assert_eq!(config.model_provider_id, "role-provider");
    assert_eq!(config.model_provider.name, "Role Provider");
}

#[tokio::test]
async fn apply_role_uses_role_model_provider_instead_of_current_profile_provider() {
    let home = TempDir::new().expect("create temp dir");
    tokio::fs::write(
        home.path().join(CONFIG_TOML_FILE),
        r#"
[model_providers.base-provider]
name = "Base Provider"
base_url = "https://base.example.com/v1"
env_key = "BASE_PROVIDER_API_KEY"
wire_api = "responses"

[model_providers.role-provider]
name = "Role Provider"
base_url = "https://role.example.com/v1"
env_key = "ROLE_PROVIDER_API_KEY"
wire_api = "responses"

[profiles.base-profile]
model_provider = "base-provider"
"#,
    )
    .await
    .expect("write config.toml");
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            config_profile: Some("base-profile".to_string()),
            ..Default::default()
        })
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await
        .expect("load config");
    let role_path = write_role_config(
        &home,
        "provider-role.toml",
        "developer_instructions = \"Stay focused\"\nmodel_provider = \"role-provider\"",
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    assert_eq!(config.active_profile, None);
    assert_eq!(config.model_provider_id, "role-provider");
    assert_eq!(config.model_provider.name, "Role Provider");
}

#[tokio::test]
async fn apply_role_uses_active_profile_model_provider_update() {
    let home = TempDir::new().expect("create temp dir");
    tokio::fs::write(
        home.path().join(CONFIG_TOML_FILE),
        r#"
[model_providers.base-provider]
name = "Base Provider"
base_url = "https://base.example.com/v1"
env_key = "BASE_PROVIDER_API_KEY"
wire_api = "responses"

[model_providers.role-provider]
name = "Role Provider"
base_url = "https://role.example.com/v1"
env_key = "ROLE_PROVIDER_API_KEY"
wire_api = "responses"

[profiles.base-profile]
model_provider = "base-provider"
model_reasoning_effort = "low"
"#,
    )
    .await
    .expect("write config.toml");
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            config_profile: Some("base-profile".to_string()),
            ..Default::default()
        })
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await
        .expect("load config");
    let role_path = write_role_config(
        &home,
        "profile-edit-role.toml",
        r#"developer_instructions = "Stay focused"

[profiles.base-profile]
model_provider = "role-provider"
model_reasoning_effort = "high"
"#,
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    assert_eq!(config.active_profile.as_deref(), Some("base-profile"));
    assert_eq!(config.model_provider_id, "role-provider");
    assert_eq!(config.model_provider.name, "Role Provider");
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
        r#"developer_instructions = "Stay focused"

[sandbox_workspace_write]
writable_roots = ["./sandbox-root"]
"#,
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    let role_layer = config
        .config_layer_stack
        .get_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ true,
        )
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
    let role_path = write_role_config(
        &home,
        "model-role.toml",
        "developer_instructions = \"Stay focused\"\nmodel = \"role-model\"",
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    assert_eq!(config.model.as_deref(), Some("role-model"));
    assert_eq!(session_flags_layer_count(&config), before_layers + 1);
}

#[cfg_attr(windows, ignore)]
#[tokio::test]
async fn apply_role_skills_config_disables_skill_for_spawned_agent() {
    let (home, mut config) = test_config_with_cli_overrides(Vec::new()).await;
    let skill_dir = home.path().join("skills").join("demo");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(
        &skill_path,
        "---\nname: demo-skill\ndescription: demo description\n---\n\n# Body\n",
    )
    .expect("write skill");
    let role_path = write_role_config(
        &home,
        "skills-role.toml",
        &format!(
            r#"developer_instructions = "Stay focused"

[[skills.config]]
path = "{}"
enabled = false
"#,
            skill_path.display()
        ),
    )
    .await;
    config.agent_roles.insert(
        "custom".to_string(),
        AgentRoleConfig {
            description: None,
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    );

    apply_role_to_config(&mut config, Some("custom"))
        .await
        .expect("custom role should apply");

    let plugins_manager = Arc::new(PluginsManager::new(home.path().to_path_buf()));
    let skills_manager = SkillsManager::new(
        home.path().to_path_buf(),
        /*bundled_skills_enabled*/ true,
    );
    let plugin_outcome = plugins_manager.plugins_for_config(&config);
    let effective_skill_roots = plugin_outcome.effective_skill_roots();
    let skills_input = skills_load_input_from_config(&config, effective_skill_roots);
    let outcome = skills_manager.skills_for_config(&skills_input);
    let skill = outcome
        .skills
        .iter()
        .find(|skill| skill.name == "demo-skill")
        .expect("demo skill should be discovered");

    assert_eq!(outcome.is_skill_enabled(skill), false);
}

#[test]
fn spawn_tool_spec_build_deduplicates_user_defined_built_in_roles() {
    let user_defined_roles = BTreeMap::from([
        (
            "explorer".to_string(),
            AgentRoleConfig {
                description: Some("user override".to_string()),
                model: None,
                config_file: None,
                spawn_mode: None,
                nickname_candidates: None,
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
            model: None,
            config_file: None,
            spawn_mode: None,
            nickname_candidates: None,
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
fn spawn_tool_spec_marks_role_locked_model_and_reasoning_effort() {
    let tempdir = TempDir::new().expect("create temp dir");
    let role_path = tempdir.path().join("researcher.toml");
    fs::write(
            &role_path,
            "developer_instructions = \"Research carefully\"\nmodel = \"gpt-5\"\nmodel_reasoning_effort = \"high\"\n",
        )
        .expect("write role config");
    let user_defined_roles = BTreeMap::from([(
        "researcher".to_string(),
        AgentRoleConfig {
            description: Some("Research carefully.".to_string()),
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    )]);

    let spec = spawn_tool_spec::build(&user_defined_roles);

    assert!(spec.contains(
            "Research carefully.\n- This role's model is set to `gpt-5` and its reasoning effort is set to `high`. These settings cannot be changed."
        ));
}

#[test]
fn spawn_tool_spec_marks_role_locked_reasoning_effort_only() {
    let tempdir = TempDir::new().expect("create temp dir");
    let role_path = tempdir.path().join("reviewer.toml");
    fs::write(
        &role_path,
        "developer_instructions = \"Review carefully\"\nmodel_reasoning_effort = \"medium\"\n",
    )
    .expect("write role config");
    let user_defined_roles = BTreeMap::from([(
        "reviewer".to_string(),
        AgentRoleConfig {
            description: Some("Review carefully.".to_string()),
            model: None,
            config_file: Some(role_path),
            spawn_mode: None,
            nickname_candidates: None,
        },
    )]);

    let spec = spawn_tool_spec::build(&user_defined_roles);

    assert!(spec.contains(
            "Review carefully.\n- This role's reasoning effort is set to `medium` and cannot be changed."
        ));
}

#[test]
fn built_in_config_file_contents_resolves_explorer_only() {
    assert_eq!(
        built_in::config_file_contents(Path::new("missing.toml")),
        None
    );
}
