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
fn default_enabled_features_are_stable() {
    for spec in crate::FEATURES {
        if spec.default_enabled {
            assert!(
                matches!(spec.stage, Stage::Stable | Stage::Removed)
                    || spec.id == Feature::TerminalResizeReflow,
                "feature `{}` is enabled by default but is not stable/removed ({:?})",
                spec.key,
                spec.stage
            );
        }
    }
}

#[test]
fn use_legacy_landlock_is_deprecated_and_disabled_by_default() {
    assert_eq!(Feature::UseLegacyLandlock.stage(), Stage::Deprecated);
    assert_eq!(Feature::UseLegacyLandlock.default_enabled(), false);
}

#[test]
fn use_linux_sandbox_bwrap_is_removed_and_disabled_by_default() {
    assert_eq!(Feature::UseLinuxSandboxBwrap.stage(), Stage::Removed);
    assert_eq!(Feature::UseLinuxSandboxBwrap.default_enabled(), false);
}

#[test]
fn image_detail_original_is_removed_and_disabled_by_default() {
    assert_eq!(Feature::ImageDetailOriginal.stage(), Stage::Removed);
    assert_eq!(Feature::ImageDetailOriginal.default_enabled(), false);
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
fn guardian_approval_is_stable_and_enabled_by_default() {
    let spec = Feature::GuardianApproval.info();

    assert_eq!(spec.stage, Stage::Stable);
    assert_eq!(Feature::GuardianApproval.default_enabled(), true);
}

#[test]
fn external_migration_is_experimental_and_disabled_by_default() {
    let spec = Feature::ExternalMigration.info();
    let stage = spec.stage;

    assert!(matches!(stage, Stage::Experimental { .. }));
    assert_eq!(stage.experimental_menu_name(), Some("External migration"));
    assert_eq!(
        stage.experimental_menu_description(),
        Some(
            "Show a startup prompt when Codex detects migratable external agent config for this machine or project."
        )
    );
    assert_eq!(stage.experimental_announcement(), None);
    assert_eq!(Feature::ExternalMigration.default_enabled(), false);
}

#[test]
fn request_permissions_is_under_development() {
    assert_eq!(
        Feature::ExecPermissionApprovals.stage(),
        Stage::UnderDevelopment
    );
    assert_eq!(Feature::ExecPermissionApprovals.default_enabled(), false);
}

#[test]
fn request_permissions_tool_is_under_development() {
    assert_eq!(
        Feature::RequestPermissionsTool.stage(),
        Stage::UnderDevelopment
    );
    assert_eq!(Feature::RequestPermissionsTool.default_enabled(), false);
}

#[test]
fn terminal_resize_reflow_is_experimental_and_enabled_by_default() {
    assert_eq!(
        feature_for_key("terminal_resize_reflow"),
        Some(Feature::TerminalResizeReflow)
    );
    assert!(matches!(
        Feature::TerminalResizeReflow.stage(),
        Stage::Experimental { .. }
    ));
    assert_eq!(Feature::TerminalResizeReflow.default_enabled(), true);
}

#[test]
fn tool_suggest_is_stable_and_enabled_by_default() {
    assert_eq!(Feature::ToolSuggest.stage(), Stage::Stable);
    assert_eq!(Feature::ToolSuggest.default_enabled(), true);
}

#[test]
fn tool_search_is_stable_and_enabled_by_default() {
    assert_eq!(Feature::ToolSearch.stage(), Stage::Stable);
    assert_eq!(Feature::ToolSearch.default_enabled(), true);
}

#[test]
fn browser_controls_are_stable_and_enabled_by_default() {
    assert_eq!(Feature::InAppBrowser.stage(), Stage::Stable);
    assert_eq!(Feature::InAppBrowser.default_enabled(), true);
    assert_eq!(
        feature_for_key("in_app_browser"),
        Some(Feature::InAppBrowser)
    );

    assert_eq!(Feature::BrowserUse.stage(), Stage::Stable);
    assert_eq!(Feature::BrowserUse.default_enabled(), true);
    assert_eq!(feature_for_key("browser_use"), Some(Feature::BrowserUse));

    assert_eq!(Feature::ComputerUse.stage(), Stage::Stable);
    assert_eq!(Feature::ComputerUse.default_enabled(), true);
    assert_eq!(feature_for_key("computer_use"), Some(Feature::ComputerUse));
}

#[test]
fn use_linux_sandbox_bwrap_is_a_removed_feature_key() {
    assert_eq!(
        feature_for_key("use_legacy_landlock"),
        Some(Feature::UseLegacyLandlock)
    );
    assert_eq!(
        feature_for_key("use_linux_sandbox_bwrap"),
        Some(Feature::UseLinuxSandboxBwrap)
    );
}

#[test]
fn image_generation_is_stable_and_enabled_by_default() {
    assert_eq!(Feature::ImageGeneration.stage(), Stage::Stable);
    assert_eq!(Feature::ImageGeneration.default_enabled(), true);
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
fn image_detail_original_is_a_removed_feature_key() {
    assert_eq!(
        feature_for_key("image_detail_original"),
        Some(Feature::ImageDetailOriginal)
    );
}

#[test]
fn js_repl_features_are_removed_feature_keys() {
    assert_eq!(Feature::JsRepl.stage(), Stage::Removed);
    assert_eq!(Feature::JsRepl.default_enabled(), false);
    assert_eq!(feature_for_key("js_repl"), Some(Feature::JsRepl));

    assert_eq!(Feature::JsReplToolsOnly.stage(), Stage::Removed);
    assert_eq!(Feature::JsReplToolsOnly.default_enabled(), false);
    assert_eq!(
        feature_for_key("js_repl_tools_only"),
        Some(Feature::JsReplToolsOnly)
    );
}

#[test]
fn tool_call_mcp_elicitation_is_stable_and_enabled_by_default() {
    assert_eq!(Feature::ToolCallMcpElicitation.stage(), Stage::Stable);
    assert_eq!(Feature::ToolCallMcpElicitation.default_enabled(), true);
}

#[test]
fn remote_control_is_under_development() {
    assert_eq!(Feature::RemoteControl.stage(), Stage::UnderDevelopment);
    assert_eq!(Feature::RemoteControl.default_enabled(), false);
}

#[test]
fn workspace_dependencies_is_stable_and_enabled_by_default() {
    assert_eq!(Feature::WorkspaceDependencies.stage(), Stage::Stable);
    assert_eq!(Feature::WorkspaceDependencies.default_enabled(), true);
    assert_eq!(
        feature_for_key("workspace_dependencies"),
        Some(Feature::WorkspaceDependencies)
    );
}

#[test]
fn telepathy_is_legacy_alias_for_chronicle() {
    assert_eq!(Feature::Chronicle.stage(), Stage::UnderDevelopment);
    assert_eq!(Feature::Chronicle.default_enabled(), false);
    assert_eq!(feature_for_key("chronicle"), Some(Feature::Chronicle));
    assert_eq!(feature_for_key("telepathy"), Some(Feature::Chronicle));
}

#[test]
fn collab_is_legacy_alias_for_multi_agent() {
    assert_eq!(feature_for_key("multi_agent"), Some(Feature::Collab));
    assert_eq!(feature_for_key("collab"), Some(Feature::Collab));
}

#[test]
fn multi_agent_is_stable_and_enabled_by_default() {
    assert_eq!(Feature::Collab.stage(), Stage::Stable);
    assert_eq!(Feature::Collab.default_enabled(), true);
}

#[test]
fn enable_fanout_is_under_development() {
    assert_eq!(Feature::SpawnCsv.stage(), Stage::UnderDevelopment);
    assert_eq!(Feature::SpawnCsv.default_enabled(), false);
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
            include_apply_patch_tool: Some(true),
            ..Default::default()
        },
        FeatureOverrides {
            web_search_request: Some(false),
            ..Default::default()
        },
    );

    assert_eq!(features.enabled(Feature::Plugins), true);
    assert_eq!(features.enabled(Feature::CodeModeOnly), true);
    assert_eq!(features.enabled(Feature::CodeMode), true);
    assert_eq!(features.enabled(Feature::ApplyPatchFreeform), true);
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
usage_hint_enabled = false
usage_hint_text = "Custom delegation guidance."
hide_spawn_agent_metadata = true
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
            usage_hint_enabled: Some(false),
            usage_hint_text: Some("Custom delegation guidance.".to_string()),
            hide_spawn_agent_metadata: Some(true),
        }))
    );
}

