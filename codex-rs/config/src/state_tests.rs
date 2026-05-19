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
fn effective_user_config_merges_user_override_after_base_layer() {
    let temp_dir = TempDir::new().expect("tempdir");
    let base_file = test_user_config_path(&temp_dir, "config.toml");
    let override_file = test_user_config_path(&temp_dir, "config.override.toml");
    let stack = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: base_file,
                    profile: None,
                },
                toml::from_str(r#"model = "base""#).expect("base config"),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::UserOverride {
                    file: override_file,
                },
                toml::from_str(r#"model = "override""#).expect("override config"),
            ),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("user override stack should be valid");

    assert_eq!(
        stack
            .effective_user_config()
            .expect("merged user config")
            .get("model")
            .and_then(toml::Value::as_str),
        Some("override")
    );
}

#[test]
fn with_user_config_preserves_user_override_precedence() {
    let temp_dir = TempDir::new().expect("tempdir");
    let base_file = test_user_config_path(&temp_dir, "config.toml");
    let override_file = test_user_config_path(&temp_dir, "config.override.toml");
    let stack = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: base_file.clone(),
                    profile: None,
                },
                toml::from_str(r#"model = "base""#).expect("base config"),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::UserOverride {
                    file: override_file,
                },
                toml::from_str(r#"model = "override""#).expect("override config"),
            ),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("user override stack should be valid");

    let updated = stack.with_user_config(
        &base_file,
        toml::from_str(r#"model = "updated-base""#).expect("updated base config"),
    );

    let user_layers = updated
        .get_user_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ false,
        )
        .into_iter()
        .map(|layer| layer.name.clone())
        .collect::<Vec<_>>();
    assert!(matches!(
        user_layers.as_slice(),
        [
            ConfigLayerSource::User { .. },
            ConfigLayerSource::UserOverride { .. },
        ]
    ));
    assert_eq!(
        updated
            .effective_user_config()
            .expect("merged user config")
            .get("model")
            .and_then(toml::Value::as_str),
        Some("override")
    );
}

#[test]
fn with_user_config_preserves_selected_non_profile_user_precedence() {
    let temp_dir = TempDir::new().expect("tempdir");
    let base_file = test_user_config_path(&temp_dir, "config.toml");
    let override_file = test_user_config_path(&temp_dir, "config.override.toml");
    let selected_file = test_user_config_path(&temp_dir, "selected.toml");
    let stack = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: base_file,
                    profile: None,
                },
                toml::from_str(r#"model = "base""#).expect("base config"),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::UserOverride {
                    file: override_file,
                },
                toml::from_str(r#"model = "override""#).expect("override config"),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: selected_file.clone(),
                    profile: None,
                },
                toml::from_str(r#"model = "selected""#).expect("selected config"),
            ),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("selected user stack should be valid");

    let updated = stack.with_user_config(
        &selected_file,
        toml::from_str(r#"model = "selected-updated""#).expect("updated selected config"),
    );
    assert_eq!(
        updated
            .effective_user_config()
            .expect("merged user config")
            .get("model")
            .and_then(toml::Value::as_str),
        Some("selected-updated")
    );
}

#[test]
fn with_user_override_config_preserves_selected_non_profile_user_precedence() {
    let temp_dir = TempDir::new().expect("tempdir");
    let base_file = test_user_config_path(&temp_dir, "config.toml");
    let override_file = test_user_config_path(&temp_dir, "config.override.toml");
    let selected_file = test_user_config_path(&temp_dir, "selected.toml");
    let stack = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: base_file,
                    profile: None,
                },
                toml::from_str(r#"model = "base""#).expect("base config"),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: selected_file,
                    profile: None,
                },
                toml::from_str(r#"model = "selected""#).expect("selected config"),
            ),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("selected user stack should be valid");

    let updated = stack.with_user_override_config(
        &override_file,
        toml::from_str(r#"model = "override""#).expect("override config"),
    );

    let user_layers = updated
        .get_user_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ false,
        )
        .into_iter()
        .map(|layer| layer.name.clone())
        .collect::<Vec<_>>();
    assert!(matches!(
        user_layers.as_slice(),
        [
            ConfigLayerSource::User { .. },
            ConfigLayerSource::UserOverride { .. },
            ConfigLayerSource::User { .. },
        ]
    ));
    assert_eq!(
        updated
            .effective_user_config()
            .expect("merged user config")
            .get("model")
            .and_then(toml::Value::as_str),
        Some("selected")
    );
}

#[test]
fn stack_rejects_base_user_config_after_sibling_override() {
    let temp_dir = TempDir::new().expect("tempdir");
    let base_file = test_user_config_path(&temp_dir, "config.toml");
    let override_file = test_user_config_path(&temp_dir, "config.override.toml");
    let err = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(
                ConfigLayerSource::UserOverride {
                    file: override_file,
                },
                toml::from_str(r#"model = "override""#).expect("override config"),
            ),
            ConfigLayerEntry::new(
                ConfigLayerSource::User {
                    file: base_file,
                    profile: None,
                },
                toml::from_str(r#"model = "base""#).expect("base config"),
            ),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect_err("base user config should not follow its sibling override");

    assert_eq!(
        err.to_string(),
        "user layers are not ordered from config.toml to config.override.toml"
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
