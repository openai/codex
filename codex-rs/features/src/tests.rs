use crate::Feature;
use crate::FeatureConfigSource;
use crate::FeatureOverrides;
use crate::FeatureToml;
use crate::Features;
use crate::FeaturesToml;
use crate::Stage;
use crate::feature_for_key;
use crate::unstable_features_warning_event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::WarningEvent;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use toml::Table;
use toml::Value as TomlValue;

#[test]
fn under_development_features_are_disabled_by_default() {
    for spec in crate::FEATURES {
        if matches!(spec.stage, Stage::UnderDevelopment) {
            assert_eq!(
                spec.default_enabled, false,
                "feature `{}` is under development and must be disabled by default",
                spec.key
            );
        }
    }
}

#[test]
fn tool_registry_config_is_not_a_feature_toggle() {
    let features: FeaturesToml = toml::from_str(
        "[tool_registry]\nerror_on_tool_collisions = true\nturn_metadata_includes_tool_info = true\n",
    )
    .expect("tool registry settings should deserialize");

    assert_eq!(
        features.tool_registry,
        Some(crate::ToolRegistryConfigToml {
            error_on_tool_collisions: Some(true),
            turn_metadata_includes_tool_info: Some(true),
        })
    );
    assert!(features.entries().is_empty());
    assert!(crate::is_known_feature_key("tool_registry"));
    assert_eq!(feature_for_key("tool_registry"), None);
}

#[test]
fn executor_capability_discovery_is_an_opt_in_map_feature() {
    let mut features = Features::with_defaults();
    assert!(!features.enabled(Feature::ExecutorCapabilityDiscovery));

    features.apply_map(&BTreeMap::from([(
        "executor_capability_discovery".to_string(),
        true,
    )]));

    assert!(features.enabled(Feature::ExecutorCapabilityDiscovery));
}

#[test]
fn default_enabled_features_are_stable() {
    for spec in crate::FEATURES {
        if spec.default_enabled {
            assert!(
                matches!(spec.stage, Stage::Stable | Stage::Removed),
                "feature `{}` is enabled by default but is not stable/removed ({:?})",
                spec.key,
                spec.stage
            );
        }
    }
}

#[test]
fn removed_apps_mcp_path_override_shapes_are_ignored() {
    let features = [
        toml::from_str::<FeaturesToml>("apps_mcp_path_override = true")
            .expect("boolean compatibility form should deserialize"),
        toml::from_str::<FeaturesToml>(
            r#"
[apps_mcp_path_override]
enabled = true
path = "/custom/mcp"
"#,
        )
        .expect("structured compatibility form should deserialize"),
    ];

    assert_eq!(
        features.map(|features| features.entries()),
        [BTreeMap::new(), BTreeMap::new()]
    );
}

#[test]
fn code_mode_only_requires_code_mode() {
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeModeOnly);
    features.normalize_dependencies();

    assert_eq!(features.enabled(Feature::CodeModeOnly), true);
    assert_eq!(features.enabled(Feature::CodeMode), true);
}

#[test]
fn code_mode_host_feature_config_preserves_boolean_toggle() {
    let features: FeaturesToml =
        toml::from_str("code_mode_host = false").expect("features table should deserialize");

    assert_eq!(features.code_mode_host, Some(FeatureToml::Enabled(false)));
    assert_eq!(
        features.entries(),
        BTreeMap::from([("code_mode_host".to_string(), false)])
    );
}

#[test]
fn guardian_v2_feature_config_preserves_boolean_toggle() {
    let features: FeaturesToml =
        toml::from_str("guardianv2 = true").expect("Guardian v2 boolean should deserialize");

    assert_eq!(features.guardianv2, Some(FeatureToml::Enabled(true)));
    assert_eq!(
        features.entries(),
        BTreeMap::from([("guardianv2".to_owned(), true)])
    );
}

