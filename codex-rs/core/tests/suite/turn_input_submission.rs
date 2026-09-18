use codex_core::NotSubmittedReason;
use codex_core::RecoverTurnRequest;
use codex_core::StartIfIdleSubmission;
use codex_core::StartThreadOptions;
use codex_core::SteerSubmission;
use codex_core::TurnInput;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_core::TurnStartOptions;
use codex_core::config::Constrained;
use codex_core::context::ContextualUserFragment;
use codex_core::context::InternalContextSource;
use codex_core::context::InternalModelContextFragment;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::TurnStartAdmission;
use codex_features::Feature;
use codex_protocol::AgentPath;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::local;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use test_case::test_case;
use tokio::sync::Barrier;
use tokio::sync::oneshot;
use tokio::time::timeout;

#[derive(Debug)]
struct TestAdmission(AtomicBool);

impl TurnStartAdmission for TestAdmission {
    fn admit_turn_start(&self) -> Option<Box<dyn Send>> {
        if self.0.load(Ordering::SeqCst) {
            None
        } else {
            Some(Box::new(()))
        }
    }
}

#[tokio::test]
async fn host_drain_rejects_turn_start_paths_without_recording_input() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(&server, responses::sse_completed("allowed")).await;
    let admission = Arc::new(TestAdmission(AtomicBool::new(true)));
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_start_admission(admission.clone());
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    assert_eq!(
        test.codex
            .start_or_steer_turn(user_message_request("rejected direct input"))
            .await?,
        TurnInputSubmission::NotSubmitted {
            reason: NotSubmittedReason::ServerDraining
        },
    );
    for request in [
        user_message_request("rejected queued input"),
        TurnInputRequest::user_input(Vec::new()),
        TurnInputRequest::new(TurnInput::ResponseItem(responses::user_message_item(
            "rejected continuation",
        ))),
    ] {
        assert_eq!(
            test.codex.start_turn_if_idle(request).await?,
            StartIfIdleSubmission::NotSubmitted {
                reason: NotSubmittedReason::ServerDraining
            },
        );
    }
    assert!(response.requests().is_empty());
    admission.0.store(false, Ordering::SeqCst);
    test.codex
        .start_turn_if_idle(user_message_request("allowed input"))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let request = response.single_request();
    assert!(request.body_contains_text("allowed input"));
    assert!(!request.body_contains_text("rejected direct input"));
    assert!(!request.body_contains_text("rejected queued input"));
    assert!(!request.body_contains_text("rejected continuation"));
    Ok(())
}

#[tokio::test]
async fn host_drain_allows_spawned_agent_input_but_not_automatic_work() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(&server, responses::sse_completed("child")).await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_start_admission(Arc::new(TestAdmission(AtomicBool::new(true))));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    let child = test
        .thread_manager
        .start_thread(StartThreadOptions {
            session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: test.session_configured.thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            })),
            environments: Some(test.codex.environment_selections().await),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?
        .thread;
    for request in [
        user_message_request("queued child input"),
        TurnInputRequest::user_input(Vec::new()),
    ] {
        assert_eq!(
            child.start_turn_if_idle(request).await?,
            StartIfIdleSubmission::NotSubmitted {
                reason: NotSubmittedReason::ServerDraining
            },
        );
    }
    assert_eq!(
        child
            .start_or_steer_turn(user_message_request("external child input"))
            .await?,
        TurnInputSubmission::NotSubmitted {
            reason: NotSubmittedReason::ServerDraining
        },
    );
    assert!(matches!(
        child
            .start_or_steer_turn(user_message_request("delegated input").on_start(
                TurnStartOptions {
                    parent_turn_id: Some("parent-turn".to_string()),
                    ..Default::default()
                }
            ))
            .await?,
        TurnInputSubmission::Started { .. }
    ));
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert!(
        response
            .single_request()
            .body_contains_text("delegated input")
    );
    Ok(())
}

