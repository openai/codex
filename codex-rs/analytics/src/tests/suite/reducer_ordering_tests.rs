//! Coordinator routing and ordering guarantees across analytics event families.

use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::AppInvocation;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CodeModeToolCallFact;
use crate::facts::CodeModeToolCallStatus;
use crate::facts::ControlToolCallFact;
use crate::facts::ControlToolCallStatus;
use crate::facts::CustomAnalyticsFact;
use crate::facts::ElicitationType;
use crate::facts::InvocationType;
use crate::facts::PluginUsedInput;
use crate::facts::SkillInvocation;
use crate::facts::SkillInvocationLocation;
use crate::facts::SkillInvokedInput;
use crate::facts::TurnProfileFact;
use crate::facts::TurnResolvedConfigFact;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::TEST_PRODUCT_CLIENT_ID;
use crate::tests::support::ingest_code_mode_facts;
use crate::tests::support::ingest_completed_command_execution_item;
use crate::tests::support::ingest_initialize;
use crate::tests::support::ingest_review_prerequisites;
use crate::tests::support::ingest_turn_prerequisites;
use crate::tests::support::sample_command_execution_item;
use crate::tests::support::sample_plugin_metadata;
use crate::tests::support::sample_turn_completed_notification;
use crate::tests::support::sample_turn_profile;
use crate::tests::support::sample_turn_resolved_config;
use crate::tests::support::sample_turn_start_response;
use crate::tests::support::sample_turn_started_notification;
use crate::tests::support::test_tracking_context;
use crate::tests::support::test_turn_metadata;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadRealtimeClosedNotification;
use codex_app_server_protocol::ThreadRealtimeStartedNotification;
use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
use codex_protocol::protocol::RealtimeConversationVersion;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn stateless_app_and_plugin_facts_preserve_arrival_order() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let tracking = test_tracking_context("thread-1", "turn-1");

    for fact in [
        CustomAnalyticsFact::AppMentioned(AppMentionedInput {
            tracking: tracking.clone(),
            mentions: vec![AppInvocation {
                connector_id: Some("calendar".to_string()),
                app_name: Some("Calendar".to_string()),
                invocation_type: Some(InvocationType::Explicit),
            }],
        }),
        CustomAnalyticsFact::AppUsed(AppUsedInput {
            tracking: tracking.clone(),
            app: AppInvocation {
                connector_id: Some("drive".to_string()),
                app_name: Some("Drive".to_string()),
                invocation_type: Some(InvocationType::Implicit),
            },
            elicitation_type: Some(ElicitationType::AuthOrLink),
        }),
        CustomAnalyticsFact::PluginUsed(PluginUsedInput {
            tracking,
            plugin: sample_plugin_metadata(),
        }),
    ] {
        reducer
            .ingest(AnalyticsFact::Custom(fact), &mut events)
            .await;
    }

    let [
        TrackEventRequest::AppMentioned(mention),
        TrackEventRequest::AppUsed(usage),
        TrackEventRequest::PluginUsed(plugin),
    ] = events.as_slice()
    else {
        panic!("expected app mention, app usage, and plugin usage in arrival order");
    };
    assert_eq!(
        [
            (
                mention.event_type,
                mention.event_params.product_client_id.as_deref()
            ),
            (
                usage.event_type,
                usage.event_params.app.product_client_id.as_deref()
            ),
            (
                plugin.event_type,
                plugin.event_params.plugin.product_client_id.as_deref()
            ),
        ],
        [
            ("codex_app_mentioned", Some(TEST_PRODUCT_CLIENT_ID)),
            ("codex_app_used", Some(TEST_PRODUCT_CLIENT_ID)),
            ("codex_plugin_used", Some(TEST_PRODUCT_CLIENT_ID)),
        ]
    );
    assert_eq!(
        usage.event_params.elicitation_type,
        Some(ElicitationType::AuthOrLink)
    );
}

#[tokio::test]
async fn unrelated_client_requests_are_ignored_by_reducer() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                request: Box::new(ClientRequest::ThreadArchive {
                    request_id: RequestId::Integer(3),
                    params: ThreadArchiveParams {
                        thread_id: "thread-2".to_string(),
                    },
                }),
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                response: Box::new(sample_turn_start_response("turn-2")),
                thread_originator: None,
            },
            &mut events,
        )
        .await;

    assert!(
        events.is_empty(),
        "unrelated requests must not create pending turn state"
    );
}

