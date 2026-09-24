use super::*;
use crate::config_toml::AgentsToml;
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
fn merge_toml_values_normalizes_legacy_agents_key_across_layers() {
    let mut base = parse_toml(
        r#"
[agents]
max_threads = 4
"#,
    );
    let overlay = parse_toml(
        r#"
[agents]
max_concurrent_threads_per_session = 7
"#,
    );

    merge_toml_values(&mut base, &overlay);

    let expected = parse_toml(
        r#"
[agents]
max_concurrent_threads_per_session = 7
"#,
    );
    assert_eq!(base, expected);

    let config: ConfigToml = base.try_into().expect("merged config should deserialize");
    assert_eq!(
        config.agents,
        Some(AgentsToml {
            max_concurrent_threads_per_session: Some(7),
            ..Default::default()
        })
    );
}

#[test]
fn merge_toml_values_normalizes_legacy_agents_key_from_overlay() {
    let mut base = parse_toml(
        r#"
[agents]
max_concurrent_threads_per_session = 4
"#,
    );
    let overlay = parse_toml(
        r#"
[agents]
max_threads = 7
"#,
    );

    merge_toml_values(&mut base, &overlay);

    let expected = parse_toml(
        r#"
[agents]
max_concurrent_threads_per_session = 7
"#,
    );
    assert_eq!(base, expected);
}

/// Feature tables added above legacy toggles retain the lower layer's enabled state.
#[test]
fn merge_multi_agent_v2_table_preserves_legacy_boolean_toggle() {
    for feature_path in ["features", "profiles.work.features"] {
        let mut base = parse_toml(&format!("[{feature_path}]\nmulti_agent_v2 = true\n"));
        let overlay = parse_toml(&format!(
            "[{feature_path}.multi_agent_v2]\nsubagent_usage_hint_text = \"Delegate carefully.\"\n",
        ));

        merge_toml_values(&mut base, &overlay);

        assert_eq!(
            base,
            parse_toml(&format!(
                "[{feature_path}.multi_agent_v2]\nenabled = true\nsubagent_usage_hint_text = \"Delegate carefully.\"\n",
            ))
        );
    }
}

/// Legacy feature toggles update enabled state without discarding nested configuration.
#[test]
fn merge_multi_agent_v2_boolean_preserves_existing_feature_table() {
    for feature_path in ["features", "profiles.work.features"] {
        let mut base = parse_toml(&format!(
            "[{feature_path}.multi_agent_v2]\nenabled = true\nsubagent_usage_hint_text = \"Delegate carefully.\"\n",
        ));
        let overlay = parse_toml(&format!("[{feature_path}]\nmulti_agent_v2 = false\n"));

        merge_toml_values(&mut base, &overlay);

        assert_eq!(
            base,
            parse_toml(&format!(
                "[{feature_path}.multi_agent_v2]\nenabled = false\nsubagent_usage_hint_text = \"Delegate carefully.\"\n",
            ))
        );
    }
}

/// Opaque desktop settings retain ordinary scalar/table replacement semantics.
#[test]
fn merge_multi_agent_v2_compatibility_excludes_opaque_desktop_paths() {
    let cases = [
        (
            "[desktop.features.multi_agent_v2]\nenabled = true\n",
            "[desktop.features]\nmulti_agent_v2 = false\n",
            "[desktop.features]\nmulti_agent_v2 = false\n",
        ),
        (
            "[desktop.features]\nmulti_agent_v2 = true\n",
            "[desktop.features.multi_agent_v2]\ncustom = true\n",
            "[desktop.features.multi_agent_v2]\ncustom = true\n",
        ),
    ];

    for (base, overlay, expected) in cases {
        let mut base = parse_toml(base);
        merge_toml_values(&mut base, &parse_toml(overlay));
        assert_eq!(base, parse_toml(expected));
    }
}

