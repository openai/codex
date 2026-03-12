use super::*;

#[test]
fn test_try_parse_error_message() {
    let text = r#"{
  "error": {
    "message": "Your refresh token has already been used to generate a new access token. Please try signing in again.",
    "type": "invalid_request_error",
    "param": null,
    "code": "refresh_token_reused"
  }
}"#;
    let message = try_parse_error_message(text);
    assert_eq!(
        message,
        "Your refresh token has already been used to generate a new access token. Please try signing in again."
    );
}

#[test]
fn test_try_parse_error_message_no_error() {
    let text = r#"{"message": "test"}"#;
    let message = try_parse_error_message(text);
    assert_eq!(message, r#"{"message": "test"}"#);
}

#[test]
fn feedback_tags_macro_compiles() {
    #[derive(Debug)]
    struct OnlyDebug;

    feedback_tags!(model = "gpt-5", cached = true, debug_only = OnlyDebug);
}

#[test]
fn normalize_thread_name_trims_and_rejects_empty() {
    assert_eq!(normalize_thread_name("   "), None);
    assert_eq!(
        normalize_thread_name("  my thread  "),
        Some("my thread".to_string())
    );
}

#[test]
fn resume_command_prefers_name_over_id() {
    let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let command = resume_command(Some("my-thread"), Some(thread_id));
    assert_eq!(command, Some("codex resume my-thread".to_string()));
}

#[test]
fn resume_command_with_only_id() {
    let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let command = resume_command(None, Some(thread_id));
    assert_eq!(
        command,
        Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
    );
}

#[test]
fn resume_command_with_no_name_or_id() {
    let command = resume_command(None, None);
    assert_eq!(command, None);
}

#[test]
fn resume_command_quotes_thread_name_when_needed() {
    let command = resume_command(Some("-starts-with-dash"), None);
    assert_eq!(
        command,
        Some("codex resume -- -starts-with-dash".to_string())
    );

    let command = resume_command(Some("two words"), None);
    assert_eq!(command, Some("codex resume 'two words'".to_string()));

    let command = resume_command(Some("quote'case"), None);
    assert_eq!(command, Some("codex resume \"quote'case\"".to_string()));
}