#[tokio::test]
async fn host_drain_allows_running_review_to_finish_its_delegate() -> anyhow::Result<()> {
    use codex_protocol::protocol::ReviewOutputEvent;
    use codex_protocol::protocol::ReviewRequest;
    use codex_protocol::protocol::ReviewTarget;

    let server = responses::start_mock_server().await;
    let expected = ReviewOutputEvent {
        overall_explanation: "review completed during drain".to_string(),
        ..Default::default()
    };
    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            ev_response_created("review"),
            responses::ev_assistant_message("result", &serde_json::to_string(&expected)?),
            ev_completed("review"),
        ]),
    )
    .await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_start_admission(Arc::new(TestAdmission(AtomicBool::new(true))));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    // The host already admitted the parent review; only its child start hits Core admission.
    test.codex
        .submit(Op::Review {
            review_request: ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "review these changes".to_string(),
                },
                user_facing_hint: None,
            },
        })
        .await?;
    let event = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ExitedReviewMode(_))
    })
    .await;
    let EventMsg::ExitedReviewMode(event) = event else {
        unreachable!()
    };
    assert_eq!(event.review_output, Some(expected));
    response.single_request();
    Ok(())
}

#[tokio::test]
async fn host_drain_closes_realtime_after_handoff_error() -> anyhow::Result<()> {
    use codex_protocol::protocol::ConversationStartParams;
    use codex_protocol::protocol::RealtimeConversationClosedEvent;
    use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
    use codex_protocol::protocol::RealtimeEvent;
    use codex_protocol::protocol::RealtimeHandoffRequested;
    use codex_protocol::protocol::RealtimeTranscriptEntry;

    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(&server, responses::sse_completed("unexpected")).await;
    let realtime = responses::start_websocket_server(vec![vec![vec![
        serde_json::json!({
            "type": "session.updated",
            "session": { "id": "draining", "instructions": "backend prompt" }
        }),
        serde_json::json!({
            "type": "conversation.handoff.requested",
            "handoff_id": "rejected",
            "item_id": "rejected",
            "input_transcript": "must not start"
        }),
    ]]])
    .await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_start_admission(Arc::new(TestAdmission(AtomicBool::new(true))));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config({
            let realtime_url = realtime.uri().to_string();
            move |config| {
                config.experimental_realtime_ws_base_url = Some(realtime_url);
                config.realtime.version = codex_config::config_toml::RealtimeWsVersion::V1;
            }
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .submit(Op::RealtimeConversationStart(ConversationStartParams {
            client_managed_handoffs: false,
            delegation_ack_filler: None,
            flush_transcript_tail_on_session_end: false,
            codex_responses_as_items: false,
            codex_response_item_prefix: None,
            codex_response_handoff_mode:
                codex_protocol::protocol::CodexResponseHandoffMode::Thinking,
            codex_response_handoff_channel_prefixes: None,
            model: None,
            output_modality: codex_protocol::protocol::RealtimeOutputModality::Audio,
            include_startup_context: false,
            initial_items: Vec::new(),
            realtime_start_instructions: None,
            realtime_end_instructions: None,
            prompt: Some(Some("backend prompt".to_string())),
            realtime_session_id: None,
            transport: None,
            version: None,
            voice: None,
        }))
        .await?;
    for expected in [
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::HandoffRequested(RealtimeHandoffRequested {
                handoff_id: "rejected".to_string(),
                item_id: "rejected".to_string(),
                input_transcript: "must not start".to_string(),
                active_transcript: vec![RealtimeTranscriptEntry {
                    role: "user".to_string(),
                    text: "must not start".to_string(),
                }],
            }),
        }),
        EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
            payload: RealtimeEvent::Error(
                "Server is draining; retry the turn after reconnecting".to_string(),
            ),
        }),
        EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent {
            reason: Some("error".to_string()),
        }),
    ] {
        let event = wait_for_event(&test.codex, |event| match event {
            EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                payload: RealtimeEvent::HandoffRequested(_) | RealtimeEvent::Error(_),
            })
            | EventMsg::RealtimeConversationClosed(_) => true,
            EventMsg::TurnStarted(_) | EventMsg::Error(_) => panic!("unexpected event: {event:?}"),
            _ => false,
        })
        .await;
        assert_eq!(
            serde_json::to_value(event)?,
            serde_json::to_value(expected)?
        );
    }
    assert!(response.requests().is_empty());
    realtime.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn host_drain_allows_mailbox_work_to_start_a_turn() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let response = responses::mount_sse_once(&server, responses::sse_completed("mailbox")).await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_start_admission(Arc::new(TestAdmission(AtomicBool::new(true))));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&server)
        .await?;
    // Mailbox input is memory-only and must be processed before the host exits.
    test.codex
        .submit(Op::InterAgentCommunication {
            communication: InterAgentCommunication::new(
                AgentPath::try_from("/root/worker").expect("valid agent path"),
                AgentPath::root(),
                Vec::new(),
                "mail while draining".to_string(),
                /*trigger_turn*/ true,
            ),
            start_options: Default::default(),
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert!(
        response
            .single_request()
            .body_contains_text("mail while draining")
    );
    Ok(())
}

fn user_message_request(text: &str) -> TurnInputRequest {
    TurnInputRequest::user_input(vec![UserInput::Text {
        text: text.to_string(),
        text_elements: Vec::new(),
    }])
}

async fn submit_user_message(
    codex: &codex_core::CodexThread,
    text: &str,
) -> codex_protocol::error::Result<TurnInputSubmission> {
    codex.start_or_steer_turn(user_message_request(text)).await
}

#[test_case(ModeKind::Default, ModeKind::Plan; "automatic input cannot enter Plan")]
#[test_case(ModeKind::Plan, ModeKind::Default; "automatic input cannot leave Plan")]
#[tokio::test]
async fn start_turn_if_idle_keeps_automatic_plan_rejections_atomic(
    current_mode: ModeKind,
    proposed_mode: ModeKind,
) {
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .build_with_auto_env(&server)
        .await
        .expect("build turn-input submission session");
    let mut collaboration_mode = test.codex.config_snapshot().await.collaboration_mode;
    collaboration_mode.mode = current_mode;
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            collaboration_mode: Some(collaboration_mode.clone()),
            ..Default::default()
        },
    )
    .await
    .expect("set the current collaboration mode");
    let current_settings = test.codex.thread_settings_snapshot().await;
    collaboration_mode.mode = proposed_mode;
    let overrides = ThreadSettingsOverrides {
        collaboration_mode: Some(collaboration_mode.clone()),
        ..Default::default()
    };
    let submission = test
        .codex
        .start_turn_if_idle(
            TurnInputRequest::new(TurnInput::ResponseItem(responses::user_message_item(
                "rejected automatic input",
            )))
            .with_thread_settings(overrides.clone()),
        )
        .await
        .expect("automatic Plan admission should return a typed rejection");
    assert_eq!(
        submission,
        StartIfIdleSubmission::NotSubmitted {
            reason: NotSubmittedReason::PlanMode,
        }
    );
    assert_eq!(
        test.codex.thread_settings_snapshot().await,
        current_settings
    );

    // Rejection releases the idle reservation, and an explicit user can make
    // either transition without receiving the rejected automatic input.
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let started = test
        .codex
        .start_turn_if_idle(
            user_message_request("explicit user input").with_thread_settings(overrides),
        )
        .await
        .expect("rejection must release the idle reservation for explicit user input");
    assert!(matches!(started, StartIfIdleSubmission::Started { .. }));
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        test.codex.config_snapshot().await.collaboration_mode,
        collaboration_mode
    );
    let request = response_mock.single_request();
    assert!(request.body_contains_text("explicit user input"));
    assert!(!request.body_contains_text("rejected automatic input"));
}

