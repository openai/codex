use codex_features::GuardianV2ConfigToml;
use codex_features::GuardianV2TranscriptConfigToml;
use codex_guardian_context::TRANSCRIPT_JSON_INSTRUCTIONS;
use codex_guardian_context::truncate_text as truncate_entry;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::TranscriptFormat;
use codex_protocol::models::ContentItem;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::GuardianV2TranscriptModelConfig;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;

use super::CLASSIFICATION_OUTPUT_INSTRUCTIONS;
use super::GuardianV2Config;

fn rendered_classifier_text(config: &GuardianV2Config, policy: &str) -> String {
    let (_, content) = config
        .render_classifier_instructions(policy, "")
        .into_parts();
    let (ContentItem::InputText { text }, _, _) = content.into_parts() else {
        panic!("classifier instructions must be text");
    };
    text
}

#[test]
fn extra_policy_reaches_templated_and_legacy_classifier_instructions() {
    let policy = "Tenant policy.";
    for (template, expected) in [
        (
            "Classify: {{ tenant_policy_config }}",
            "Classify: Tenant policy.",
        ),
        (
            "Legacy classifier.",
            "Legacy classifier.\n\n# Security Policy\nTenant policy.",
        ),
    ] {
        let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
            transcript_mode: Some(TranscriptFormat::Json),
            classifier_instructions: Some(template.to_owned()),
            ..Default::default()
        })
        .unwrap();
        for (extra_policy, suffix) in [
            ("", ""),
            (" \n\t", ""),
            (
                " Extra {{ tenant_policy_config }} and {{ extra_policy }}. ",
                "\n\nExtra {{ tenant_policy_config }} and {{ extra_policy }}.",
            ),
        ] {
            let (_, content) = config
                .render_classifier_instructions(policy, extra_policy)
                .into_parts();
            let (ContentItem::InputText { text }, _, _) = content.into_parts() else {
                panic!("classifier instructions must be text");
            };
            assert_eq!(
                text,
                format!(
                    "{TRANSCRIPT_JSON_INSTRUCTIONS}\n\n{expected}{suffix}\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
                )
            );
        }
    }
}

#[test]
fn template_policy_is_substituted_before_the_single_truncation() {
    let instructions = ResolvedModelMessages::bundled().guardian_classifier_instructions();
    for max_tokens in [100, 256, 1_000, 2_000] {
        let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
            transcript_mode: Some(TranscriptFormat::Json),
            max_classifier_instruction_tokens: Some(max_tokens),
            ..Default::default()
        })
        .unwrap();
        let policy = "The actual tenant policy.";
        assert_eq!(
            rendered_classifier_text(&config, policy),
            format!(
                "{TRANSCRIPT_JSON_INSTRUCTIONS}\n\n{}",
                truncate_entry(
                    &instructions.replace("{{ tenant_policy_config }}", policy),
                    max_tokens,
                )
            )
        );
        assert_eq!(config.classifier_instructions, instructions);
    }
}

#[test]
fn evaluated_configuration_preserves_gate_and_caps_only_configured_prompt() {
    let instructions = ResolvedModelMessages::bundled().guardian_classifier_instructions();
    let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        transcript_mode: Some(TranscriptFormat::Json),
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
        let expected = truncate_entry(
            &instructions.replace("{{ tenant_policy_config }}", &policy),
            /*max_tokens*/ 30_000,
        );
        assert_eq!(
            rendered_classifier_text(&config, &policy),
            format!("{TRANSCRIPT_JSON_INSTRUCTIONS}\n\n{expected}")
        );
    }
}

