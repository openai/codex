use super::LoaderOverrides;
use super::load_config_layers_state;
use crate::config::CONFIG_TOML_FILE;
use crate::config_loader::ConfigRequirements;
use crate::config_loader::config_requirements::ConfigRequirementsToml;
use crate::config_loader::load_requirements_toml;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use serial_test::serial;
use tempfile::tempdir;
use toml::Value as TomlValue;

struct CwdGuard {
    previous: std::path::PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).expect("restore cwd");
    }
}

fn set_test_cwd(path: &std::path::Path) -> CwdGuard {
    let previous = std::env::current_dir().expect("read cwd");
    std::env::set_current_dir(path).expect("set cwd");
    CwdGuard { previous }
}

#[tokio::test]
#[serial]
async fn merges_managed_config_layer_on_top() {
    let tmp = tempdir().expect("tempdir");
    let _cwd = set_test_cwd(tmp.path());
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

    let state = load_config_layers_state(
        tmp.path(),
        tmp.path(),
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
#[serial]
async fn returns_empty_when_all_layers_missing() {
    let tmp = tempdir().expect("tempdir");
    let _cwd = set_test_cwd(tmp.path());
    let managed_path = tmp.path().join("managed_config.toml");
    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path),
        #[cfg(target_os = "macos")]
        managed_preferences_base64: None,
    };

    let layers = load_config_layers_state(
        tmp.path(),
        tmp.path(),
        &[] as &[(String, TomlValue)],
        overrides,
    )
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
#[serial]
async fn managed_preferences_take_highest_precedence() {
    use base64::Engine;

    let managed_payload = r#"
[nested]
value = "managed"
flag = false
"#;
    let encoded = base64::prelude::BASE64_STANDARD.encode(managed_payload.as_bytes());
    let tmp = tempdir().expect("tempdir");
    let _cwd = set_test_cwd(tmp.path());
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

    let state = load_config_layers_state(
        tmp.path(),
        tmp.path(),
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

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn loads_repo_local_config_from_cwd_only() -> anyhow::Result<()> {
    struct CwdGuard {
        previous: std::path::PathBuf,
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            // If this fails, we want the test to fail loudly rather than silently
            // leaking state into other tests.
            std::env::set_current_dir(&self.previous).expect("restore cwd");
        }
    }

    let tmp = tempdir()?;

    let codex_home = tmp.path().join("home");
    std::fs::create_dir_all(&codex_home)?;
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"value = "user"
"#,
    )?;

    let repo = tmp.path().join("repo");
    let nested = repo.join("a").join("b");
    std::fs::create_dir_all(&nested)?;

    std::fs::create_dir_all(repo.join(".codex"))?;
    std::fs::write(
        repo.join(".codex").join(CONFIG_TOML_FILE),
        r#"value = "root"
"#,
    )?;

    std::fs::create_dir_all(repo.join("a").join(".codex"))?;
    std::fs::write(
        repo.join("a").join(".codex").join(CONFIG_TOML_FILE),
        r#"value = "sub"
"#,
    )?;

    std::fs::create_dir_all(nested.join(".codex"))?;
    std::fs::write(
        nested.join(".codex").join(CONFIG_TOML_FILE),
        r#"value = "cwd"
"#,
    )?;

    let guard = CwdGuard {
        previous: std::env::current_dir()?,
    };
    std::env::set_current_dir(&nested)?;

    let overrides = LoaderOverrides {
        managed_config_path: Some(tmp.path().join("managed_config.toml")),
        #[cfg(target_os = "macos")]
        managed_preferences_base64: None,
    };

    let state = load_config_layers_state(
        &codex_home,
        &nested,
        &[] as &[(String, TomlValue)],
        overrides,
    )
    .await
    .expect("load config layers");

    assert!(
        state.get_user_layer().is_none(),
        "Codex-Mine policy: user layer should be ignored when cwd-local config exists"
    );

    let binding = state.effective_config();
    let table = binding.as_table().expect("top-level table expected");
    assert_eq!(
        table.get("value"),
        Some(&TomlValue::String("cwd".to_string()))
    );

    std::fs::remove_file(nested.join(".codex").join(CONFIG_TOML_FILE))?;
    let overrides = LoaderOverrides {
        managed_config_path: Some(tmp.path().join("managed_config.toml")),
        #[cfg(target_os = "macos")]
        managed_preferences_base64: None,
    };
    let state_no_cwd_local = load_config_layers_state(
        &codex_home,
        &nested,
        &[] as &[(String, TomlValue)],
        overrides,
    )
    .await
    .expect("load config layers");
    assert!(
        state_no_cwd_local.get_user_layer().is_some(),
        "Codex-Mine policy: user layer should load when no cwd-local config exists"
    );
    let binding = state_no_cwd_local.effective_config();
    let table = binding.as_table().expect("top-level table expected");
    assert_eq!(
        table.get("value"),
        Some(&TomlValue::String("user".to_string()))
    );

    drop(guard);
    Ok(())
}
