//! Mailbox scheduling preserves deferred responses and drains graceful interrupts.
//! Queued mail reaches the next request without losing completed response items.

use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_features::Feature;
use codex_protocol::AgentPath;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use tokio::sync::oneshot;

#[derive(Clone, Copy)]
enum Boundary {
    Reasoning,
    Commentary,
    PartialAnswer,
}

#[tracing_test::traced_test]
#[test_case(Boundary::Reasoning, false; "reasoning_default")]
#[test_case(Boundary::Commentary, false; "commentary_default")]
#[test_case(Boundary::PartialAnswer, false; "partial_answer_default")]
#[test_case(Boundary::Reasoning, true; "reasoning_deferred")]
#[test_case(Boundary::Commentary, true; "commentary_deferred")]
#[test_case(Boundary::PartialAnswer, true; "partial_answer_deferred")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mailbox_preemption_preserves_response_when_deferred(
    boundary: Boundary,
    defer_mailbox_preemption: bool,
) -> anyhow::Result<()> {
    let (release, gate) = oneshot::channel();
    let (added, done, boundary_name) = match boundary {
        Boundary::Reasoning => (
            responses::ev_reasoning_item_added("boundary", &["Preparing the next action"]),
            responses::ev_reasoning_item("boundary", &["Preparing the next action"], &[]),
            "reasoning",
        ),
        Boundary::Commentary | Boundary::PartialAnswer => {
            let phase = match boundary {
                Boundary::Commentary => "commentary",
                Boundary::PartialAnswer => "partial_answer",
                Boundary::Reasoning => unreachable!(),
            };
            let mut done = responses::ev_assistant_message("boundary", "I will update the plan.");
            done["item"]["phase"] = json!(phase);
            let mut added = responses::ev_message_item_added("boundary", "");
            added["item"]["phase"] = json!(phase);
            (added, done, phase)
        }
    };
    let (streaming, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![responses::ev_response_created("first"), added]),
            },
            StreamingSseChunk {
                gate: Some(gate),
                body: responses::sse(vec![
                    done,
                    responses::ev_function_call(
                        "planned-action",
                        "update_plan",
                        r#"{"plan":[{"step":"Reply to the user","status":"in_progress"}]}"#,
                    ),
                    responses::ev_completed("first"),
                ]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                responses::ev_response_created("follow-up"),
                responses::ev_assistant_message("answer", "The worker update is now included."),
                responses::ev_completed("follow-up"),
            ]),
        }],
    ])
    .await;
    let config_server = responses::start_mock_server().await;
    let base_url = format!("{}/v1", streaming.uri());
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url);
            config.update_plan_enabled = true;
            config
                .features
                .set_enabled(Feature::DeferMailboxPreemption, defer_mailbox_preemption)
                .expect("set mailbox policy");
            // The gated server captures uncompressed request bodies for assertions.
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("disable request compression");
        })
        .build_with_auto_env(&config_server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![super::text(
            "Plan the next action while the worker finishes.",
        )]))
        .await?;
    wait_for_event(
        &test.codex,
        |event| matches!(event, EventMsg::ItemStarted(item) if item.item.id() == "boundary"),
    )
    .await;
    for message in ["Worker found the result.", "Worker checked the result."] {
        test.codex
            .submit(Op::InterAgentCommunication {
                communication: InterAgentCommunication::new(
                    AgentPath::root().join("worker").expect("worker path"),
                    AgentPath::root(),
                    Vec::new(),
                    message.to_string(),
                    /*trigger_turn*/ false,
                ),
                start_options: Default::default(),
            })
            .await?;
    }
    // The barrier confirms that both messages were queued before finishing the item.
    test.codex
        .submit(Op::RealtimeConversationListVoices)
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RealtimeConversationListVoicesResponse(_))
    })
    .await;
    release.send(()).expect("release response");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let logs = String::from_utf8(
        tracing_test::internal::global_buf()
            .lock()
            .expect("test log buffer is not poisoned")
            .clone(),
    )?;
    let thread_id = test.session_configured.thread_id.to_string();
    assert_eq!(
        logs.lines()
            .filter(|line| line.contains("codex.mailbox_preemption") && line.contains(&thread_id))
            .count(),
        usize::from(!defer_mailbox_preemption),
    );
    let requests = streaming
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice::<Value>(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(requests.len(), 2);
    let input = requests[1]["input"].as_array().expect("follow-up input");
    let output = input
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == "planned-action");
    assert_eq!(
        output.map(|item| &item["output"]),
        defer_mailbox_preemption.then_some(&json!("Plan updated")),
    );
    let messages = input
        .iter()
        .filter(|item| item["type"] == "agent_message")
        .map(|item| item["content"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        vec![
            json!([{"type": "input_text", "text": "Worker found the result."}]),
            json!([{"type": "input_text", "text": "Worker checked the result."}]),
        ],
    );
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    insta::assert_snapshot!(
        format!("mailbox_preemption_{boundary_name}_deferred_{defer_mailbox_preemption}"),
        context_snapshot::format_context_snapshot(
            "Worker messages arrive during a response. Deferred mailbox preemption lets the planned tool execute before the next request receives both messages.",
            &entries,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    streaming.shutdown().await;
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MailboxDrain {
    CompleteWithoutAcknowledgement,
    CompleteBeforeInterrupt,
    BufferedItems,
    UserSteering,
}

#[test_case(MailboxDrain::CompleteWithoutAcknowledgement; "without_acknowledgement")]
#[test_case(MailboxDrain::CompleteBeforeInterrupt; "completion_already_buffered")]
#[test_case(MailboxDrain::BufferedItems; "buffered_items")]
#[test_case(MailboxDrain::UserSteering; "user_steering_during_drain")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mailbox_preemption_drains_websocket(scenario: MailboxDrain) -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));

    let boundary = responses::ev_reasoning_item("boundary", &["Preparing the next action"], &[]);
    let mut exchanges = mailbox_initial_responses(&boundary);
    let mut completed = responses::ev_completed_with_tokens("first", /*total_tokens*/ 42);
    let mut expected = expected_mailbox_input();
    let trailing = match scenario {
        // The bridge can finish its single item without acknowledging the interrupt.
        MailboxDrain::CompleteWithoutAcknowledgement | MailboxDrain::CompleteBeforeInterrupt => {
            vec![completed]
        }
        MailboxDrain::BufferedItems | MailboxDrain::UserSteering => {
            let mut call = responses::ev_function_call(
                "buffered-tool",
                "update_plan",
                r#"{"plan":[{"step":"Must not run before reading mail","status":"in_progress"}]}"#,
            );
            call["item"]["id"] = json!("buffered-tool-item");
            completed["type"] = json!("response.incomplete");
            completed["response"]["status"] = json!("incomplete");
            completed["response"]["incomplete_details"] = json!({"reason": "interrupted"});
            // The completed call remains in server history, paired with an aborted output.
            expected.insert(
                0,
                json!({
                    "type": "function_call_output", "call_id": "buffered-tool",
                    "output": "aborted before dispatch",
                }),
            );
            vec![
                json!({"type": "response.interrupt.accepted", "response_id": "first"}),
                responses::ev_reasoning_item_added("buffered", &[]),
                responses::ev_reasoning_item("buffered", &["Already completed"], &[]),
                call,
                responses::ev_reasoning_item_added("discarded", &[]),
                responses::ev_reasoning_summary_text_delta("Unfinished reasoning"),
                json!({"type": "response.output_item.interrupted", "item_id": "discarded", "output_index": 3}),
                completed,
            ]
        }
    };
    if scenario == MailboxDrain::CompleteBeforeInterrupt {
        exchanges[1].extend(trailing);
    } else {
        exchanges.push(trailing);
    }
    exchanges.push(vec![
        responses::ev_response_created("follow-up"),
        responses::ev_assistant_message("answer", "The worker updates are included."),
        responses::ev_completed("follow-up"),
    ]);
    let server = responses::start_websocket_server(vec![exchanges]).await;
    let release = server.gate_response_event(&boundary);
    let release_drain = (scenario == MailboxDrain::UserSteering).then(|| {
        server.gate_response_event(
            &json!({"type": "response.interrupt.accepted", "response_id": "first"}),
        )
    });
    let test = start_mailbox_preemption(&server, release).await?;
    if let Some(release_drain) = release_drain {
        server
            .wait_for_request(/*connection_index*/ 0, /*request_index*/ 2)
            .await;
        let steering = "Use both worker updates.";
        let submission = test
            .codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![super::text(steering)]))
            .await?;
        assert!(matches!(submission, TurnInputSubmission::Steered { .. }));
        // Steering must remain queued while the interrupted response is still draining.
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_secs(1),
                wait_for_event(&test.codex, |event| {
                    assert!(
                        !matches!(event, EventMsg::Error(_) | EventMsg::StreamError(_)),
                        "{event:?}"
                    );
                    matches!(event, EventMsg::UserMessage(_))
                }),
            )
            .await
            .is_err(),
            "steering must not abandon the mailbox drain"
        );
        expected.insert(
            1,
            json!({
                "type": "message", "role": "user",
                "content": [{"type": "input_text", "text": steering}],
            }),
        );
        release_drain.send(()).expect("release mailbox drain");
    }
    finish_mailbox_preemption(test).await?;

    let requests = server.single_connection();
    assert_eq!(requests[0].body_json()["generate"], false);
    if scenario != MailboxDrain::CompleteBeforeInterrupt {
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests[2].body_json(),
            json!({
                "type": "response.interrupt", "response_id": "first", "mode": "discard_partial_items",
            })
        );
    }
    let follow_up = requests.last().expect("follow-up request").body_json();
    assert_eq!(follow_up["type"], "response.create");
    assert_eq!(follow_up["previous_response_id"], "first");
    let input = responses::strip_response_item_ids_from_json(responses::strip_metadata_from_json(
        follow_up["input"].clone(),
    ));
    assert_eq!(input, json!(expected));
    server.shutdown().await;
    Ok(())
}

