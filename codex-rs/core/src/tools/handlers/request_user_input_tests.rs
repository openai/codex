use super::*;
use pretty_assertions::assert_eq;

#[test]
fn request_user_input_mode_availability_defaults_to_plan_only() {
    assert!(ModeKind::Plan.allows_request_user_input());
    assert!(!ModeKind::Default.allows_request_user_input());
    assert!(!ModeKind::Execute.allows_request_user_input());
    assert!(!ModeKind::PairProgramming.allows_request_user_input());
}

#[test]
fn request_user_input_unavailable_messages_respect_default_mode_feature_flag() {
    assert_eq!(
        request_user_input_unavailable_message(ModeKind::Plan, false),
        None
    );
    assert_eq!(
        request_user_input_unavailable_message(ModeKind::Default, false),
        Some("request_user_input is unavailable in Default mode".to_string())
    );
    assert_eq!(
        request_user_input_unavailable_message(ModeKind::Default, true),
        None
    );
    assert_eq!(
        request_user_input_unavailable_message(ModeKind::Execute, false),
        Some("request_user_input is unavailable in Execute mode".to_string())
    );
    assert_eq!(
        request_user_input_unavailable_message(ModeKind::PairProgramming, false),
        Some("request_user_input is unavailable in Pair Programming mode".to_string())
    );
}

#[test]
fn request_user_input_tool_description_mentions_available_modes() {
    assert_eq!(
            request_user_input_tool_description(false),
            "Request user input for one to three short questions and wait for the response. This tool is only available in Plan mode.".to_string()
        );
    assert_eq!(
            request_user_input_tool_description(true),
            "Request user input for one to three short questions and wait for the response. This tool is only available in Default or Plan mode.".to_string()
        );
}
