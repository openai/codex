use super::LoaderOverrides;
use super::load_config_layers_state;
use crate::config::ConfigBuilder;
use crate::config::ConfigOverrides;
use crate::config::ConfigToml;
use crate::config::ConstraintError;
use crate::config::ProjectConfig;
use crate::config_loader::CloudRequirementsLoader;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigLoadError;
use crate::config_loader::ConfigRequirements;
use crate::config_loader::ConfigRequirementsToml;
use crate::config_loader::ConfigRequirementsWithSources;
use crate::config_loader::RequirementSource;
use crate::config_loader::load_requirements_toml;
use crate::config_loader::version_for_toml;
use codex_config::CONFIG_TOML_FILE;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::protocol::AskForApproval;
#[cfg(target_os = "macos")]
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::Path;
use tempfile::tempdir;
use toml::Value as TomlValue;

fn config_error_from_io(err: &std::io::Error) -> &super::ConfigError {
    err.get_ref()
        .and_then(|err| err.downcast_ref::<ConfigLoadError>())
        .map(ConfigLoadError::config_error)
        .expect("expected ConfigLoadError")
}

async fn make_config_for_test(
    codex_home: &Path,
    project_path: &Path,
    trust_level: TrustLevel,
    project_root_markers: Option<Vec<String>>,
) -> std::io::Result<()> {
    tokio::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        toml::to_string(&ConfigToml {
            projects: Some(HashMap::from([(
                project_path.to_string_lossy().to_string(),
                ProjectConfig {
                    trust_level: Some(trust_level),
                },
            )])),
            project_root_markers,
            ..Default::default()
        })
        .expect("serialize config"),
    )
    .await
}

#[tokio::test]
async fn cli_overrides_resolve_relative_paths_against_cwd() -> std::io::Result<()> {
    let codex_home = tempdir().expect("tempdir");
    let cwd_dir = tempdir().expect("tempdir");
    let cwd_path = cwd_dir.path().to_path_buf();

    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(vec![(
            "log_dir".to_string(),
            TomlValue::String("run-logs".to_string()),
        )])
        .harness_overrides(ConfigOverrides {
            cwd: Some(cwd_path.clone()),
            ..Default::default()
        })
        .build()
        .await?;

    let expected = AbsolutePathBuf::resolve_path_against_base("run-logs", cwd_path)?;
    assert_eq!(config.log_dir, expected.to_path_buf());
    Ok(())
}

#[tokio::test]
async fn returns_config_error_for_invalid_user_config_toml() {
    let tmp = tempdir().expect("tempdir");
    let contents = "model = \"gpt-4\"\ninvalid = [";
    let config_path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&config_path, contents).expect("write config");

    let cwd = AbsolutePathBuf::try_from(tmp.path()).expect("cwd");
    let err = load_config_layers_state(
        tmp.path(),
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await
    .expect_err("expected error");

    let config_error = config_error_from_io(&err);
    let expected_toml_error = toml::from_str::<TomlValue>(contents).expect_err("parse error");
    let expected_config_error =
        super::config_error_from_toml(&config_path, contents, expected_toml_error);
    assert_eq!(config_error, &expected_config_error);
}

#[tokio::test]
async fn returns_config_error_for_invalid_managed_config_toml() {
    let tmp = tempdir().expect("tempdir");
    let managed_path = tmp.path().join("managed_config.toml");
    let contents = "model = \"gpt-4\"\ninvalid = [";
    std::fs::write(&managed_path, contents).expect("write managed config");

    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path.clone()),
        ..Default::default()
    };

    let cwd = AbsolutePathBuf::try_from(tmp.path()).expect("cwd");
    let err = load_config_layers_state(
        tmp.path(),
        Some(cwd),
        &[] as &[(String, TomlValue)],
        overrides,
        CloudRequirementsLoader::default(),
    )
    .await
    .expect_err("expected error");

    let config_error = config_error_from_io(&err);
    let expected_toml_error = toml::from_str::<TomlValue>(contents).expect_err("parse error");
    let expected_config_error =
        super::config_error_from_toml(&managed_path, contents, expected_toml_error);
    assert_eq!(config_error, &expected_config_error);
}

#[tokio::test]
async fn returns_config_error_for_schema_error_in_user_config() {
    let tmp = tempdir().expect("tempdir");
    let contents = "model_context_window = \"not_a_number\"";
    let config_path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&config_path, contents).expect("write config");

    let err = ConfigBuilder::default()
        .codex_home(tmp.path().to_path_buf())
        .fallback_cwd(Some(tmp.path().to_path_buf()))
        .build()
        .await
        .expect_err("expected error");

    let config_error = config_error_from_io(&err);
    let _guard = codex_utils_absolute_path::AbsolutePathBufGuard::new(tmp.path());
    let expected_config_error =
        codex_config::config_error_from_typed_toml::<ConfigToml>(&config_path, contents)
            .expect("schema error");
    assert_eq!(config_error, &expected_config_error);
}

#[test]
fn schema_error_points_to_feature_value() {
    let tmp = tempdir().expect("tempdir");
    let contents = "[features]\ncollaboration_modes = \"true\"";
    let config_path = tmp.path().join(CONFIG_TOML_FILE);
    std::fs::write(&config_path, contents).expect("write config");

    let _guard = codex_utils_absolute_path::AbsolutePathBufGuard::new(tmp.path());
    let error = codex_config::config_error_from_typed_toml::<ConfigToml>(&config_path, contents)
        .expect("schema error");

    let value_line = contents.lines().nth(1).expect("value line");
    let value_column = value_line.find("\"true\"").expect("value") + 1;
    assert_eq!(error.range.start.line, 2);
    assert_eq!(error.range.start.column, value_column);
}