#[tokio::test]
async fn unrelated_client_responses_are_ignored_by_reducer() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_initialize(&mut reducer, &mut events).await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(9),
                response: Box::new(ClientResponsePayload::ThreadArchive(
                    ThreadArchiveResponse {},
                )),
                thread_originator: None,
            },
            &mut events,
        )
        .await;

    assert!(events.is_empty());
}

#[tokio::test]
async fn turn_and_tool_events_read_current_trusted_root() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    ingest_review_prerequisites(&mut reducer, &mut events).await;
    let turn_metadata = test_turn_metadata(Some("root-ancestor"));
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                TurnResolvedConfigFact {
                    turn_metadata: turn_metadata.clone(),
                    ..sample_turn_resolved_config("thread-1", "turn-1")
                },
            ))),
            &mut events,
        )
        .await;

    ingest_completed_command_execution_item(
        &mut reducer,
        &mut events,
        "thread-1",
        "before-conflict",
    )
    .await;
    let queued_control_call = ControlToolCallFact {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        turn_metadata: turn_metadata.clone(),
        call_id: "queued-control".to_string(),
        cell_id: None,
        tool_name: "view_image".to_string(),
        started_at_ms: 998,
        completed_at_ms: 1_000,
        status: ControlToolCallStatus::Completed,
    };
    let queued_code_mode_call = CodeModeToolCallFact::Completed {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        turn_metadata: turn_metadata.clone(),
        call_id: "queued-code-mode".to_string(),
        cell_id: None,
        tool_name: "exec".to_string(),
        started_at_ms: 998,
        completed_at_ms: 1_000,
        status: CodeModeToolCallStatus::Completed,
    };
    *turn_metadata.root_turn_id.lock().expect("root turn ID") = None;
    ingest_completed_command_execution_item(
        &mut reducer,
        &mut events,
        "thread-1",
        "after-conflict",
    )
    .await;
    for fact in [
        AnalyticsFact::Custom(CustomAnalyticsFact::ControlToolCall(queued_control_call)),
        AnalyticsFact::Custom(CustomAnalyticsFact::CodeModeToolCall(queued_code_mode_call)),
        AnalyticsFact::Custom(CustomAnalyticsFact::TurnProfile(Box::new(
            TurnProfileFact {
                turn_id: "turn-1".to_string(),
                profile: sample_turn_profile(),
            },
        ))),
        AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
            "thread-1",
            "turn-1",
            AppServerTurnStatus::Completed,
            /*codex_error_info*/ None,
        ))),
    ] {
        reducer.ingest(fact, &mut events).await;
    }

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        payload
            .as_array()
            .expect("events array")
            .iter()
            .map(|event| json!({
                "event_type": event["event_type"],
                "turn_id": event["event_params"]["turn_id"],
                "item_id": event["event_params"].get("item_id"),
                "root_turn_id": event["event_params"].get("root_turn_id").expect("root field"),
            }))
            .collect::<Vec<_>>(),
        vec![
            json!({"event_type": "codex_command_execution_event", "turn_id": "turn-1", "item_id": "before-conflict", "root_turn_id": "root-ancestor"}),
            json!({"event_type": "codex_command_execution_event", "turn_id": "turn-1", "item_id": "after-conflict", "root_turn_id": null}),
            json!({"event_type": "codex_control_tool_call_event", "turn_id": "turn-1", "item_id": "queued-control", "root_turn_id": null}),
            json!({"event_type": "codex_dynamic_tool_call_event", "turn_id": "turn-1", "item_id": "queued-code-mode", "root_turn_id": null}),
            json!({"event_type": "codex_turn_event", "turn_id": "turn-1", "item_id": null, "root_turn_id": null}),
        ]
    );
}

