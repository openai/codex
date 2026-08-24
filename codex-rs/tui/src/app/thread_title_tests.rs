use super::THREAD_TITLE_MAX_CHARS;
use super::THREAD_TITLE_PROMPT_MAX_BYTES;
use super::THREAD_TITLE_RECENT_MESSAGES;
use super::parse_thread_title;
use super::recent_conversation_messages;
use super::recent_conversation_thread_title_prompt;
use super::thread_title_prompt;
use crate::app::session_lifecycle::ThreadAttachPresentation;
use crate::app::test_support::make_test_app;
use crate::app_command::AppCommand;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::test_support::PathBufExt;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::models::MessagePhase;
use core_test_support::responses;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use tokio::sync::mpsc::unbounded_channel;

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

#[tokio::test]
async fn slash_rename_generates_editable_title_through_embedded_app_server()
-> color_eyre::Result<()> {
    let server = wiremock::MockServer::start().await;
    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("title-response"),
            responses::ev_assistant_message("title-message", r#"{"title":"Fix login timeout"}"#),
            responses::ev_completed("title-response"),
        ]),
    )
    .await;
    let codex_home = tempdir()?;
    let provider_id = "thread-title-test";
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "model = \"gpt-5.2\"\n\
             model_provider = \"{provider_id}\"\n\n\
             [model_providers.{provider_id}]\n\
             name = \"Thread title test\"\n\
             base_url = \"{}/v1\"\n\
             wire_api = \"responses\"\n\
             request_max_retries = 0\n\
             stream_max_retries = 0\n",
            server.uri()
        ),
    )?;

    let mut app = make_test_app().await;
    let (event_tx, mut event_rx) = unbounded_channel();
    app.app_event_tx = AppEventSender::new(event_tx);
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite =
        codex_state::SqliteConfig::new_for_testing(codex_home.path().to_path_buf().abs());
    app.config.model = Some("gpt-5.2".to_string());
    app.config.model_provider_id = provider_id.to_string();
    app.config.model_provider = ModelProviderInfo {
        name: "Thread title test".to_string(),
        base_url: Some(format!("{}/v1", server.uri())),
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        ..ModelProviderInfo::default()
    };

    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.replace_chat_widget_with_app_server_thread(
        &mut tui,
        started,
        ThreadAttachPresentation::SessionLineage,
        /*initial_user_message*/ None,
    )
    .await?;
    app.ensure_thread_channel(thread_id)
        .store
        .lock()
        .await
        .turns
        .push(Turn {
            id: "existing-turn".to_string(),
            items: vec![title_user_message("user-message", "Fix the login timeout")],
            items_view: Default::default(),
            status: TurnStatus::Completed,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        });
    while event_rx.try_recv().is_ok() {}

    app.chat_widget.apply_external_edit("/rename".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Generating"));

    enum TitleDriveEvent {
        Ui(Box<AppEvent>),
        Server(AppServerEvent),
    }

    loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 10), async {
            tokio::select! {
                event = event_rx.recv() => event.map(|event| TitleDriveEvent::Ui(Box::new(event))),
                event = app_server.next_event() => event.map(TitleDriveEvent::Server),
            }
        })
        .await?
        .ok_or_else(|| color_eyre::eyre::eyre!("title event stream ended"))?;

        match event {
            TitleDriveEvent::Ui(event) => {
                let generated = matches!(event.as_ref(), AppEvent::GeneratedThreadTitle { .. });
                app.handle_event(&mut tui, &mut app_server, *event).await?;
                if generated {
                    break;
                }
            }
            TitleDriveEvent::Server(event) => {
                app.handle_app_server_event(&app_server, event).await;
            }
        }
    }

    let popup = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    assert!(popup.contains("Fix login timeout"));
    assert!(!popup.contains("Generating a title suggestion"));
    assert!(app.temporary_structured_requests.is_empty());

    let request = response.single_request();
    assert!(
        request
            .body_json()
            .to_string()
            .contains("Fix the login timeout")
    );

    app_server.shutdown().await?;
    Ok(())
}

#[test]
fn recent_conversation_messages_escape_markup_and_ignore_commentary() {
    let items = vec![
        title_user_message("user-1", "  Investigate <flaky> & slow tests  "),
        title_agent_message(
            "commentary",
            "Checking files",
            Some(MessagePhase::Commentary),
        ),
        title_user_message("user-2", "Fix > flaky tests"),
        title_agent_message(
            "answer",
            "  Tests now pass  ",
            Some(MessagePhase::FinalAnswer),
        ),
    ];

    assert_eq!(
        recent_conversation_messages(&items),
        Some(
            "<conversation>\n<message role=\"user\">Investigate &lt;flaky&gt; &amp; slow tests</message>\n<message role=\"user\">Fix &gt; flaky tests</message>\n<message role=\"assistant\">Tests now pass</message>\n</conversation>"
                .to_string()
        )
    );
}

#[test]
fn recent_conversation_messages_strip_ide_context_before_escaping() {
    let ide_context = "x".repeat(THREAD_TITLE_PROMPT_MAX_BYTES + 1);
    let user_message = format!(
        "# Context from my IDE setup:\n{ide_context}\n## My request for Codex:\n\nEarlier request\n## My request for Codex:\n  Fix <login> & retries  "
    );
    let items = vec![title_user_message("user-1", &user_message)];

    assert_eq!(
        recent_conversation_messages(&items),
        Some(
            "<conversation>\n<message role=\"user\">Fix &lt;login&gt; &amp; retries</message>\n</conversation>"
                .to_string()
        )
    );
}