#[tokio::test]
async fn merges_managed_config_layer_on_top() {
    let tmp = tempdir().expect("tempdir");
    let managed_path = tmp.path().join("managed_config.toml");

    std::fs::write(
        tmp.path().join(CONFIG_TOML_FILE),
        r#"foo = 1

[nested]
value = "base"
"#,
    )
    .expect("write base");
    std::fs::write(
        &managed_path,
        r#"foo = 2

[nested]
value = "managed_config"
extra = true
"#,
    )
    .expect("write managed config");

    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path),
        #[cfg(target_os = "macos")]
        managed_preferences_base64: None,
        macos_managed_config_requirements_base64: None,
    };

    let cwd = AbsolutePathBuf::try_from(tmp.path()).expect("cwd");
    let state = load_config_layers_state(
        tmp.path(),
        Some(cwd),
        &[] as &[(String, TomlValue)],
        overrides,
        CloudRequirementsLoader::default(),
    )
    .await
    .expect("load config");
    let loaded = state.effective_config();
    let table = loaded.as_table().expect("top-level table expected");

    assert_eq!(table.get("foo"), Some(&TomlValue::Integer(2)));
    let nested = table
        .get("nested")
        .and_then(|v| v.as_table())
        .expect("nested");
    assert_eq!(
        nested.get("value"),
        Some(&TomlValue::String("managed_config".to_string()))
    );
    assert_eq!(nested.get("extra"), Some(&TomlValue::Boolean(true)));
}

