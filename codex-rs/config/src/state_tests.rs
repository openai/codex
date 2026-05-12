use super::*;
use codex_utils_absolute_path::AbsolutePathBuf;
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
fn config_toml_layers_ignore_remote_control_feature_override() {
    let layer = ConfigLayerEntry::new(
        ConfigLayerSource::System {
            file: AbsolutePathBuf::resolve_path_against_base("config.toml", std::env::temp_dir()),
        },
        toml::from_str(
            r#"
[features]
plugins = true
remote_control = true
"#,
        )
        .expect("config TOML should parse"),
    );
    let stack = ConfigLayerStack::new(
        vec![layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("single layer stack should be valid");

    let effective = stack.effective_config();
    let features = effective
        .get("features")
        .and_then(TomlValue::as_table)
        .expect("features table should be present");
    assert_eq!(features.get("plugins"), Some(&TomlValue::Boolean(true)));
    assert_eq!(features.get("remote_control"), None);
}

#[test]
fn session_flags_preserve_remote_control_feature_override() {
    let layer = ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        toml::from_str(
            r#"
[features]
remote_control = true
"#,
        )
        .expect("config TOML should parse"),
    );
    let stack = ConfigLayerStack::new(
        vec![layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("single layer stack should be valid");

    let effective = stack.effective_config();
    let features = effective
        .get("features")
        .and_then(TomlValue::as_table)
        .expect("features table should be present");
    assert_eq!(
        features.get("remote_control"),
        Some(&TomlValue::Boolean(true))
    );
}