#[test]
fn guardian_v2_feature_config_deserializes_classifier_and_transcript_settings() {
    let features: FeaturesToml = toml::from_str(
        r#"
[guardianv2]
enabled = true
classifier_instructions = "Review this action"
review_threshold = 0.65
reasoning_effort = "minimal"
max_action_tokens = 512
max_classifier_instruction_tokens = 256

[guardianv2.transcript]
sources = ["tool_outputs", "reasoning"]
max_message_entry_tokens = 128
max_tool_entry_tokens = 128
max_message_transcript_tokens = 512
max_tool_transcript_tokens = 256
max_recent_non_user_entries = 12
"#,
    )
    .expect("Guardian v2 settings should deserialize");

    assert_eq!(
        features.guardianv2,
        Some(FeatureToml::Config(crate::GuardianV2ConfigToml {
            enabled: Some(true),
            classifier_instructions: Some("Review this action".to_owned()),
            review_threshold: Some(0.65),
            reasoning_effort: Some(codex_protocol::openai_models::ReasoningEffort::Minimal),
            max_action_tokens: Some(512),
            max_classifier_instruction_tokens: Some(256),
            transcript: Some(crate::GuardianV2TranscriptConfigToml {
                sources: Some(vec![
                    crate::GuardianV2TranscriptSource::ToolOutputs,
                    crate::GuardianV2TranscriptSource::Reasoning,
                ]),
                max_message_entry_tokens: Some(128),
                max_tool_entry_tokens: Some(128),
                max_message_transcript_tokens: Some(512),
                max_tool_transcript_tokens: Some(256),
                max_recent_non_user_entries: Some(12),
            }),
        }))
    );
    assert_eq!(
        features.entries(),
        BTreeMap::from([("guardianv2".to_owned(), true)])
    );
}

#[test]
fn guardian_v2_feature_config_rejects_invalid_settings() {
    for setting in [
        "max_action_tokens",
        "max_classifier_instruction_tokens",
        "transcript.max_message_entry_tokens",
        "transcript.max_tool_entry_tokens",
        "transcript.max_message_transcript_tokens",
        "transcript.max_tool_transcript_tokens",
    ] {
        for value in [99, 100_001] {
            let settings = format!("[guardianv2]\n{setting} = {value}");
            assert!(
                toml::from_str::<FeaturesToml>(&settings).is_err(),
                "out-of-range Guardian v2 setting should fail: {setting} = {value}"
            );
        }
    }

    for settings in [
        "review_threshold = -0.1",
        "review_threshold = 1.1",
        "review_threshold = nan",
        "transcript.max_recent_non_user_entries = 0",
        "transcript.max_message_entry_tokens = 200\ntranscript.max_message_transcript_tokens = 100",
        "transcript.max_tool_entry_tokens = 200\ntranscript.max_tool_transcript_tokens = 100",
    ] {
        assert!(
            toml::from_str::<FeaturesToml>(&format!("[guardianv2]\n{settings}")).is_err(),
            "invalid Guardian v2 settings should fail: {settings}"
        );
    }
}

#[test]
fn guardian_v2_feature_config_accepts_boundary_values() {
    for (threshold, tokens) in [(0.0, 100), (1.0, 100_000)] {
        let features: FeaturesToml = toml::from_str(&format!(
            "[guardianv2]\n\
             review_threshold = {threshold:.1}\n\
             max_action_tokens = {tokens}\n\
             max_classifier_instruction_tokens = {tokens}\n\
             [guardianv2.transcript]\n\
             max_message_entry_tokens = {tokens}\n\
             max_tool_entry_tokens = {tokens}\n\
             max_message_transcript_tokens = {tokens}\n\
             max_tool_transcript_tokens = {tokens}\n\
             max_recent_non_user_entries = 1"
        ))
        .expect("Guardian v2 boundary settings should deserialize");

        assert!(matches!(features.guardianv2, Some(FeatureToml::Config(_))));
    }

    for setting in [
        "max_message_transcript_tokens",
        "max_tool_transcript_tokens",
    ] {
        let features: FeaturesToml =
            toml::from_str(&format!("[guardianv2.transcript]\n{setting} = 100"))
                .expect("partial Guardian v2 transcript settings should deserialize");

        assert!(matches!(features.guardianv2, Some(FeatureToml::Config(_))));
    }
}

#[test]
fn code_mode_host_feature_config_deserializes_fallback_setting() {
    let features: FeaturesToml = toml::from_str(
        r#"
[code_mode_host]
enabled = true
disable_in_process_fallback = true
"#,
    )
    .expect("features table should deserialize");

    assert_eq!(
        features.code_mode_host,
        Some(FeatureToml::Config(crate::CodeModeHostConfigToml {
            enabled: Some(true),
            disable_in_process_fallback: Some(true),
        }))
    );
    assert_eq!(
        features.entries(),
        BTreeMap::from([("code_mode_host".to_string(), true)])
    );
}

