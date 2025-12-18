use super::LoaderOverrides;
use super::load_config_layers_state;
use crate::config::CONFIG_TOML_FILE;
use crate::config_loader::ConfigRequirements;
use crate::config_loader::config_requirements::ConfigRequirementsToml;
use crate::config_loader::load_requirements_toml;
use codex_protocol::protocol::AskForApproval;
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
    };

    let state = load_config_layers_state(tmp.path(), &[] as &[(String, TomlValue)], overrides)
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
    };

    let layers = load_config_layers_state(tmp.path(), &[] as &[(String, TomlValue)], overrides)
        .await
        .expect("load layers");
    assert!(
        layers.get_user_layer().is_none(),
        "no user layer when CODEX_HOME/config.toml does not exist"
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
        num_system_layers, 0,
        "managed config layer should be absent when file missing"
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

    let managed_payload = r#"
[nested]
value = "managed"
flag = false
"#;
    let encoded = base64::prelude::BASE64_STANDARD.encode(managed_payload.as_bytes());
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
        managed_preferences_base64: Some(encoded),
    };

    let state = load_config_layers_state(tmp.path(), &[] as &[(String, TomlValue)], overrides)
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

    let mut config_requirements_toml = ConfigRequirementsToml::default();
    load_requirements_toml(&mut config_requirements_toml, &requirements_file).await?;

    assert_eq!(
        config_requirements_toml.allowed_approval_policies,
        Some(vec![AskForApproval::Never, AskForApproval::OnRequest])
    );

    let config_requirements: ConfigRequirements = config_requirements_toml.try_into()?;
    assert_eq!(
        config_requirements.approval_policy.value(),
        AskForApproval::OnRequest
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
