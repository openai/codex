use super::LoaderOverrides;
use super::load_config_layers_state;
use crate::config::CONFIG_TOML_FILE;
use crate::config::ConfigBuilder;
use crate::config::ConfigOverrides;
use crate::config_loader::ConfigLayerEntry;
use crate::config_loader::ConfigRequirements;
use crate::config_loader::config_requirements::ConfigRequirementsWithSources;
use crate::config_loader::fingerprint::version_for_toml;
use crate::config_loader::load_requirements_toml;
use codex_protocol::protocol::AskForApproval;
#[cfg(target_os = "macos")]
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use toml::Value as TomlValue;

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
        managed_preferences_base64: None,
        macos_managed_config_requirements_base64: None,
    };

    let cwd = AbsolutePathBuf::try_from(tmp.path()).expect("cwd");
    let layers = load_config_layers_state(
        tmp.path(),
        Some(cwd),
        &[] as &[(String, TomlValue)],
        overrides,
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
            version: version_for_toml(&TomlValue::Table(toml::map::Map::new())),
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
    let expected_system_layers = if cfg!(unix) { 1 } else { 0 };
    assert_eq!(
        num_system_layers, expected_system_layers,
        "system layer should be present only on unix"
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

    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path),
        managed_preferences_base64: Some(
            base64::prelude::BASE64_STANDARD.encode(
                r#"
[nested]
value = "managed"
flag = false
"#
                .as_bytes(),
            ),
        ),
        macos_managed_config_requirements_base64: None,
    };

    let cwd = AbsolutePathBuf::try_from(tmp.path()).expect("cwd");
    let state = load_config_layers_state(
        tmp.path(),
        Some(cwd),
        &[] as &[(String, TomlValue)],
        overrides,
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
    )
    .await?;

    assert_eq!(
        state.requirements().approval_policy.value(),
        AskForApproval::Never
    );
    assert_eq!(
        *state.requirements().sandbox_policy.get(),
        SandboxPolicy::ReadOnly
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
    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;
    let layers = load_config_layers_state(
        &codex_home,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
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
experimental_instructions_file = "root.txt"
"#;
    let nested_cfg = r#"
experimental_instructions_file = "child.txt"
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
async fn project_layer_is_added_when_dot_codex_exists_without_config_toml() -> std::io::Result<()> {
    let tmp = tempdir()?;
    let project_root = tmp.path().join("project");
    let nested = project_root.join("child");
    tokio::fs::create_dir_all(&nested).await?;
    tokio::fs::create_dir_all(project_root.join(".codex")).await?;
    tokio::fs::write(project_root.join(".git"), "gitdir: here").await?;

    let codex_home = tmp.path().join("home");
    tokio::fs::create_dir_all(&codex_home).await?;
    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;
    let layers = load_config_layers_state(
        &codex_home,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
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
            version: version_for_toml(&TomlValue::Table(toml::map::Map::new())),
        }],
        project_layers
    );

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
    tokio::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"
project_root_markers = [".hg"]
"#,
    )
    .await?;

    let cwd = AbsolutePathBuf::from_absolute_path(&nested)?;
    let layers = load_config_layers_state(
        &codex_home,
        Some(cwd),
        &[] as &[(String, TomlValue)],
        LoaderOverrides::default(),
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