#[test]
fn multi_agent_v2_feature_config_usage_hint_enabled_does_not_enable_feature() {
    let features_toml: FeaturesToml = toml::from_str(
        r#"
[multi_agent_v2]
usage_hint_enabled = false
"#,
    )
    .expect("features table should deserialize");
    let features = Features::from_sources(
        FeatureConfigSource {
            features: Some(&features_toml),
            ..Default::default()
        },
        FeatureConfigSource::default(),
        FeatureOverrides::default(),
    );

    assert_eq!(features.enabled(Feature::MultiAgentV2), false);
    assert_eq!(features_toml.entries(), BTreeMap::new());
    assert_eq!(
        features_toml.multi_agent_v2,
        Some(crate::FeatureToml::Config(crate::MultiAgentV2ConfigToml {
            enabled: None,
            max_concurrent_threads_per_session: None,
            usage_hint_enabled: Some(false),
            usage_hint_text: None,
            hide_spawn_agent_metadata: None,
        }))
    );
}

#[test]
fn unstable_warning_event_only_mentions_enabled_under_development_features() {
    let mut configured_features = Table::new();
    configured_features.insert("child_agents_md".to_string(), TomlValue::Boolean(true));
    configured_features.insert("personality".to_string(), TomlValue::Boolean(true));
    configured_features.insert("unknown".to_string(), TomlValue::Boolean(true));

    let mut features = Features::with_defaults();
    features.enable(Feature::ChildAgentsMd);

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
    assert!(message.contains("child_agents_md"));
    assert!(!message.contains("personality"));
    assert!(message.contains("/tmp/config.toml"));
}