#[test_case(2; "disconnect_before_interrupt")]
#[test_case(3; "disconnect_after_interrupt")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mailbox_preemption_recovers_from_disconnect(
    requests_before_disconnect: usize,
) -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));

    let boundary = responses::ev_reasoning_item("boundary", &["Preparing the next action"], &[]);
    let mut first_connection = mailbox_initial_responses(&boundary);
    // Close after the boundary, or after receiving the interrupt, without completing.
    first_connection.resize(requests_before_disconnect, Vec::new());
    let server = responses::start_websocket_server(vec![
        first_connection,
        vec![vec![
            responses::ev_response_created("follow-up"),
            responses::ev_assistant_message("answer", "The worker updates are included."),
            responses::ev_completed("follow-up"),
        ]],
    ])
    .await;
    let release = server.gate_response_event(&boundary);
    let test = start_mailbox_preemption(&server, release).await?;
    finish_mailbox_preemption(test).await?;

    // The first request after reconnect must include mail, rather than retry without it.
    let connections = server.connections();
    assert_eq!(connections.len(), 2);
    assert_eq!(connections[0].len(), requests_before_disconnect);
    assert_eq!(connections[1].len(), 1);
    let follow_up = connections[1][0].body_json();
    assert_eq!(follow_up["type"], "response.create");
    assert_eq!(follow_up["previous_response_id"], Value::Null);
    let input = responses::strip_response_item_ids_from_json(responses::strip_metadata_from_json(
        follow_up["input"].clone(),
    ));
    let mail = input
        .as_array()
        .expect("input array")
        .iter()
        .filter(|item| item["type"] == "agent_message")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(mail, expected_mailbox_input());
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mailbox_preemption_preserves_mail_on_content_filter_retry() -> anyhow::Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));

    let boundary = responses::ev_reasoning_item("boundary", &["Preparing the next action"], &[]);
    let mut first_connection = mailbox_initial_responses(&boundary);
    first_connection.push(vec![json!({
        "type": "response.incomplete",
        "response": {"id": "first", "incomplete_details": {"reason": "content_filter"}},
    })]);
    let server = responses::start_websocket_server(vec![
        first_connection,
        vec![
            vec![
                responses::ev_response_created("recovery"),
                responses::ev_function_call(
                    "planned-action",
                    "update_plan",
                    r#"{"plan":[{"step":"Reply after reading mail","status":"in_progress"}]}"#,
                ),
                responses::ev_completed("recovery"),
            ],
            vec![
                responses::ev_response_created("follow-up"),
                responses::ev_assistant_message("answer", "The worker updates are included."),
                responses::ev_completed("follow-up"),
            ],
        ],
    ])
    .await;
    let release = server.gate_response_event(&boundary);
    let test = start_mailbox_preemption(&server, release).await?;
    let mut plan_updates = 0;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        if let EventMsg::StreamError(error) = event {
            assert_eq!(error.additional_details.as_deref(), Some(
                "stream disconnected before completion: Incomplete response returned, reason: content_filter"
            ));
        }
        if matches!(event, EventMsg::PlanUpdate(_)) {
            plan_updates += 1;
        }
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(plan_updates, 1);
    test.codex.shutdown_and_wait().await?;

    let connections = server.connections();
    assert_eq!(connections.len(), 2);
    assert_eq!(connections[0].len(), 3);
    assert_eq!(connections[1].len(), 2);
    // The tool-first retry must already contain both queued mail and filter guidance.
    let recovery = connections[1][0].body_json();
    let input = responses::strip_response_item_ids_from_json(responses::strip_metadata_from_json(
        recovery["input"].clone(),
    ));
    let input = input.as_array().expect("recovery input");
    let mail = input
        .iter()
        .filter(|item| item["type"] == "agent_message")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(mail, expected_mailbox_input());
    assert_eq!(
        input
            .iter()
            .filter(|item| item["role"] == "developer")
            .filter(|item| item["content"][0]["text"]
                .as_str()
                .is_some_and(|text| text.contains("<content_filter_guidance>")))
            .count(),
        1,
    );
    server.shutdown().await;
    Ok(())
}

