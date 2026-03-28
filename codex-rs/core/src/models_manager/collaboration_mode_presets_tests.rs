use super::*;
use pretty_assertions::assert_eq;

#[test]
fn preset_names_use_mode_display_names() {
    assert_eq!(plan_preset().name, ModeKind::Plan.display_name());
    assert_eq!(
        default_preset(CollaborationModesConfig::default()).name,
        ModeKind::Default.display_name()
    );
    assert_eq!(
        plan_preset().reasoning_effort,
        Some(Some(ReasoningEffort::Medium))
    );
}

#[test]
fn default_mode_instructions_replace_mode_names_placeholder() {
    let default_instructions = default_preset(CollaborationModesConfig {
        default_mode_request_user_input: true,
    })
    .developer_instructions
    .expect("default preset should include instructions")
    .expect("default instructions should be set");

    assert!(!default_instructions.contains("{{KNOWN_MODE_NAMES}}"));
    assert!(!default_instructions.contains("{{REQUEST_USER_INPUT_AVAILABILITY}}"));
    assert!(!default_instructions.contains("{{ASKING_QUESTIONS_GUIDANCE}}"));

    let known_mode_names = format_mode_names(&TUI_VISIBLE_COLLABORATION_MODES);
    let expected_snippet = format!("Known mode names are {known_mode_names}.");
    assert!(default_instructions.contains(&expected_snippet));

    let expected_availability_message = request_user_input_availability_message(
        ModeKind::Default,
        /*default_mode_request_user_input*/ true,
    );
    assert!(default_instructions.contains(&expected_availability_message));
    assert!(default_instructions.contains("prefer using the `request_user_input` tool"));
}

#[test]
fn default_mode_instructions_use_plain_text_questions_when_feature_disabled() {
    let default_instructions = default_preset(CollaborationModesConfig::default())
        .developer_instructions
        .expect("default preset should include instructions")
        .expect("default instructions should be set");

    assert!(!default_instructions.contains("prefer using the `request_user_input` tool"));
    assert!(
        default_instructions.contains("ask the user directly with a concise plain-text question")
    );
}
