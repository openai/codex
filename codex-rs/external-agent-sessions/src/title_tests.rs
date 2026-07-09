use super::*;

#[test]
fn preserves_valid_custom_title_unchanged() {
    let custom_title = "<system-reminder>Keep this custom title</system-reminder>";

    assert_eq!(
        SessionTitleCandidates {
            custom_title: Some(custom_title.to_string()),
            ai_title: Some("AI title".to_string()),
            fallback_title: Some("fallback title".to_string()),
        }
        .select(),
        Some(custom_title.to_string())
    );
}

#[test]
fn preserves_valid_ai_title_unchanged_without_custom_title() {
    let ai_title = "<command-message>Keep this AI title</command-message>";

    assert_eq!(
        SessionTitleCandidates {
            custom_title: None,
            ai_title: Some(ai_title.to_string()),
            fallback_title: Some("fallback title".to_string()),
        }
        .select(),
        Some(ai_title.to_string())
    );
}

#[test]
fn strips_nested_repeated_and_multiline_leading_control_wrappers() {
    let message = "\
        <system-reminder>\n\
        outer context\n\
        <command-message>\n\
        nested context\n\
        </command-message>\n\
        </system-reminder>\n\
        <ide_opened_file>\n\
        src/auth.rs\n\
        </ide_opened_file>\n\
        \n\
        Fix auth flow\n\
        Additional details";

    assert_eq!(
        fallback_title_from_user_message(message),
        Some("Fix auth flow".to_string())
    );
}

#[test]
fn strips_observed_external_agent_control_wrapper_families() {
    let cases = [
        "<task-notification>\n\
         <task-id>abc123</task-id>\n\
         <status>completed</status>\n\
         </task-notification>\n\
         Fix auth flow",
        "<command-message>review</command-message>\n\
         <command-name>/review</command-name>\n\
         <command-args>src/auth.rs</command-args>\n\
         Fix auth flow",
        "<local-command-caveat>Command output follows</local-command-caveat>\n\
         <local-command-stdout>tests passed</local-command-stdout>\n\
         Fix auth flow",
        "<local-command-stderr>tests failed</local-command-stderr>\n\
         Fix auth flow",
        "<ide_selection>src/auth.rs:1-5</ide_selection>\n\
         Fix auth flow",
    ];

    for message in cases {
        assert_eq!(
            fallback_title_from_user_message(message),
            Some("Fix auth flow".to_string())
        );
    }
}

#[test]
fn returns_no_candidate_for_empty_or_control_only_messages() {
    assert_eq!(fallback_title_from_user_message(""), None);
    assert_eq!(
        fallback_title_from_user_message(
            "<command-message>review</command-message>\n\
             <system-reminder>context</system-reminder>"
        ),
        None
    );
}

#[test]
fn uses_first_meaningful_line_from_ordinary_messages() {
    assert_eq!(
        fallback_title_from_user_message("\n  \n  Fix auth flow  \nAdditional details"),
        Some("Fix auth flow".to_string())
    );
}

#[test]
fn preserves_unknown_and_user_authored_angle_bracket_text() {
    assert_eq!(
        fallback_title_from_user_message("<user-note>Keep this text</user-note> Fix auth flow"),
        Some("<user-note>Keep this text</user-note> Fix auth flow".to_string())
    );
    assert_eq!(
        fallback_title_from_user_message("Explain <system-reminder> tags"),
        Some("Explain <system-reminder> tags".to_string())
    );
}

#[test]
fn bounds_fallback_titles_to_120_characters() {
    let message = "x".repeat(121);
    let title = fallback_title_from_user_message(&message).expect("title");

    assert_eq!(title.chars().count(), SESSION_TITLE_MAX_LEN);
    assert_eq!(title, format!("{}...", "x".repeat(117)));
}
