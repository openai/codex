use super::*;
use crate::ModelsManagerConfig;
use codex_protocol::config_types::Personality;
use codex_protocol::openai_models::ApprovalMessages;
use pretty_assertions::assert_eq;

fn config_with_personality(personality: Option<Personality>) -> ModelsManagerConfig {
    ModelsManagerConfig {
        personality_enabled: true,
        personality,
        ..Default::default()
    }
}

#[test]
fn base_instruction_override_preserves_catalog_approval_messages() {
    let mut model = model_info_from_slug("unknown-model");
    let approvals = ApprovalMessages {
        on_request: Some("user approvals".to_string()),
        on_request_auto_review: Some("auto approvals".to_string()),
    };
    model.model_messages = Some(ModelMessages {
        instructions_template: Some("template".to_string()),
        instructions_variables: Some(ModelInstructionsVariables {
            personality_default: Some("default".to_string()),
            personality_friendly: Some("friendly".to_string()),
            personality_pragmatic: Some("pragmatic".to_string()),
        }),
        approvals: Some(approvals.clone()),
    });
    let config = ModelsManagerConfig {
        base_instructions: Some("override".to_string()),
        ..Default::default()
    };

    let updated = with_config_overrides(model, &config);

    assert_eq!(
        updated.model_messages,
        Some(ModelMessages {
            instructions_template: None,
            instructions_variables: None,
            approvals: Some(approvals),
        })
    );
}

#[test]
fn disabled_personality_preserves_catalog_approval_messages() {
    let mut model = model_info_from_slug("unknown-model");
    let approvals = ApprovalMessages {
        on_request: Some("user approvals".to_string()),
        on_request_auto_review: None,
    };
    model.model_messages = Some(ModelMessages {
        instructions_template: Some("template".to_string()),
        instructions_variables: None,
        approvals: Some(approvals.clone()),
    });
    let config = ModelsManagerConfig {
        personality_enabled: false,
        ..Default::default()
    };

    let updated = with_config_overrides(model, &config);

    assert_eq!(
        updated.model_messages,
        Some(ModelMessages {
            instructions_template: None,
            instructions_variables: None,
            approvals: Some(approvals),
        })
    );
}

#[test]
fn personality_none_strips_catalog_instruction_sources_through_the_next_h1() {
    let cases = [
        (
            "Intro\n\n# Personality\n\nRemove me\n\n## Writing Style\n\nRemove me too\n\n# Safety\n\nKeep me",
            "Intro\n\n# Safety\n\nKeep me",
        ),
        ("Intro\n\n# Personality\n\nRemove me", "Intro\n\n"),
        (
            "Intro\n\n## Personality\n\nKeep me",
            "Intro\n\n## Personality\n\nKeep me",
        ),
        (
            "Intro\n\n# Personality \n\nKeep me",
            "Intro\n\n# Personality \n\nKeep me",
        ),
        (
            "Intro\r\n\r\n# Personality\r\n\r\nRemove me\r\n\r\n## Writing Style\r\n\r\nRemove me too\r\n\r\n# General\r\n\r\nKeep me",
            "Intro\r\n\r\n# General\r\n\r\nKeep me",
        ),
    ];
    let config = config_with_personality(Some(Personality::None));

    for (instructions, expected) in cases {
        let mut model = model_info_from_slug("unknown-model");
        model.base_instructions = instructions.to_string();
        model.model_messages = Some(ModelMessages {
            instructions_template: Some(instructions.to_string()),
            instructions_variables: None,
            approvals: None,
        });

        let updated = with_config_overrides(model, &config);
        let instructions_template = updated
            .model_messages
            .as_ref()
            .and_then(|messages| messages.instructions_template.as_deref());

        assert_eq!(
            (updated.base_instructions.as_str(), instructions_template),
            (expected, Some(expected))
        );
    }
}

#[test]
fn baked_personality_section_is_preserved_without_enabled_explicit_none() {
    let instructions = "Intro\n# Personality\nKeep me\n# General\nKeep me too";
    let configs = [
        config_with_personality(/*personality*/ None),
        config_with_personality(Some(Personality::Friendly)),
        config_with_personality(Some(Personality::Pragmatic)),
        ModelsManagerConfig {
            personality: Some(Personality::None),
            ..Default::default()
        },
    ];

    for config in configs {
        let mut model = model_info_from_slug("unknown-model");
        model.base_instructions = instructions.to_string();

        assert_eq!(
            with_config_overrides(model, &config).base_instructions,
            instructions
        );
    }
}

#[test]
fn model_context_window_override_clamps_to_max_context_window() {
    let mut model = model_info_from_slug("unknown-model");
    model.context_window = Some(273_000);
    model.max_context_window = Some(400_000);
    let config = ModelsManagerConfig {
        model_context_window: Some(500_000),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.context_window = Some(400_000);

    assert_eq!(updated, expected);
}

#[test]
fn model_context_window_uses_model_value_without_override() {
    let mut model = model_info_from_slug("unknown-model");
    model.context_window = Some(273_000);
    model.max_context_window = Some(400_000);
    let config = ModelsManagerConfig::default();

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}
