use super::LoaderOverrides;
use super::load_config_layers_state;
use crate::config::CONFIG_TOML_FILE;
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

    let state =
        load_config_layers_state(tmp.path(), None, &[] as &[(String, TomlValue)], overrides)
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

    let layers =
        load_config_layers_state(tmp.path(), None, &[] as &[(String, TomlValue)], overrides)
            .await
            .expect("load layers");
    let base_table = layers.user.config.as_table().expect("base table expected");
    assert!(
        base_table.is_empty(),
        "expected empty base layer when configs missing"
    );
    assert!(
        layers.system.is_none(),
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

#[tokio::test]
async fn repo_local_config_toml_overrides_user_config_toml() {
    let codex_home = tempdir().expect("tempdir codex_home");
    let managed_path = codex_home.path().join("managed_config.toml");
    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path),
        #[cfg(target_os = "macos")]
        managed_preferences_base64: None,
    };

    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"foo = 1

[nested]
value = "base"
"#,
    )
    .expect("write base");

    let repo = tempdir().expect("tempdir repo");
    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .expect("git init");

    std::fs::create_dir_all(repo.path().join(".codex")).expect("create .codex");
    std::fs::write(
        repo.path().join(".codex").join(CONFIG_TOML_FILE),
        r#"foo = 2

[nested]
value = "repo"
"#,
    )
    .expect("write repo config");

    let state = load_config_layers_state(
        codex_home.path(),
        Some(repo.path()),
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
        Some(&TomlValue::String("repo".to_string()))
    );
}

#[tokio::test]
async fn repo_local_mcp_servers_replace_user_mcp_servers() {
    let codex_home = tempdir().expect("tempdir codex_home");
    let managed_path = codex_home.path().join("managed_config.toml");
    let overrides = LoaderOverrides {
        managed_config_path: Some(managed_path),
        #[cfg(target_os = "macos")]
        managed_preferences_base64: None,
    };

    std::fs::write(
        codex_home.path().join(CONFIG_TOML_FILE),
        r#"
[mcp_servers.global]
command = "echo"
"#,
    )
    .expect("write base");

    let repo = tempdir().expect("tempdir repo");
    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .expect("git init");

    std::fs::create_dir_all(repo.path().join(".codex")).expect("create .codex");
    std::fs::write(
        repo.path().join(".codex").join(CONFIG_TOML_FILE),
        r#"
[mcp_servers.project]
command = "pwd"
"#,
    )
    .expect("write repo config");

    let state = load_config_layers_state(
        codex_home.path(),
        Some(repo.path()),
        &[] as &[(String, TomlValue)],
        overrides,
    )
    .await
    .expect("load config");

    let loaded = state.effective_config();
    let mcp_servers = loaded
        .get("mcp_servers")
        .and_then(|value| value.as_table())
        .expect("mcp_servers table");

    assert!(
        !mcp_servers.contains_key("global"),
        "repo-local mcp_servers should replace user config mcp_servers"
    );
    assert!(mcp_servers.contains_key("project"));
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

    let state =
        load_config_layers_state(tmp.path(), None, &[] as &[(String, TomlValue)], overrides)
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
