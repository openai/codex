use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn test_user_config_path(temp_dir: &TempDir, file_name: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path(temp_dir.path().join(file_name))
        .expect("test user config path should be absolute")
}

#[test]
fn origins_use_canonical_key_aliases() {
    let layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::from_str(
            r#"
[memories]
no_memories_if_mcp_or_web_search = true
"#,
        )
        .expect("config TOML should parse"),
    );
    let metadata = layer.metadata();
    let stack = ConfigLayerStack::new(
        vec![layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("single layer stack should be valid");

    let origins = stack.origins();

    assert_eq!(
        origins.get("memories.disable_on_external_context"),
        Some(&metadata)
    );
    assert!(
        !origins.contains_key("memories.no_memories_if_mcp_or_web_search"),
        "legacy key should be canonicalized before origin recording"
    );
}

#[test]
fn active_user_layer_is_highest_precedence_user_layer() {
    let temp_dir = TempDir::new().expect("tempdir");
    let base_file = test_user_config_path(&temp_dir, "config.toml");
    let profile_file = test_user_config_path(&temp_dir, "work.config.toml");
    let base_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: base_file,
            profile: None,
        },
        toml::from_str(
            r#"
model = "base"
approval_policy = "on-failure"
"#,
        )
        .expect("base config"),
    );
    let profile_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: profile_file.clone(),
            profile: Some("work".to_string()),
        },
        toml::from_str(r#"model = "profile""#).expect("profile config"),
    );
    let stack = ConfigLayerStack::new(
        vec![base_layer, profile_layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("multiple user layers should be valid");

    assert_eq!(stack.get_user_config_file(), Some(&profile_file));
    assert_eq!(
        stack
            .effective_user_config()
            .expect("merged user config")
            .get("model")
            .and_then(toml::Value::as_str),
        Some("profile")
    );
    assert_eq!(
        stack
            .effective_user_config()
            .expect("merged user config")
            .get("approval_policy")
            .and_then(toml::Value::as_str),
        Some("on-failure")
    );
}

#[test]
fn with_user_config_updates_matching_user_layer_without_replacing_active_profile() {
    let temp_dir = TempDir::new().expect("tempdir");
    let base_file = test_user_config_path(&temp_dir, "config.toml");
    let profile_file = test_user_config_path(&temp_dir, "work.config.toml");
    let base_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: base_file.clone(),
            profile: None,
        },
        toml::from_str(r#"model = "base""#).expect("base config"),
    );
    let profile_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: profile_file.clone(),
            profile: Some("work".to_string()),
        },
        toml::from_str(r#"approval_policy = "on-failure""#).expect("profile config"),
    );
    let stack = ConfigLayerStack::new(
        vec![base_layer, profile_layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("multiple user layers should be valid");

    let updated = stack.with_user_config(
        &base_file,
        toml::from_str(r#"model = "updated-base""#).expect("updated base config"),
    );

    assert_eq!(updated.get_user_config_file(), Some(&profile_file));
    assert_eq!(
        updated
            .effective_user_config()
            .expect("merged user config")
            .get("model")
            .and_then(toml::Value::as_str),
        Some("updated-base")
    );
    assert_eq!(
        updated
            .effective_user_config()
            .expect("merged user config")
            .get("approval_policy")
            .and_then(toml::Value::as_str),
        Some("on-failure")
    );
}

#[test]
fn project_override_layer_must_immediately_follow_its_project_layer() {
    let temp_dir = TempDir::new().expect("tempdir");
    let dot_codex_folder = AbsolutePathBuf::from_absolute_path(temp_dir.path().join(".codex"))
        .expect("project config folder should be absolute");
    let empty_config = || toml::Value::Table(toml::map::Map::new());
    let project_override = || {
        ConfigLayerEntry::new(
            ConfigLayerSource::ProjectOverride {
                dot_codex_folder: dot_codex_folder.clone(),
            },
            empty_config(),
        )
    };
    let err = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::Project {
                    dot_codex_folder: dot_codex_folder.clone(),
                },
                empty_config(),
            ),
            project_override(),
            project_override(),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect_err("duplicate project override layer should be rejected");

    assert_eq!(
        err.to_string(),
        "project override layer must follow its project layer"
    );
}
