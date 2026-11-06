//! Output gates allow inference immediately, hold complete output, and cancel with the turn.
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::GuardianAssessment;
use codex_extension_api::ModelRequestContributor;
use codex_extension_api::ModelRequestInput;
use codex_extension_api::ModelRequestKind;
use codex_extension_api::ModelResponseInterceptor;
use codex_extension_api::ModelResponseStream;
use codex_extension_api::ResponseEvent;
use codex_features::Feature;
use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::responses::{self};
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Semaphore;
use tokio::time::Duration;
use tokio::time::timeout;

#[derive(Debug)]
struct Gates {
    delivered: Semaphore,
    output_received: Semaphore,
    output_waiting: Semaphore,
    cancelled: Semaphore,
    output_items: std::sync::atomic::AtomicUsize,
    guardian_assessments: std::sync::Mutex<Vec<(GuardianAssessment, usize)>>,
    metadata_event_types: std::sync::Mutex<Vec<&'static str>>,
}
#[derive(Debug)]
struct Contributor(Arc<Gates>);
impl ModelRequestContributor for Contributor {
    fn request(&self, input: ModelRequestInput<'_>) -> Option<Box<dyn ModelResponseInterceptor>> {
        input
            .client_metadata
            .get_or_insert_default()
            .insert("test-interceptor".into(), "present".into());
        if input.kind == ModelRequestKind::Warmup {
            return None;
        }
        Some(Box::new(Permit {
            gates: self.0.clone(),
            released: false,
        }))
    }
}
struct Permit {
    gates: Arc<Gates>,
    released: bool,
}
impl ModelResponseInterceptor for Permit {
    fn intercept(mut self: Box<Self>, mut stream: ModelResponseStream) -> ModelResponseStream {
        Box::pin(
            futures::stream::once(async move {
                let mut events = Vec::new();
                let mut output_items = 0;
                while let Some(event) = stream.next().await {
                    let metadata_event_type = match &event {
                        Ok(ResponseEvent::ModelVerifications(_)) => Some("verification"),
                        Ok(ResponseEvent::TurnModerationMetadata(_)) => Some("moderation"),
                        Ok(ResponseEvent::SafetyBuffering(_)) => Some("buffering"),
                        Ok(ResponseEvent::GuardianAssessment(_)) => Some("guardian"),
                        _ => None,
                    };
                    if let Some(metadata_event_type) = metadata_event_type {
                        self.gates
                            .metadata_event_types
                            .lock()
                            .expect("metadata events mutex is not poisoned")
                            .push(metadata_event_type);
                    }
                    if let Ok(ResponseEvent::GuardianAssessment(assessment)) = &event {
                        self.gates
                            .guardian_assessments
                            .lock()
                            .expect("guardian assessments mutex is not poisoned")
                            .push((assessment.clone(), output_items));
                    }
                    if matches!(&event, Ok(ResponseEvent::OutputItemDone(_))) {
                        output_items += 1;
                        self.gates.output_received.add_permits(1);
                    }
                    let completed = matches!(&event, Ok(ResponseEvent::Completed { .. }));
                    events.push(event);
                    if completed {
                        self.gates
                            .output_items
                            .store(output_items, std::sync::atomic::Ordering::SeqCst);
                        self.gates.output_waiting.add_permits(1);
                        self.gates
                            .delivered
                            .acquire()
                            .await
                            .expect("delivery semaphore remains open")
                            .forget();
                        self.released = true;
                        break;
                    }
                }
                futures::stream::iter(events)
            })
            .flatten(),
        )
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        if !self.released {
            self.gates.cancelled.add_permits(1);
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct BudgetCache(pub(super) AtomicBool);

impl ModelRequestContributor for BudgetCache {
    fn request(&self, _input: ModelRequestInput<'_>) -> Option<Box<dyn ModelResponseInterceptor>> {
        None
    }

    fn budget_exhausted(&self, _thread_id: &str) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[tokio::test]
#[test_case::test_case(false; "budget available")]
#[test_case::test_case(true; "budget exhausted")]
async fn budget_is_checked_again_after_tool_call(exhausted: bool) -> Result<()> {
    const CALL_ID: &str = "status-call";
    const TOOL_NAME: &str = "check_status";
    const TOOL_OUTPUT: &str = "Release ready.";

    let server = responses::start_mock_server().await;
    let model =
        responses::mount_function_call_agent_response(&server, CALL_ID, "{}", TOOL_NAME).await;
    let budget = Arc::new(BudgetCache::default());
    let mut registry = ExtensionRegistryBuilder::new();
    registry.model_request_contributor(budget.clone());
    let mut builder = test_codex().with_extensions(Arc::new(registry.build()));
    let test = builder.build_with_auto_env(&server).await?;
    let mut options = test.start_thread_options().await;
    options.dynamic_tools = vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
        name: TOOL_NAME.into(),
        description: "Check the release status.".into(),
        input_schema: json!({"type": "object", "properties": {}}),
        defer_loading: false,
    })];
    let thread = test.thread_manager.start_thread(options).await?.thread;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Check the release status.".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(
        &thread,
        |event| matches!(event, EventMsg::DynamicToolCallRequest(call) if call.call_id == CALL_ID),
    )
    .await;
    budget.0.store(exhausted, Ordering::Relaxed);
    thread
        .submit(Op::DynamicToolResponse {
            id: CALL_ID.into(),
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: TOOL_OUTPUT.into(),
                }],
                success: true,
            },
        })
        .await?;
    let EventMsg::TurnComplete(completed) =
        wait_for_event(&thread, |event| matches!(event, EventMsg::TurnComplete(_))).await
    else {
        unreachable!();
    };
    let request_count = responses::received_responses_requests(&server).await.len();
    if exhausted {
        assert_eq!(
            completed.error.and_then(|error| error.codex_error_info),
            Some(CodexErrorInfo::SessionBudgetExceeded)
        );
        assert_eq!(request_count, 1);
    } else {
        assert_eq!(completed.error, None);
        assert_eq!(completed.last_agent_message.as_deref(), Some("done"));
        assert_eq!(request_count, 2);
        assert_eq!(
            model
                .completion
                .single_request()
                .function_call_output_content_and_success(CALL_ID),
            Some((Some(TOOL_OUTPUT.into()), None))
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_request_gates_output_and_cancellation() -> Result<()> {
    let server = responses::start_mock_server().await;
    let response = sse(vec![
        ev_response_created("r1"),
        json!({
            "type": "response.metadata", "response_id": "r1",
            "metadata": {"guardian": {"item_id": "m1", "requires_review": false}}
        }),
        ev_assistant_message("m1", "first"),
        ev_assistant_message("m2", "done"),
        ev_completed("r1"),
    ]);
    let model = responses::mount_sse_sequence(&server, vec![response.clone(), response]).await;
    let gates = Arc::new(Gates {
        delivered: Semaphore::new(0),
        output_received: Semaphore::new(0),
        output_waiting: Semaphore::new(0),
        cancelled: Semaphore::new(0),
        output_items: std::sync::atomic::AtomicUsize::new(0),
        guardian_assessments: std::sync::Mutex::new(Vec::new()),
        metadata_event_types: std::sync::Mutex::new(Vec::new()),
    });
    let mut registry = ExtensionRegistryBuilder::new();
    registry.model_request_contributor(Arc::new(Contributor(gates.clone())));
    let mut builder = test_codex().with_extensions(Arc::new(registry.build()));
    let test = builder.build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: vec![],
        }]))
        .await?;
    timeout(Duration::from_secs(10), gates.output_waiting.acquire())
        .await??
        .forget();
    assert_eq!(model.requests().len(), 1);
    assert_eq!(
        *gates.guardian_assessments.lock().unwrap(),
        vec![(
            GuardianAssessment {
                response_id: "r1".into(),
                item_id: "m1".into(),
                requires_review: false,
            },
            0
        )]
    );
    assert_eq!(
        gates.output_items.load(std::sync::atomic::Ordering::SeqCst),
        2
    );
    assert_eq!(
        model.single_request().body_json()["client_metadata"]["test-interceptor"],
        "present"
    );
    // Drain earlier events and prove no assistant output passes a closed delivery gate.
    assert!(
        timeout(
            Duration::from_millis(100),
            wait_for_event(&test.codex, |e| matches!(
                e,
                EventMsg::AgentMessage(_) | EventMsg::TurnComplete(_)
            ))
        )
        .await
        .is_err()
    );
    gates.delivered.add_permits(1);
    wait_for_event(&test.codex, |e| matches!(e, EventMsg::TurnComplete(_))).await;
    assert_eq!(gates.cancelled.available_permits(), 0);

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "again".into(),
            text_elements: vec![],
        }]))
        .await?;
    timeout(Duration::from_secs(10), gates.output_waiting.acquire())
        .await??
        .forget();
    test.codex.submit(Op::Interrupt).await?;
    timeout(Duration::from_secs(10), gates.cancelled.acquire())
        .await??
        .forget();
    assert_eq!(model.requests().len(), 2);
    assert!(
        !model.requests()[1]
            .body_json()
            .to_string()
            .contains("requires_review")
    );
    Ok(())
}