#[tokio::test]
async fn returns_empty_when_all_layers_missing() {
    let tmp = tempdir().expect("tempdir");
    let managed_path = tmp.path().join("managed_config.toml");

    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path),
        #[cfg(target_os = "macos")]
        // Force managed preferences to resolve as empty so this test does not
        // inherit non-empty machine-specific managed state.
        managed_preferences_base64: Some(String::new()),
        macos_managed_config_requirements_base64: None,
    };

    let cwd = AbsolutePathBuf::try_from(tmp.path()).expect("cwd");
    let layers = load_config_layers_state(
        tmp.path(),
        Some(cwd),
        &[] as &[(String, TomlValue)],
        overrides,
        CloudRequirementsLoader::default(),
    )
    .await
    .expect("load layers");
    let user_layer = layers
        .get_user_layer()
        .expect("expected a user layer even when CODEX_HOME/config.toml does not exist");
    assert_eq!(
        &ConfigLayerEntry {
            name: super::ConfigLayerSource::User {
                file: AbsolutePathBuf::resolve_path_against_base(CONFIG_TOML_FILE, tmp.path())
                    .expect("resolve user config.toml path")
            },
            config: TomlValue::Table(toml::map::Map::new()),
            raw_toml: None,
            version: version_for_toml(&TomlValue::Table(toml::map::Map::new())),
            disabled_reason: None,
        },
        user_layer,
    );
    assert_eq!(
        user_layer.config,
        TomlValue::Table(toml::map::Map::new()),
        "expected empty config for user layer when config.toml does not exist"
    );

    let binding = layers.effective_config();
    let base_table = binding.as_table().expect("base table expected");
    assert!(
        base_table.is_empty(),
        "expected empty base layer when configs missing"
    );
    let num_system_layers = layers
        .layers_high_to_low()
        .iter()
        .filter(|layer| matches!(layer.name, super::ConfigLayerSource::System { .. }))
        .count();
    assert_eq!(
        num_system_layers, 1,
        "system layer should always be present"
    );

    #[cfg(not(target_os = "macos"))]
    {
        let effective = layers.effective_config();
        let table = effective.as_table().expect("top-level table expected");
        assert!(
            table.is_empty(),
            "expected empty table when configs missing"
        );
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn managed_preferences_take_highest_precedence() {
    use base64::Engine;

    let tmp = tempdir().expect("tempdir");
    let managed_path = tmp.path().join("managed_config.toml");

    std::fs::write(
        tmp.path().join(CONFIG_TOML_FILE),
        r#"[nested]
value = "base"
"#,
    )
    .expect("write base");
    std::fs::write(
        &managed_path,
        r#"[nested]
value = "managed_config"
flag = true
"#,
    )
    .expect("write managed config");
    let raw_managed_preferences = r#"
# managed profile
[nested]
value = "managed"
flag = false
"#;

    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path),
        managed_preferences_base64: Some(
            base64::prelude::BASE64_STANDARD.encode(raw_managed_preferences.as_bytes()),
        ),
        macos_managed_config_requirements_base64: None,
    };

    let cwd = AbsolutePathBuf::try_from(tmp.path()).expect("cwd");
    let state = load_config_layers_state(
        tmp.path(),
        Some(cwd),
        &[] as &[(String, TomlValue)],
        overrides,
        CloudRequirementsLoader::default(),
    )
    .await
    .expect("load config");
    let loaded = state.effective_config();
    let nested = loaded
        .get("nested")
        .and_then(|v| v.as_table())
        .expect("nested table");
    assert_eq!(
        nested.get("value"),
        Some(&TomlValue::String("managed".to_string()))
    );
    assert_eq!(nested.get("flag"), Some(&TomlValue::Boolean(false)));
    let mdm_layer = state
        .layers_high_to_low()
        .into_iter()
        .find(|layer| {
            matches!(
                layer.name,
                super::ConfigLayerSource::LegacyManagedConfigTomlFromMdm
            )
        })
        .expect("mdm layer");
    let raw = mdm_layer.raw_toml().expect("preserved mdm toml");
    assert!(raw.contains("# managed profile"));
    assert!(raw.contains("value = \"managed\""));
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn managed_preferences_requirements_are_applied() -> anyhow::Result<()> {
    use base64::Engine;

    let tmp = tempdir()?;

    let state = load_config_layers_state(
        tmp.path(),
        Some(AbsolutePathBuf::try_from(tmp.path())?),
        &[] as &[(String, TomlValue)],
        LoaderOverrides {
            managed_config_path: Some(tmp.path().join("managed_config.toml")),
            managed_preferences_base64: Some(String::new()),
            macos_managed_config_requirements_base64: Some(
                base64::prelude::BASE64_STANDARD.encode(
                    r#"
allowed_approval_policies = ["never"]
allowed_sandbox_modes = ["read-only"]
"#
                    .as_bytes(),
                ),
            ),
        },
        CloudRequirementsLoader::default(),
    )
    .await?;

    assert_eq!(
        state.requirements().approval_policy.value(),
        AskForApproval::Never
    );
    assert_eq!(
        *state.requirements().sandbox_policy.get(),
        SandboxPolicy::new_read_only_policy()
    );
    assert!(
        state
            .requirements()
            .approval_policy
            .can_set(&AskForApproval::OnRequest)
            .is_err()
    );
    assert!(
        state
            .requirements()
            .sandbox_policy
            .can_set(&SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                read_only_access: Default::default(),
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            })
            .is_err()
    );

    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn managed_preferences_requirements_take_precedence() -> anyhow::Result<()> {
    use base64::Engine;

    let tmp = tempdir()?;
    let managed_path = tmp.path().join("managed_config.toml");

    tokio::fs::write(&managed_path, "approval_policy = \"on-request\"\n").await?;

    let state = load_config_layers_state(
        tmp.path(),
        Some(AbsolutePathBuf::try_from(tmp.path())?),
        &[] as &[(String, TomlValue)],
        LoaderOverrides {
            managed_config_path: Some(managed_path),
            managed_preferences_base64: Some(String::new()),
            macos_managed_config_requirements_base64: Some(
                base64::prelude::BASE64_STANDARD.encode(
                    r#"
allowed_approval_policies = ["never"]
"#
                    .as_bytes(),
                ),
            ),
        },
        CloudRequirementsLoader::default(),
    )
    .await?;

    assert_eq!(
        state.requirements().approval_policy.value(),
        AskForApproval::Never
    );
    assert!(
        state
            .requirements()
            .approval_policy
            .can_set(&AskForApproval::OnRequest)
            .is_err()
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn load_requirements_toml_produces_expected_constraints() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let requirements_file = tmp.path().join("requirements.toml");
    tokio::fs::write(
        &requirements_file,
        r#"
allowed_approval_policies = ["never", "on-request"]
allowed_web_search_modes = ["cached"]
enforce_residency = "us"
"#,
    )
    .await?;

    let mut config_requirements_toml = ConfigRequirementsWithSources::default();
    load_requirements_toml(&mut config_requirements_toml, &requirements_file).await?;

    assert_eq!(
        config_requirements_toml
            .allowed_approval_policies
            .as_deref()
            .cloned(),
        Some(vec![AskForApproval::Never, AskForApproval::OnRequest])
    );
    assert_eq!(
        config_requirements_toml
            .allowed_web_search_modes
            .as_deref()
            .cloned(),
        Some(vec![crate::config_loader::WebSearchModeRequirement::Cached])
    );
    let config_requirements: ConfigRequirements = config_requirements_toml.try_into()?;
    assert_eq!(
        config_requirements.approval_policy.value(),
        AskForApproval::Never
    );
    config_requirements
        .approval_policy
        .can_set(&AskForApproval::Never)?;
    assert!(
        config_requirements
            .approval_policy
            .can_set(&AskForApproval::OnFailure)
            .is_err()
    );
    assert_eq!(
        config_requirements.web_search_mode.value(),
        WebSearchMode::Cached
    );
    config_requirements
        .web_search_mode
        .can_set(&WebSearchMode::Cached)?;
    config_requirements
        .web_search_mode
        .can_set(&WebSearchMode::Cached)?;
    config_requirements
        .web_search_mode
        .can_set(&WebSearchMode::Disabled)?;
    assert!(
        config_requirements
            .web_search_mode
            .can_set(&WebSearchMode::Live)
            .is_err()
    );
    assert_eq!(
        config_requirements.enforce_residency.value(),
        Some(crate::config_loader::ResidencyRequirement::Us)
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn cloud_requirements_take_precedence_over_mdm_requirements() -> anyhow::Result<()> {
    use base64::Engine;

    let tmp = tempdir()?;
    let state = load_config_layers_state(
        tmp.path(),
        Some(AbsolutePathBuf::try_from(tmp.path())?),
        &[] as &[(String, TomlValue)],
        LoaderOverrides {
            macos_managed_config_requirements_base64: Some(
                base64::prelude::BASE64_STANDARD.encode(
                    r#"
allowed_approval_policies = ["on-request"]
"#
                    .as_bytes(),
                ),
            ),
            ..LoaderOverrides::default()
        },
        CloudRequirementsLoader::new(async {
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_sandbox_modes: None,
                allowed_web_search_modes: None,
                mcp_servers: None,
                rules: None,
                enforce_residency: None,
                network: None,
            })
        }),
    )
    .await?;

    assert_eq!(
        state.requirements().approval_policy.value(),
        AskForApproval::Never
    );
    assert_eq!(
        state
            .requirements()
            .approval_policy
            .can_set(&AskForApproval::OnRequest),
        Err(ConstraintError::InvalidValue {
            field_name: "approval_policy",
            candidate: "OnRequest".into(),
            allowed: "[Never]".into(),
            requirement_source: RequirementSource::CloudRequirements,
        })
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn cloud_requirements_are_not_overwritten_by_system_requirements() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let requirements_file = tmp.path().join("requirements.toml");
    tokio::fs::write(
        &requirements_file,
        r#"
allowed_approval_policies = ["on-request"]
"#,
    )
    .await?;

    let mut config_requirements_toml = ConfigRequirementsWithSources::default();
    config_requirements_toml.merge_unset_fields(
        RequirementSource::CloudRequirements,
        ConfigRequirementsToml {
            allowed_approval_policies: Some(vec![AskForApproval::Never]),
            allowed_sandbox_modes: None,
            allowed_web_search_modes: None,
            mcp_servers: None,
            rules: None,
            enforce_residency: None,
            network: None,
        },
    );
    load_requirements_toml(&mut config_requirements_toml, &requirements_file).await?;

    assert_eq!(
        config_requirements_toml
            .allowed_approval_policies
            .as_ref()
            .map(|sourced| sourced.value.clone()),
        Some(vec![AskForApproval::Never])
    );
    assert_eq!(
        config_requirements_toml
            .allowed_approval_policies
            .as_ref()
            .map(|sourced| sourced.source.clone()),
        Some(RequirementSource::CloudRequirements)
    );

    Ok(())
}

#[tokio::test]
async fn load_config_layers_includes_cloud_requirements() -> anyhow::Result<()> {
    let tmp = tempdir()?;
    let codex_home = tmp.path().join("home");
    tokio::fs::create_dir_all(&codex_home).await?;
    let cwd = AbsolutePathBuf::from_absolute_path(tmp.path())?;

    let requirements = ConfigRequirementsToml {
        allowed_approval_policies: Some(vec![AskForApproval::Never]),
        allowed_sandbox_modes: None,
        allowed_web_search_modes: None,
        mcp_servers: None,
        rules: None,
        enforce_residency: None,
        network: None,
    };
    let expected = requirements.clone();
    let cloud_requirements = CloudRequirementsLoader::new(async move { Some(requirements) });

    let layers = load_config_layers_state(
        &codex_home,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        cloud_requirements,
    )
    .await?;

    assert_eq!(
        layers.requirements_toml().allowed_approval_policies,
        expected.allowed_approval_policies
    );
    assert_eq!(
        layers
            .requirements()
            .approval_policy
            .can_set(&AskForApproval::OnRequest),
        Err(ConstraintError::InvalidValue {
            field_name: "approval_policy",
            candidate: "OnRequest".into(),
            allowed: "[Never]".into(),
            requirement_source: RequirementSource::CloudRequirements,
        })
    );

    Ok(())
}

#[tokio::test]
async fn project_layers_prefer_closest_cwd() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let project_root = tmp.path().join("project");
    let nested = project_root.join("child");
    tokio::fs::create_dir_all(nested.join(".codex")).await?;
    tokio::fs::create_dir_all(project_root.join(".codex")).await?;
    tokio::fs::write(project_root.join(".git"), "gitdir: here").await?;

    tokio::fs::write(
        project_root.join(".codex").join(CONFIG_TOML_FILE),
        "foo = \"root\"\n",
    )
    .await?;
    tokio::fs::write(
        nested.join(".codex").join(CONFIG_TOML_FILE),
        "foo = \"child\"\n",
    )
    .await?;

    let codex_home = tmp.path().join("home");
    tokio::fs::create_dir_all(&codex_home).await?;
    make_config_for_test(&codex_home, &project_root, TrustLevel::Trusted, None).await?;
    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;
    let layers = load_config_layers_state(
        &codex_home,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;

    let project_layers: Vec<_> = layers
        .layers_high_to_low()
        .into_iter()
        .filter_map(|layer| match &layer.name {
            super::ConfigLayerSource::Project { dot_codex_folder } => Some(dot_codex_folder),
            _ => None,
        })
        .collect();
    assert_eq!(project_layers.len(), 2);
    assert_eq!(project_layers[0].as_path(), nested.join(".codex").as_path());
    assert_eq!(
        project_layers[1].as_path(),
        project_root.join(".codex").as_path()
    );

    let config = layers.effective_config();
    let foo = config
        .get("foo")
        .and_then(TomlValue::as_str)
        .expect("foo entry");
    assert_eq!(foo, "child");
    Ok(())
}

#[tokio::test]
async fn project_paths_resolve_relative_to_dot_codex_and_override_in_order() -> std::io::Result<()>
{
    let tmp = tempdir()?;
    let project_root = tmp.path().join("project");
    let nested = project_root.join("child");
    tokio::fs::create_dir_all(project_root.join(".codex")).await?;
    tokio::fs::create_dir_all(nested.join(".codex")).await?;
    tokio::fs::write(project_root.join(".git"), "gitdir: here").await?;

    let root_cfg = r#"
model_instructions_file = "root.txt"
"#;
    let nested_cfg = r#"
model_instructions_file = "child.txt"
"#;
    tokio::fs::write(project_root.join(".codex").join(CONFIG_TOML_FILE), root_cfg).await?;
    tokio::fs::write(nested.join(".codex").join(CONFIG_TOML_FILE), nested_cfg).await?;
    tokio::fs::write(
        project_root.join(".codex").join("root.txt"),
        "root instructions",
    )
    .await?;
    tokio::fs::write(
        nested.join(".codex").join("child.txt"),
        "child instructions",
    )
    .await?;

    let codex_home = tmp.path().join("home");
    tokio::fs::create_dir_all(&codex_home).await?;
    make_config_for_test(&codex_home, &project_root, TrustLevel::Trusted, None).await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home)
        .harness_overrides(ConfigOverrides {
            cwd: Some(nested.clone()),
            ..ConfigOverrides::default()
        })
        .build()
        .await?;

    assert_eq!(
        config.base_instructions.as_deref(),
        Some("child instructions")
    );

    Ok(())
}

#[tokio::test]
async fn cli_override_model_instructions_file_sets_base_instructions() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let codex_home = tmp.path().join("home");
    tokio::fs::create_dir_all(&codex_home).await?;
    tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), "").await?;

    let cwd = tmp.path().join("work");
    tokio::fs::create_dir_all(&cwd).await?;

    let instructions_path = tmp.path().join("instr.md");
    tokio::fs::write(&instructions_path, "cli override instructions").await?;

    let cli_overrides = vec![(
        "model_instructions_file".to_string(),
        TomlValue::String(instructions_path.to_string_lossy().to_string()),
    )];

    let config = ConfigBuilder::default()
        .codex_home(codex_home)
        .cli_overrides(cli_overrides)
        .harness_overrides(ConfigOverrides {
            cwd: Some(cwd),
            ..ConfigOverrides::default()
        })
        .build()
        .await?;

    assert_eq!(
        config.base_instructions.as_deref(),
        Some("cli override instructions")
    );

    Ok(())
}