#[tokio::test]
async fn recover_turn_if_idle_preserves_id_and_resumes_plan_mode() {
    let server = responses::start_mock_server().await;
    let response_mock =
        responses::mount_sse_once(&server, responses::sse_completed("resp-1")).await;
    let test = test_codex()
        .build_with_auto_env(&server)
        .await
        .expect("build recovered turn session");
    let turn_id = "durable-recovered-turn";

    let submission = test
        .codex
        .recover_turn_if_idle(RecoverTurnRequest {
            turn_id: turn_id.to_string(),
            thread_settings: ThreadSettingsOverrides {
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Plan,
                    settings: Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
            trace: None,
            cyber_access_program: None,
        })
        .await
        .expect("recovered turn should start");
    assert_eq!(
        submission,
        StartIfIdleSubmission::Started {
            turn_id: turn_id.to_string(),
        }
    );
    assert_eq!(
        test.codex.config_snapshot().await.collaboration_mode.mode,
        ModeKind::Plan
    );

    let started = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnStarted(_))
    })
    .await;
    let EventMsg::TurnStarted(started) = started else {
        unreachable!("wait_for_event returned unexpected event");
    };
    assert_eq!(started.turn_id, turn_id);
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response_mock.single_request();
    let turn_metadata: Value = serde_json::from_str(
        request
            .header("x-codex-turn-metadata")
            .as_deref()
            .expect("recovered turn should include turn metadata"),
    )
    .expect("recovered turn metadata should be valid JSON");
    assert_eq!(turn_metadata["turn_trigger"].as_str(), Some("retry"));
    let user_input_groups = request.message_input_text_groups("user");
    assert_eq!(user_input_groups.len(), 1);
    assert_eq!(user_input_groups[0].len(), 1);
    assert!(user_input_groups[0][0].starts_with("<environment_context>"));
}

