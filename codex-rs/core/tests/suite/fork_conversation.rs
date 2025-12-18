use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::NewConversation;
use codex_core::built_in_model_providers;
use codex_core::parse_turn_item;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::RolloutLine;
use codex_protocol::items::TurnItem;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::skip_if_no_network;
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
    skip_if_no_network!();

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

    let conversation_manager = ConversationManager::with_models_provider(
        CodexAuth::from_api_key("dummy"),
        config.model_provider.clone(),
    );
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
                items: vec![UserInput::Text {
                    text: text.to_string(),
                }],
            })
            .await
            .unwrap();
        let _ = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
    }

    // Request history from the base conversation to obtain rollout path.
    let base_path = codex.rollout_path();

    // GetHistory flushes before returning the path; no wait needed.

    // Helper: read rollout items (excluding SessionMeta) from a JSONL path.
    let read_items = |p: &std::path::Path| -> Vec<RolloutItem> {
        let text = std::fs::read_to_string(p).expect("read rollout file");
        let mut items: Vec<RolloutItem> = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(line).expect("jsonl line");
            let rl: RolloutLine = serde_json::from_value(v).expect("rollout line");
            match rl.item {
                RolloutItem::SessionMeta(_) => {}
                other => items.push(other),
            }
        }
        items
    };

    // Compute expected prefixes after each fork by truncating base rollout
    // strictly before the nth user input (0-based).
    let base_items = read_items(&base_path);
    let find_user_input_positions = |items: &[RolloutItem]| -> Vec<usize> {
        let mut pos = Vec::new();
        for (i, it) in items.iter().enumerate() {
            if let RolloutItem::ResponseItem(response_item) = it
                && let Some(TurnItem::UserMessage(_)) = parse_turn_item(response_item)
            {
                // Consider any user message as an input boundary; recorder stores both EventMsg and ResponseItem.
                // We specifically look for input items, which are represented as ContentItem::InputText.
                pos.push(i);
            }
        }
        pos
    };
    let user_inputs = find_user_input_positions(&base_items);

    // After cutting at nth user input (n=1 → second user message), cut strictly before that input.
    let cut1 = user_inputs.get(1).copied().unwrap_or(0);
    let expected_after_first: Vec<RolloutItem> = base_items[..cut1].to_vec();

    // After dropping again (n=1 on fork1), compute expected relative to fork1's rollout.

    // Fork once with n=1 → drops the last user input and everything after.
    let NewConversation {
        conversation: codex_fork1,
        ..
    } = conversation_manager
        .fork_conversation(1, config_for_fork.clone(), base_path.clone())
        .await
        .expect("fork 1");

    let fork1_path = codex_fork1.rollout_path();

    // GetHistory on fork1 flushed; the file is ready.
    let fork1_items = read_items(&fork1_path);
    pretty_assertions::assert_eq!(
        serde_json::to_value(&fork1_items).unwrap(),
        serde_json::to_value(&expected_after_first).unwrap()
    );

    // Fork again with n=0 → drops the (new) last user message, leaving only the first.
    let NewConversation {
        conversation: codex_fork2,
        ..
    } = conversation_manager
        .fork_conversation(0, config_for_fork.clone(), fork1_path.clone())
        .await
        .expect("fork 2");

    let fork2_path = codex_fork2.rollout_path();
    // GetHistory on fork2 flushed; the file is ready.
    let fork1_items = read_items(&fork1_path);
    let fork1_user_inputs = find_user_input_positions(&fork1_items);
    let cut_last_on_fork1 = fork1_user_inputs
        .get(fork1_user_inputs.len().saturating_sub(1))
        .copied()
        .unwrap_or(0);
    let expected_after_second: Vec<RolloutItem> = fork1_items[..cut_last_on_fork1].to_vec();
    let fork2_items = read_items(&fork2_path);
    pretty_assertions::assert_eq!(
        serde_json::to_value(&fork2_items).unwrap(),
        serde_json::to_value(&expected_after_second).unwrap()
    );
}