#[tokio::test]
async fn project_layer_is_added_when_dot_codex_exists_without_config_toml() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let project_root = tmp.path().join("project");
    let nested = project_root.join("child");
    tokio::fs::create_dir_all(&nested).await?;
    tokio::fs::create_dir_all(project_root.join(".codex")).await?;
    tokio::fs::write(project_root.join(".git"), "gitdir: here").await?;

    let codex_home = tmp.path().join("home");
    tokio::fs::create_dir_all(&codex_home).await?;
    make_config_for_test(&codex_home, &project_root, TrustLevel::Trusted, None).await?;
    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;
    let layers = load_config_layers_state(
        &codex_home,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;

    let project_layers: Vec<_> = layers
        .layers_high_to_low()
        .into_iter()
        .filter(|layer| matches!(layer.name, super::ConfigLayerSource::Project { .. }))
        .collect();
    assert_eq!(
        vec![&ConfigLayerEntry {
            name: super::ConfigLayerSource::Project {
                dot_codex_folder: AbsolutePathBuf::from_absolute_path(project_root.join(".codex"))?,
            },
            config: TomlValue::Table(toml::map::Map::new()),
            raw_toml: None,
            version: version_for_toml(&TomlValue::Table(toml::map::Map::new())),
            disabled_reason: None,
        }],
        project_layers
    );

    Ok(())
}