/// Internal continuation creates a new turn without adding user authorization.
#[tokio::test]
async fn continue_turn_if_idle_starts_new_turn_with_internal_input() {
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_model("gpt-5.6-sol")
        .with_config(|config| {
            config
                .features
                .enable(codex_features::Feature::FastMode)
                .unwrap();
        })
        .build_with_auto_env(&server)
        .await
        .unwrap();
    responses::mount_sse_once(&server, responses::sse_completed("original")).await;
    let TurnInputSubmission::Started {
        turn_id: previous_turn_id,
    } = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Do the work".to_string(),
            text_elements: Vec::new(),
        }]))
        .await
        .unwrap()
    else {
        panic!("original turn did not start")
    };
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let mock = responses::mount_sse_once(&server, responses::sse_completed("continued")).await;
    let input = ContextualUserFragment::into(InternalModelContextFragment::new(
        InternalContextSource::from_static("daemon_recovery"),
        "Continue the interrupted work.",
    ));
    let schema = serde_json::json!({"type":"object","properties":{},"additionalProperties":false});
    let submission = test
        .codex
        .continue_turn_if_idle(
            TurnInputRequest::new(TurnInput::ResponseItem(input.clone())).on_start(
                TurnStartOptions {
                    final_output_json_schema: Some(schema.clone()),
                    service_tier: Some("priority".to_string()),
                    root_turn_id: Some("originating-turn".to_string()),
                    ..Default::default()
                },
            ),
            previous_turn_id.clone(),
        )
        .await
        .unwrap();
    let TurnInputSubmission::Started { turn_id } = submission else {
        panic!("continuation did not start")
    };
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::UserMessage(_)));
        assert!(!matches!(event, EventMsg::ItemCompleted(event)
            if matches!(event.item, codex_protocol::items::TurnItem::UserMessage(_))));
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let request = mock.single_request();
    assert!(request.has_content_kinds(&["daemon_recovery.internal_context"]));
    let metadata: Value = serde_json::from_str(
        &request
            .header("x-codex-turn-metadata")
            .expect("turn metadata"),
    )
    .unwrap();
    assert_eq!(metadata["root_turn_id"], "originating-turn");
    let body = request.body_json();
    assert_eq!(body["text"]["format"]["schema"], schema);
    assert_eq!(body["service_tier"], "priority");
    // The continuation has completed, but the saved previous ID is still stale.
    assert_eq!(
        test.codex
            .continue_turn_if_idle(
                TurnInputRequest::new(TurnInput::ResponseItem(ContextualUserFragment::into(
                    InternalModelContextFragment::new(
                        InternalContextSource::from_static("daemon_recovery"),
                        "Continue."
                    ),
                ))),
                previous_turn_id,
            )
            .await
            .unwrap(),
        TurnInputSubmission::NotSubmitted {
            reason: NotSubmittedReason::Superseded
        }
    );
    assert_eq!(mock.requests().len(), 1);
    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            approval_policy: Some(AskForApproval::OnRequest),
            ..Default::default()
        },
    )
    .await
    .expect("update permissions before continuation admission");
    assert_eq!(
        test.codex
            .continue_turn_if_idle(
                TurnInputRequest::new(TurnInput::ResponseItem(input)),
                turn_id,
            )
            .await
            .unwrap(),
        TurnInputSubmission::NotSubmitted {
            reason: NotSubmittedReason::Superseded
        }
    );
    assert_eq!(mock.requests().len(), 1);
}