#[tokio::test]
async fn model_request_gates_websocket_output_but_not_warmup() -> Result<()> {
    let completed = ev_completed("response");
    let server = responses::start_websocket_server(vec![vec![
        vec![ev_response_created("warm"), ev_completed("warm")],
        vec![
            ev_response_created("response"),
            json!({
                "type": "response.metadata", "response_id": "response",
                "metadata": {
                    "guardian": {"item_id": "message", "requires_review": true},
                    "openai_verification_recommendation": ["trusted_access_for_cyber"],
                    "openai_chatgpt_moderation_metadata": {"presentation": "inline"},
                    "type": "safety_buffering", "use_cases": ["cyber"],
                    "reasons": ["user_risk"], "future_metadata": true
                }
            }),
            json!({
                "type": "response.metadata", "response_id": "response",
                "metadata": {
                    "guardian": {"item_id": "other", "requires_review": "false"},
                    "openai_verification_recommendation": ["trusted_access_for_cyber"],
                    "openai_chatgpt_moderation_metadata": {"presentation": "inline"},
                    "type": "safety_buffering", "use_cases": ["cyber"],
                    "reasons": ["user_risk"]
                }
            }),
            ev_assistant_message("message", "done"),
        ],
        vec![
            serde_json::json!({"type": "response.interrupt.accepted", "response_id": "response"}),
            completed.clone(),
        ],
        vec![],
    ]])
    .await;
    let release_completion = server.gate_response_event(&completed);
    let gates = Arc::new(Gates {
        delivered: Semaphore::new(0),
        output_received: Semaphore::new(0),
        output_waiting: Semaphore::new(0),
        cancelled: Semaphore::new(0),
        output_items: std::sync::atomic::AtomicUsize::new(0),
        guardian_assessments: std::sync::Mutex::new(Vec::new()),
        metadata_event_types: std::sync::Mutex::new(Vec::new()),
    });
    let mut registry = ExtensionRegistryBuilder::new();
    registry.model_request_contributor(Arc::new(Contributor(gates.clone())));
    let mut builder = test_codex()
        .with_model("gpt-5.2")
        .with_model_info_override("gpt-5.2", |model_info| {
            model_info.use_responses_lite = true;
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::InterruptOnCancel)
                .expect("enable InterruptOnCancel feature");
        })
        .with_extensions(Arc::new(registry.build()));
    let test = builder.build_with_websocket_server(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: vec![],
        }]))
        .await?;
    timeout(Duration::from_secs(10), gates.output_received.acquire())
        .await??
        .forget();
    let requests = server.single_connection();
    assert_eq!(
        *gates
            .metadata_event_types
            .lock()
            .expect("metadata events mutex is not poisoned"),
        vec![
            "verification",
            "moderation",
            "buffering",
            "guardian",
            "verification",
            "moderation",
            "buffering",
        ]
    );
    assert_eq!(
        *gates.guardian_assessments.lock().unwrap(),
        vec![(
            GuardianAssessment {
                response_id: "response".into(),
                item_id: "message".into(),
                requires_review: true,
            },
            0
        )]
    );
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body_json()["generate"], false);
    assert_eq!(
        requests[0].body_json()["client_metadata"]["test-interceptor"],
        "present"
    );
    assert_eq!(
        requests[1].body_json()["client_metadata"]["test-interceptor"],
        "present"
    );
    // Neither a terminal response nor the cleanup deadline may release this interceptor.
    tokio::time::pause();
    {
        let cancelled = async {
            test.codex.submit(Op::Interrupt).await?;
            timeout(Duration::from_secs(10), gates.cancelled.acquire())
                .await??
                .forget();
            server
                .wait_for_request(/*connection_index*/ 0, /*request_index*/ 2)
                .await;
            anyhow::Ok(())
        };
        tokio::pin!(cancelled);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "interceptor remained alive during cancelled response cleanup"
            );
            tokio::select! {
                result = &mut cancelled => { result?; break; }
                // Keep the runtime runnable so paused time cannot auto-advance during I/O.
                () = tokio::task::yield_now() => {}
            }
        }
    }
    tokio::time::resume();
    release_completion
        .send(())
        .expect("release interrupted completion");
    assert!(
        server
            .wait_for_closed_connections(/*expected*/ 1, Duration::from_secs(5))
            .await
    );
    server.shutdown().await;
    Ok(())
}