#[tokio::test]
async fn codex_home_is_not_loaded_as_project_layer_from_home_dir() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let home_dir = tmp.path().join("home");
    let codex_home = home_dir.join(".codex");
    tokio::fs::create_dir_all(&codex_home).await?;
    tokio::fs::write(codex_home.join(CONFIG_TOML_FILE), "foo = \"user\"\n").await?;

    let cwd = AbsolutePathBuf::from_absolute_path(&home_dir)?;
    let layers = load_config_layers_state(
        &codex_home,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;

    let project_layers: Vec<_> = layers
        .get_layers(
            super::ConfigLayerStackOrdering::HighestPrecedenceFirst,
            true,
        )
        .into_iter()
        .filter(|layer| matches!(layer.name, super::ConfigLayerSource::Project { .. }))
        .collect();
    let expected: Vec<&ConfigLayerEntry> = Vec::new();
    assert_eq!(expected, project_layers);
    assert_eq!(
        layers.effective_config().get("foo"),
        Some(&TomlValue::String("user".to_string()))
    );

    Ok(())
}

#[tokio::test]
async fn codex_home_within_project_tree_is_not_double_loaded() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let project_root = tmp.path().join("project");
    let nested = project_root.join("child");
    let project_dot_codex = project_root.join(".codex");
    let nested_dot_codex = nested.join(".codex");

    tokio::fs::create_dir_all(&nested_dot_codex).await?;
    tokio::fs::create_dir_all(project_root.join(".git")).await?;
    tokio::fs::write(nested_dot_codex.join(CONFIG_TOML_FILE), "foo = \"child\"\n").await?;

    tokio::fs::create_dir_all(&project_dot_codex).await?;
    make_config_for_test(&project_dot_codex, &project_root, TrustLevel::Trusted, None).await?;
    let user_config_path = project_dot_codex.join(CONFIG_TOML_FILE);
    let user_config_contents = tokio::fs::read_to_string(&user_config_path).await?;
    tokio::fs::write(
        &user_config_path,
        format!("foo = \"user\"\n{user_config_contents}"),
    )
    .await?;

    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;
    let layers = load_config_layers_state(
        &project_dot_codex,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;

    let project_layers: Vec<_> = layers
        .get_layers(
            super::ConfigLayerStackOrdering::HighestPrecedenceFirst,
            true,
        )
        .into_iter()
        .filter(|layer| matches!(layer.name, super::ConfigLayerSource::Project { .. }))
        .collect();

    let child_config: TomlValue = toml::from_str("foo = \"child\"\n").expect("parse child config");
    assert_eq!(
        vec![&ConfigLayerEntry {
            name: super::ConfigLayerSource::Project {
                dot_codex_folder: AbsolutePathBuf::from_absolute_path(&nested_dot_codex)?,
            },
            config: child_config.clone(),
            raw_toml: None,
            version: version_for_toml(&child_config),
            disabled_reason: None,
        }],
        project_layers
    );
    assert_eq!(
        layers.effective_config().get("foo"),
        Some(&TomlValue::String("child".to_string()))
    );

    Ok(())
}

