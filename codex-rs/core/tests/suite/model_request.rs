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
use tokio::sync::Semaphore;
use tokio::time::Duration;
use tokio::time::timeout;

#[derive(Debug)]
struct Gates {
    delivered: Semaphore,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_request_gates_websocket_output_but_not_warmup() -> Result<()> {
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
            ev_completed("response"),
        ],
    ]])
    .await;
    let gates = Arc::new(Gates {
        delivered: Semaphore::new(0),
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
        .with_extensions(Arc::new(registry.build()));
    let test = builder.build_with_websocket_server(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "hello".into(),
            text_elements: vec![],
        }]))
        .await?;
    timeout(Duration::from_secs(10), gates.output_waiting.acquire())
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
    test.codex.submit(Op::Interrupt).await?;
    timeout(Duration::from_secs(10), gates.cancelled.acquire())
        .await??
        .forget();
    server.shutdown().await;
    Ok(())
}
