use codex_features::GuardianV2ConfigToml;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::TruncationPolicy;
use pretty_assertions::assert_eq;

use super::DEFAULT_CLASSIFIER_INSTRUCTIONS;
use super::GuardianV2Config;
use crate::async_scorer::transcript::truncate_entry;

#[test]
fn template_policy_is_substituted_before_the_single_truncation() {
    for max_tokens in [256, 1_000, 2_000] {
        let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
            max_classifier_instruction_tokens: Some(max_tokens),
            ..Default::default()
        })
        .unwrap();
        let policy = "The actual tenant policy.";
        assert_eq!(
            config.render_classifier_instructions(policy),
            truncate_entry(
                &DEFAULT_CLASSIFIER_INSTRUCTIONS.replace("{{ tenant_policy_config }}", policy),
                max_tokens,
            )
        );
        assert_eq!(
            config.classifier_instructions,
            DEFAULT_CLASSIFIER_INSTRUCTIONS
        );
    }
}

#[test]
fn evaluated_configuration_preserves_rendered_prompt_and_gate() {
    let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        classifier_instructions: Some(DEFAULT_CLASSIFIER_INSTRUCTIONS.to_owned()),
        review_threshold: Some(0.5),
        reasoning_effort: Some(ReasoningEffort::Low),
        max_classifier_instruction_tokens: Some(30_000),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(config.review_threshold, 0.5);
    assert_eq!(config.reasoning_effort, ReasoningEffort::Low);
    assert_eq!(config.max_classifier_instruction_tokens, 30_000);
    assert_eq!(
        config.classifier_instructions,
        DEFAULT_CLASSIFIER_INSTRUCTIONS
    );

    for policy in ["Tenant policy.".to_owned(), "é".repeat(80_000)] {
        // This is the exact pre-review rendering path used by the eval config.
        let previous = truncate_entry(
            &truncate_entry(DEFAULT_CLASSIFIER_INSTRUCTIONS, /*max_tokens*/ 30_000)
                .replace("{{ tenant_policy_config }}", &policy),
            /*max_tokens*/ 30_000,
        );
        assert_eq!(config.render_classifier_instructions(&policy), previous);
    }
}

#[test]
fn legacy_custom_prompt_keeps_its_rendering_and_threshold() {
    let template = "legacy instructions ".repeat(200);
    let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        classifier_instructions: Some(template.clone()),
        max_classifier_instruction_tokens: Some(256),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(config.review_threshold, 0.8);
    let expected = truncate_entry(
        &format!(
            "{}\n\n# Security Policy\nTenant policy.",
            truncate_entry(&template, /*max_tokens*/ 256),
        ),
        /*max_tokens*/ 256,
    );
    assert_eq!(
        config.render_classifier_instructions("Tenant policy."),
        expected
    );
    assert!(expected.len() <= TruncationPolicy::Tokens(256).byte_budget());
}

#[test]
fn model_prompt_and_explicit_threshold_precedence_are_preserved() {
    let defaults = GuardianV2ModelConfig {
        classifier_instructions: Some("Model-owned instructions.".to_owned()),
        ..Default::default()
    };
    let builtin = GuardianV2Config::from_overrides(GuardianV2ConfigToml::default()).unwrap();
    assert_eq!(builtin.review_threshold, 0.5);
    assert_eq!(
        builtin
            .with_model_defaults(Some(&defaults))
            .unwrap()
            .review_threshold,
        0.8,
    );

    let model_threshold = GuardianV2ModelConfig {
        review_threshold_basis_points: Some(6_000),
        ..defaults
    };
    assert_eq!(
        builtin
            .with_model_defaults(Some(&model_threshold))
            .unwrap()
            .review_threshold,
        0.6,
    );
    let explicit = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        review_threshold: Some(0.5),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        explicit
            .with_model_defaults(Some(&model_threshold))
            .unwrap()
            .review_threshold,
        0.5,
    );
}