#[tokio::test]
async fn project_layers_disabled_when_untrusted_or_unknown() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let project_root = tmp.path().join("project");
    let nested = project_root.join("child");
    tokio::fs::create_dir_all(nested.join(".codex")).await?;
    tokio::fs::write(
        nested.join(".codex").join(CONFIG_TOML_FILE),
        "foo = \"child\"\n",
    )
    .await?;

    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;

    let codex_home_untrusted = tmp.path().join("home_untrusted");
    tokio::fs::create_dir_all(&codex_home_untrusted).await?;
    make_config_for_test(
        &codex_home_untrusted,
        &project_root,
        TrustLevel::Untrusted,
        None,
    )
    .await?;
    let untrusted_config_path = codex_home_untrusted.join(CONFIG_TOML_FILE);
    let untrusted_config_contents = tokio::fs::read_to_string(&untrusted_config_path).await?;
    tokio::fs::write(
        &untrusted_config_path,
        format!("foo = \"user\"\n{untrusted_config_contents}"),
    )
    .await?;

    let layers_untrusted = load_config_layers_state(
        &codex_home_untrusted,
        Some(cwd.clone()),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;
    let project_layers_untrusted: Vec<_> = layers_untrusted
        .get_layers(
            super::ConfigLayerStackOrdering::HighestPrecedenceFirst,
            true,
        )
        .into_iter()
        .filter(|layer| matches!(layer.name, super::ConfigLayerSource::Project { .. }))
        .collect();
    assert_eq!(project_layers_untrusted.len(), 1);
    assert!(
        project_layers_untrusted[0].disabled_reason.is_some(),
        "expected untrusted project layer to be disabled"
    );
    assert_eq!(
        project_layers_untrusted[0].config.get("foo"),
        Some(&TomlValue::String("child".to_string()))
    );
    assert_eq!(
        layers_untrusted.effective_config().get("foo"),
        Some(&TomlValue::String("user".to_string()))
    );

    let codex_home_unknown = tmp.path().join("home_unknown");
    tokio::fs::create_dir_all(&codex_home_unknown).await?;
    tokio::fs::write(
        codex_home_unknown.join(CONFIG_TOML_FILE),
        "foo = \"user\"\n",
    )
    .await?;

    let layers_unknown = load_config_layers_state(
        &codex_home_unknown,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;
    let project_layers_unknown: Vec<_> = layers_unknown
        .get_layers(
            super::ConfigLayerStackOrdering::HighestPrecedenceFirst,
            true,
        )
        .into_iter()
        .filter(|layer| matches!(layer.name, super::ConfigLayerSource::Project { .. }))
        .collect();
    assert_eq!(project_layers_unknown.len(), 1);
    assert!(
        project_layers_unknown[0].disabled_reason.is_some(),
        "expected unknown-trust project layer to be disabled"
    );
    assert_eq!(
        project_layers_unknown[0].config.get("foo"),
        Some(&TomlValue::String("child".to_string()))
    );
    assert_eq!(
        layers_unknown.effective_config().get("foo"),
        Some(&TomlValue::String("user".to_string()))
    );

    Ok(())
}

#[tokio::test]
async fn invalid_project_config_ignored_when_untrusted_or_unknown() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let project_root = tmp.path().join("project");
    let nested = project_root.join("child");
    tokio::fs::create_dir_all(nested.join(".codex")).await?;
    tokio::fs::write(project_root.join(".git"), "gitdir: here").await?;
    tokio::fs::write(nested.join(".codex").join(CONFIG_TOML_FILE), "foo =").await?;

    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;
    let cases = [
        ("untrusted", Some(TrustLevel::Untrusted)),
        ("unknown", None),
    ];

    for (name, trust_level) in cases {
        let codex_home = tmp.path().join(format!("home_{name}"));
        tokio::fs::create_dir_all(&codex_home).await?;
        let config_path = codex_home.join(CONFIG_TOML_FILE);

        if let Some(trust_level) = trust_level {
            make_config_for_test(&codex_home, &project_root, trust_level, None).await?;
            let config_contents = tokio::fs::read_to_string(&config_path).await?;
            tokio::fs::write(&config_path, format!("foo = \"user\"\n{config_contents}")).await?;
        } else {
            tokio::fs::write(&config_path, "foo = \"user\"\n").await?;
        }

        let layers = load_config_layers_state(
            &codex_home,
            Some(cwd.clone()),
            &[] as &[(String, TomlValue)],
            LoaderOverrides::default(),
            CloudRequirementsLoader::default(),
        )
        .await?;
        let project_layers: Vec<_> = layers
            .get_layers(
                super::ConfigLayerStackOrdering::HighestPrecedenceFirst,
                true,
            )
            .into_iter()
            .filter(|layer| matches!(layer.name, super::ConfigLayerSource::Project { .. }))
            .collect();
        assert_eq!(
            project_layers.len(),
            1,
            "expected one project layer for {name}"
        );
        assert!(
            project_layers[0].disabled_reason.is_some(),
            "expected {name} project layer to be disabled"
        );
        assert_eq!(
            project_layers[0].config,
            TomlValue::Table(toml::map::Map::new())
        );
        assert_eq!(
            layers.effective_config().get("foo"),
            Some(&TomlValue::String("user".to_string()))
        );
    }

    Ok(())
}