#[test]
fn from_sources_ignores_removed_terminal_resize_reflow_feature_key() {
    let features_toml = FeaturesToml::from(BTreeMap::from([(
        "terminal_resize_reflow".to_string(),
        false,
    )]));

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features, Features::with_defaults());
    assert_eq!(features.enabled(Feature::TerminalResizeReflow), true);
}

#[test]
fn image_generation_extension_alias_is_supported() {
    assert_eq!(
        feature_for_key("imagegenext"),
        Some(Feature::ImageGeneration)
    );
}

#[test]
fn image_generation_toggle_controls_extension_backed_generation() {
    let mut entries = BTreeMap::new();
    entries.insert("image_generation".to_string(), false);
    let mut features = Features::with_defaults();
    features.apply_map(&entries);
    assert!(!features.enabled(Feature::ImageGeneration));

    entries.insert("image_generation".to_string(), true);
    features.disable(Feature::ImageGeneration);
    features.apply_map(&entries);
    assert!(features.enabled(Feature::ImageGeneration));
}

#[test]
fn canonical_image_generation_toggle_wins_over_extension_alias() {
    for (canonical, alias) in [(false, true), (true, false)] {
        let entries = BTreeMap::from([
            ("image_generation".to_string(), canonical),
            ("imagegenext".to_string(), alias),
        ]);
        let mut features = Features::with_defaults();
        features.apply_map(&entries);
        assert_eq!(features.enabled(Feature::ImageGeneration), canonical);
    }
}

#[test]
fn use_legacy_landlock_config_records_deprecation_notice() {
    let mut entries = BTreeMap::new();
    entries.insert("use_legacy_landlock".to_string(), true);

    let mut features = Features::with_defaults();
    features.apply_map(&entries);

    let usages = features.legacy_feature_usages().collect::<Vec<_>>();
    assert_eq!(usages.len(), 1);
    assert_eq!(usages[0].alias, "features.use_legacy_landlock");
    assert_eq!(usages[0].feature, Feature::UseLegacyLandlock);
    assert_eq!(
        usages[0].summary,
        "`[features].use_legacy_landlock` is deprecated and will be removed soon."
    );
    assert_eq!(
        usages[0].details.as_deref(),
        Some("Remove this setting to stop opting into the legacy Linux sandbox behavior.")
    );
}

#[test]
fn remote_control_config_is_ignored() {
    let mut entries = BTreeMap::new();
    entries.insert("remote_control".to_string(), true);

    let mut features = Features::with_defaults();
    features.apply_map(&entries);

    assert_eq!(features.enabled(Feature::RemoteControl), false);
}

#[test]
fn telepathy_is_legacy_alias_for_chronicle() {
    assert_eq!(feature_for_key("chronicle"), Some(Feature::Chronicle));
    assert_eq!(feature_for_key("telepathy"), Some(Feature::Chronicle));
}

#[test]
fn collab_is_legacy_alias_for_multi_agent() {
    assert_eq!(feature_for_key("multi_agent"), Some(Feature::Collab));
    assert_eq!(feature_for_key("collab"), Some(Feature::Collab));
}

#[test]
fn codex_hooks_is_legacy_alias_for_hooks() {
    assert_eq!(feature_for_key("hooks"), Some(Feature::CodexHooks));
    assert_eq!(feature_for_key("codex_hooks"), Some(Feature::CodexHooks));
}

#[test]
fn apps_require_feature_flag_and_chatgpt_auth() {
    let mut features = Features::with_defaults();
    assert!(!features.apps_enabled_for_auth(/*has_chatgpt_auth*/ false));

    features.enable(Feature::Apps);
    assert!(!features.apps_enabled_for_auth(/*has_chatgpt_auth*/ false));
    assert!(features.apps_enabled_for_auth(/*has_chatgpt_auth*/ true));
}

