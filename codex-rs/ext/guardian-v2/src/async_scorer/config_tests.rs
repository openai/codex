use codex_features::GuardianV2ConfigToml;
use codex_features::GuardianV2ReviewScopeConfigToml;
use codex_features::GuardianV2TranscriptConfigToml;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::GuardianV2TranscriptModelConfig;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;

use super::CLASSIFICATION_OUTPUT_INSTRUCTIONS;
use super::DEFAULT_CLASSIFIER_INSTRUCTIONS;
use super::GuardianV2Config;
use super::GuardianV2ReviewScope;
use crate::async_scorer::transcript::truncate_entry;

#[test]
fn sandboxed_exec_commands_are_excluded_by_default() {
    let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml::default()).unwrap();

    assert_eq!(
        config.review_scope,
        GuardianV2ReviewScope::Standard {
            sandboxed_exec_commands: false,
        }
    );
}

#[test]
fn sandboxed_exec_commands_can_be_included() {
    let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        review_scope: Some(GuardianV2ReviewScopeConfigToml {
            computer_use_only: None,
            sandboxed_exec_commands: Some(true),
        }),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(
        config.review_scope,
        GuardianV2ReviewScope::Standard {
            sandboxed_exec_commands: true,
        }
    );
}

#[test]
fn computer_use_only_takes_precedence_over_sandboxed_exec_commands() {
    let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        review_scope: Some(GuardianV2ReviewScopeConfigToml {
            computer_use_only: Some(true),
            sandboxed_exec_commands: Some(true),
        }),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(config.review_scope, GuardianV2ReviewScope::ComputerUseOnly);
}

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
    assert_eq!(config.max_classifier_instruction_tokens, Some(30_000));
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

#[test]
fn legacy_classifier_prompts_keep_the_output_contract_after_truncation() {
    let prompt = "Return a JSON action_risk score. ".repeat(200);
    let policy = "Require approval for unsafe actions.";
    let model_defaults = GuardianV2ModelConfig {
        classifier_instructions: Some(prompt.clone()),
        max_classifier_instruction_tokens: Some(100),
        ..Default::default()
    };
    let local_override = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        classifier_instructions: Some(prompt.clone()),
        max_classifier_instruction_tokens: Some(100),
        ..Default::default()
    })
    .unwrap();
    let model_override = GuardianV2Config::from_overrides(GuardianV2ConfigToml::default())
        .unwrap()
        .with_model_defaults(Some(&model_defaults))
        .unwrap();

    for config in [local_override, model_override] {
        let rendered = config.render_classifier_instructions(policy);
        assert_eq!(
            rendered,
            truncate_entry(
                &format!(
                    "{prompt}\n\n# Security Policy\n{policy}\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
                ),
                /*max_tokens*/ 100,
            )
        );
        assert!(rendered.ends_with(CLASSIFICATION_OUTPUT_INSTRUCTIONS));
    }
}

#[test]
fn model_runtime_settings_preserve_local_overrides() {
    let prompt = "legacy instructions ".repeat(3_000);
    let defaults = GuardianV2ModelConfig {
        classifier_instructions: Some(prompt.clone()),
        max_classifier_instruction_tokens: Some(256),
        max_tool_call_lag: Some(1),
        reuse_parent_compaction: Some(false),
        transcript: Some(GuardianV2TranscriptModelConfig {
            include_images: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    };
    let inherited = GuardianV2Config::from_overrides(GuardianV2ConfigToml::default())
        .unwrap()
        .with_model_defaults(Some(&defaults))
        .unwrap();
    assert_eq!(
        (
            inherited.max_classifier_instruction_tokens,
            inherited.max_tool_call_lag,
            inherited.reuse_parent_compaction,
            inherited.transcript.include_images,
        ),
        (Some(256), 1, false, true)
    );

    let overridden = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        max_classifier_instruction_tokens: Some(512),
        max_tool_call_lag: Some(4),
        reuse_parent_compaction: Some(true),
        transcript: Some(GuardianV2TranscriptConfigToml {
            include_images: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    })
    .unwrap()
    .with_model_defaults(Some(&defaults))
    .unwrap();
    assert_eq!(
        (
            overridden.max_classifier_instruction_tokens,
            overridden.max_tool_call_lag,
            overridden.reuse_parent_compaction,
            overridden.transcript.include_images,
        ),
        (Some(512), 4, true, false)
    );

    let uncapped_defaults = GuardianV2ModelConfig {
        max_classifier_instruction_tokens: None,
        ..defaults
    };
    let uncapped = inherited
        .with_model_defaults(Some(&uncapped_defaults))
        .unwrap();
    assert_eq!(
        uncapped.render_classifier_instructions("Tenant policy."),
        format!(
            "{prompt}\n\n# Security Policy\nTenant policy.\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
        )
    );
}