#[tokio::test]
async fn completed_background_tool_item_emits_after_turn_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();
    let turn_metadata = test_turn_metadata(Some("root-background"));

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ false,
        /*include_started*/ true,
        /*include_token_usage*/ false,
    )
    .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                TurnResolvedConfigFact {
                    turn_metadata: turn_metadata.clone(),
                    ..sample_turn_resolved_config("thread-2", "turn-2")
                },
            ))),
            &mut out,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ItemStarted(
                ItemStartedNotification {
                    thread_id: "thread-2".to_string(),
                    turn_id: "turn-2".to_string(),
                    started_at_ms: 998,
                    item: sample_command_execution_item(
                        CommandExecutionStatus::InProgress,
                        /*exit_code*/ None,
                        /*duration_ms*/ None,
                    ),
                },
            ))),
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                /*codex_error_info*/ None,
            ))),
            &mut out,
        )
        .await;

    assert_eq!(
        out.iter()
            .filter(|event| matches!(event, TrackEventRequest::TurnEvent(_)))
            .count(),
        1
    );
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ItemCompleted(
                ItemCompletedNotification {
                    thread_id: "thread-2".to_string(),
                    turn_id: "turn-2".to_string(),
                    completed_at_ms: 1_000,
                    item: sample_command_execution_item(
                        CommandExecutionStatus::Completed,
                        Some(0),
                        Some(1),
                    ),
                },
            ))),
            &mut out,
        )
        .await;

    assert_eq!(
        out.iter()
            .filter(|event| matches!(event, TrackEventRequest::TurnEvent(_)))
            .count(),
        1
    );
    assert!(
        out.iter()
            .any(|event| matches!(event, TrackEventRequest::CommandExecution(_)))
    );
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::ControlToolCall(ControlToolCallFact {
                thread_id: "thread-2".to_string(),
                turn_id: "turn-2".to_string(),
                turn_metadata: turn_metadata.clone(),
                call_id: "late-view-image".to_string(),
                cell_id: None,
                tool_name: "view_image".to_string(),
                started_at_ms: 998,
                completed_at_ms: 1_001,
                status: ControlToolCallStatus::Completed,
            })),
            &mut out,
        )
        .await;
    ingest_code_mode_facts(
        &mut reducer,
        &mut out,
        [CodeModeToolCallFact::Completed {
            thread_id: "thread-2".to_string(),
            turn_id: "turn-2".to_string(),
            turn_metadata,
            call_id: "late-code-mode".to_string(),
            cell_id: None,
            tool_name: "exec".to_string(),
            started_at_ms: 998,
            completed_at_ms: 1_001,
            status: CodeModeToolCallStatus::Interrupted,
        }],
    )
    .await;
    reducer.flush(&mut out);

    assert_eq!(
        out.iter()
            .map(|event| {
                let event = serde_json::to_value(event).expect("serialize event");
                json!({
                    "event_type": event["event_type"],
                    "turn_id": event["event_params"]["turn_id"],
                    "root_turn_id": event["event_params"]["root_turn_id"],
                })
            })
            .collect::<Vec<_>>(),
        vec![
            json!({"event_type": "codex_turn_event", "turn_id": "turn-2", "root_turn_id": "root-background"}),
            json!({"event_type": "codex_command_execution_event", "turn_id": "turn-2", "root_turn_id": "root-background"}),
            json!({"event_type": "codex_control_tool_call_event", "turn_id": "turn-2", "root_turn_id": "root-background"}),
            json!({"event_type": "codex_dynamic_tool_call_event", "turn_id": "turn-2", "root_turn_id": "root-background"}),
        ]
    );
}

#[tokio::test]
async fn item_completed_without_turn_state_does_not_create_turn_state() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ItemCompleted(
                ItemCompletedNotification {
                    thread_id: "thread-2".to_string(),
                    turn_id: "turn-2".to_string(),
                    completed_at_ms: 1_000,
                    item: sample_command_execution_item(
                        CommandExecutionStatus::Completed,
                        Some(0),
                        Some(1),
                    ),
                },
            ))),
            &mut out,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                /*codex_error_info*/ None,
            ))),
            &mut out,
        )
        .await;

    assert!(out.is_empty());
}