/// Concurrent submissions must start exactly one turn and steer the other message.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_input_submission_reports_started_and_steered_for_concurrent_submissions() {
    let (release_response, response_gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![ev_response_created("resp-1")]),
            },
            StreamingSseChunk {
                gate: Some(response_gate),
                body: responses::sse(vec![ev_completed("resp-1")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        }],
    ])
    .await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .build_with_streaming_server(&server)
        .await
        .expect("build turn-input submission session");
    let codex = Arc::clone(&test.codex);
    let barrier = Arc::new(Barrier::new(3));

    let first_submission = tokio::spawn({
        let codex = Arc::clone(&codex);
        let barrier = Arc::clone(&barrier);
        async move {
            barrier.wait().await;
            submit_user_message(codex.as_ref(), "first message").await
        }
    });
    let second_submission = tokio::spawn({
        let codex = Arc::clone(&codex);
        let barrier = Arc::clone(&barrier);
        async move {
            barrier.wait().await;
            submit_user_message(codex.as_ref(), "second message").await
        }
    });
    barrier.wait().await;

    timeout(
        Duration::from_secs(5),
        server.wait_for_request_count(/*count*/ 1),
    )
    .await
    .expect("the started turn should reach its first model request");
    release_response
        .send(())
        .expect("response gate should remain open");

    let (first_submission, second_submission) = timeout(Duration::from_secs(5), async {
        tokio::join!(first_submission, second_submission)
    })
    .await
    .expect("both concurrent submissions should resolve once their messages are submitted");
    let first_submission = first_submission
        .expect("first submission task should finish")
        .expect("first user message should be submitted");
    let second_submission = second_submission
        .expect("second submission task should finish")
        .expect("second user message should be submitted");
    let (started_turn_id, steered_turn_id, started_message) =
        match (&first_submission, &second_submission) {
            (
                TurnInputSubmission::Started { turn_id: started },
                TurnInputSubmission::Steered { turn_id: steered },
            ) => (started, steered, "first message"),
            (
                TurnInputSubmission::Steered { turn_id: steered },
                TurnInputSubmission::Started { turn_id: started },
            ) => (started, steered, "second message"),
            _ => panic!(
                "concurrent messages must start exactly one turn and steer the other: \
             {first_submission:?}, {second_submission:?}"
            ),
        };
    assert_eq!(started_turn_id, steered_turn_id);

    wait_for_event(codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);
    let request_bodies: Vec<Value> = requests
        .iter()
        .map(|request| serde_json::from_slice(request).expect("parse model request"))
        .collect();
    assert!(request_bodies[0].to_string().contains(started_message));
    assert!(request_bodies[1].to_string().contains("first message"));
    assert!(request_bodies[1].to_string().contains("second message"));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_input_submission_applies_thread_settings_only_after_accepted_input() {
    let (release_response, response_gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![ev_response_created("resp-1")]),
            },
            StreamingSseChunk {
                gate: Some(response_gate),
                body: responses::sse(vec![ev_completed("resp-1")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        }],
    ])
    .await;
    let test = test_codex()
        .with_model("gpt-5.4")
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        })
        .build_with_streaming_server(&server)
        .await
        .expect("build approval-constrained turn-input submission session");
    let codex = &test.codex;

    let started = submit_user_message(codex, "start turn")
        .await
        .expect("first message should start a turn");
    let TurnInputSubmission::Started { turn_id } = started else {
        panic!("first message should start a turn");
    };
    timeout(
        Duration::from_secs(5),
        server.wait_for_request_count(/*count*/ 1),
    )
    .await
    .expect("started turn should reach its first model request");

    let steered_cwd = test.config.cwd.join("steered-environment");
    let steered_environments =
        TurnEnvironmentSelections::new(steered_cwd.clone(), vec![local(steered_cwd)]);
    let steered = codex
        .start_or_steer_turn(
            user_message_request("steer active turn").with_thread_settings(
                ThreadSettingsOverrides {
                    approval_policy: Some(AskForApproval::Never),
                    environments: Some(steered_environments.clone()),
                    ..Default::default()
                },
            ),
        )
        .await
        .expect("persistent settings should not reject a steer");
    assert_eq!(steered, TurnInputSubmission::Steered { turn_id });
    assert_eq!(
        codex.config_snapshot().await.approval_policy,
        AskForApproval::Never
    );
    assert_eq!(
        codex.environment_selections().await,
        steered_environments.environments
    );

    release_response
        .send(())
        .expect("response gate should remain open");
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let rejected_cwd = test.config.cwd.join("rejected-environment");
    let rejected = codex
        .steer_turn(
            user_message_request("no active turn").with_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::OnRequest),
                environments: Some(TurnEnvironmentSelections::new(
                    rejected_cwd.clone(),
                    vec![local(rejected_cwd)],
                )),
                ..Default::default()
            }),
            "missing-turn".to_string(),
        )
        .await
        .expect("idle steer should return a typed rejection");
    assert_eq!(
        rejected,
        SteerSubmission::NotSubmitted {
            reason: NotSubmittedReason::NoActiveTurn,
        }
    );
    assert_eq!(
        codex.config_snapshot().await.approval_policy,
        AskForApproval::Never
    );
    assert_eq!(
        codex.environment_selections().await,
        steered_environments.environments
    );
    server.shutdown().await;
}

