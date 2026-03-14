use codex_core::NewThread;
use codex_core::parse_turn_item;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
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

fn find_user_input_positions(items: &[RolloutItem]) -> Vec<usize> {
    let mut pos = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if let RolloutItem::ResponseItem(response_item) = it
            && let Some(TurnItem::UserMessage(_)) = parse_turn_item(response_item)
        {
            pos.push(i);
        }
    }
    pos
}

fn truncate_before_nth_user_message(
    items: &[RolloutItem],
    nth_user_message: usize,
) -> Vec<RolloutItem> {
    if nth_user_message == usize::MAX {
        return items.to_vec();
    }
    let user_inputs = find_user_input_positions(items);
    let Some(cut_idx) = user_inputs.get(nth_user_message).copied() else {
        return Vec::new();
    };
    items[..cut_idx].to_vec()
}

fn read_items_materialized(p: &std::path::Path) -> Vec<RolloutItem> {
    let text =
        std::fs::read_to_string(p).unwrap_or_else(|err| panic!("read rollout file {p:?}: {err}"));
    let mut items: Vec<RolloutItem> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|err| panic!("jsonl line parse: {err}"));
        let rl: RolloutLine =
            serde_json::from_value(v).unwrap_or_else(|err| panic!("rollout line parse: {err}"));
        match rl.item {
            RolloutItem::SessionMeta(_) => {}
            RolloutItem::ForkReference(reference) => {
                let parent_items = read_items_materialized(&reference.rollout_path);
                items.extend(truncate_before_nth_user_message(
                    &parent_items,
                    reference.nth_user_message,
                ));
            }
            other => items.push(other),
        }
    }
    items
}

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
                items: vec![UserInput::Text {
                    text: text.to_string(),
                    text_elements: Vec::new(),
                }],
                final_output_json_schema: None,
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
    let base_items = read_items_materialized(&base_path);
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
        .fork_thread(1, config_for_fork.clone(), base_path.clone(), false, None)
        .await
        .expect("fork 1");

    let fork1_path = codex_fork1.rollout_path().expect("rollout path");

    // GetHistory on fork1 flushed; the file is ready.
    let fork1_items = read_items_materialized(&fork1_path);
    assert!(fork1_items.len() > expected_after_first.len());
    pretty_assertions::assert_eq!(
        serde_json::to_value(&fork1_items[..expected_after_first.len()]).unwrap(),
        serde_json::to_value(&expected_after_first).unwrap()
    );

    // Fork again with n=0 → drops the (new) last user message, leaving only the first.
    let NewThread {
        thread: codex_fork2,
        ..
    } = thread_manager
        .fork_thread(0, config_for_fork.clone(), fork1_path.clone(), false, None)
        .await
        .expect("fork 2");

    let fork2_path = codex_fork2.rollout_path().expect("rollout path");
    // GetHistory on fork2 flushed; the file is ready.
    let fork1_items = read_items_materialized(&fork1_path);
    let fork1_user_inputs = find_user_input_positions(&fork1_items);
    let cut_last_on_fork1 = fork1_user_inputs
        .get(fork1_user_inputs.len().saturating_sub(1))
        .copied()
        .unwrap_or(0);
    let expected_after_second: Vec<RolloutItem> = fork1_items[..cut_last_on_fork1].to_vec();
    let fork2_items = read_items_materialized(&fork2_path);
    assert!(fork2_items.len() > expected_after_second.len());
    pretty_assertions::assert_eq!(
        serde_json::to_value(&fork2_items[..expected_after_second.len()]).unwrap(),
        serde_json::to_value(&expected_after_second).unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_thread_session_configured_preserves_parent_and_history() {
    skip_if_no_network!();

    let server = MockServer::start().await;
    let sse = sse(vec![ev_response_created("resp"), ev_completed("resp")]);
    let response = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(response)
        .expect(1)
        .mount(&server)
        .await;

    let mut builder = test_codex();
    let test = builder.build(&server).await.expect("create conversation");
    let codex = test.codex.clone();
    let thread_manager = test.thread_manager.clone();
    let config_for_fork = test.config.clone();
    let parent_thread_id = test.session_configured.session_id;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "seed".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();
    let _ = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let base_path = codex.rollout_path().expect("rollout path");

    let NewThread {
        thread_id: child_thread_id,
        session_configured,
        ..
    } = thread_manager
        .fork_thread(usize::MAX, config_for_fork, base_path, false, None)
        .await
        .expect("fork thread");

    pretty_assertions::assert_eq!(session_configured.forked_from_id, Some(parent_thread_id));
    assert_ne!(child_thread_id, parent_thread_id);
}