#[tokio::test]
async fn voice_handoff_attributes_plugin_events_after_realtime_closes() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    ingest_turn_prerequisites(
        &mut reducer,
        &mut events,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ false,
        /*include_token_usage*/ true,
    )
    .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ThreadRealtimeStarted(
                ThreadRealtimeStartedNotification {
                    thread_id: "thread-2".to_string(),
                    realtime_session_id: Some("work-voice-123".to_string()),
                    version: RealtimeConversationVersion::V2,
                },
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::RealtimeHandoffRequested {
                thread_id: "thread-2".to_string(),
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ThreadRealtimeClosed(
                ThreadRealtimeClosedNotification {
                    thread_id: "thread-2".to_string(),
                    reason: None,
                },
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                "thread-2", "turn-2",
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(AppUsedInput {
                tracking: test_tracking_context("thread-2", "turn-2"),
                app: AppInvocation {
                    connector_id: Some("drive".to_string()),
                    app_name: Some("Drive".to_string()),
                    invocation_type: Some(InvocationType::Implicit),
                },
                elicitation_type: None,
            })),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::SkillInvoked(SkillInvokedInput {
                tracking: test_tracking_context("thread-2", "turn-2"),
                invocations: vec![SkillInvocation {
                    skill_name: "sample:doc".to_string(),
                    location: SkillInvocationLocation::Resource {
                        id: "resource-1".to_string(),
                        skill_id: Some("sample:doc".to_string()),
                        scope: None,
                    },
                    plugin_id: Some("sample@test".to_string()),
                    remote_plugin_id: None,
                    invocation_type: InvocationType::Explicit,
                }],
            })),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                /*codex_error_info*/ None,
            ))),
            &mut events,
        )
        .await;

    for event_type in ["codex_app_used", "skill_invocation", "codex_turn_event"] {
        let event = events
            .iter()
            .map(|event| serde_json::to_value(event).expect("serialize analytics event"))
            .find(|event| event["event_type"] == event_type)
            .unwrap_or_else(|| panic!("missing {event_type}"));
        assert_eq!(event["event_params"]["voice_session_id"], "work-voice-123");
    }
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                "thread-2", "turn-3",
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(AppUsedInput {
                tracking: test_tracking_context("thread-2", "turn-3"),
                app: AppInvocation {
                    connector_id: Some("drive".to_string()),
                    app_name: Some("Drive".to_string()),
                    invocation_type: Some(InvocationType::Implicit),
                },
                elicitation_type: None,
            })),
            &mut events,
        )
        .await;
    let text_app = serde_json::to_value(events.last().expect("text app event"))
        .expect("serialize text app event");
    assert_eq!(text_app["event_params"]["voice_session_id"], json!(null));
}

#[tokio::test]
async fn voice_handoff_steering_active_turn_does_not_tag_next_text_turn() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                "thread-2", "turn-2",
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ThreadRealtimeStarted(
                ThreadRealtimeStartedNotification {
                    thread_id: "thread-2".to_string(),
                    realtime_session_id: Some("work-voice-123".to_string()),
                    version: RealtimeConversationVersion::V2,
                },
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::RealtimeHandoffRequested {
                thread_id: "thread-2".to_string(),
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ThreadRealtimeClosed(
                ThreadRealtimeClosedNotification {
                    thread_id: "thread-2".to_string(),
                    reason: None,
                },
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(AppUsedInput {
                tracking: test_tracking_context("thread-2", "turn-2"),
                app: AppInvocation {
                    connector_id: Some("drive".to_string()),
                    app_name: Some("Drive".to_string()),
                    invocation_type: Some(InvocationType::Implicit),
                },
                elicitation_type: None,
            })),
            &mut events,
        )
        .await;
    let voice_app = serde_json::to_value(events.last().expect("voice app event"))
        .expect("serialize voice app event");
    assert_eq!(
        voice_app["event_params"]["voice_session_id"],
        "work-voice-123"
    );

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                /*codex_error_info*/ None,
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                "thread-2", "turn-3",
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(AppUsedInput {
                tracking: test_tracking_context("thread-2", "turn-3"),
                app: AppInvocation {
                    connector_id: Some("drive".to_string()),
                    app_name: Some("Drive".to_string()),
                    invocation_type: Some(InvocationType::Implicit),
                },
                elicitation_type: None,
            })),
            &mut events,
        )
        .await;
    let text_app = serde_json::to_value(events.last().expect("text app event"))
        .expect("serialize text app event");
    assert_eq!(text_app["event_params"]["voice_session_id"], json!(null));
}