fn mailbox_initial_responses(boundary: &Value) -> Vec<Vec<Value>> {
    vec![
        vec![
            responses::ev_response_created("warmup"),
            responses::ev_completed("warmup"),
        ],
        vec![
            responses::ev_response_created("first"),
            responses::ev_reasoning_item_added("boundary", &["Preparing the next action"]),
            boundary.clone(),
        ],
    ]
}

fn expected_mailbox_input() -> Vec<Value> {
    ["Worker found the result.", "Worker checked the result."]
        .into_iter()
        .map(|message| {
            json!({
                "type": "agent_message", "author": "/root/worker", "recipient": "/root",
                "content": [{"type": "input_text", "text": message}],
            })
        })
        .collect()
}

async fn start_mailbox_preemption(
    server: &responses::WebSocketTestServer,
    release: oneshot::Sender<()>,
) -> anyhow::Result<TestCodex> {
    let base_url = format!("{}/v1", server.uri());
    let config_server = responses::start_mock_server().await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_model_info_override("gpt-5.4", |model_info| {
            model_info.use_responses_lite = true;
        })
        .with_config(move |config| {
            config
                .features
                .enable(Feature::InstantInterrupt)
                .expect("enable InstantInterrupt feature");
            config.update_plan_enabled = true;
            config.model_provider.base_url = Some(base_url);
            config.model_provider.supports_websockets = true;
        })
        .build_with_auto_env(&config_server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![super::text(
            "Plan the next action while the worker finishes.",
        )]))
        .await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::ItemStarted(item) if item.item.id() == "boundary")
    })
    .await;
    for message in ["Worker found the result.", "Worker checked the result."] {
        test.codex
            .submit(Op::InterAgentCommunication {
                communication: InterAgentCommunication::new(
                    AgentPath::root().join("worker").expect("worker path"),
                    AgentPath::root(),
                    Vec::new(),
                    message.to_string(),
                    /*trigger_turn*/ false,
                ),
                start_options: Default::default(),
            })
            .await?;
    }
    // The barrier confirms that both messages were queued before finishing the item.
    test.codex
        .submit(Op::RealtimeConversationListVoices)
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RealtimeConversationListVoicesResponse(_))
    })
    .await;
    release.send(()).expect("release response");
    Ok(test)
}

async fn finish_mailbox_preemption(test: TestCodex) -> anyhow::Result<()> {
    let mut started_items = Vec::new();
    let mut completed_items = Vec::new();
    wait_for_event(&test.codex, |event| {
        assert!(
            !matches!(event, EventMsg::Error(_) | EventMsg::StreamError(_)),
            "{event:?}"
        );
        assert!(
            !matches!(event, EventMsg::PlanUpdate(_)),
            "buffered tool must not run"
        );
        match event {
            EventMsg::ItemStarted(item) => started_items.push(item.item.id()),
            EventMsg::ItemCompleted(item) => completed_items.push(item.item.id()),
            EventMsg::ReasoningContentDelta(delta) => assert_ne!(delta.item_id, "discarded"),
            _ => {}
        }
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    // The boundary's start was consumed above. Every newly exposed item must finish.
    completed_items.retain(|id| id != "boundary");
    assert_eq!(started_items, completed_items);
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
