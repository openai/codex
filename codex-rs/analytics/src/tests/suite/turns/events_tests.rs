//! Turn lifecycle, prerequisites, counts, image preparation, and event payload tests.

use crate::events::CodexTurnEventRequest;
use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::ControlToolCallFact;
use crate::facts::ControlToolCallStatus;
use crate::facts::CustomAnalyticsFact;
use crate::facts::ImageDetailSetting;
use crate::facts::ImagePreparationFact;
use crate::facts::ImagePreparationMetadata;
use crate::facts::ThreadInitializationMode;
use crate::facts::TurnCodexErrorFact;
use crate::facts::TurnResolvedConfigFact;
use crate::facts::TurnStatus;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::ingest_turn_prerequisites;
use crate::tests::support::sample_app_server_client_metadata;
use crate::tests::support::sample_command_execution_item;
use crate::tests::support::sample_runtime_metadata;
use crate::tests::support::sample_turn_completed_notification;
use crate::tests::support::sample_turn_resolved_config;
use crate::tests::support::test_turn_metadata;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::ImageGenerationItem;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::McpToolCallAppContext;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SubAgentActivityKind;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadRealtimeStartedNotification;
use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
use codex_app_server_protocol::WebSearchItem;
use codex_protocol::error::CodexErr;
use codex_protocol::error::UsageLimitReachedError;
use codex_protocol::protocol::RealtimeConversationVersion;
use codex_protocol::protocol::ThreadSource;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn image_preparation_fact_is_included_in_turn_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    ingest_turn_prerequisites(
        &mut reducer,
        &mut events,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ true,
        /*include_token_usage*/ false,
    )
    .await;

    let metadata = ImagePreparationMetadata {
        message_role: None,
        item_id: Some("call-1".to_string()),
        effective_detail: ImageDetailSetting::High,
        source_width: 2_048,
        source_height: 2_048,
        prepared_width: 1_600,
        prepared_height: 1_600,
    };
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::ImagePreparation(Box::new(
                ImagePreparationFact {
                    turn_id: "turn-2".to_string(),
                    metadata: metadata.clone(),
                },
            ))),
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

    let [TrackEventRequest::TurnEvent(event)] = events.as_slice() else {
        panic!("expected one turn event");
    };
    assert_eq!(event.event_params.image_preparations, vec![metadata]);
}

