use codex_features::GuardianV2ConfigToml;
use codex_features::GuardianV2TranscriptConfigToml;
use codex_guardian_context::truncate_text as truncate_entry;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::models::ContentItem;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::GuardianV2TranscriptModelConfig;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;

use super::CLASSIFICATION_OUTPUT_INSTRUCTIONS;
use super::GuardianV2Config;

fn rendered_classifier_text(config: &GuardianV2Config, policy: &str) -> String {
    let (_, content) = config.render_classifier_instructions(policy).into_parts();
    let (ContentItem::InputText { text }, _) = content.into_parts() else {
        panic!("classifier instructions must be text");
    };
    text
}

#[test]
fn template_policy_is_substituted_before_the_single_truncation() {
    let instructions = ResolvedModelMessages::bundled().guardian_classifier_instructions();
    for max_tokens in [256, 1_000, 2_000] {
        let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
            max_classifier_instruction_tokens: Some(max_tokens),
            ..Default::default()
        })
        .unwrap();
        let policy = "The actual tenant policy.";
        assert_eq!(
            rendered_classifier_text(&config, policy),
            truncate_entry(
                &instructions.replace("{{ tenant_policy_config }}", policy),
                max_tokens,
            )
        );
        assert_eq!(config.classifier_instructions, instructions);
    }
}

#[test]
fn evaluated_configuration_preserves_rendered_prompt_and_gate() {
    let instructions = ResolvedModelMessages::bundled().guardian_classifier_instructions();
    let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        classifier_instructions: Some(instructions.to_owned()),
        review_threshold: Some(0.5),
        reasoning_effort: Some(ReasoningEffort::Low),
        max_classifier_instruction_tokens: Some(30_000),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(config.review_threshold, 0.5);
    assert_eq!(config.reasoning_effort, ReasoningEffort::Low);
    assert_eq!(config.max_classifier_instruction_tokens, Some(30_000));
    assert_eq!(config.classifier_instructions, instructions);

    for policy in ["Tenant policy.".to_owned(), "é".repeat(80_000)] {
        // This is the exact pre-review rendering path used by the eval config.
        let previous = truncate_entry(
            &truncate_entry(instructions, /*max_tokens*/ 30_000)
                .replace("{{ tenant_policy_config }}", &policy),
            /*max_tokens*/ 30_000,
        );
        assert_eq!(rendered_classifier_text(&config, &policy), previous);
    }
}

#[test]
fn model_prompt_and_explicit_threshold_precedence_are_preserved() {
    let builtin = GuardianV2Config::from_overrides(GuardianV2ConfigToml::default()).unwrap();
    assert_eq!(builtin.review_threshold, 0.5);
    for prompt in [
        "Model-owned instructions.",
        "",
        ResolvedModelMessages::bundled().guardian_classifier_instructions(),
    ] {
        let defaults = GuardianV2ModelConfig {
            classifier_instructions: Some(prompt.to_owned()),
            ..Default::default()
        };
        let resolved = builtin.with_model_defaults(Some(&defaults)).unwrap();
        assert_eq!(
            (
                resolved.classifier_instructions.clone(),
                resolved.review_threshold
            ),
            (prompt.to_owned(), 0.8),
        );
        assert_eq!(
            resolved
                .with_model_defaults(/*model_defaults*/ None)
                .unwrap(),
            builtin
        );

        let local = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
            classifier_instructions: Some(String::new()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(local.with_model_defaults(Some(&defaults)).unwrap(), local);
    }

    let model_threshold = GuardianV2ModelConfig {
        classifier_instructions: Some("Model-owned instructions.".to_owned()),
        review_threshold_basis_points: Some(6_000),
        ..Default::default()
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
        let rendered = rendered_classifier_text(&config, policy);
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
        rendered_classifier_text(&uncapped, "Tenant policy."),
        format!(
            "{prompt}\n\n# Security Policy\nTenant policy.\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
        )
    );
}