#[test]
fn from_sources_applies_base_profile_and_overrides() {
    let mut base_entries = BTreeMap::new();
    base_entries.insert("plugins".to_string(), true);
    let base_features = FeaturesToml {
        entries: base_entries,
        ..Default::default()
    };

    let mut profile_entries = BTreeMap::new();
    profile_entries.insert("code_mode_only".to_string(), true);
    let profile_features = FeaturesToml {
        entries: profile_entries,
        ..Default::default()
    };

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&base_features),
            ..Default::default()
        },
        FeatureConfigSource {
            features: Some(&profile_features),
            ..Default::default()
        },
        FeatureOverrides {
            web_search_request: Some(false),
        },
    );

    assert_eq!(features.enabled(Feature::Plugins), true);
    assert_eq!(features.enabled(Feature::CodeModeOnly), true);
    assert_eq!(features.enabled(Feature::CodeMode), true);
    assert_eq!(features.enabled(Feature::ApplyPatchFreeform), false);
    assert_eq!(features.enabled(Feature::WebSearchRequest), false);
}

#[test]
fn from_sources_ignores_removed_image_detail_original_feature_key() {
    let features_toml = FeaturesToml::from(BTreeMap::from([(
        "image_detail_original".to_string(),
        true,
    )]));

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features, Features::with_defaults());
}

#[test]
fn from_sources_ignores_removed_resize_all_images_feature_key() {
    let features_toml =
        FeaturesToml::from(BTreeMap::from([("resize_all_images".to_string(), false)]));

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features, Features::with_defaults());
}

#[test]
fn from_sources_ignores_removed_item_ids_feature_key() {
    let features_toml = FeaturesToml::from(BTreeMap::from([("item_ids".to_string(), false)]));

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features, Features::with_defaults());
    assert_eq!(features.enabled(Feature::ItemIds), true);
}

#[test]
fn from_sources_ignores_removed_undo_feature_key() {
    let features_toml = FeaturesToml::from(BTreeMap::from([("undo".to_string(), true)]));

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features, Features::with_defaults());
}

#[test]
fn from_sources_ignores_removed_js_repl_feature_keys() {
    let features_toml = FeaturesToml::from(BTreeMap::from([
        ("js_repl".to_string(), true),
        ("js_repl_tools_only".to_string(), true),
    ]));

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features, Features::with_defaults());
}

#[test]
fn from_sources_ignores_removed_apply_patch_freeform_feature_key() {
    let features_toml =
        FeaturesToml::from(BTreeMap::from([("apply_patch_freeform".to_string(), true)]));

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features, Features::with_defaults());
}

#[test]
fn from_sources_ignores_removed_plugin_hooks_feature_key() {
    let features_toml = FeaturesToml::from(BTreeMap::from([("plugin_hooks".to_string(), true)]));

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features, Features::with_defaults());
}

#[test]
fn from_sources_ignores_removed_tool_search_always_defer_mcp_tools_feature_key() {
    let features_toml = FeaturesToml::from(BTreeMap::from([(
        "tool_search_always_defer_mcp_tools".to_string(),
        false,
    )]));

    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features, Features::with_defaults());
}

#[test]
fn multi_agent_v2_feature_config_deserializes_boolean_toggle() {
    let features: FeaturesToml = toml::from_str(
        r#"
multi_agent_v2 = true
"#,
    )
    .expect("features table should deserialize");

    assert_eq!(
        features.entries(),
        BTreeMap::from([("multi_agent_v2".to_string(), true)])
    );
    assert_eq!(features.multi_agent_v2, Some(FeatureToml::Enabled(true)));
}