/// CLI overrides preserve the multi-agent toggle and nested options in either ordering.
#[test]
fn multi_agent_v2_cli_overrides_preserve_boolean_and_nested_configuration() {
    for feature_path in ["features", "profiles.work.features"] {
        let instructions = (
            format!("{feature_path}.multi_agent_v2.subagent_usage_hint_text"),
            TomlValue::String("Delegate carefully.".to_string()),
        );
        let enabled = (
            format!("{feature_path}.multi_agent_v2"),
            TomlValue::Boolean(true),
        );
        let feature_table = (
            format!("{feature_path}.multi_agent_v2"),
            parse_toml("subagent_usage_hint_text = \"Delegate carefully.\"\n"),
        );
        let expected = parse_toml(&format!(
            "[{feature_path}.multi_agent_v2]\nenabled = true\nsubagent_usage_hint_text = \"Delegate carefully.\"\n",
        ));

        for overrides in [
            vec![enabled.clone(), instructions.clone()],
            vec![instructions, enabled.clone()],
            vec![enabled.clone(), feature_table.clone()],
            vec![feature_table, enabled],
        ] {
            assert_eq!(crate::build_cli_overrides_layer(&overrides), expected);
        }
    }
}

#[test]
fn code_mode_overrides_preserve_enabled_state_and_cell_overhead() {
    for feature_path in ["features", "profiles.work.features"] {
        for enabled in [true, false] {
            let toggle = (
                format!("{feature_path}.code_mode"),
                TomlValue::Boolean(enabled),
            );
            let overhead = (
                format!("{feature_path}.code_mode.experimental_show_cell_overhead"),
                TomlValue::Boolean(true),
            );
            let expected = parse_toml(&format!(
                "[{feature_path}.code_mode]\nenabled = {enabled}\nexperimental_show_cell_overhead = true\n",
            ));

            // Adding a display option must preserve the toggle in either override order.
            for overrides in [
                vec![toggle.clone(), overhead.clone()],
                vec![overhead, toggle],
            ] {
                assert_eq!(crate::build_cli_overrides_layer(&overrides), expected);
            }

            let boolean_layer = parse_toml(&format!("[{feature_path}]\ncode_mode = {enabled}\n"));
            let table_layer = parse_toml(&format!(
                "[{feature_path}.code_mode]\nexperimental_show_cell_overhead = true\n",
            ));
            for (mut base, overlay) in [
                (boolean_layer.clone(), table_layer.clone()),
                (table_layer, boolean_layer),
            ] {
                merge_toml_values(&mut base, &overlay);
                assert_eq!(base, expected);
            }
        }
    }
}

#[test]
fn sleep_tool_overrides_preserve_disabled_state_and_mode() {
    for feature_path in ["features", "profiles.work.features"] {
        let disabled = (
            format!("{feature_path}.sleep_tool"),
            TomlValue::Boolean(false),
        );
        let mode = (
            format!("{feature_path}.sleep_tool.mode"),
            TomlValue::String("always_on".to_string()),
        );
        let expected = parse_toml(&format!(
            "[{feature_path}.sleep_tool]\nenabled = false\nmode = \"always_on\"\n",
        ));
        for overrides in [vec![disabled.clone(), mode.clone()], vec![mode, disabled]] {
            assert_eq!(crate::build_cli_overrides_layer(&overrides), expected);
        }

        let boolean_layer = parse_toml(&format!("[{feature_path}]\nsleep_tool = false\n"));
        let table_layer = parse_toml(&format!(
            "[{feature_path}.sleep_tool]\nmode = \"always_on\"\n",
        ));
        for (mut base, overlay) in [
            (boolean_layer.clone(), table_layer.clone()),
            (table_layer, boolean_layer),
        ] {
            merge_toml_values(&mut base, &overlay);
            assert_eq!(base, expected);
        }
    }
}

#[test]
fn network_proxy_feature_overrides_preserve_credential_broker_configuration() {
    let enabled = (
        "features.network_proxy".to_string(),
        TomlValue::Boolean(true),
    );
    let broker = (
        "features.network_proxy.credential_broker".to_string(),
        TomlValue::Boolean(true),
    );
    let expected =
        parse_toml("[features.network_proxy]\nenabled = true\ncredential_broker = true\n");

    for overrides in [vec![enabled.clone(), broker.clone()], vec![broker, enabled]] {
        assert_eq!(crate::build_cli_overrides_layer(&overrides), expected);
    }

    let mut base = parse_toml("[features]\nnetwork_proxy = true\n");
    merge_toml_values(
        &mut base,
        &parse_toml("[features.network_proxy]\ncredential_broker = true\n"),
    );
    assert_eq!(base, expected);
}

