use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::protocol::ConversationHistoryResponseEvent;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// Build minimal SSE stream with completed marker using the JSON fixture.
fn sse_completed(id: &str) -> String {
    core_test_support::load_sse_fixture_with_id("tests/fixtures/completed_template.json", id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_conversation_twice_drops_to_first_message() {
    // Start a mock server that completes three turns.
    let server = MockServer::start().await;
    let sse = sse_completed("resp");
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse.clone(), "text/event-stream");

    // Expect three calls to /v1/responses – one per user input.
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .expect(3)
        .mount(&server)
        .await;

    // Configure Codex to use the mock server.
    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = model_provider.clone();
    let config_for_fork = config.clone();

    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy"));
    let NewConversation {
        conversation: codex,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create conversation");

    // Send three user messages; wait for three completed turns.
    for text in ["first", "second", "third"] {
        codex
            .submit(Op::UserInput {
                items: vec![InputItem::Text {
                    text: text.to_string(),
                }],
            })
            .await
            .unwrap();
        let _ = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
    }

    // Request history from the base conversation.
    codex.submit(Op::GetHistory).await.unwrap();
    let base_history =
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::ConversationHistory(_))).await;

    // Capture entries from the base history and compute expected prefixes after each fork.
    let entries_after_three = match &base_history {
        EventMsg::ConversationHistory(ConversationHistoryResponseEvent { entries, .. }) => {
            entries.clone()
        }
        _ => panic!("expected ConversationHistory event"),
    };
    // History layout for this test:
    // [0] user instructions,
    // [1] environment context,
    // [2] "first" user message,
    // [3] "second" user message,
    // [4] "third" user message.

    // Fork 1: drops the last user message and everything after.
    let expected_after_first = vec![
        entries_after_three[0].clone(),
        entries_after_three[1].clone(),
        entries_after_three[2].clone(),
        entries_after_three[3].clone(),
    ];

    // Fork 2: drops the last user message and everything after.
    // [0] user instructions,
    // [1] environment context,
    // [2] "first" user message,
    let expected_after_second = vec![
        entries_after_three[0].clone(),
        entries_after_three[1].clone(),
        entries_after_three[2].clone(),
    ];

    // Fork once with n=1 → drops the last user message and everything after.
    let NewConversation {
        conversation: codex_fork1,
        ..
    } = conversation_manager
        .fork_conversation(entries_after_three.clone(), 1, config_for_fork.clone())
        .await
        .expect("fork 1");

    codex_fork1.submit(Op::GetHistory).await.unwrap();
    let fork1_history = wait_for_event(&codex_fork1, |ev| {
        matches!(ev, EventMsg::ConversationHistory(_))
    })
    .await;
    let entries_after_first_fork = match &fork1_history {
        EventMsg::ConversationHistory(ConversationHistoryResponseEvent { entries, .. }) => {
            assert!(matches!(
                fork1_history,
                EventMsg::ConversationHistory(ConversationHistoryResponseEvent { ref entries, .. }) if *entries == expected_after_first
            ));
            entries.clone()
        }
        _ => panic!("expected ConversationHistory event after first fork"),
    };

    // Fork again with n=1 → drops the (new) last user message, leaving only the first.
    let NewConversation {
        conversation: codex_fork2,
        ..
    } = conversation_manager
        .fork_conversation(entries_after_first_fork.clone(), 1, config_for_fork.clone())
        .await
        .expect("fork 2");

    codex_fork2.submit(Op::GetHistory).await.unwrap();
    let fork2_history = wait_for_event(&codex_fork2, |ev| {
        matches!(ev, EventMsg::ConversationHistory(_))
    })
    .await;
    assert!(matches!(
        fork2_history,
        EventMsg::ConversationHistory(ConversationHistoryResponseEvent { ref entries, .. }) if *entries == expected_after_second
    ));
}
