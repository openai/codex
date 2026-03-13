use super::*;

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