#[tokio::test]
async fn start_or_steer_turn_requires_matching_active_output_schema() {
    let (release_response, response_gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![ev_response_created("resp-1")]),
            },
            StreamingSseChunk {
                gate: Some(response_gate),
                body: responses::sse(vec![ev_completed("resp-1")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        }],
    ])
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        })
        .build_with_streaming_server(&server)
        .await
        .expect("build turn-input submission session");
    let codex = &test.codex;
    let active_schema: Value = serde_json::from_str(
        r#"{"type":"object","properties":{"answer":{"type":"string"},"count":{"type":"number"}},"required":["answer","count"]}"#,
    )
    .expect("parse active schema");
    let matching_schema_with_different_object_order: Value = serde_json::from_str(
        r#"{"required":["answer","count"],"properties":{"count":{"type":"number"},"answer":{"type":"string"}},"type":"object"}"#,
    )
    .expect("parse matching schema");
    let different_schema: Value = serde_json::from_str(
        r#"{"type":"object","properties":{"answer":{"type":"number"}},"required":["answer"]}"#,
    )
    .expect("parse different schema");

    let started = codex
        .start_or_steer_turn(
            user_message_request("start turn").on_start(TurnStartOptions {
                final_output_json_schema: Some(active_schema),
                ..Default::default()
            }),
        )
        .await
        .expect("first message should start a turn");
    let TurnInputSubmission::Started { turn_id } = started else {
        panic!("first message should start a turn");
    };
    timeout(
        Duration::from_secs(5),
        server.wait_for_request_count(/*count*/ 1),
    )
    .await
    .expect("started turn should reach its first model request");

    let rejected = codex
        .start_or_steer_turn(
            user_message_request("rejected steer")
                .with_thread_settings(ThreadSettingsOverrides {
                    approval_policy: Some(AskForApproval::Never),
                    ..Default::default()
                })
                .on_start(TurnStartOptions {
                    final_output_json_schema: Some(different_schema),
                    ..Default::default()
                }),
        )
        .await
        .expect("schema mismatch should return a typed rejection");
    assert_eq!(
        rejected,
        TurnInputSubmission::NotSubmitted {
            reason: NotSubmittedReason::ActiveTurnOutputSchemaMismatch,
        }
    );
    assert_eq!(
        codex.config_snapshot().await.approval_policy,
        AskForApproval::OnRequest
    );

    let steered = codex
        .start_or_steer_turn(
            user_message_request("accepted steer").on_start(TurnStartOptions {
                final_output_json_schema: Some(matching_schema_with_different_object_order),
                ..Default::default()
            }),
        )
        .await
        .expect("matching schema should steer");
    assert_eq!(steered, TurnInputSubmission::Steered { turn_id });

    release_response
        .send(())
        .expect("response gate should remain open");
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let requests = server.requests().await;
    assert_eq!(requests.len(), 2);
    let second_request = String::from_utf8_lossy(&requests[1]);
    assert!(second_request.contains("accepted steer"));
    assert!(!second_request.contains("rejected steer"));
    server.shutdown().await;
}