#[tokio::test]
async fn cli_overrides_with_relative_paths_do_not_break_trust_check() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let project_root = tmp.path().join("project");
    let nested = project_root.join("child");
    tokio::fs::create_dir_all(&nested).await?;
    tokio::fs::write(project_root.join(".git"), "gitdir: here").await?;

    let codex_home = tmp.path().join("home");
    tokio::fs::create_dir_all(&codex_home).await?;
    make_config_for_test(&codex_home, &project_root, TrustLevel::Trusted, None).await?;

    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;
    let cli_overrides = vec![(
        "model_instructions_file".to_string(),
        TomlValue::String("relative.md".to_string()),
    )];

    load_config_layers_state(
        &codex_home,
        Some(cwd),
        &cli_overrides,
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn project_root_markers_supports_alternate_markers() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let project_root = tmp.path().join("project");
    let nested = project_root.join("child");
    tokio::fs::create_dir_all(project_root.join(".codex")).await?;
    tokio::fs::create_dir_all(nested.join(".codex")).await?;
    tokio::fs::write(project_root.join(".hg"), "hg").await?;
    tokio::fs::write(
        project_root.join(".codex").join(CONFIG_TOML_FILE),
        "foo = \"root\"\n",
    )
    .await?;
    tokio::fs::write(
        nested.join(".codex").join(CONFIG_TOML_FILE),
        "foo = \"child\"\n",
    )
    .await?;

    let codex_home = tmp.path().join("home");
    tokio::fs::create_dir_all(&codex_home).await?;
    make_config_for_test(
        &codex_home,
        &project_root,
        TrustLevel::Trusted,
        Some(vec![".hg".to_string()]),
    )
    .await?;

    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;
    let layers = load_config_layers_state(
        &codex_home,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
        CloudRequirementsLoader::default(),
    )
    .await?;

    let project_layers: Vec<_> = layers
        .layers_high_to_low()
        .into_iter()
        .filter_map(|layer| match &layer.name {
            super::ConfigLayerSource::Project { dot_codex_folder } => Some(dot_codex_folder),
            _ => None,
        })
        .collect();
    assert_eq!(project_layers.len(), 2);
    assert_eq!(project_layers[0].as_path(), nested.join(".codex").as_path());
    assert_eq!(
        project_layers[1].as_path(),
        project_root.join(".codex").as_path()
    );

    let merged = layers.effective_config();
    let foo = merged
        .get("foo")
        .and_then(TomlValue::as_str)
        .expect("foo entry");
    assert_eq!(foo, "child");

    Ok(())
}

mod requirements_exec_policy_tests {
    use crate::config_loader::ConfigLayerEntry;
    use crate::config_loader::ConfigLayerStack;
    use crate::config_loader::ConfigRequirements;
    use crate::config_loader::ConfigRequirementsToml;
    use crate::config_loader::ConfigRequirementsWithSources;
    use crate::config_loader::RequirementSource;
    use crate::exec_policy::load_exec_policy;
    use codex_app_server_protocol::ConfigLayerSource;
    use codex_config::RequirementsExecPolicyDecisionToml;
    use codex_config::RequirementsExecPolicyParseError;
    use codex_config::RequirementsExecPolicyPatternTokenToml;
    use codex_config::RequirementsExecPolicyPrefixRuleToml;
    use codex_config::RequirementsExecPolicyToml;
    use codex_execpolicy::Decision;
    use codex_execpolicy::Evaluation;
    use codex_execpolicy::RuleMatch;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use tempfile::tempdir;
    use toml::Value as TomlValue;
    use toml::from_str;

    fn tokens(cmd: &[&str]) -> Vec<String> {
        cmd.iter().map(std::string::ToString::to_string).collect()
    }

    fn panic_if_called(_: &[String]) -> Decision {
        panic!("rule should match so heuristic should not be called");
    }

    fn config_stack_for_dot_codex_folder_with_requirements(
        dot_codex_folder: &Path,
        requirements: ConfigRequirements,
    ) -> ConfigLayerStack {
        let dot_codex_folder = AbsolutePathBuf::from_absolute_path(dot_codex_folder)
            .expect("absolute dot_codex_folder");
        let layer = ConfigLayerEntry::new(
            ConfigLayerSource::Project { dot_codex_folder },
            TomlValue::Table(Default::default()),
        );
        ConfigLayerStack::new(vec![layer], requirements, ConfigRequirementsToml::default())
            .expect("ConfigLayerStack")
    }

    fn requirements_from_toml(toml_str: &str) -> ConfigRequirements {
        let config: ConfigRequirementsToml = from_str(toml_str).expect("parse requirements toml");
        let mut with_sources = ConfigRequirementsWithSources::default();
        with_sources.merge_unset_fields(RequirementSource::Unknown, config);
        ConfigRequirements::try_from(with_sources).expect("requirements")
    }

    #[test]
    fn parses_single_prefix_rule_from_raw_toml() -> anyhow::Result<()> {
        let toml_str = r#"
prefix_rules = [
    { pattern = [{ token = "rm" }], decision = "forbidden" },
]
"#;

        let parsed: RequirementsExecPolicyToml = from_str(toml_str)?;

        assert_eq!(
            parsed,
            RequirementsExecPolicyToml {
                prefix_rules: vec![RequirementsExecPolicyPrefixRuleToml {
                    pattern: vec![RequirementsExecPolicyPatternTokenToml {
                        token: Some("rm".to_string()),
                        any_of: None,
                    }],
                    decision: Some(RequirementsExecPolicyDecisionToml::Forbidden),
                    justification: None,
                }],
            }
        );

        Ok(())
    }

    #[test]
    fn parses_multiple_prefix_rules_from_raw_toml() -> anyhow::Result<()> {
        let toml_str = r#"
prefix_rules = [
    { pattern = [{ token = "rm" }], decision = "forbidden" },
    { pattern = [{ token = "git" }, { any_of = ["push", "commit"] }], decision = "prompt", justification = "review changes before push or commit" },
]
"#;

        let parsed: RequirementsExecPolicyToml = from_str(toml_str)?;

        assert_eq!(
            parsed,
            RequirementsExecPolicyToml {
                prefix_rules: vec![
                    RequirementsExecPolicyPrefixRuleToml {
                        pattern: vec![RequirementsExecPolicyPatternTokenToml {
                            token: Some("rm".to_string()),
                            any_of: None,
                        }],
                        decision: Some(RequirementsExecPolicyDecisionToml::Forbidden),
                        justification: None,
                    },
                    RequirementsExecPolicyPrefixRuleToml {
                        pattern: vec![
                            RequirementsExecPolicyPatternTokenToml {
                                token: Some("git".to_string()),
                                any_of: None,
                            },
                            RequirementsExecPolicyPatternTokenToml {
                                token: None,
                                any_of: Some(vec!["push".to_string(), "commit".to_string()]),
                            },
                        ],
                        decision: Some(RequirementsExecPolicyDecisionToml::Prompt),
                        justification: Some("review changes before push or commit".to_string()),
                    },
                ],
            }
        );

        Ok(())
    }

