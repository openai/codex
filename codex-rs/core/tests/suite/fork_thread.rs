use codex_core::ForkSnapshot;
use codex_core::NewThread;
use codex_core::parse_turn_item;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ResumedHistory;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_thread_twice_drops_to_first_message() {
    skip_if_no_network!();

    // Start a mock server that completes three turns.
    let server = MockServer::start().await;
    let sse = sse(vec![ev_response_created("resp"), ev_completed("resp")]);
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

    let mut builder = test_codex();
    let test = builder.build(&server).await.expect("create conversation");
    let codex = test.codex.clone();
    let thread_manager = test.thread_manager.clone();
    let config_for_fork = test.config.clone();

    // Send three user messages; wait for three completed turns.
    for text in ["first", "second", "third"] {
        codex
            .submit(Op::UserInput {
                environments: None,
                items: vec![UserInput::Text {
                    text: text.to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
                responsesapi_client_metadata: None,
            })
            .await
            .unwrap();
        let _ = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    }

    // Request history from the base conversation to obtain rollout path.
    let base_path = codex.rollout_path().expect("rollout path");

    // GetHistory flushes before returning the path; no wait needed.

    // Compute expected prefixes after each fork by truncating base rollout
    // strictly before the nth user input (0-based).
    let base_items = read_rollout_items(&base_path);
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
    let NewThread {
        thread: codex_fork1,
        ..
    } = thread_manager
        .fork_thread(
            ForkSnapshot::TruncateBeforeNthUserMessage(1),
            config_for_fork.clone(),
            codex_core::thread_store_from_config(&config_for_fork),
            base_path.clone(),
            /*persist_extended_history*/ false,
            /*parent_trace*/ None,
        )
        .await
        .expect("fork 1");

    let fork1_path = codex_fork1.rollout_path().expect("rollout path");

    // GetHistory on fork1 flushed; the file is ready.
    let fork1_items = read_rollout_items(&fork1_path);
    pretty_assertions::assert_eq!(
        serde_json::to_value(&fork1_items).unwrap(),
        serde_json::to_value(&expected_after_first).unwrap()
    );

    // Fork again with n=0 → drops the (new) last user message, leaving only the first.
    let NewThread {
        thread: codex_fork2,
        ..
    } = thread_manager
        .fork_thread(
            ForkSnapshot::TruncateBeforeNthUserMessage(0),
            config_for_fork.clone(),
            codex_core::thread_store_from_config(&config_for_fork),
            fork1_path.clone(),
            /*persist_extended_history*/ false,
            /*parent_trace*/ None,
        )
        .await
        .expect("fork 2");

    let fork2_path = codex_fork2.rollout_path().expect("rollout path");
    // GetHistory on fork2 flushed; the file is ready.
    let fork1_items = read_rollout_items(&fork1_path);
    let fork1_user_inputs = find_user_input_positions(&fork1_items);
    let cut_last_on_fork1 = fork1_user_inputs
        .get(fork1_user_inputs.len().saturating_sub(1))
        .copied()
        .unwrap_or(0);
    let expected_after_second: Vec<RolloutItem> = fork1_items[..cut_last_on_fork1].to_vec();
    let fork2_items = read_rollout_items(&fork2_path);
    pretty_assertions::assert_eq!(
        serde_json::to_value(&fork2_items).unwrap(),
        serde_json::to_value(&expected_after_second).unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_thread_from_history_does_not_require_source_rollout_path() {
    skip_if_no_network!();

    let server = MockServer::start().await;
    let sse = sse(vec![ev_response_created("resp"), ev_completed("resp")]);
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(sse, "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut builder = test_codex();
    let test = builder.build(&server).await.expect("create conversation");
    let codex = test.codex.clone();
    let thread_manager = test.thread_manager.clone();

    codex
        .submit(Op::UserInput {
            environments: None,
            items: vec![UserInput::Text {
                text: "fork me from stored history".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await
        .unwrap();
    let _ = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let source_path = codex.rollout_path().expect("source rollout path");
    let source_items = read_rollout_items(&source_path);
    let NewThread {
        thread: forked_thread,
        ..
    } = thread_manager
        .fork_thread_from_history(
            ForkSnapshot::Interrupted,
            test.config.clone(),
            codex_core::thread_store_from_config(&test.config),
            InitialHistory::Resumed(ResumedHistory {
                conversation_id: test.session_configured.session_id,
                history: source_items.clone(),
                rollout_path: None,
            }),
            /*persist_extended_history*/ false,
            /*parent_trace*/ None,
        )
        .await
        .expect("fork from stored history");

    let forked_path = forked_thread.rollout_path().expect("forked rollout path");
    let forked_items = read_rollout_items(&forked_path);
    let forked_items = forked_items
        .iter()
        .map(|item| serde_json::to_value(item).unwrap())
        .collect::<Vec<_>>();
    let source_items = source_items
        .iter()
        .map(|item| serde_json::to_value(item).unwrap())
        .collect::<Vec<_>>();
    assert!(
        forked_items.starts_with(&source_items),
        "forked history should start with the supplied source history"
    );
}

fn read_rollout_items(path: &std::path::Path) -> Vec<RolloutItem> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => panic!("failed to read rollout file {}: {err}", path.display()),
    };
    let mut items: Vec<RolloutItem> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(err) => panic!("failed to parse rollout JSON line `{line}`: {err}"),
        };
        let rl: RolloutLine = match serde_json::from_value(v) {
            Ok(line) => line,
            Err(err) => panic!("failed to parse rollout line `{line}`: {err}"),
        };
        match rl.item {
            RolloutItem::SessionMeta(_) => {}
            other => items.push(other),
        }
    }
    items
}