/// The same final answer carries the targeted public flag on completion and in legacy history.
#[tokio::test]
async fn final_answer_carries_its_moderation_decision() -> Result<()> {
    use core_test_support::wait_for_event_match;
    let server = responses::start_mock_server().await;
    let mut answer = responses::ev_assistant_message("msg-42", "Answer");
    answer["item"]["phase"] = json!("final_answer");
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            json!({"type":"response.metadata","metadata":{"openai_chatgpt_moderation_metadata":{
                "final_answer_moderation":{"item_id":"msg-42","moderated":true},
                "generation":{"private":"classifier details"}
            }}}),
            answer,
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let test = test_codex().build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "test".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let item = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ItemCompleted(done) if done.item.id() == "msg-42" => Some(done.item.clone()),
        _ => None,
    })
    .await;
    let codex_protocol::items::TurnItem::AgentMessage(message) = item else {
        panic!("expected assistant answer");
    };
    let public = serde_json::to_value(&message)?;
    assert_eq!(public["moderation"], json!({"moderated": true}));
    let [EventMsg::AgentMessage(legacy)] = &message.as_legacy_events()[..] else {
        panic!("expected one legacy answer");
    };
    assert_eq!(
        serde_json::to_value(&legacy.moderation)?,
        json!({"item_id":"msg-42","moderated":true})
    );
    Ok(())
}