#[test]
fn multi_agent_v2_feature_config_deserializes_table() {
    let features: FeaturesToml = toml::from_str(
        r#"
[multi_agent_v2]
enabled = true
max_concurrent_threads_per_session = 4
min_wait_timeout_ms = 2500
max_wait_timeout_ms = 120000
default_wait_timeout_ms = 30000
usage_hint_enabled = false
usage_hint_text = "Custom delegation guidance."
root_agent_usage_hint_text = "Root guidance."
subagent_usage_hint_text = "Subagent guidance."
subagent_developer_instructions = "Delegate carefully."
multi_agent_mode_hint_text = "Custom mode guidance."
tool_namespace = "agents"
hide_spawn_agent_metadata = true
expose_spawn_agent_model_overrides = true
wait_agent_enabled = false
non_code_mode_only = true
"#,
    )
    .expect("features table should deserialize");

    assert_eq!(
        features.entries(),
        BTreeMap::from([("multi_agent_v2".to_string(), true)])
    );
    assert_eq!(
        features.multi_agent_v2,
        Some(crate::FeatureToml::Config(crate::MultiAgentV2ConfigToml {
            enabled: Some(true),
            max_concurrent_threads_per_session: Some(4),
            min_wait_timeout_ms: Some(2500),
            max_wait_timeout_ms: Some(120000),
            default_wait_timeout_ms: Some(30000),
            usage_hint_enabled: Some(false),
            usage_hint_text: Some("Custom delegation guidance.".to_string()),
            root_agent_usage_hint_text: Some("Root guidance.".to_string()),
            subagent_usage_hint_text: Some("Subagent guidance.".to_string()),
            subagent_developer_instructions: Some("Delegate carefully.".to_string()),
            multi_agent_mode_hint_text: Some("Custom mode guidance.".to_string()),
            tool_namespace: Some("agents".to_string()),
            hide_spawn_agent_metadata: Some(true),
            expose_spawn_agent_model_overrides: Some(true),
            wait_agent_enabled: Some(false),
            non_code_mode_only: Some(true),
        }))
    );
}

#[test]
fn non_prefixed_mcp_tool_names_feature_config_deserializes_boolean_toggle() {
    let features: FeaturesToml = toml::from_str("non_prefixed_mcp_tool_names = true")
        .expect("features table should deserialize");

    assert_eq!(
        features.entries(),
        BTreeMap::from([("non_prefixed_mcp_tool_names".to_string(), true)])
    );
    assert_eq!(
        features.non_prefixed_mcp_tool_names,
        Some(FeatureToml::Enabled(true))
    );
}

#[test]
fn non_prefixed_mcp_tool_names_feature_config_deserializes_table() {
    let features: FeaturesToml = toml::from_str(
        r#"
[non_prefixed_mcp_tool_names]
enabled = true
server_names = ["history", "notes"]
"#,
    )
    .expect("features table should deserialize");

    assert_eq!(
        features.entries(),
        BTreeMap::from([("non_prefixed_mcp_tool_names".to_string(), true)])
    );
    assert_eq!(
        features.non_prefixed_mcp_tool_names,
        Some(FeatureToml::Config(
            crate::NonPrefixedMcpToolNamesConfigToml {
                enabled: Some(true),
                server_names: Some(vec!["history".to_string(), "notes".to_string()]),
            }
        ))
    );
}

#[test]
fn unstable_warning_event_only_mentions_enabled_under_development_features() {
    let mut configured_features = Table::new();
    configured_features.insert(
        "apply_patch_streaming_events".to_string(),
        TomlValue::Boolean(true),
    );
    configured_features.insert("personality".to_string(), TomlValue::Boolean(true));
    configured_features.insert("unknown".to_string(), TomlValue::Boolean(true));

    let mut features = Features::with_defaults();
    features.enable(Feature::ApplyPatchStreamingEvents);

    let warning = unstable_features_warning_event(
        Some(&configured_features),
        /*suppress_unstable_features_warning*/ false,
        &features,
        "/tmp/config.toml",
    )
    .expect("warning event");

    let EventMsg::Warning(WarningEvent { message }) = warning.msg else {
        panic!("expected warning event");
    };
    assert!(message.contains("apply_patch_streaming_events"));
    assert!(!message.contains("personality"));
    assert!(message.contains("/tmp/config.toml"));
}

#[test]
fn unstable_warning_event_ignores_enabled_structured_stable_feature() {
    let configured_features: Table = toml::from_str(
        r#"
multi_agent_v2 = { enabled = true, tool_namespace = "agents" }
code_mode = true
"#,
    )
    .expect("features table should deserialize");

    let mut features = Features::with_defaults();
    features.enable(Feature::MultiAgentV2);
    features.enable(Feature::CodeMode);

    let warning = unstable_features_warning_event(
        Some(&configured_features),
        /*suppress_unstable_features_warning*/ false,
        &features,
        "/tmp/config.toml",
    )
    .expect("warning event");

    let EventMsg::Warning(WarningEvent { message }) = warning.msg else {
        panic!("expected warning event");
    };
    assert_eq!(
        "Under-development features enabled: code_mode. Under-development features are incomplete and may behave unpredictably. To suppress this warning, set `suppress_unstable_features_warning = true` in /tmp/config.toml.".to_string(),
        message
    );
}
