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

/// Legacy feature toggles own the semantic enabled leaf after layered merging.
#[test]
fn origins_attribute_multi_agent_v2_enabled_to_overriding_boolean_layer() {
    let temp_dir = TempDir::new().expect("tempdir");
    let user_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: test_user_config_path(&temp_dir, "config.toml"),
            profile: None,
        },
        toml::from_str(
            "[features.multi_agent_v2]\nenabled = true\nsubagent_usage_hint_text = \"keep\"\n",
        )
        .expect("user config"),
    );
    let user_metadata = user_layer.metadata();
    let session_layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::from_str("[features]\nmulti_agent_v2 = false\n").expect("session config"),
    );
    let session_metadata = session_layer.metadata();
    let stack = ConfigLayerStack::new(
        vec![user_layer, session_layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("layer stack should be valid");

    let origins = stack.origins();

    assert_eq!(
        origins.get("features.multi_agent_v2.enabled"),
        Some(&session_metadata)
    );
    assert_eq!(
        origins.get("features.multi_agent_v2.subagent_usage_hint_text"),
        Some(&user_metadata)
    );
}

#[test]
fn enabled_layers_validate_shell_environment_policy() {
    let layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::from_str(
            r#"
[shell_environment_policy]
exclude = ["LEGACY_*"]

[shell_environment_policy.filters]
"CANONICAL_*" = "include"
"#,
        )
        .expect("session config"),
    );

    let error = ConfigLayerStack::new(
        vec![layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect_err("enabled layers should be validated");

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("cannot mix `filters` with legacy `exclude` or `include_only`")
    );
}

#[test]
fn disabled_layers_do_not_validate_shell_environment_policy() {
    let layer = ConfigLayerEntry::new_disabled(
        ConfigLayerSource::Project {
            dot_codex_folder: AbsolutePathBuf::from_absolute_path("/untrusted/.codex")
                .expect("project path should be absolute"),
        },
        toml::from_str(
            r#"
[shell_environment_policy]
exclude = ["LEGACY_*"]

[shell_environment_policy.filters]
"CANONICAL_*" = "include"
"#,
        )
        .expect("project config"),
        "project is untrusted",
    );

    ConfigLayerStack::new(
        vec![layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("disabled layers should not be validated");
}

#[test]
fn enabled_layers_only_validate_representation_sensitive_shell_policy_fields() {
    let cases = [
        r#"shell_environment_policy = 17"#,
        r#"
[shell_environment_policy]
inherit = "invalid"
set = ["invalid"]
"#,
    ];

    for contents in cases {
        let layer = ConfigLayerEntry::new(
            ConfigLayerSource::SessionFlags,
            toml::from_str(contents).expect("session config"),
        );

        ConfigLayerStack::new(
            vec![layer],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("unrelated shell policy fields should retain normal overlay semantics");
    }
}

#[test]
fn with_user_config_rejects_malformed_shell_policy_filter_fields() {
    let temp_dir = TempDir::new().expect("tempdir");
    let config_file = test_user_config_path(&temp_dir, "config.toml");
    let cases = [
        r#"
[shell_environment_policy]
exclude = ["SECRET_*", 17]
"#,
        r#"
[shell_environment_policy.filters]
"SECRET_*" = "keep"
"#,
        r#"
[shell_environment_policy]
exclude = ["SECRET_*"]

[shell_environment_policy.filters]
"PATH" = "include"
"#,
        r#"
[shell_environment_policy.filters]
"SECRET_*" = "exclude"
"secret_*" = "include"
"#,
    ];

    for contents in cases {
        let error = ConfigLayerStack::default()
            .with_user_config(&config_file, toml::from_str(contents).expect("user config"))
            .expect_err("malformed shell policy filter fields should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
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
approval_policy = "on-request"
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
        Some("on-request")
    );
}

#[test]
fn layer_iterators_preserve_precedence_and_disabled_layers() {
    let temp_dir = TempDir::new().expect("tempdir");
    let user_source = ConfigLayerSource::User {
        file: test_user_config_path(&temp_dir, "config.toml"),
        profile: None,
    };
    let project_source = ConfigLayerSource::Project {
        dot_codex_folder: test_user_config_path(&temp_dir, ".codex"),
    };
    let session_source = ConfigLayerSource::SessionFlags;
    let empty_config = TomlValue::Table(toml::map::Map::new());
    let stack = ConfigLayerStack::new(
        vec![
            ConfigLayerEntry::new(user_source.clone(), empty_config.clone()),
            ConfigLayerEntry::new_disabled(
                project_source.clone(),
                empty_config.clone(),
                "project is untrusted",
            ),
            ConfigLayerEntry::new(session_source.clone(), empty_config),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("layer stack should be valid");

    assert_eq!(
        stack
            .layers_low_to_high()
            .map(|layer| &layer.name)
            .collect::<Vec<_>>(),
        vec![&user_source, &session_source]
    );
    assert_eq!(
        stack
            .layers_high_to_low()
            .map(|layer| &layer.name)
            .collect::<Vec<_>>(),
        vec![&session_source, &user_source]
    );
    assert_eq!(
        stack
            .all_layers_low_to_high()
            .map(|layer| &layer.name)
            .collect::<Vec<_>>(),
        vec![&user_source, &project_source, &session_source]
    );
    assert_eq!(
        stack
            .all_layers_high_to_low()
            .map(|layer| &layer.name)
            .collect::<Vec<_>>(),
        vec![&session_source, &project_source, &user_source]
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
        toml::from_str(r#"approval_policy = "on-request""#).expect("profile config"),
    );
    let stack = ConfigLayerStack::new(
        vec![base_layer, profile_layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("multiple user layers should be valid");

    let updated = stack
        .with_user_config(
            &base_file,
            toml::from_str(r#"model = "updated-base""#).expect("updated base config"),
        )
        .expect("updated user layer should be valid");

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
        Some("on-request")
    );
}
