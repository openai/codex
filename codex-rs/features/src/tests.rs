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
fn enable_fanout_normalization_enables_multi_agent_one_way() {
    let mut enable_fanout_features = Features::with_defaults();
    enable_fanout_features.enable(Feature::SpawnCsv);
    enable_fanout_features.normalize_dependencies();
    assert_eq!(enable_fanout_features.enabled(Feature::SpawnCsv), true);
    assert_eq!(enable_fanout_features.enabled(Feature::Collab), true);

    let mut collab_features = Features::with_defaults();
    collab_features.enable(Feature::Collab);
    collab_features.normalize_dependencies();
    assert_eq!(collab_features.enabled(Feature::Collab), true);
    assert_eq!(collab_features.enabled(Feature::SpawnCsv), false);
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
multi_agent_mode_hint_text = "Custom mode guidance."
tool_namespace = "agents"
hide_spawn_agent_metadata = true
expose_spawn_agent_model_overrides = true
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
            multi_agent_mode_hint_text: Some("Custom mode guidance.".to_string()),
            tool_namespace: Some("agents".to_string()),
            hide_spawn_agent_metadata: Some(true),
            expose_spawn_agent_model_overrides: Some(true),
            non_code_mode_only: Some(true),
        }))
    );
}

#[test]
fn materialize_resolved_enabled_writes_all_features_and_preserves_custom_config() {
    let mut features = Features::with_defaults();
    features.enable(Feature::CodeMode);
    features.enable(Feature::MultiAgentV2);
    features.enable(Feature::NetworkProxy);
    features.enable(Feature::RespectSystemProxy);

    let mut features_toml = FeaturesToml {
        multi_agent_v2: Some(FeatureToml::Config(crate::MultiAgentV2ConfigToml {
            enabled: Some(false),
            min_wait_timeout_ms: Some(2500),
            ..Default::default()
        })),
        network_proxy: Some(FeatureToml::Config(crate::NetworkProxyConfigToml {
            enabled: Some(false),
            proxy_url: Some("http://127.0.0.1:43128".to_string()),
            ..Default::default()
        })),
        entries: BTreeMap::new(),
        ..Default::default()
    };

    features_toml.materialize_resolved_enabled(&features);

    let entries = features_toml.entries();
    for spec in crate::FEATURES {
        assert_eq!(
            entries.get(spec.key),
            Some(&features.enabled(spec.id)),
            "{}",
            spec.key
        );
    }
    assert_eq!(
        features_toml.multi_agent_v2,
        Some(FeatureToml::Config(crate::MultiAgentV2ConfigToml {
            enabled: Some(true),
            min_wait_timeout_ms: Some(2500),
            ..Default::default()
        }))
    );
    assert_eq!(
        features_toml.network_proxy,
        Some(FeatureToml::Config(crate::NetworkProxyConfigToml {
            enabled: Some(true),
            proxy_url: Some("http://127.0.0.1:43128".to_string()),
            ..Default::default()
        }))
    );
    let replayed = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );
    assert_eq!(replayed.enabled(Feature::ApplyPatchFreeform), false);
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
fn unstable_warning_event_mentions_enabled_structured_under_development_feature() {
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
        "Under-development features enabled: code_mode, multi_agent_v2. Under-development features are incomplete and may behave unpredictably. To suppress this warning, set `suppress_unstable_features_warning = true` in /tmp/config.toml.".to_string(),
        message
    );
}