#[test]
fn higher_priority_credential_provider_replaces_overlapping_sources() {
    for (feature_path, base_source, overlay_source, keep_default) in [
        ("features", "VENDOR_TOKEN", "VENDOR_TOKEN", false),
        (
            "profiles.work.features",
            "VENDOR_TOKEN",
            "VENDOR_TOKEN",
            false,
        ),
        ("features", "vendor_token", "VENDOR_TOKEN", !cfg!(windows)),
    ] {
        let default_provider = format!(
            "[{feature_path}.network_proxy.credentials.default]\n\
             env = ['{base_source}']\n\
             url_prefixes = ['https://default.example']\n"
        );
        let shared_provider = format!(
            "[{feature_path}.network_proxy.credentials.shared]\n\
             env = ['SHARED_TOKEN']\n\
             patterns = ['shared_[a-z]+']\n"
        );
        let mut base = parse_toml(&format!("{default_provider}\n{shared_provider}"));
        let overlay = parse_toml(&format!(
            "[{feature_path}.network_proxy.credentials.override]\n\
             env = ['{overlay_source}']\n\
             url_prefixes = ['https://override.example']\n\
             [{feature_path}.network_proxy.credentials.shared]\n\
             url_prefixes = ['https://shared.example']\n"
        ));

        merge_toml_values(&mut base, &overlay);

        let retained_default = if keep_default { &default_provider } else { "" };
        let expected = parse_toml(&format!(
            "{retained_default}\n\
             [{feature_path}.network_proxy.credentials.override]\n\
             env = ['{overlay_source}']\n\
             url_prefixes = ['https://override.example']\n\
             [{feature_path}.network_proxy.credentials.shared]\n\
             env = ['SHARED_TOKEN']\n\
             patterns = ['shared_[a-z]+']\n\
             url_prefixes = ['https://shared.example']\n"
        ));
        assert_eq!(base, expected, "feature path: {feature_path}");
    }
}

#[test]
fn credential_provider_remapping_preserves_inherited_settings() {
    let base = parse_toml(
        r#"
[features.network_proxy.credentials.a]
env = ["OLD_AUTH"]
patterns = ["acme_[a-z]{24}"]
url_prefixes = ["https://a.example"]
auth = ["token"]
[features.network_proxy.credentials.b]
env = ["OTHER_AUTH"]
patterns = ["other_[a-z]{24}"]
url_prefixes = ["https://b.example"]
auth = ["bearer"]
"#,
    );

    for new_a in ["NEW_AUTH", "OTHER_AUTH"] {
        let mut merged = base.clone();
        let mut expected = base.clone();
        let providers = &mut expected["features"]["network_proxy"]["credentials"];
        providers["a"]["env"] = TomlValue::Array(vec![new_a.into()]);
        providers["b"]["env"] = TomlValue::Array(vec!["OLD_AUTH".into()]);
        let overlay = parse_toml(&format!(
            "[features.network_proxy.credentials.a]\n\
             env = ['{new_a}']\n\
             [features.network_proxy.credentials.b]\n\
             env = ['OLD_AUTH']\n"
        ));

        merge_toml_values(&mut merged, &overlay);

        assert_eq!(merged, expected);
    }
}

