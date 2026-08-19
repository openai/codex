use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageDelivery;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_user_message_async_emits_item_and_does_not_end_the_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const CALL_ID: &str = "async-message-call";
    const MESSAGE: &str = "Still investigating.";

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(
                    CALL_ID,
                    "functions",
                    "send_user_message_async",
                    &json!({ "message": MESSAGE }).to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("final-message", "Finished."),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.2", |model| {
            model
                .experimental_supported_tools
                .push("send_user_message_async".to_string());
        })
        .build_with_auto_env(&server)
        .await?;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Keep me updated.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;

    let started = wait_for_event_match(test.codex.as_ref(), |event| {
        let EventMsg::ItemStarted(event) = event else {
            return None;
        };
        let TurnItem::AgentMessage(message) = &event.item else {
            return None;
        };
        if message.delivery != Some(AgentMessageDelivery::Async) {
            return None;
        }
        Some(message.clone())
    })
    .await;
    assert_eq!(
        serde_json::to_value(&started)?,
        serde_json::to_value(AgentMessageItem {
            id: CALL_ID.to_string(),
            content: vec![AgentMessageContent::Text {
                text: MESSAGE.to_string(),
            }],
            phase: Some(MessagePhase::FinalAnswer),
            memory_citation: None,
            delivery: Some(AgentMessageDelivery::Async),
        })?
    );

    let completed = wait_for_event_match(test.codex.as_ref(), |event| {
        let EventMsg::ItemCompleted(event) = event else {
            return None;
        };
        let TurnItem::AgentMessage(message) = &event.item else {
            return None;
        };
        if message.delivery != Some(AgentMessageDelivery::Async) {
            return None;
        }
        Some(message.clone())
    })
    .await;
    assert_eq!(
        serde_json::to_value(completed)?,
        serde_json::to_value(started)?
    );

    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0].body_json()["tools"]
            .as_array()
            .is_some_and(|tools| {
                tools.iter().any(|tool| {
                    tool["type"] == "function" && tool["name"] == "send_user_message_async"
                })
            }),
        "the async message tool should be directly visible to the model"
    );
    assert_eq!(
        requests[1].function_call_output_text(CALL_ID),
        Some(r#"{"accepted":true}"#.to_string())
    );
    let has_synthetic_assistant_message = requests[1].input().into_iter().any(|item| {
        item["type"] == "message"
            && item["role"] == "assistant"
            && item.to_string().contains(MESSAGE)
    });
    assert!(
        !has_synthetic_assistant_message,
        "the user-visible item should not inject a synthetic assistant message into model context"
    );

    Ok(())
}
