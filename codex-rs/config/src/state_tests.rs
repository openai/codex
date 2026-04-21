use super::*;
use pretty_assertions::assert_eq;

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
    let base_file = AbsolutePathBuf::from_absolute_path(PathBuf::from("/tmp/codex/config.toml"))
        .expect("absolute base path");
    let profile_file =
        AbsolutePathBuf::from_absolute_path(PathBuf::from("/tmp/codex/work.config.toml"))
            .expect("absolute profile path");
    let base_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User { file: base_file },
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
    let base_file = AbsolutePathBuf::from_absolute_path(PathBuf::from("/tmp/codex/config.toml"))
        .expect("absolute base path");
    let profile_file =
        AbsolutePathBuf::from_absolute_path(PathBuf::from("/tmp/codex/work.config.toml"))
            .expect("absolute profile path");
    let base_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: base_file.clone(),
        },
        toml::from_str(r#"model = "base""#).expect("base config"),
    );
    let profile_layer = ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: profile_file.clone(),
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