#[test]
fn turn_event_serializes_expected_shape() {
    let event = TrackEventRequest::TurnEvent(Box::new(CodexTurnEventRequest {
        event_type: "codex_turn_event",
        event_params: crate::events::CodexTurnEventParams {
            thread_id: "thread-2".to_string(),
            session_id: "session-thread-2".to_string(),
            turn_id: "turn-2".to_string(),
            active_plugin_ids_at_turn_start: Some(vec![
                "plugins~Plugin_example".to_string(),
                "test@marketplace".to_string(),
            ]),
            voice_session_id: None,
            root_turn_id: Some("turn-2".to_string()),
            turn_trigger: Some("user".to_string()),
            codex_turn_source: Some("composer".to_string()),
            app_server_client: sample_app_server_client_metadata(),
            runtime: sample_runtime_metadata(),
            submission_type: None,
            ephemeral: false,
            thread_source: Some(ThreadSource::User),
            initialization_mode: ThreadInitializationMode::New,
            subagent_source: None,
            parent_thread_id: None,
            model: Some("gpt-5".to_string()),
            model_provider: "openai".to_string(),
            sandbox_policy: Some("read_only"),
            reasoning_effort: Some("high".to_string()),
            reasoning_summary: Some("detailed".to_string()),
            service_tier: "flex".to_string(),
            approval_policy: "on-request".to_string(),
            approvals_reviewer: "auto_review".to_string(),
            guardian_v2_enabled: true,
            sandbox_network_access: true,
            collaboration_mode: Some("plan"),
            personality: Some("pragmatic".to_string()),
            workspace_kind: Some("projectless".to_string()),
            num_input_images: 2,
            image_preparations: vec![ImagePreparationMetadata {
                message_role: Some("user".to_string()),
                item_id: None,
                effective_detail: ImageDetailSetting::High,
                source_width: 2_048,
                source_height: 2_048,
                prepared_width: 1_600,
                prepared_height: 1_600,
            }],
            is_first_turn: true,
            status: Some(TurnStatus::Completed),
            explicit_client_interrupt_requested_at_ms: None,
            turn_error: None,
            codex_error_kind: None,
            codex_error_http_status_code: None,
            usage_limit_window_minutes: None,
            steer_count: Some(0),
            total_tool_call_count: None,
            shell_command_count: None,
            file_change_count: None,
            mcp_tool_call_count: None,
            dynamic_tool_call_count: None,
            subagent_tool_call_count: None,
            web_search_count: None,
            image_generation_count: None,
            input_tokens: None,
            cached_input_tokens: None,
            cache_write_input_tokens: None,
            output_tokens: None,
            reasoning_output_tokens: None,
            total_tokens: None,
            before_first_sampling_ms: 100,
            sampling_ms: 700,
            compaction_ms: 40,
            between_sampling_overhead_ms: 50,
            tool_blocking_ms: 250,
            after_last_sampling_ms: 94,
            sampling_request_count: 2,
            sampling_retry_count: 1,
            duration_ms: Some(1234),
            started_at: Some(455),
            completed_at: Some(456),
        },
    }));

    let payload = serde_json::to_value(&event).expect("serialize turn event");
    let expected = serde_json::from_str::<serde_json::Value>(
        r#"{
            "event_type": "codex_turn_event",
            "event_params": {
                "thread_id": "thread-2",
                "session_id": "session-thread-2",
                "turn_id": "turn-2",
                "active_plugin_ids_at_turn_start": ["plugins~Plugin_example", "test@marketplace"],
                "voice_session_id": null,
                "root_turn_id": "turn-2",
                "turn_trigger": "user",
                "codex_turn_source": "composer",
                "submission_type": null,
                "app_server_client": {
                    "product_client_id": "codex_cli_rs",
                    "client_name": "codex-tui",
                    "client_version": "1.0.0",
                    "rpc_transport": "stdio",
                    "experimental_api_enabled": true
                },
                "runtime": {
                    "codex_rs_version": "0.1.0",
                    "runtime_os": "macos",
                    "runtime_os_version": "15.3.1",
                    "runtime_arch": "aarch64"
                },
                "ephemeral": false,
                "thread_source": "user",
                "initialization_mode": "new",
                "subagent_source": null,
                "parent_thread_id": null,
                "model": "gpt-5",
                "model_provider": "openai",
                "sandbox_policy": "read_only",
                "reasoning_effort": "high",
                "reasoning_summary": "detailed",
                "service_tier": "flex",
                "approval_policy": "on-request",
                "approvals_reviewer": "auto_review",
                "guardian_v2_enabled": true,
                "sandbox_network_access": true,
                "collaboration_mode": "plan",
                "personality": "pragmatic",
                "workspace_kind": "projectless",
                "num_input_images": 2,
                "image_preparations": [{
                    "message_role": "user",
                    "item_id": null,
                    "effective_detail": "high",
                    "source_width": 2048,
                    "source_height": 2048,
                    "prepared_width": 1600,
                    "prepared_height": 1600
                }],
                "is_first_turn": true,
                "status": "completed",
                "explicit_client_interrupt_requested_at_ms": null,
                "turn_error": null,
                "codex_error_kind": null,
                "codex_error_http_status_code": null,
                "usage_limit_window_minutes": null,
                "steer_count": 0,
                "total_tool_call_count": null,
                "shell_command_count": null,
                "file_change_count": null,
                "mcp_tool_call_count": null,
                "dynamic_tool_call_count": null,
                "subagent_tool_call_count": null,
                "web_search_count": null,
                "image_generation_count": null,
                "input_tokens": null,
                "cached_input_tokens": null,
                "cache_write_input_tokens": null,
                "output_tokens": null,
                "reasoning_output_tokens": null,
                "total_tokens": null,
                "before_first_sampling_ms": 100,
                "sampling_ms": 700,
                "compaction_ms": 40,
                "between_sampling_overhead_ms": 50,
                "tool_blocking_ms": 250,
                "after_last_sampling_ms": 94,
                "sampling_request_count": 2,
                "sampling_retry_count": 1,
                "duration_ms": 1234,
                "started_at": 455,
                "completed_at": 456
            }
        }"#,
    )
    .expect("parse expected turn event");

    assert_eq!(payload, expected);
}

