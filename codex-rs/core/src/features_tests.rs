use super::*;

use pretty_assertions::assert_eq;

#[test]
fn under_development_features_are_disabled_by_default() {
    for spec in FEATURES {
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
    for spec in FEATURES {
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
fn guardian_approval_is_experimental_and_user_toggleable() {
    let spec = Feature::GuardianApproval.info();
    let stage = spec.stage;

    assert!(matches!(stage, Stage::Experimental { .. }));
    assert_eq!(
        stage.experimental_menu_name(),
        Some("Automatic approval review")
    );
    assert_eq!(
            stage.experimental_menu_description().map(str::to_owned),
            Some(
                "Dispatch `on-request` approval prompts (for e.g. sandbox escapes or blocked network access) to a carefully-prompted security reviewer subagent rather than blocking the agent on your input.".to_string()
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
fn tool_suggest_is_under_development() {
    assert_eq!(Feature::ToolSuggest.stage(), Stage::UnderDevelopment);
    assert_eq!(Feature::ToolSuggest.default_enabled(), false);
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
fn image_detail_original_feature_is_under_development() {
    assert_eq!(
        Feature::ImageDetailOriginal.stage(),
        Stage::UnderDevelopment
    );
    assert_eq!(Feature::ImageDetailOriginal.default_enabled(), false);
}

#[test]
fn collab_is_legacy_alias_for_multi_agent() {
    assert_eq!(feature_for_key("multi_agent"), Some(Feature::Collab));
    assert_eq!(feature_for_key("collab"), Some(Feature::Collab));
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
    assert!(!features.apps_enabled_for_auth(None));

    features.enable(Feature::Apps);
    assert!(!features.apps_enabled_for_auth(None));

    let api_key_auth = CodexAuth::from_api_key("test-api-key");
    assert!(!features.apps_enabled_for_auth(Some(&api_key_auth)));

    let chatgpt_auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    assert!(features.apps_enabled_for_auth(Some(&chatgpt_auth)));
}