    #[test]
    fn converts_rules_toml_into_internal_policy_representation() -> anyhow::Result<()> {
        let toml_str = r#"
prefix_rules = [
    { pattern = [{ token = "rm" }], decision = "forbidden" },
]
"#;

        let parsed: RequirementsExecPolicyToml = from_str(toml_str)?;
        let policy = parsed.to_policy()?;

        assert_eq!(
            policy.check(&tokens(&["rm", "-rf", "/tmp"]), &panic_if_called),
            Evaluation {
                decision: Decision::Forbidden,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["rm"]),
                    decision: Decision::Forbidden,
                    justification: None,
                }],
            }
        );

        Ok(())
    }

    #[test]
    fn head_any_of_expands_into_multiple_program_rules() -> anyhow::Result<()> {
        let toml_str = r#"
prefix_rules = [
    { pattern = [{ any_of = ["git", "hg"] }, { token = "status" }], decision = "prompt" },
]
"#;
        let parsed: RequirementsExecPolicyToml = from_str(toml_str)?;
        let policy = parsed.to_policy()?;

        assert_eq!(
            policy.check(&tokens(&["git", "status"]), &panic_if_called),
            Evaluation {
                decision: Decision::Prompt,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git", "status"]),
                    decision: Decision::Prompt,
                    justification: None,
                }],
            }
        );
        assert_eq!(
            policy.check(&tokens(&["hg", "status"]), &panic_if_called),
            Evaluation {
                decision: Decision::Prompt,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["hg", "status"]),
                    decision: Decision::Prompt,
                    justification: None,
                }],
            }
        );

        Ok(())
    }

    #[test]
    fn missing_decision_is_rejected() -> anyhow::Result<()> {
        let toml_str = r#"
prefix_rules = [
    { pattern = [{ token = "rm" }] },
]
"#;

        let parsed: RequirementsExecPolicyToml = from_str(toml_str)?;
        let err = parsed.to_policy().expect_err("missing decision");

        assert!(matches!(
            err,
            RequirementsExecPolicyParseError::MissingDecision { rule_index: 0 }
        ));
        Ok(())
    }

    #[test]
    fn allow_decision_is_rejected() -> anyhow::Result<()> {
        let toml_str = r#"
prefix_rules = [
    { pattern = [{ token = "rm" }], decision = "allow" },
]
"#;

        let parsed: RequirementsExecPolicyToml = from_str(toml_str)?;
        let err = parsed.to_policy().expect_err("allow decision not allowed");

        assert!(matches!(
            err,
            RequirementsExecPolicyParseError::AllowDecisionNotAllowed { rule_index: 0 }
        ));
        Ok(())
    }

    #[test]
    fn empty_prefix_rules_is_rejected() -> anyhow::Result<()> {
        let toml_str = r#"
prefix_rules = []
"#;

        let parsed: RequirementsExecPolicyToml = from_str(toml_str)?;
        let err = parsed.to_policy().expect_err("empty prefix rules");

        assert!(matches!(
            err,
            RequirementsExecPolicyParseError::EmptyPrefixRules
        ));
        Ok(())
    }

    #[tokio::test]
    async fn loads_requirements_exec_policy_without_rules_files() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let requirements = requirements_from_toml(
            r#"
                [rules]
                prefix_rules = [
                    { pattern = [{ token = "rm" }], decision = "forbidden" },
                ]
            "#,
        );
        let config_stack =
            config_stack_for_dot_codex_folder_with_requirements(temp_dir.path(), requirements);

        let policy = load_exec_policy(&config_stack).await?;

        assert_eq!(
            policy.check_multiple([vec!["rm".to_string()]].iter(), &panic_if_called),
            Evaluation {
                decision: Decision::Forbidden,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: vec!["rm".to_string()],
                    decision: Decision::Forbidden,
                    justification: None,
                }],
            }
        );

        Ok(())
    }

    #[tokio::test]
    async fn merges_requirements_exec_policy_with_file_rules() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let policy_dir = temp_dir.path().join("rules");
        std::fs::create_dir_all(&policy_dir)?;
        std::fs::write(
            policy_dir.join("deny.rules"),
            r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
        )?;

        let requirements = requirements_from_toml(
            r#"
                [rules]
                prefix_rules = [
                    { pattern = [{ token = "git" }, { token = "push" }], decision = "prompt" },
                ]
            "#,
        );
        let config_stack =
            config_stack_for_dot_codex_folder_with_requirements(temp_dir.path(), requirements);

        let policy = load_exec_policy(&config_stack).await?;

        assert_eq!(
            policy.check_multiple([vec!["rm".to_string()]].iter(), &panic_if_called),
            Evaluation {
                decision: Decision::Forbidden,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: vec!["rm".to_string()],
                    decision: Decision::Forbidden,
                    justification: None,
                }],
            }
        );
        assert_eq!(
            policy.check_multiple(
                [vec!["git".to_string(), "push".to_string()]].iter(),
                &panic_if_called
            ),
            Evaluation {
                decision: Decision::Prompt,
                matched_rules: vec![RuleMatch::PrefixRuleMatch {
                    matched_prefix: vec!["git".to_string(), "push".to_string()],
                    decision: Decision::Prompt,
                    justification: None,
                }],
            }
        );

        Ok(())
    }
}