#[test]
fn model_prompt_and_explicit_threshold_precedence_are_preserved() {
    let builtin = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        transcript_mode: Some(TranscriptFormat::Json),
        ..Default::default()
    })
    .unwrap();
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
            transcript_mode: Some(TranscriptFormat::Json),
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
        transcript_mode: Some(TranscriptFormat::Json),
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
fn model_defaults_preserve_transcript_mode_and_matching_instructions() {
    for format in [TranscriptFormat::Line, TranscriptFormat::Json] {
        let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
            transcript_mode: Some(format),
            ..Default::default()
        })
        .unwrap()
        .with_model_defaults(Some(&GuardianV2ModelConfig {
            classifier_instructions: Some("Model-owned prompt.".to_owned()),
            ..Default::default()
        }))
        .unwrap();
        assert_eq!(config.transcript.format, format);
        let expected = format!(
            "Model-owned prompt.\n\n# Security Policy\nPolicy.\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
        );
        assert_eq!(
            rendered_classifier_text(&config, "Policy."),
            match format {
                TranscriptFormat::Line => expected,
                TranscriptFormat::Json => format!("{TRANSCRIPT_JSON_INSTRUCTIONS}\n\n{expected}"),
            }
        );
    }
}

#[test]
fn classifier_caps_preserve_complete_provenance_and_output_contract() {
    let prompt = "Return a JSON action_risk score. ".repeat(200);
    let policy = "Require approval for unsafe actions.";
    let model_defaults = GuardianV2ModelConfig {
        classifier_instructions: Some(prompt.clone()),
        max_classifier_instruction_tokens: Some(100),
        ..Default::default()
    };
    let local_override = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        transcript_mode: Some(TranscriptFormat::Json),
        classifier_instructions: Some(prompt),
        max_classifier_instruction_tokens: Some(100),
        ..Default::default()
    })
    .unwrap();
    let model_override = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        transcript_mode: Some(TranscriptFormat::Json),
        ..Default::default()
    })
    .unwrap()
    .with_model_defaults(Some(&model_defaults))
    .unwrap();

    for config in [local_override, model_override] {
        let rendered = rendered_classifier_text(&config, policy);
        let configured_prompt = rendered
            .strip_prefix(&format!("{TRANSCRIPT_JSON_INSTRUCTIONS}\n\n"))
            .expect("complete provenance instructions must survive the 100-token cap");
        assert!(
            configured_prompt.len()
                <= codex_protocol::protocol::TruncationPolicy::Tokens(100).byte_budget()
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
        async_classifier_conversation_token_limit: Some(80_000),
        reuse_parent_compaction: Some(false),
        transcript: Some(GuardianV2TranscriptModelConfig {
            include_images: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    };
    let inherited = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        transcript_mode: Some(TranscriptFormat::Json),
        ..Default::default()
    })
    .unwrap()
    .with_model_defaults(Some(&defaults))
    .unwrap();
    assert_eq!(
        (
            inherited.max_classifier_instruction_tokens,
            inherited.max_tool_call_lag,
            inherited.async_classifier_conversation_token_limit,
            inherited.reuse_parent_compaction,
            inherited.transcript.include_images,
        ),
        (Some(256), 1, 80_000, false, true)
    );

    let overridden = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
        transcript_mode: Some(TranscriptFormat::Json),
        max_classifier_instruction_tokens: Some(512),
        max_tool_call_lag: Some(4),
        async_classifier_conversation_token_limit: Some(120_000),
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
            overridden.async_classifier_conversation_token_limit,
            overridden.reuse_parent_compaction,
            overridden.transcript.include_images,
        ),
        (Some(512), 4, 120_000, true, false)
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
            "{TRANSCRIPT_JSON_INSTRUCTIONS}\n\n{prompt}\n\n# Security Policy\nTenant policy.\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}"
        )
    );
}

#[test]
fn classifier_mode_uses_local_override_then_model_default() {
    use codex_protocol::openai_models::AsyncClassifierMode;
    for local in [
        None,
        Some(AsyncClassifierMode::Snapshot),
        Some(AsyncClassifierMode::Conversation),
    ] {
        let config = GuardianV2Config::from_overrides(GuardianV2ConfigToml {
            transcript_mode: Some(TranscriptFormat::Json),
            async_classifier_mode: local,
            ..Default::default()
        })
        .unwrap();
        let defaults = GuardianV2ModelConfig {
            async_classifier_mode: Some(AsyncClassifierMode::Conversation),
            ..Default::default()
        };
        assert_eq!(
            config.classifier_mode(Some(&defaults)),
            local.unwrap_or(AsyncClassifierMode::Conversation)
        );
        assert_eq!(
            config.classifier_mode(/*model_defaults*/ None),
            local.unwrap_or_default()
        );
    }
}
