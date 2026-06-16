use super::*;
use crate::config_toml::ConfigToml;
use crate::types::MemoriesToml;
use pretty_assertions::assert_eq;

fn parse_toml(value: &str) -> TomlValue {
    toml::from_str(value).expect("TOML should parse")
}

#[test]
fn merge_toml_values_normalizes_legacy_key_from_base_layer() {
    let mut base = parse_toml(
        r#"
[memories]
no_memories_if_mcp_or_web_search = false
"#,
    );
    let overlay = parse_toml(
        r#"
[memories]
disable_on_external_context = true
"#,
    );

    merge_toml_values(&mut base, &overlay);

    let expected = parse_toml(
        r#"
[memories]
disable_on_external_context = true
"#,
    );
    assert_eq!(base, expected);

    let config: ConfigToml = base.try_into().expect("merged config should deserialize");
    assert_eq!(
        config.memories,
        Some(MemoriesToml {
            disable_on_external_context: Some(true),
            ..Default::default()
        })
    );
}

#[test]
fn merge_toml_values_normalizes_legacy_key_from_overlay_layer() {
    let mut base = parse_toml(
        r#"
[memories]
disable_on_external_context = false
"#,
    );
    let overlay = parse_toml(
        r#"
[memories]
no_memories_if_mcp_or_web_search = true
"#,
    );

    merge_toml_values(&mut base, &overlay);

    let expected = parse_toml(
        r#"
[memories]
disable_on_external_context = true
"#,
    );
    assert_eq!(base, expected);

    let config: ConfigToml = base.try_into().expect("merged config should deserialize");
    assert_eq!(
        config.memories,
        Some(MemoriesToml {
            disable_on_external_context: Some(true),
            ..Default::default()
        })
    );
}

#[test]
fn merge_toml_values_prefers_canonical_key_when_one_layer_has_both_names() {
    let mut base = TomlValue::Table(toml::map::Map::new());
    let overlay = parse_toml(
        r#"
[memories]
disable_on_external_context = true
no_memories_if_mcp_or_web_search = false
"#,
    );

    merge_toml_values(&mut base, &overlay);

    let expected = parse_toml(
        r#"
[memories]
disable_on_external_context = true
"#,
    );
    assert_eq!(base, expected);
}

#[test]
fn merge_toml_values_normalizes_permission_network_domains_before_overlaying() {
    let mut base = parse_toml(
        r#"
[permissions.dev.network.domains]
"example.com" = "deny"
"#,
    );
    let overlay = parse_toml(
        r#"
[permissions.dev.network.domains]
"EXAMPLE.COM" = "allow"
"#,
    );

    merge_toml_values(&mut base, &overlay);

    let expected = parse_toml(
        r#"
[permissions.dev.network.domains]
"example.com" = "allow"
"#,
    );
    assert_eq!(base, expected);
}

#[test]
fn shell_environment_policy_legacy_array_overlay_replaces_legacy_array() {
    let mut base = parse_toml(
        r#"
[shell_environment_policy]
exclude = ["LOW_*", "SHARED_*"]
"#,
    );
    let overlay = parse_toml(
        r#"
[shell_environment_policy]
exclude = ["HIGH_*"]
"#,
    );

    merge_toml_values(&mut base, &overlay);

    assert_eq!(base, overlay);
}

#[test]
fn shell_environment_policy_filters_overlay_merges_by_key_case_insensitively() {
    let mut base = parse_toml(
        r#"
[shell_environment_policy.filters]
"FLIP_*" = "exclude"
"KEEP_*" = "include"
"#,
    );
    let overlay = parse_toml(
        r#"
[shell_environment_policy.filters]
"ADD_*" = "exclude"
"flip_*" = "include"
"#,
    );

    merge_toml_values(&mut base, &overlay);

    assert_eq!(
        base,
        parse_toml(
            r#"
[shell_environment_policy.filters]
"add_*" = "exclude"
"flip_*" = "include"
"keep_*" = "include"
"#,
        )
    );
}

#[test]
fn shell_environment_policy_filters_override_lower_legacy_arrays_by_pattern() {
    let mut base = parse_toml(
        r#"
[shell_environment_policy]
exclude = ["FLIP_TO_INCLUDE", "KEEP_EXCLUDED"]
include_only = ["FLIP_TO_EXCLUDE", "KEEP_INCLUDED"]
"#,
    );
    let overlay = parse_toml(
        r#"
[shell_environment_policy.filters]
"ADD_INCLUDED" = "include"
"FLIP_TO_EXCLUDE" = "exclude"
"FLIP_TO_INCLUDE" = "include"
"#,
    );

    merge_toml_values(&mut base, &overlay);

    assert_eq!(
        base,
        parse_toml(
            r#"
[shell_environment_policy.filters]
"add_included" = "include"
"flip_to_exclude" = "exclude"
"flip_to_include" = "include"
"keep_excluded" = "exclude"
"keep_included" = "include"
"#,
        )
    );
}

#[test]
fn shell_environment_policy_legacy_array_replaces_lower_filters_for_its_action() {
    let mut base = parse_toml(
        r#"
[shell_environment_policy.filters]
"FLIP_TO_EXCLUDE" = "include"
"LOW_EXCLUDED" = "exclude"
"KEEP_INCLUDED" = "include"
"#,
    );
    let overlay = parse_toml(
        r#"
[shell_environment_policy]
exclude = ["FLIP_TO_EXCLUDE", "HIGH_EXCLUDED"]
"#,
    );

    merge_toml_values(&mut base, &overlay);

    assert_eq!(
        base,
        parse_toml(
            r#"
[shell_environment_policy]
exclude = ["FLIP_TO_EXCLUDE", "HIGH_EXCLUDED"]
include_only = ["keep_included"]
"#,
        )
    );
}