#[test]
fn recent_conversation_messages_keep_only_the_latest_substantive_items() {
    let items = (0..THREAD_TITLE_RECENT_MESSAGES + 2)
        .map(|index| title_user_message(&index.to_string(), &format!("message-{index}")))
        .collect::<Vec<_>>();

    let expected_messages = (2..THREAD_TITLE_RECENT_MESSAGES + 2)
        .map(|index| format!("<message role=\"user\">message-{index}</message>"))
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(
        recent_conversation_messages(&items),
        Some(format!(
            "<conversation>\n{expected_messages}\n</conversation>"
        ))
    );
}

#[test]
fn recent_conversation_messages_require_substantive_content() {
    assert_eq!(recent_conversation_messages(&[]), None);

    let items = vec![
        title_user_message("blank", " \n\t "),
        title_agent_message("commentary", "Thinking", Some(MessagePhase::Commentary)),
    ];

    assert_eq!(recent_conversation_messages(&items), None);
}

#[test]
fn recent_conversation_prompt_trims_messages_and_prioritizes_latest_request() {
    let prompt = recent_conversation_thread_title_prompt(
        "  User: Fix login\nAssistant: Investigating the auth flow  \n",
    );

    assert_eq!(
        prompt,
        format!(
            "{EXPECTED_THREAD_TITLE_INSTRUCTIONS}\nPrioritize the current task and latest substantive user request.\n\nRecent conversation messages:\nUser: Fix login\nAssistant: Investigating the auth flow"
        )
    );
}

#[test]
fn recent_conversation_prompt_keeps_latest_unicode_characters() {
    let conversation = format!("discarded-{}x", "🚀".repeat(THREAD_TITLE_PROMPT_MAX_BYTES));
    let prompt = recent_conversation_thread_title_prompt(&conversation);
    let expected_instructions = format!(
        "{EXPECTED_THREAD_TITLE_INSTRUCTIONS}\nPrioritize the current task and latest substantive user request.\n\n"
    );
    let prefix = format!("{expected_instructions}Recent conversation messages:\n");
    let remaining_bytes = THREAD_TITLE_PROMPT_MAX_BYTES - prefix.len();
    let expected = format!("{}x", "🚀".repeat((remaining_bytes - 1) / '🚀'.len_utf8()));

    assert_eq!(
        prompt.rsplit_once("Recent conversation messages:\n"),
        Some((expected_instructions.as_str(), expected.as_str()))
    );
    assert!(prompt.len() <= THREAD_TITLE_PROMPT_MAX_BYTES);
}

#[test]
fn recent_conversation_prompt_preserves_latest_user_request_and_complete_markup() {
    let items = vec![
        title_user_message("user", "Fix <authentication> & login regressions"),
        title_agent_message(
            "assistant",
            &format!("{}&<>", "🚀".repeat(THREAD_TITLE_PROMPT_MAX_BYTES)),
            Some(MessagePhase::FinalAnswer),
        ),
    ];
    let conversation = recent_conversation_messages(&items).expect("substantive conversation");
    let prompt = recent_conversation_thread_title_prompt(&conversation);

    assert!(prompt.len() <= THREAD_TITLE_PROMPT_MAX_BYTES);
    assert!(prompt.contains("<conversation>\n"));
    assert!(prompt.contains("</conversation>"));
    assert!(prompt.contains(
        "<message role=\"user\">Fix &lt;authentication&gt; &amp; login regressions</message>"
    ));
    assert!(prompt.contains("<message role=\"assistant\">"));
    assert_eq!(prompt.matches("<message role=").count(), 2);
    assert_eq!(prompt.matches("</message>").count(), 2);
}

#[test]
fn recent_conversation_prompt_never_splits_an_escaped_markup_entity() {
    let items = vec![title_user_message(
        "user",
        &"<&>".repeat(THREAD_TITLE_PROMPT_MAX_BYTES),
    )];
    let conversation = recent_conversation_messages(&items).expect("substantive conversation");
    let prompt = recent_conversation_thread_title_prompt(&conversation);

    assert!(prompt.len() <= THREAD_TITLE_PROMPT_MAX_BYTES);
    let text = conversation
        .strip_prefix("<conversation>\n<message role=\"user\">")
        .and_then(|text| text.strip_suffix("</message>\n</conversation>"))
        .expect("complete message markup");
    for fragment in text.split('&').skip(/*n*/ 1) {
        assert!(
            fragment.starts_with("lt;")
                || fragment.starts_with("gt;")
                || fragment.starts_with("amp;")
        );
    }
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

fn title_user_message(id: &str, text: &str) -> ThreadItem {
    ThreadItem::UserMessage {
        id: id.to_string(),
        client_id: None,
        content: vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
    }
}

fn title_agent_message(id: &str, text: &str, phase: Option<MessagePhase>) -> ThreadItem {
    ThreadItem::AgentMessage {
        id: id.to_string(),
        text: text.to_string(),
        phase,
        memory_citation: None,
        delivery: None,
    }
}