#[tokio::test]
async fn turn_event_preserves_first_received_plugin_inventory() {
    for plugin_ids in [
        None,
        Some(vec![]),
        Some(vec!["initial@marketplace".to_string()]),
    ] {
        let mut reducer = AnalyticsReducer::default();
        let mut out = Vec::new();
        ingest_turn_prerequisites(
            &mut reducer,
            &mut out,
            /*include_initialize*/ true,
            /*include_resolved_config*/ false,
            /*include_started*/ true,
            /*include_token_usage*/ false,
        )
        .await;

        let config = TurnResolvedConfigFact {
            active_plugin_ids_at_turn_start: plugin_ids.clone(),
            ..sample_turn_resolved_config("thread-2", "turn-2")
        };
        let later_config = TurnResolvedConfigFact {
            active_plugin_ids_at_turn_start: Some(vec!["later@marketplace".to_string()]),
            is_first_turn: false,
            ..config.clone()
        };
        for fact in [
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(config))),
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                later_config,
            ))),
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                /*codex_error_info*/ None,
            ))),
        ] {
            reducer.ingest(fact, &mut out).await;
        }

        let [TrackEventRequest::TurnEvent(event)] = out.as_slice() else {
            panic!("expected one turn event");
        };
        assert_eq!(
            (
                &event.event_params.active_plugin_ids_at_turn_start,
                event.event_params.is_first_turn,
            ),
            (&plugin_ids, false),
        );
    }
}

#[tokio::test]
async fn turn_lifecycle_emits_turn_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ true,
        /*include_token_usage*/ true,
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

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(payload["event_type"], json!("codex_turn_event"));
    assert_eq!(payload["event_params"]["thread_id"], json!("thread-2"));
    assert_eq!(
        payload["event_params"]["session_id"],
        json!("session-thread-2")
    );
    assert_eq!(payload["event_params"]["turn_id"], json!("turn-2"));
    assert_eq!(
        (
            payload["event_params"].get("turn_trigger"),
            payload["event_params"].get("codex_turn_source"),
        ),
        (
            Some(&serde_json::Value::Null),
            Some(&serde_json::Value::Null)
        )
    );
    assert_eq!(
        payload["event_params"]["app_server_client"],
        json!({
            "product_client_id": "codex-tui",
            "client_name": "codex-tui",
            "client_version": "1.0.0",
            "rpc_transport": "stdio",
            "experimental_api_enabled": null,
        })
    );
    assert_eq!(
        payload["event_params"]["runtime"],
        json!({
            "codex_rs_version": "0.1.0",
            "runtime_os": "macos",
            "runtime_os_version": "15.3.1",
            "runtime_arch": "aarch64",
        })
    );
    assert!(payload["event_params"].get("product_client_id").is_none());
    assert_eq!(payload["event_params"]["guardian_v2_enabled"], json!(false));
    assert_eq!(payload["event_params"]["ephemeral"], json!(false));
    assert_eq!(payload["event_params"]["workspace_kind"], json!(null));
    assert_eq!(payload["event_params"]["num_input_images"], json!(1));
    assert_eq!(payload["event_params"]["status"], json!("completed"));
    assert_eq!(payload["event_params"]["steer_count"], json!(0));
    assert_eq!(payload["event_params"]["total_tool_call_count"], json!(0));
    assert_eq!(payload["event_params"]["shell_command_count"], json!(0));
    assert_eq!(payload["event_params"]["file_change_count"], json!(0));
    assert_eq!(payload["event_params"]["mcp_tool_call_count"], json!(0));
    assert_eq!(payload["event_params"]["dynamic_tool_call_count"], json!(0));
    assert_eq!(
        payload["event_params"]["subagent_tool_call_count"],
        json!(0)
    );
    assert_eq!(payload["event_params"]["web_search_count"], json!(0));
    assert_eq!(payload["event_params"]["image_generation_count"], json!(0));
    assert_eq!(payload["event_params"]["started_at"], json!(455));
    assert_eq!(payload["event_params"]["completed_at"], json!(456));
    assert_eq!(payload["event_params"]["duration_ms"], json!(1234));
    assert_eq!(payload["event_params"]["input_tokens"], json!(123));
    assert_eq!(payload["event_params"]["cached_input_tokens"], json!(45));
    assert_eq!(
        payload["event_params"]["cache_write_input_tokens"],
        json!(7)
    );
    assert_eq!(payload["event_params"]["output_tokens"], json!(140));
    assert_eq!(
        payload["event_params"]["reasoning_output_tokens"],
        json!(13)
    );
    assert_eq!(payload["event_params"]["total_tokens"], json!(321));
}