/// Repeated opaque desktop overrides continue to replace their previous value.
#[test]
fn multi_agent_v2_cli_compatibility_excludes_opaque_desktop_paths() {
    let path = "desktop.features.multi_agent_v2".to_string();
    let enabled = (path.clone(), TomlValue::Boolean(true));
    let feature_table = (path, parse_toml("custom = true\n"));

    assert_eq!(
        crate::build_cli_overrides_layer(&[enabled.clone(), feature_table.clone()]),
        parse_toml("[desktop.features.multi_agent_v2]\ncustom = true\n")
    );
    assert_eq!(
        crate::build_cli_overrides_layer(&[feature_table, enabled]),
        parse_toml("[desktop.features]\nmulti_agent_v2 = true\n")
    );
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
fn shell_environment_policy_set_overlay_preserves_case_distinct_keys() {
    let mut base = parse_toml(
        r#"
[features.network_proxy.credentials.stripe]
env = ["STRIPE_API_KEY"]
patterns = ["stripe_[a-z]+"]
url_prefix_from_env = "STRIPE_HOST"

[shell_environment_policy.set]
stripe_api_key = "stale"
stripe_host = "stale.example"
foo = "lowercase"
"#,
    );
    let overlay = parse_toml(
        r#"
[shell_environment_policy.set]
STRIPE_API_KEY = "trusted"
STRIPE_HOST = "trusted.example"
FOO = "uppercase"
"#,
    );

    merge_toml_values(&mut base, &overlay);

    let values = base["shell_environment_policy"]["set"]
        .as_table()
        .expect("shell environment overrides");
    assert_eq!(
        values.get("foo").and_then(TomlValue::as_str),
        Some("lowercase")
    );
    assert_eq!(
        values.get("FOO").and_then(TomlValue::as_str),
        Some("uppercase")
    );
    assert_eq!(values.len(), 6);
}

#[test]
fn later_credential_provider_preserves_case_distinct_environment_overrides() {
    let mut base = parse_toml(
        "[shell_environment_policy.set]\n\
         stripe_api_key = 'stale'\n\
         stripe_host = 'stale.example'",
    );
    let override_layer = parse_toml(
        "[shell_environment_policy.set]\n\
         STRIPE_API_KEY = 'previous'\n\
         STRIPE_HOST = 'previous.example'",
    );
    let final_override_layer = parse_toml(
        "[shell_environment_policy.set]\n\
         stripe_api_key = 'trusted'\n\
         stripe_host = 'current.example'",
    );
    let provider_layer = parse_toml(
        "[features.network_proxy.credentials.stripe]\n\
         env = ['STRIPE_API_KEY']\n\
         url_prefix_from_env = 'STRIPE_HOST'",
    );

    merge_toml_values(&mut base, &override_layer);
    merge_toml_values(&mut base, &final_override_layer);
    merge_toml_values(&mut base, &provider_layer);

    assert_eq!(
        base["shell_environment_policy"]["set"],
        parse_toml(
            "stripe_api_key = 'trusted'\n\
             stripe_host = 'current.example'\n\
             STRIPE_API_KEY = 'previous'\n\
             STRIPE_HOST = 'previous.example'",
        )
    );
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
fn shell_environment_policy_filters_overlay_merges_unicode_keys_case_insensitively() {
    let mut base = parse_toml(
        r#"
[shell_environment_policy.filters]
"СЕКРЕТ_*" = "exclude"
"#,
    );
    let overlay = parse_toml(
        r#"
[shell_environment_policy.filters]
"секрет_*" = "include"
"#,
    );

    merge_toml_values(&mut base, &overlay);

    assert_eq!(base, overlay);
}

#[test]
fn shell_environment_policy_filters_replace_lower_legacy_filter_fields() {
    let mut base = parse_toml(
        r#"
[shell_environment_policy]
inherit = "core"
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
[shell_environment_policy]
inherit = "core"

[shell_environment_policy.filters]
"ADD_INCLUDED" = "include"
"FLIP_TO_EXCLUDE" = "exclude"
"FLIP_TO_INCLUDE" = "include"
"#,
        )
    );
}

#[test]
fn shell_environment_policy_legacy_arrays_replace_lower_filters() {
    let mut base = parse_toml(
        r#"
[shell_environment_policy]
inherit = "core"

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
inherit = "core"
exclude = ["FLIP_TO_EXCLUDE", "HIGH_EXCLUDED"]
"#,
        )
    );
}

#[test]
fn empty_shell_environment_filter_representations_replace_the_other_form() {
    let cases = [
        (
            r#"[shell_environment_policy]
exclude = ["AWS_*"]
include_only = ["PATH"]
"#,
            r#"[shell_environment_policy.filters]
"#,
        ),
        (
            r#"[shell_environment_policy.filters]
"AWS_*" = "include"
"#,
            r#"[shell_environment_policy]
exclude = []
"#,
        ),
    ];

    for (base, overlay) in cases {
        let mut base = parse_toml(base);
        let overlay = parse_toml(overlay);

        merge_toml_values(&mut base, &overlay);

        assert_eq!(base, overlay);
    }
}

#[test]
fn sparse_overlay_preserves_unvisited_base_aliases() {
    let mut base = parse_toml(
        "[memories]\nno_memories_if_mcp_or_web_search = false\n[agents]\nmax_threads = 2",
    );
    let overlay = parse_toml("[agents]\nmax_depth = 3");

    merge_toml_values(&mut base, &overlay);

    assert_eq!(
        base,
        parse_toml(
            "[memories]\nno_memories_if_mcp_or_web_search = false\n[agents]\nmax_concurrent_threads_per_session = 2\nmax_depth = 3",
        ),
    );
}
