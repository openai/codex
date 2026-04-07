use crate::Feature;
use crate::FeatureConfigSource;
use crate::FeatureOverrides;
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
                matches!(spec.stage, Stage::Stable | Stage::Removed),
                "feature `{}` is enabled by default but is not stable/removed ({:?})",
                spec.key,
                spec.stage
            );
        }
    }
}

#[test]
fn use_legacy_landlock_is_stable_and_disabled_by_default() {
    assert_eq!(Feature::UseLegacyLandlock.stage(), Stage::Stable);
    assert_eq!(Feature::UseLegacyLandlock.default_enabled(), false);
}

#[test]
fn use_linux_sandbox_bwrap_is_removed_and_disabled_by_default() {
    assert_eq!(Feature::UseLinuxSandboxBwrap.stage(), Stage::Removed);
    assert_eq!(Feature::UseLinuxSandboxBwrap.default_enabled(), false);
}

#[test]
fn js_repl_is_experimental_and_user_toggleable() {
    let spec = Feature::JsRepl.info();
    let stage = spec.stage;
    let expected_node_version = include_str!("../../node-version.txt").trim_end();

    assert!(matches!(stage, Stage::Experimental { .. }));
    assert_eq!(stage.experimental_menu_name(), Some("JavaScript REPL"));
    assert_eq!(
        stage.experimental_menu_description().map(str::to_owned),
        Some(format!(
            "Enable a persistent Node-backed JavaScript REPL for interactive website debugging and other inline JavaScript execution capabilities. Requires Node >= v{expected_node_version} installed."
        ))
    );
    assert_eq!(Feature::JsRepl.default_enabled(), false);
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
fn guardian_approval_is_experimental_and_user_toggleable() {
    let spec = Feature::GuardianApproval.info();
    let stage = spec.stage;

    assert!(matches!(stage, Stage::Experimental { .. }));
    assert_eq!(stage.experimental_menu_name(), Some("Guardian Approvals"));
    assert_eq!(
        stage.experimental_menu_description().map(str::to_owned),
        Some(
            "When Codex needs approval for higher-risk actions (e.g. sandbox escapes or blocked network access), route eligible approval requests to a carefully-prompted security reviewer subagent rather than blocking the agent on your input. This can consume significantly more tokens because it runs a subagent on every approval request.".to_string()
        )
    );
    assert_eq!(stage.experimental_announcement(), None);
    assert_eq!(Feature::GuardianApproval.default_enabled(), false);
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
fn tool_suggest_is_stable_and_enabled_by_default() {
    assert_eq!(Feature::ToolSuggest.stage(), Stage::Stable);
    assert_eq!(Feature::ToolSuggest.default_enabled(), true);
}

#[test]
fn tool_search_is_under_development_and_disabled_by_default() {
    assert_eq!(Feature::ToolSearch.stage(), Stage::UnderDevelopment);
    assert_eq!(Feature::ToolSearch.default_enabled(), false);
}

#[test]
fn general_analytics_is_under_development_and_disabled_by_default() {
    assert_eq!(Feature::GeneralAnalytics.stage(), Stage::UnderDevelopment);
    assert_eq!(Feature::GeneralAnalytics.default_enabled(), false);
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
fn image_generation_is_under_development() {
    assert_eq!(Feature::ImageGeneration.stage(), Stage::UnderDevelopment);
    assert_eq!(Feature::ImageGeneration.default_enabled(), false);
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
    };

    let mut profile_entries = BTreeMap::new();
    profile_entries.insert("code_mode_only".to_string(), true);
    let profile_features = FeaturesToml {
        entries: profile_entries,
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