#[tokio::test]
async fn turn_event_counts_completed_tool_items() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

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
                    turn_metadata: test_turn_metadata(Some("root-ancestor")),
                    ..sample_turn_resolved_config("thread-2", "turn-2")
                },
            ))),
            &mut out,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ThreadRealtimeStarted(
                ThreadRealtimeStartedNotification {
                    thread_id: "thread-2".to_string(),
                    realtime_session_id: Some("work-voice-456".to_string()),
                    version: RealtimeConversationVersion::V2,
                },
            ))),
            &mut out,
        )
        .await;

    let mcp_tool_call_item = |status, duration_ms| ThreadItem::McpToolCall {
        id: "mcp-1".to_string(),
        server: "server".to_string(),
        tool: "search".to_string(),
        status,
        arguments: json!({}),
        app_context: Some(McpToolCallAppContext {
            connector_id: "connector-test".to_string(),
            link_id: None,
            resource_uri: None,
            app_name: None,
            action_name: None,
        }),
        mcp_app_resource_uri: None,
        mcp_app_ui: None,
        plugin_id: Some("sample@test".to_string()),
        read_only_hint: None,
        result: None,
        error: None,
        duration_ms,
    };
    let completed_tool_items = vec![
        sample_command_execution_item(CommandExecutionStatus::Completed, Some(0), Some(1)),
        ThreadItem::FileChange {
            id: "file-change-1".to_string(),
            changes: Vec::new(),
            status: PatchApplyStatus::Completed,
        },
        mcp_tool_call_item(McpToolCallStatus::Completed, Some(2)),
        ThreadItem::DynamicToolCall {
            id: "dynamic-1".to_string(),
            // A name collision with code-mode exec must not change the event boundary.
            namespace: Some("custom".to_string()),
            tool: "exec".to_string(),
            arguments: json!({}),
            status: DynamicToolCallStatus::Completed,
            content_items: None,
            success: Some(true),
            duration_ms: Some(3),
        },
        ThreadItem::CollabAgentToolCall {
            id: "collab-1".to_string(),
            tool: CollabAgentTool::SpawnAgent,
            status: CollabAgentToolCallStatus::Completed,
            sender_thread_id: "thread-2".to_string(),
            receiver_thread_ids: vec!["thread-child".to_string()],
            prompt: Some("help".to_string()),
            model: Some("gpt-5".to_string()),
            reasoning_effort: None,
            agents_states: Default::default(),
        },
        ThreadItem::SubAgentActivity {
            id: "sub-agent-activity-1".to_string(),
            kind: SubAgentActivityKind::Interacted,
            agent_thread_id: "thread-child".to_string(),
            agent_path: "/root/child".to_string(),
        },
        ThreadItem::SubAgentActivity {
            id: "sub-agent-activity-completed".to_string(),
            kind: SubAgentActivityKind::Completed,
            agent_thread_id: "thread-child".to_string(),
            agent_path: "/root/child".to_string(),
        },
        ThreadItem::WebSearch(WebSearchItem {
            id: "web-1".to_string(),
            query: "codex".to_string(),
            action: None,
            results: None,
        }),
        ThreadItem::ImageGeneration(ImageGenerationItem {
            id: "image-1".to_string(),
            status: "completed".to_string(),
            revised_prompt: None,
            result: "ok".to_string(),
            transparent_background: None,
            failure: None,
            saved_path: None,
            imagegen_request_id: Some("req-imagegen-123".to_string()),
            generation_id: Some("gen-image-123".to_string()),
        }),
    ];

    for item in &completed_tool_items {
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(ServerNotification::ItemStarted(
                    ItemStartedNotification {
                        thread_id: "thread-2".to_string(),
                        turn_id: "turn-2".to_string(),
                        started_at_ms: 998,
                        item: item.clone(),
                    },
                ))),
                &mut out,
            )
            .await;
    }

    for item in completed_tool_items {
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: "thread-2".to_string(),
                        turn_id: "turn-2".to_string(),
                        completed_at_ms: 1_000,
                        item,
                    },
                ))),
                &mut out,
            )
            .await;
    }

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::ControlToolCall(ControlToolCallFact {
                thread_id: "thread-2".to_string(),
                turn_id: "turn-2".to_string(),
                turn_metadata: test_turn_metadata(Some("root-ancestor")),
                call_id: "control-1".to_string(),
                cell_id: None,
                tool_name: "update_plan".to_string(),
                started_at_ms: 998,
                completed_at_ms: 1_000,
                status: ControlToolCallStatus::Completed,
            })),
            &mut out,
        )
        .await;
    reducer.flush(&mut out);

    let payload = serde_json::to_value(&out).expect("serialize tool item events");
    let emitted_tool_events = payload
        .as_array()
        .expect("tool item events array")
        .iter()
        .map(|event| {
            (
                event["event_type"].as_str().expect("tool item event type"),
                event["event_params"]["session_id"]
                    .as_str()
                    .expect("tool item event session ID"),
                event["event_params"]["turn_id"]
                    .as_str()
                    .expect("tool item event turn ID"),
                event["event_params"]["root_turn_id"]
                    .as_str()
                    .expect("tool item event root turn ID"),
                event["event_params"]["tool_event_type"].as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        emitted_tool_events,
        [
            ("codex_command_execution_event", None),
            ("codex_file_change_event", None),
            ("codex_mcp_tool_call_event", None),
            ("codex_dynamic_tool_call_event", None),
            ("codex_web_search_event", None),
            ("codex_image_generation_event", None),
            ("codex_collab_agent_tool_call_event", None),
            ("codex_control_tool_call_event", None),
        ]
        .map(|(event_type, tool_event_type)| {
            (
                event_type,
                "session-thread-2",
                "turn-2",
                "root-ancestor",
                tool_event_type,
            )
        })
        .to_vec()
    );

    let image_generation_event = out
        .iter()
        .find(|event| matches!(event, TrackEventRequest::ImageGeneration(_)))
        .expect("image generation event should be emitted");
    let payload =
        serde_json::to_value(image_generation_event).expect("serialize image generation event");
    assert_eq!(
        payload["event_params"]["imagegen_request_id"],
        json!("req-imagegen-123")
    );
    assert_eq!(
        payload["event_params"]["generation_id"],
        json!("gen-image-123")
    );

    let mcp_tool_call_event = out
        .iter()
        .find(|event| matches!(event, TrackEventRequest::McpToolCall(_)))
        .expect("MCP tool call event should be emitted");
    let payload = serde_json::to_value(mcp_tool_call_event).expect("serialize MCP tool call event");
    assert_eq!(payload["event_params"]["plugin_id"], json!("sample@test"));
    assert_eq!(
        payload["event_params"]["voice_session_id"],
        "work-voice-456"
    );
    assert_eq!(
        payload["event_params"]["connector_id"],
        json!("connector-test")
    );

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

    let turn_event = out
        .iter()
        .find(|event| matches!(event, TrackEventRequest::TurnEvent(_)))
        .expect("turn event should be emitted");
    let payload = serde_json::to_value(turn_event).expect("serialize turn event");
    assert_eq!(payload["event_params"]["root_turn_id"], "root-ancestor");
    assert_eq!(
        payload["event_params"]["voice_session_id"],
        "work-voice-456"
    );
    assert_eq!(payload["event_params"]["total_tool_call_count"], json!(9));
    assert_eq!(payload["event_params"]["shell_command_count"], json!(1));
    assert_eq!(payload["event_params"]["file_change_count"], json!(1));
    assert_eq!(payload["event_params"]["mcp_tool_call_count"], json!(1));
    assert_eq!(payload["event_params"]["dynamic_tool_call_count"], json!(1));
    assert_eq!(
        payload["event_params"]["subagent_tool_call_count"],
        json!(2)
    );
    assert_eq!(payload["event_params"]["web_search_count"], json!(1));
    assert_eq!(payload["event_params"]["image_generation_count"], json!(1));
}

#[tokio::test]
async fn turn_does_not_emit_without_required_prerequisites() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ false,
        /*include_resolved_config*/ true,
        /*include_started*/ false,
        /*include_token_usage*/ false,
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

    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ false,
        /*include_started*/ false,
        /*include_token_usage*/ false,
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
async fn turn_lifecycle_emits_failed_turn_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ true,
        /*include_token_usage*/ false,
    )
    .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnCodexError(Box::new(
                TurnCodexErrorFact::from_codex_err(
                    "thread-2".to_string(),
                    "turn-2".to_string(),
                    &CodexErr::InvalidRequest("unknown turn environment id `env-2`".to_string()),
                ),
            ))),
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Failed,
                Some(codex_app_server_protocol::CodexErrorInfo::BadRequest),
            ))),
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(payload["event_params"]["status"], json!("failed"));
    assert_eq!(payload["event_params"]["turn_error"], json!("badRequest"));
    assert_eq!(
        payload["event_params"]["codex_error_kind"],
        json!("invalid_request")
    );
    assert_eq!(
        payload["event_params"]["codex_error_http_status_code"],
        json!(null)
    );
    assert_eq!(
        payload["event_params"]["usage_limit_window_minutes"],
        json!(null)
    );
}

