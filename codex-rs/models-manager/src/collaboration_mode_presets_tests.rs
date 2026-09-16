use super::*;
use pretty_assertions::assert_eq;

#[test]
fn preset_names_use_mode_display_names() {
    let presets = builtin_collaboration_mode_presets();
    let [plan, default] = presets.as_slice() else {
        panic!("expected Plan and Default presets");
    };
    assert_eq!(plan.name, ModeKind::Plan.display_name());
    assert_eq!(default.name, ModeKind::Default.display_name());
    assert_eq!(plan.model, None);
    assert_eq!(plan.reasoning_effort, Some(Some(ReasoningEffort::Medium)));
    assert_eq!(default.model, None);
    assert_eq!(default.reasoning_effort, None);
}

#[test]
fn default_mode_instructions_follow_user_input_tool_availability() {
    let presets = builtin_collaboration_mode_presets();
    let default_instructions = presets[1]
        .developer_instructions
        .as_ref()
        .expect("default preset should include instructions")
        .as_ref()
        .expect("default instructions should be set");

    assert!(default_instructions.contains(
        "Use the `request_user_input` tool only when it is listed in the available tools"
    ));
    assert!(
        default_instructions.contains("Ask the user directly with one concise plain-text question")
    );
}
