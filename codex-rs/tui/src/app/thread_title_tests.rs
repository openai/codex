use super::THREAD_TITLE_MAX_CHARS;
use super::THREAD_TITLE_PROMPT_MAX_BYTES;
use super::parse_thread_title;
use super::thread_title_prompt;
use crate::app::test_support::make_test_app;
use crate::app_command::AppCommand;
use pretty_assertions::assert_eq;

const EXPECTED_THREAD_TITLE_INSTRUCTIONS: &str = concat!(
    "Generate a concise, single-line task title of at most 36 characters ",
    "and under five words where possible. Start with an imperative verb. ",
    "Capitalize only the first word unless the user's language, proper nouns, ",
    "acronyms, or code terms require otherwise. Preserve ticket references ",
    "exactly. Write in the user's language. Do not use quotes, markdown, ",
    "or trailing punctuation. Do not answer the request."
);

#[test]
fn trims_user_message_in_title_prompt() {
    let prompt = thread_title_prompt("  Fix the login form  \n");

    assert_eq!(
        prompt,
        format!("{EXPECTED_THREAD_TITLE_INSTRUCTIONS}\n\nUser prompt:\nFix the login form")
    );
}

#[test]
fn truncates_title_prompt_without_splitting_unicode() {
    let user_message = "🚀".repeat(THREAD_TITLE_PROMPT_MAX_BYTES);
    let prompt = thread_title_prompt(&user_message);
    let expected_instructions = format!("{EXPECTED_THREAD_TITLE_INSTRUCTIONS}\n\n");
    let prefix = format!("{expected_instructions}User prompt:\n");
    let available_bytes = THREAD_TITLE_PROMPT_MAX_BYTES - prefix.len();
    let expected = "🚀".repeat(available_bytes / '🚀'.len_utf8());

    assert_eq!(
        prompt.rsplit_once("User prompt:\n"),
        Some((expected_instructions.as_str(), expected.as_str()))
    );
    assert!(prompt.len() <= THREAD_TITLE_PROMPT_MAX_BYTES);
}

#[test]
fn bounds_the_entire_title_prompt_for_dense_unicode() {
    let repeated_characters = THREAD_TITLE_PROMPT_MAX_BYTES * 3;
    for message in [
        "🚀".repeat(repeated_characters),
        "漢".repeat(repeated_characters),
        "x".repeat(repeated_characters),
    ] {
        let prompt = thread_title_prompt(&message);

        assert!(
            prompt.len() <= THREAD_TITLE_PROMPT_MAX_BYTES,
            "{} bytes for {:?}",
            prompt.len(),
            message.chars().next()
        );
    }
}

#[tokio::test]
async fn manual_rename_invalidates_pending_automatic_title_before_notification()
-> color_eyre::Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app_server
        .thread_set_name(thread_id, "Provisional title".to_string())
        .await?;
    app.chat_widget
        .expect_automatic_thread_name("Provisional title".to_string());

    app.submit_thread_op(
        &mut app_server,
        thread_id,
        AppCommand::set_thread_name("Manual title".to_string()),
    )
    .await?;

    assert_eq!(
        app.chat_widget.thread_name(),
        Some("Manual title".to_string())
    );

    app_server.shutdown().await?;
    Ok(())
}

#[test]
fn normalizes_generated_title_whitespace() {
    assert_eq!(
        parse_thread_title(r#"{"title":"  Fix  \n\t login   errors  "}"#),
        Some("Fix login errors".to_string())
    );
}

#[test]
fn removes_wrapping_quotes_and_trailing_punctuation_from_generated_titles() {
    for title in [
        r#""Fix login errors!""#,
        "'Fix login errors?'",
        "`Fix login errors.`",
        "“Fix login errors!”",
    ] {
        let response = serde_json::json!({ "title": title }).to_string();

        assert_eq!(
            parse_thread_title(&response),
            Some("Fix login errors".to_string()),
            "response: {response}"
        );
    }
}

#[test]
fn preserves_meaningful_leading_punctuation_in_generated_titles() {
    for (title, expected) in [
        (".NET migration.", ".NET migration"),
        ("!important styling!", "!important styling"),
    ] {
        let response = serde_json::json!({ "title": title }).to_string();

        assert_eq!(
            parse_thread_title(&response),
            Some(expected.to_string()),
            "response: {response}"
        );
    }
}

#[test]
fn rejects_invalid_or_empty_generated_titles() {
    for response in [
        "",
        "not json",
        "null",
        "true",
        "42",
        "[]",
        r#"["title"]"#,
        r#""plain title""#,
        "{}",
        r#"{"title":7}"#,
        r#"{"title":"valid","extra":true}"#,
        r#"{"title":""}"#,
        r#"{"title":"  \t  "}"#,
    ] {
        assert_eq!(parse_thread_title(response), None, "response: {response}");
    }
}

#[test]
fn truncates_generated_titles_without_splitting_unicode() {
    let expected = "🚀".repeat(THREAD_TITLE_MAX_CHARS);
    let response = serde_json::json!({ "title": format!("{expected}x") }).to_string();

    assert_eq!(parse_thread_title(&response), Some(expected));
}
