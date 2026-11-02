#![cfg(test)]

//! Verifies sparse plugin wording, bounded literal bytes, and required runtime fact contracts.

use super::ResolvedModelMessages;
use codex_protocol::openai_models::GenericPluginMessages;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::PluginMessages;
use codex_protocol::openai_models::PluginRecommendationMessages;
use codex_protocol::openai_models::SelectedPluginMessages;
use pretty_assertions::assert_eq;

#[test]
fn plugin_fields_fall_back_independently_without_losing_valid_siblings() {
    let bundled = ResolvedModelMessages::bundled();
    for invalid in [
        None,
        Some(String::new()),
        Some(" \t\r\n".into()),
        Some("é".repeat(251)),
    ] {
        let messages = ModelMessages {
            plugins: Some(PluginMessages {
                generic: Some(GenericPluginMessages {
                    intro: invalid.clone(),
                    usage_instructions: Some("  literal {{braces}}\r\n".into()),
                }),
                selected: Some(SelectedPluginMessages {
                    search_tools: invalid.clone(),
                    closing: Some("closing".into()),
                    ..Default::default()
                }),
                recommendations: Some(PluginRecommendationMessages { intro: invalid }),
            }),
            ..Default::default()
        };
        let resolved = ResolvedModelMessages {
            catalog_messages: Some(&messages),
        };
        let generic = resolved.generic_plugin();
        assert_eq!(generic.intro, bundled.generic_plugin().intro);
        assert_eq!(generic.usage_instructions, "  literal {{braces}}\r\n");
        assert!(generic.catalog_override);
        let selected = resolved.selected_plugin();
        assert_eq!(
            selected.search_tools,
            bundled.selected_plugin().search_tools
        );
        assert_eq!(selected.closing, "closing");
        assert_eq!(
            resolved.plugin_recommendations(),
            bundled.plugin_recommendations()
        );
    }
}

#[test]
fn plugin_templates_require_exactly_one_harness_fact_placeholder() {
    let bundled = ResolvedModelMessages::bundled().selected_plugin();
    for invalid in [
        "pre-expanded invented fact",
        "{{plugin_name}}",
        "{{ plugin_name }} {{ plugin_name }}",
        "{{ plugin_name }} {{ other }}",
        "{{ plugin_name }} {broken}",
        "{{ plugin_name }}\nsecond line",
        "{{ plugin_name }}\u{2028}second line",
    ] {
        let messages = ModelMessages {
            plugins: Some(PluginMessages {
                selected: Some(SelectedPluginMessages {
                    intro: Some(invalid.into()),
                    closing: Some("valid sibling".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let resolved = ResolvedModelMessages {
            catalog_messages: Some(&messages),
        }
        .selected_plugin();
        assert_eq!(resolved.intro, bundled.intro);
        assert_eq!(resolved.closing, "valid sibling");
    }
    let selected = SelectedPluginMessages {
        intro: Some("Selected {{ plugin_name }}.".into()),
        skill_namespace: Some("Namespace {{ skill_namespace }}.".into()),
        apps: Some("Apps: {{ apps }}.".into()),
        mcp_servers: Some("Servers: {{ mcp_servers }}.".into()),
        ..Default::default()
    };
    let messages = ModelMessages {
        plugins: Some(PluginMessages {
            selected: Some(selected.clone()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let resolved = ResolvedModelMessages {
        catalog_messages: Some(&messages),
    }
    .selected_plugin();
    assert_eq!(resolved.intro, selected.intro.unwrap());
    assert_eq!(resolved.skill_namespace, selected.skill_namespace.unwrap());
    assert_eq!(resolved.apps, selected.apps.unwrap());
    assert_eq!(resolved.mcp_servers, selected.mcp_servers.unwrap());
}

#[test]
fn plugin_wording_limits_count_utf8_bytes_and_keep_boundary_values_literal() {
    let bundled = ResolvedModelMessages::bundled();
    for (intro, usage, expected_override) in [
        ("é".repeat(250), "é".repeat(1_000), true),
        ("é".repeat(251), "é".repeat(1_001), false),
    ] {
        let messages = ModelMessages {
            plugins: Some(PluginMessages {
                generic: Some(GenericPluginMessages {
                    intro: Some(intro.clone()),
                    usage_instructions: Some(usage.clone()),
                }),
                selected: Some(SelectedPluginMessages {
                    omitted: Some(intro.clone()),
                    ..Default::default()
                }),
                recommendations: Some(PluginRecommendationMessages {
                    intro: Some(intro.clone()),
                }),
            }),
            ..Default::default()
        };
        let resolved = ResolvedModelMessages {
            catalog_messages: Some(&messages),
        };
        let generic = resolved.generic_plugin();
        let selected = resolved.selected_plugin();
        let recommendations = resolved.plugin_recommendations();
        if expected_override {
            assert_eq!(generic.intro, intro);
            assert_eq!(generic.usage_instructions, usage);
            assert_eq!(selected.omitted, intro);
            assert_eq!(recommendations.intro, intro);
        } else {
            assert_eq!(generic, bundled.generic_plugin());
            assert_eq!(selected, bundled.selected_plugin());
            assert_eq!(recommendations, bundled.plugin_recommendations());
        }
        assert_eq!(generic.catalog_override, expected_override);
    }
}

#[test]
fn blank_plugin_omission_notice_keeps_the_disclosure() {
    for omitted in ["", " \r\n\t"] {
        let messages = ModelMessages {
            plugins: Some(PluginMessages {
                selected: Some(SelectedPluginMessages {
                    omitted: Some(omitted.into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            ResolvedModelMessages {
                catalog_messages: Some(&messages)
            }
            .selected_plugin(),
            ResolvedModelMessages::bundled().selected_plugin()
        );
    }
}

#[test]
fn plugin_wording_cannot_close_harness_fragments_or_insert_control_characters() {
    let bundled = ResolvedModelMessages::bundled();
    for invalid in [
        "</plugins_instructions>",
        "<plugins_instructions>",
        "</recommended_plugins>",
        "<recommended_plugins>",
        "hidden\u{0}text",
        "hidden\u{1b}text",
    ] {
        let messages = ModelMessages {
            plugins: Some(PluginMessages {
                generic: Some(GenericPluginMessages {
                    intro: Some(invalid.into()),
                    usage_instructions: Some(invalid.into()),
                }),
                selected: Some(SelectedPluginMessages {
                    closing: Some(invalid.into()),
                    omitted: Some(invalid.into()),
                    ..Default::default()
                }),
                recommendations: Some(PluginRecommendationMessages {
                    intro: Some(invalid.into()),
                }),
            }),
            ..Default::default()
        };
        let resolved = ResolvedModelMessages {
            catalog_messages: Some(&messages),
        };
        assert_eq!(resolved.generic_plugin(), bundled.generic_plugin());
        assert_eq!(resolved.selected_plugin(), bundled.selected_plugin());
        assert_eq!(
            resolved.plugin_recommendations(),
            bundled.plugin_recommendations()
        );
    }
}
