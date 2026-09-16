use super::*;
use crate::FeatureRequirementsToml;
use crate::RequirementSource;
use crate::Sourced;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn explicit_catalog_policy_preserves_precedence_over_the_legacy_switch() {
    let catalog: GuardianModelPolicy = serde_json::from_value(json!({
        "computer_use": "adaptive", "shell": "disabled"
    }))
    .unwrap();
    for legacy in [json!(false), json!({"enabled": false})] {
        let legacy = serde_json::from_value(legacy).unwrap();
        for managed in [None, Some(false), Some(true)] {
            let requirements = ConfigRequirements {
                feature_requirements: managed.map(|enabled| {
                    Sourced::new(
                        FeatureRequirementsToml {
                            entries: [("guardianv2".to_owned(), enabled)].into(),
                        },
                        RequirementSource::Unknown,
                    )
                }),
                ..Default::default()
            };
            let mut model = test_model();
            model.guardian = Some(catalog.clone());
            assert_eq!(
                GuardianPolicyLoader::new(Some(&legacy), &requirements).resolve(Some(&model)),
                catalog
            );
        }
    }
}

#[test]
fn legacy_scope_applies_only_to_models_without_catalog_policy() {
    let legacy = serde_json::from_value(json!({
        "enabled": true,
        "review_scope": {"computer_use_only": false, "sandboxed_exec_commands": true}
    }))
    .unwrap();
    let loader = GuardianPolicyLoader::new(Some(&legacy), &ConfigRequirements::default());
    let expected = GuardianModelPolicy {
        computer_use: Some(GuardianReviewMode::Adaptive),
        shell: Some(GuardianReviewMode::Adaptive),
        file_changes: Some(GuardianReviewMode::Adaptive),
        mcp: Some(GuardianReviewMode::Adaptive),
        network: Some(GuardianReviewMode::Adaptive),
        permissions: Some(GuardianReviewMode::Adaptive),
        other_tools: GuardianReviewMode::Adaptive,
        unscored_action: GuardianUnscoredAction::AgeScore,
        initial_cua_call: Some(false),
        sandboxed_exec_commands: Some(true),
    };
    assert_eq!(loader.resolve(/*model*/ None), expected);
    let catalog: GuardianModelPolicy =
        serde_json::from_value(json!({"computer_use": "adaptive"})).unwrap();
    let mut model = test_model();
    model.guardian = Some(catalog.clone());
    assert_eq!(loader.resolve(Some(&model)), catalog);
}

#[test]
fn required_models_preserve_cua_allowance_and_disable_all_tools_scoring() {
    let requirements = ConfigRequirements {
        auto_review_required_models: Some(Sourced::new(
            BTreeSet::from(["protected-model".to_owned()]),
            RequirementSource::Unknown,
        )),
        ..Default::default()
    };
    let legacy = serde_json::from_value(json!({
        "enabled": true, "review_scope": {"computer_use_only": false}
    }))
    .unwrap();
    let cua_only = FeatureToml::Enabled(true);
    for legacy in [Some(&cua_only), Some(&legacy)] {
        let loader = GuardianPolicyLoader::new(legacy, &requirements);
        let ordinary = loader.resolve(/*model*/ None);
        let mut expected = ordinary.clone();
        expected.disable_scoring();
        if ordinary.allows_initial_cua_call() {
            expected.computer_use = ordinary.computer_use;
        }
        let mut actual = ordinary;
        requirements.constrain_guardian_policy(&mut actual, "provider/protected-model");
        assert_eq!(actual, expected);
    }
}

#[test]
fn legacy_cua_opt_in_and_disabled_feature_preserve_scoring_behavior() {
    for legacy in [
        json!(false),
        json!({"review_scope": {"computer_use_only": true}}),
        json!(true),
    ] {
        let enabled = legacy == json!(true);
        let legacy = serde_json::from_value(legacy).unwrap();
        let loader = GuardianPolicyLoader::new(Some(&legacy), &ConfigRequirements::default());
        for required in [false, true] {
            let mut model = test_model();
            model.node_repl_auto_review_required = required;
            let policy = loader.resolve(Some(&model));
            assert_eq!(policy.scoring_enabled(), enabled && required);
            assert_eq!(model.computer_use_review_required(), required);
        }
    }
}

fn test_model() -> ModelInfo {
    serde_json::from_value(json!({
        "slug": "test-model", "display_name": "test-model",
        "supported_reasoning_levels": [], "shell_type": "shell_command",
        "visibility": "list", "supported_in_api": true, "priority": 0,
        "support_verbosity": false, "truncation_policy": {"mode": "bytes", "limit": 10000},
        "experimental_supported_tools": []
    }))
    .unwrap()
}
