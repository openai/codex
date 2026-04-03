use codex_protocol::AgentPath;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_message_item_added;
use core_test_support::responses::ev_output_text_delta;
use core_test_support::responses::ev_reasoning_item;
use core_test_support::responses::ev_reasoning_item_added;
use core_test_support::responses::ev_response_created;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tokio::sync::oneshot;

fn ev_message_item_done(id: &str, text: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": id,
            "content": [{"type": "output_text", "text": text}]
        }
    })
}

fn sse_event(event: Value) -> String {
    responses::sse(vec![event])
}

fn message_input_texts(body: &Value, role: &str) -> Vec<String> {
    body.get("input")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
        .filter(|item| item.get("role").and_then(Value::as_str) == Some(role))
        .filter_map(|item| item.get("content").and_then(Value::as_array))
        .flatten()
        .filter(|span| span.get("type").and_then(Value::as_str) == Some("input_text"))
        .filter_map(|span| span.get("text").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "TODO(aibrahim): flaky"]
async fn injected_user_input_triggers_follow_up_request_with_deltas() {
    let (gate_completed_tx, gate_completed_rx) = oneshot::channel();

    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_message_item_added("msg-1", "")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_output_text_delta("first ")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_output_text_delta("turn")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_message_item_done("msg-1", "first turn")),
        },
        StreamingSseChunk {
            gate: Some(gate_completed_rx),
            body: sse_event(ev_completed("resp-1")),
        },
    ];

    let second_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-2")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_completed("resp-2")),
        },
    ];

    let (server, _completions) =
        start_streaming_sse_server(vec![first_chunks, second_chunks]).await;

    let codex = test_codex()
        .with_model("gpt-5.1")
        .build_with_streaming_server(&server)
        .await
        .unwrap()
        .codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first prompt".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |event| {
        matches!(event, EventMsg::AgentMessageContentDelta(_))
    })
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "second prompt".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    let _ = gate_completed_tx.send(());

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);

    let first_body: Value = serde_json::from_slice(&requests[0]).expect("parse first request");
    let second_body: Value = serde_json::from_slice(&requests[1]).expect("parse second request");

    let first_texts = message_input_texts(&first_body, "user");
    assert!(first_texts.iter().any(|text| text == "first prompt"));
    assert!(!first_texts.iter().any(|text| text == "second prompt"));

    let second_texts = message_input_texts(&second_body, "user");
    assert!(second_texts.iter().any(|text| text == "first prompt"));
    assert!(second_texts.iter().any(|text| text == "second prompt"));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_inter_agent_mail_triggers_follow_up_after_reasoning_item() {
    let (gate_reasoning_done_tx, gate_reasoning_done_rx) = oneshot::channel();

    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_reasoning_item_added("reason-1", &["thinking"])),
        },
        StreamingSseChunk {
            gate: Some(gate_reasoning_done_rx),
            body: responses::sse(vec![
                ev_reasoning_item("reason-1", &["thinking"], &[]),
                ev_message_item_added("msg-stale", ""),
                ev_output_text_delta("stale final"),
                ev_message_item_done("msg-stale", "stale final"),
                ev_completed("resp-1"),
            ]),
        },
    ];

    let second_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-2")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_completed("resp-2")),
        },
    ];

    let (server, _completions) =
        start_streaming_sse_server(vec![first_chunks, second_chunks]).await;

    let codex = test_codex()
        .with_model("gpt-5.1")
        .build_with_streaming_server(&server)
        .await
        .unwrap()
        .codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first prompt".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |event| {
        matches!(
            event,
            EventMsg::ItemStarted(item_started)
                if matches!(&item_started.item, TurnItem::Reasoning(_))
        )
    })
    .await;

    codex
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new(
                AgentPath::try_from("/root/worker").expect("worker path should parse"),
                AgentPath::root(),
                Vec::new(),
                "queued child update".to_string(),
                /*trigger_turn*/ false,
            ),
        })
        .await
        .unwrap();

    let _ = gate_reasoning_done_tx.send(());

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);

    let first_body: Value = serde_json::from_slice(&requests[0]).expect("parse first request");
    let second_body: Value = serde_json::from_slice(&requests[1]).expect("parse second request");

    let first_texts = message_input_texts(&first_body, "user");
    assert!(first_texts.iter().any(|text| text == "first prompt"));

    let second_body_text = second_body.to_string();
    assert!(
        second_body_text.contains("queued child update"),
        "second request should include queued queue-only child mail"
    );
    assert!(
        !second_body_text.contains("stale final"),
        "second request should not include answer text from the preempted response"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_inter_agent_mail_triggers_follow_up_after_commentary_message_item() {
    let (gate_message_done_tx, gate_message_done_rx) = oneshot::channel();

    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_message_item_added("msg-1", "")),
        },
        StreamingSseChunk {
            gate: Some(gate_message_done_rx),
            body: responses::sse(vec![
                ev_output_text_delta("first answer"),
                serde_json::json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "message",
                        "role": "assistant",
                        "id": "msg-1",
                        "content": [{"type": "output_text", "text": "first answer"}],
                        "phase": "commentary",
                    }
                }),
                ev_message_item_added("msg-stale", ""),
                ev_output_text_delta("stale final"),
                ev_message_item_done("msg-stale", "stale final"),
                ev_completed("resp-1"),
            ]),
        },
    ];

    let second_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-2")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_completed("resp-2")),
        },
    ];

    let (server, _completions) =
        start_streaming_sse_server(vec![first_chunks, second_chunks]).await;

    let codex = test_codex()
        .with_model("gpt-5.1")
        .build_with_streaming_server(&server)
        .await
        .unwrap()
        .codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first prompt".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |event| {
        matches!(
            event,
            EventMsg::ItemStarted(item_started)
                if matches!(&item_started.item, TurnItem::AgentMessage(_))
        )
    })
    .await;

    codex
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new(
                AgentPath::try_from("/root/worker").expect("worker path should parse"),
                AgentPath::root(),
                Vec::new(),
                "queued child update".to_string(),
                /*trigger_turn*/ false,
            ),
        })
        .await
        .unwrap();

    let _ = gate_message_done_tx.send(());

    let final_message = wait_for_event(&codex, |event| {
        matches!(
            event,
            EventMsg::AgentMessage(message) if message.message == "first answer"
        )
    })
    .await;
    assert!(matches!(final_message, EventMsg::AgentMessage(_)));

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);

    let first_body: Value = serde_json::from_slice(&requests[0]).expect("parse first request");
    let second_body: Value = serde_json::from_slice(&requests[1]).expect("parse second request");

    let first_texts = message_input_texts(&first_body, "user");
    assert!(first_texts.iter().any(|text| text == "first prompt"));

    let second_body_text = second_body.to_string();
    assert!(
        second_body_text.contains("queued child update"),
        "second request should include queued queue-only child mail"
    );
    assert!(
        second_body_text.contains("first answer"),
        "follow-up request should include the fully consumed first assistant answer"
    );
    assert!(
        !second_body_text.contains("stale final"),
        "second request should not include answer text after the first completed message item"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_input_does_not_preempt_after_reasoning_item() {
    let (gate_reasoning_done_tx, gate_reasoning_done_rx) = oneshot::channel();

    let first_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-1")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_reasoning_item_added("reason-1", &["thinking"])),
        },
        StreamingSseChunk {
            gate: Some(gate_reasoning_done_rx),
            body: responses::sse(vec![
                ev_reasoning_item("reason-1", &["thinking"], &[]),
                ev_message_item_added("msg-1", ""),
                ev_output_text_delta("first answer"),
                ev_message_item_done("msg-1", "first answer"),
                ev_completed("resp-1"),
            ]),
        },
    ];

    let second_chunks = vec![
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_response_created("resp-2")),
        },
        StreamingSseChunk {
            gate: None,
            body: sse_event(ev_completed("resp-2")),
        },
    ];

    let (server, _completions) =
        start_streaming_sse_server(vec![first_chunks, second_chunks]).await;

    let codex = test_codex()
        .with_model("gpt-5.1")
        .build_with_streaming_server(&server)
        .await
        .unwrap()
        .codex;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first prompt".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |event| {
        matches!(
            event,
            EventMsg::ItemStarted(item_started)
                if matches!(&item_started.item, TurnItem::Reasoning(_))
        )
    })
    .await;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "second prompt".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    let _ = gate_reasoning_done_tx.send(());

    let final_message = wait_for_event(&codex, |event| {
        matches!(
            event,
            EventMsg::AgentMessage(message) if message.message == "first answer"
        )
    })
    .await;
    assert!(matches!(final_message, EventMsg::AgentMessage(_)));

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);

    let first_body: Value = serde_json::from_slice(&requests[0]).expect("parse first request");
    let second_body: Value = serde_json::from_slice(&requests[1]).expect("parse second request");

    let first_texts = message_input_texts(&first_body, "user");
    assert!(first_texts.iter().any(|text| text == "first prompt"));
    assert!(!first_texts.iter().any(|text| text == "second prompt"));

    let second_user_texts = message_input_texts(&second_body, "user");
    assert!(second_user_texts.iter().any(|text| text == "first prompt"));
    assert!(second_user_texts.iter().any(|text| text == "second prompt"));

    let second_body_text = second_body.to_string();
    assert!(
        second_body_text.contains("first answer"),
        "follow-up request should include the fully consumed first assistant answer"
    );

    server.shutdown().await;
}