#[tokio::test]
async fn turn_event_reports_decisive_usage_limit_window() {
    let usage_limit = |limit_window_minutes| {
        CodexErr::UsageLimitReached(UsageLimitReachedError {
            plan_type: None,
            resets_at: None,
            rate_limits: None,
            promo_message: None,
            rate_limit_reached_type: None,
            limit_window_minutes,
        })
    };
    let cases = [
        (
            usage_limit(Some(300)),
            codex_app_server_protocol::CodexErrorInfo::UsageLimitExceeded,
            json!(["usageLimitExceeded", "usage_limit_reached", 300]),
        ),
        (
            usage_limit(Some(10_080)),
            codex_app_server_protocol::CodexErrorInfo::UsageLimitExceeded,
            json!(["usageLimitExceeded", "usage_limit_reached", 10080]),
        ),
        (
            usage_limit(None),
            codex_app_server_protocol::CodexErrorInfo::UsageLimitExceeded,
            json!(["usageLimitExceeded", "usage_limit_reached", null]),
        ),
        (
            CodexErr::InvalidRequest("invalid request".to_string()),
            codex_app_server_protocol::CodexErrorInfo::BadRequest,
            json!(["badRequest", "invalid_request", null]),
        ),
    ];

    for (error, error_info, expected) in cases {
        let mut reducer = AnalyticsReducer::default();
        let mut out = Vec::new();
        ingest_turn_prerequisites(
            &mut reducer,
            &mut out,
            /*include_initialize*/ true,
            /*include_resolved_config*/ true,
            /*include_started*/ true,
            /*include_token_usage*/ false,
        )
        .await;
        reducer
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::TurnCodexError(Box::new(
                    TurnCodexErrorFact::from_codex_err(
                        "thread-2".to_string(),
                        "turn-2".to_string(),
                        &error,
                    ),
                ))),
                &mut out,
            )
            .await;
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                    "thread-2",
                    "turn-2",
                    AppServerTurnStatus::Failed,
                    Some(error_info),
                ))),
                &mut out,
            )
            .await;

        let [event] = out.as_slice() else {
            panic!("expected one turn event");
        };
        let payload = serde_json::to_value(event).expect("serialize turn event");
        let params = &payload["event_params"];
        assert_eq!(
            json!([
                params["turn_error"],
                params["codex_error_kind"],
                params["usage_limit_window_minutes"],
            ]),
            expected
        );
    }
}

#[tokio::test]
async fn turn_completed_without_started_notification_emits_null_started_at() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(
        &mut reducer,
        &mut out,
        /*include_initialize*/ true,
        /*include_resolved_config*/ true,
        /*include_started*/ false,
        /*include_token_usage*/ false,
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

    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(payload["event_params"]["started_at"], json!(null));
    assert_eq!(payload["event_params"]["duration_ms"], json!(1234));
    assert_eq!(payload["event_params"]["input_tokens"], json!(null));
    assert_eq!(payload["event_params"]["cached_input_tokens"], json!(null));
    assert_eq!(payload["event_params"]["output_tokens"], json!(null));
    assert_eq!(
        payload["event_params"]["reasoning_output_tokens"],
        json!(null)
    );
    assert_eq!(payload["event_params"]["total_tokens"], json!(null));
}