#[test_case(Vec::new(), "local"; "automatic")]
#[test_case(vec![UserInput::Text { text: "Do the work".into(), text_elements: Vec::new() }], "local"; "user")]
#[cfg_attr(unix, test_case(Vec::new(), "remote"; "remote_stays_idle"))]
#[tokio::test]
async fn sampling_is_ready_for_daemon_recovery(
    input: Vec<UserInput>,
    executor: &str,
) -> anyhow::Result<()> {
    let (release, gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
        gate: Some(gate),
        body: responses::sse_completed("automatic"),
    }]])
    .await;
    #[cfg(unix)]
    let remote = if executor == "remote" {
        Some(super::multi_exec_server_sandbox::ExecServerProcess::start().await?)
    } else {
        None
    };
    let mut builder = test_codex();
    #[cfg(unix)]
    if let Some(remote) = &remote {
        builder = builder.with_exec_server_url(&remote.websocket_url);
    }
    let test = builder.build_with_streaming_server(&server).await?;
    let StartIfIdleSubmission::Started { turn_id } = test
        .codex
        .start_turn_if_idle(TurnInputRequest::user_input(input))
        .await?
    else {
        panic!("sampling should start");
    };
    timeout(
        Duration::from_secs(5),
        server.wait_for_request_count(/*count*/ 1),
    )
    .await?;
    let active = test.codex.interrupted_turn().await;
    assert_eq!(
        active.map(|(id, _, _)| id),
        (executor == "local").then_some(turn_id)
    );
    release.send(()).expect("sampling is waiting");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn daemon_recovery_includes_local_environment_that_finished_starting() -> anyhow::Result<()> {
    let (release, gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                ev_response_created("wait"),
                responses::ev_function_call(
                    "wait-local",
                    "wait_for_environment",
                    r#"{"environment_id":"local"}"#,
                ),
                ev_completed("wait"),
            ]),
        }],
        vec![StreamingSseChunk {
            gate: Some(gate),
            body: responses::sse_completed("done"),
        }],
    ])
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.features.enable(Feature::DeferredExecutor).unwrap();
        })
        .build_with_streaming_server(&server)
        .await?;
    let cwd = test.config.cwd.join("new-workspace");
    std::fs::create_dir(&cwd)?;
    let selection = local(cwd.clone());
    // A different workspace starts a new attachment. On this single-threaded runtime,
    // turn startup captures it before the spawned setup task can run.
    let started = test
        .codex
        .start_turn_if_idle(
            user_message_request("wait for the environment").with_thread_settings(
                ThreadSettingsOverrides {
                    environments: Some(TurnEnvironmentSelections::new(
                        cwd,
                        vec![selection.clone()],
                    )),
                    ..Default::default()
                },
            ),
        )
        .await?;
    let StartIfIdleSubmission::Started { turn_id } = started else {
        anyhow::bail!("turn should start");
    };

    timeout(
        Duration::from_secs(5),
        server.wait_for_request_count(/*count*/ 2),
    )
    .await?;
    let request: Value = serde_json::from_slice(&server.requests().await[1])?;
    let output = request["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == "wait-local")
        .expect("the second request should contain the wait result");
    assert_eq!(
        serde_json::from_str::<Value>(output["output"].as_str().unwrap())?,
        serde_json::json!({"environment_id": "local", "status": "ready"}),
    );
    assert_eq!(
        test.codex
            .interrupted_turn()
            .await
            .map(|(id, _, environment)| (id, environment)),
        Some((turn_id, selection)),
    );
    release.send(()).expect("the model response is waiting");
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    Ok(())
}
