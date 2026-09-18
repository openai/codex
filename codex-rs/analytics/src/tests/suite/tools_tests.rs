//! Tool item, Code Mode correlation, and artifact event tests.

use crate::events::AppServerRpcTransport;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexCommandExecutionEventParams;
use crate::events::CodexCommandExecutionEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::CodexToolItemEventBase;
use crate::events::FinalApprovalOutcome;
use crate::events::ToolEventType;
use crate::events::ToolItemTerminalStatus;
use crate::events::TrackEventRequest;
use crate::events::current_runtime_metadata;
use crate::facts::AnalyticsFact;
use crate::facts::ArtifactOperation;
use crate::facts::ArtifactOperationInput;
use crate::facts::ArtifactOperationLifecycle;
use crate::facts::CodeModeToolCallFact;
use crate::facts::CodeModeToolCallStatus;
use crate::facts::ControlToolCallFact;
use crate::facts::ControlToolCallStatus;
use crate::facts::CustomAnalyticsFact;
use crate::facts::ElicitationType;
use crate::facts::McpToolCallElicitation;
use crate::facts::TurnResolvedConfigFact;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::TEST_PRODUCT_CLIENT_ID;
use crate::tests::support::ingest_code_mode_facts;
use crate::tests::support::ingest_completed_command_execution_item;
use crate::tests::support::ingest_review_prerequisites;
use crate::tests::support::ingest_turn_prerequisites;
use crate::tests::support::sample_command_execution_item;
use crate::tests::support::sample_turn_completed_notification;
use crate::tests::support::sample_turn_resolved_config;
use crate::tests::support::sample_turn_started_notification;
use crate::tests::support::test_tracking_context;
use crate::tests::support::test_turn_metadata;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandAction;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ImageGenerationItem;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::McpToolCallAppContext;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
use codex_protocol::protocol::ThreadSource;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use serde_json::json;

fn sample_command_execution_item_with_actions(
    status: CommandExecutionStatus,
    exit_code: Option<i32>,
    duration_ms: Option<i64>,
    command_actions: Vec<CommandAction>,
    plugin_id: Option<&str>,
    script_path: Option<&str>,
) -> ThreadItem {
    let mut item = sample_command_execution_item(status, exit_code, duration_ms);
    let ThreadItem::CommandExecution {
        command_actions: item_command_actions,
        plugin_id: item_plugin_id,
        script_path: item_script_path,
        ..
    } = &mut item
    else {
        unreachable!("sample command execution item should be CommandExecution");
    };
    *item_command_actions = command_actions;
    *item_plugin_id = plugin_id.map(str::to_string);
    *item_script_path = script_path.map(str::to_string);
    item
}

fn sampling_response(
    turn_id: &str,
    response_id: &str,
    tool_call_ids: &[&str],
) -> CodeModeToolCallFact {
    CodeModeToolCallFact::SamplingResponseCompleted {
        thread_id: "thread-1".into(),
        turn_id: turn_id.into(),
        response_id: response_id.into(),
        tool_call_ids: tool_call_ids.iter().map(|id| (*id).into()).collect(),
    }
}

#[test]
fn command_execution_event_serializes_expected_shape() {
    let event = TrackEventRequest::CommandExecution(CodexCommandExecutionEventRequest {
        event_type: "codex_command_execution_event",
        event_params: CodexCommandExecutionEventParams {
            model_slug: None,
            reasoning_effort: None,
            base: CodexToolItemEventBase {
                thread_id: "thread-1".to_string(),
                session_id: "session-thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                root_turn_id: Some("root-turn".to_string()),
                item_id: "item-1".to_string(),
                cell_id: None,
                parent_call_id: None,
                originating_response_id: None,
                subsequent_response_id: None,
                app_server_client: CodexAppServerClientMetadata {
                    product_client_id: "codex_tui".to_string(),
                    client_name: Some("codex-tui".to_string()),
                    client_version: Some("1.2.3".to_string()),
                    rpc_transport: AppServerRpcTransport::Websocket,
                    experimental_api_enabled: Some(true),
                },
                runtime: CodexRuntimeMetadata {
                    codex_rs_version: "0.99.0".to_string(),
                    runtime_os: "macos".to_string(),
                    runtime_os_version: "15.3.1".to_string(),
                    runtime_arch: "aarch64".to_string(),
                },
                thread_source: Some(ThreadSource::User),
                subagent_source: None,
                parent_thread_id: None,
                tool_name: "shell".to_string(),
                tool_event_type: Some(ToolEventType::ModelToolCall),
                started_at_ms: 123_000,
                completed_at_ms: 125_000,
                duration_ms: Some(2000),
                execution_duration_ms: Some(1900),
                review_count: 0,
                guardian_review_count: 0,
                user_review_count: 0,
                final_approval_outcome: FinalApprovalOutcome::NotNeeded,
                terminal_status: ToolItemTerminalStatus::Completed,
                failure_kind: None,
                requested_additional_permissions: false,
                requested_network_access: false,
            },
            plugin_id: Some("sample@openai-curated".to_string()),
            script_path: Some("scripts/run.py".to_string()),
            command_execution_source: CommandExecutionSource::Agent,
            exit_code: Some(0),
            command_total_action_count: 4,
            command_read_action_count: 1,
            command_list_files_action_count: 1,
            command_search_action_count: 1,
            command_unknown_action_count: 1,
        },
    });

    let payload = serde_json::to_value(&event).expect("serialize command execution event");
    let mut expected = json!({
        "event_type": "codex_command_execution_event",
        "event_params": {
            "model_slug": null,
            "reasoning_effort": null,
            "thread_id": "thread-1",
            "session_id": "session-thread-1",
            "turn_id": "turn-1",
            "root_turn_id": "root-turn",
            "item_id": "item-1",
            "cell_id": null,
            "parent_call_id": null,
            "originating_response_id": null,
            "subsequent_response_id": null,
            "app_server_client": {
                "product_client_id": "codex_tui",
                "client_name": "codex-tui",
                "client_version": "1.2.3",
                "rpc_transport": "websocket",
                "experimental_api_enabled": true
            },
            "runtime": {
                "codex_rs_version": "0.99.0",
                "runtime_os": "macos",
                "runtime_os_version": "15.3.1",
                "runtime_arch": "aarch64"
            },
            "thread_source": "user",
            "subagent_source": null,
            "parent_thread_id": null,
            "tool_name": "shell",
            "started_at_ms": 123000,
            "completed_at_ms": 125000,
            "duration_ms": 2000,
            "execution_duration_ms": 1900,
            "review_count": 0,
            "guardian_review_count": 0,
            "user_review_count": 0,
            "final_approval_outcome": "not_needed",
            "terminal_status": "completed",
            "failure_kind": null,
            "requested_additional_permissions": false,
            "requested_network_access": false,
            "plugin_id": "sample@openai-curated",
            "script_path": "scripts/run.py",
            "command_execution_source": "agent",
            "exit_code": 0,
            "command_total_action_count": 4,
            "command_read_action_count": 1,
            "command_list_files_action_count": 1,
            "command_search_action_count": 1,
            "command_unknown_action_count": 1
        }
    });
    // Keep this field separate to stay within json!'s macro recursion limit.
    expected["event_params"]["tool_event_type"] = json!("model_tool_call");
    assert_eq!(payload, expected);
}

#[tokio::test]
async fn item_lifecycle_notifications_publish_command_execution_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    ingest_review_prerequisites(&mut reducer, &mut events).await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                "thread-1", "turn-1",
            ))),
            &mut events,
        )
        .await;
    for model in ["invoking-model", "later-model"] {
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(ServerNotification::ItemStarted(
                    ItemStartedNotification {
                        thread_id: "thread-1".to_string(),
                        turn_id: "turn-1".to_string(),
                        started_at_ms: 1_000,
                        item: {
                            let mut item = sample_command_execution_item(
                                CommandExecutionStatus::InProgress,
                                /*exit_code*/ None,
                                /*duration_ms*/ None,
                            );
                            if let ThreadItem::CommandExecution { model_context, .. } = &mut item {
                                *model_context =
                                    Some(codex_protocol::items::ModelInvocationContext {
                                        model_slug: model.to_string(),
                                        reasoning_effort: Some("max".to_string()),
                                    });
                            }
                            item
                        },
                    },
                ))),
                &mut events,
            )
            .await;
    }
    assert!(
        events.is_empty(),
        "tool item event should emit on completion"
    );

    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ItemCompleted(
                ItemCompletedNotification {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    completed_at_ms: 1_045,
                    item: sample_command_execution_item_with_actions(
                        CommandExecutionStatus::Completed,
                        Some(0),
                        Some(42),
                        vec![
                            CommandAction::Read {
                                command: "cat README.md".to_string(),
                                name: "README.md".to_string(),
                                path: test_path_buf("/tmp/README.md").abs().into(),
                            },
                            CommandAction::ListFiles {
                                command: "ls".to_string(),
                                path: None,
                            },
                            CommandAction::Search {
                                command: "rg TODO".to_string(),
                                query: Some("TODO".to_string()),
                                path: None,
                            },
                            CommandAction::Unknown {
                                command: "cargo test".to_string(),
                            },
                        ],
                        Some("sample@openai-curated"),
                        Some("scripts/run.py"),
                    ),
                },
            ))),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_params"]["model_slug"], "invoking-model");
    assert_eq!(payload[0]["event_params"]["reasoning_effort"], "max");
    assert_eq!(payload[0]["event_type"], "codex_command_execution_event");
    assert_eq!(payload[0]["event_params"]["thread_id"], "thread-1");
    assert_eq!(payload[0]["event_params"]["session_id"], "session-thread-1");
    assert_eq!(payload[0]["event_params"]["turn_id"], "turn-1");
    assert_eq!(payload[0]["event_params"]["item_id"], "item-1");
    assert_eq!(payload[0]["event_params"]["tool_name"], "shell");
    assert_eq!(
        payload[0]["event_params"]["plugin_id"],
        "sample@openai-curated"
    );
    assert_eq!(payload[0]["event_params"]["script_path"], "scripts/run.py");
    assert_eq!(
        payload[0]["event_params"]["command_execution_source"],
        "agent"
    );
    assert_eq!(payload[0]["event_params"]["terminal_status"], "completed");
    assert_eq!(
        payload[0]["event_params"]["final_approval_outcome"],
        "unknown"
    );
    assert_eq!(
        payload[0]["event_params"]["failure_kind"],
        serde_json::Value::Null
    );
    assert_eq!(payload[0]["event_params"]["exit_code"], 0);
    assert_eq!(payload[0]["event_params"]["command_total_action_count"], 4);
    assert_eq!(payload[0]["event_params"]["command_read_action_count"], 1);
    assert_eq!(
        payload[0]["event_params"]["command_list_files_action_count"],
        1
    );
    assert_eq!(payload[0]["event_params"]["command_search_action_count"], 1);
    assert_eq!(
        payload[0]["event_params"]["command_unknown_action_count"],
        1
    );
    assert_eq!(payload[0]["event_params"]["started_at_ms"], 1_000);
    assert_eq!(payload[0]["event_params"]["completed_at_ms"], 1_045);
    assert_eq!(payload[0]["event_params"]["duration_ms"], 45);
    assert_eq!(payload[0]["event_params"]["execution_duration_ms"], 42);
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["client_name"],
        "codex-tui"
    );
    assert_eq!(payload[0]["event_params"]["thread_source"], "user");
}

#[tokio::test]
async fn collaborator_tool_events_keep_response_ids_when_completion_races_sampling() {
    for response_first in [false, true] {
        let mut reducer = AnalyticsReducer::default();
        let mut events = Vec::new();
        ingest_review_prerequisites(&mut reducer, &mut events).await;
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                    "thread-1", "turn-1",
                ))),
                &mut events,
            )
            .await;
        let item = ThreadItem::CollabAgentToolCall {
            id: "call-1".into(),
            tool: CollabAgentTool::SendMessage,
            status: CollabAgentToolCallStatus::Failed,
            sender_thread_id: "thread-1".into(),
            receiver_thread_ids: Vec::new(),
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: Default::default(),
        };
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(ServerNotification::ItemStarted(
                    ItemStartedNotification {
                        thread_id: "thread-1".into(),
                        turn_id: "turn-1".into(),
                        started_at_ms: 1_000,
                        item: item.clone(),
                    },
                ))),
                &mut events,
            )
            .await;
        let response = AnalyticsFact::Custom(CustomAnalyticsFact::CodeModeToolCall(
            sampling_response("turn-1", "response-1", &["call-1"]),
        ));
        let completion = AnalyticsFact::Notification(Box::new(ServerNotification::ItemCompleted(
            ItemCompletedNotification {
                thread_id: "thread-1".into(),
                turn_id: "turn-1".into(),
                completed_at_ms: 1_010,
                item,
            },
        )));
        let facts = if response_first {
            [response, completion]
        } else {
            [completion, response]
        };
        for fact in facts {
            reducer.ingest(fact, &mut events).await;
            assert!(events.is_empty(), "emitted before response correlation");
        }
        ingest_code_mode_facts(
            &mut reducer,
            &mut events,
            [sampling_response("turn-1", "response-2", &[])],
        )
        .await;
        let payload = serde_json::to_value(&events).expect("serialize collaborator event");
        assert_eq!(payload.as_array().expect("events array").len(), 1);
        let params = &payload[0]["event_params"];
        assert_eq!(
            json!({
                "type": payload[0]["event_type"],
                "item": params["item_id"],
                "origin": params["originating_response_id"],
                "subsequent": params["subsequent_response_id"],
                "status": params["terminal_status"],
                "tool_event_type": params["tool_event_type"],
            }),
            json!({
                "type": "codex_collab_agent_tool_call_event",
                "item": "call-1",
                "origin": "response-1",
                "subsequent": "response-2",
                "status": "failed",
                "tool_event_type": "model_tool_call",
            }),
        );
    }
}

#[tokio::test]
async fn code_mode_exec_wait_and_child_events_share_cell_and_response_ids() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    ingest_review_prerequisites(&mut reducer, &mut events).await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                TurnResolvedConfigFact {
                    turn_metadata: test_turn_metadata(Some("root-a")),
                    ..sample_turn_resolved_config("thread-1", "turn-1")
                },
            ))),
            &mut events,
        )
        .await;

    let completed = |turn_id: &str, root_turn_id: &str, call_id: &str, tool_name: &str| {
        CodeModeToolCallFact::Completed {
            thread_id: "thread-1".into(),
            turn_id: turn_id.into(),
            turn_metadata: test_turn_metadata(Some(root_turn_id)),
            call_id: call_id.into(),
            cell_id: Some("cell-1".into()),
            tool_name: tool_name.into(),
            started_at_ms: 1_000,
            completed_at_ms: 1_010,
            status: CodeModeToolCallStatus::Completed,
        }
    };
    ingest_code_mode_facts(
        &mut reducer,
        &mut events,
        [
            sampling_response("turn-1", "resp-a", &["exec-1"]),
            CodeModeToolCallFact::CellStarted {
                thread_id: "thread-1".into(),
                turn_id: "turn-1".into(),
                call_id: "exec-1".into(),
                cell_id: "cell-1".into(),
            },
            CodeModeToolCallFact::ChildStarted {
                thread_id: "thread-1".into(),
                turn_id: "turn-1".into(),
                call_id: "child-1".into(),
                cell_id: "cell-1".into(),
            },
            completed("turn-1", "root-a", "exec-1", "exec"),
        ],
    )
    .await;
    ingest_completed_command_execution_item(&mut reducer, &mut events, "thread-1", "child-1").await;
    assert!(events.is_empty());

    ingest_code_mode_facts(
        &mut reducer,
        &mut events,
        [
            sampling_response("turn-1", "resp-b", &[]),
            sampling_response("turn-2", "resp-c", &["wait-1"]),
            completed("turn-2", "root-b", "wait-1", "wait"),
            sampling_response("turn-2", "resp-d", &[]),
        ],
    )
    .await;

    let actual = events
        .iter()
        .map(|event| {
            let event = serde_json::to_value(event).expect("serialize tool event");
            serde_json::json!({
                "item": event["event_params"]["item_id"],
                "tool_event_type": event["event_params"]["tool_event_type"],
                "root": event["event_params"]["root_turn_id"],
                "cell": event["event_params"]["cell_id"],
                "parent": event["event_params"]["parent_call_id"],
                "origin": event["event_params"]["originating_response_id"],
                "subsequent": event["event_params"]["subsequent_response_id"],
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            serde_json::json!({"tool_event_type":"model_tool_call","item":"exec-1","root":"root-a","cell":"cell-1","parent":null,"origin":"resp-a","subsequent":"resp-b"}),
            serde_json::json!({"tool_event_type":"inner_tool_call","item":"child-1","root":"root-a","cell":"cell-1","parent":"exec-1","origin":"resp-a","subsequent":"resp-b"}),
            serde_json::json!({"tool_event_type":"model_tool_call","item":"wait-1","root":"root-b","cell":"cell-1","parent":"exec-1","origin":"resp-c","subsequent":"resp-d"}),
        ]
    );
}

#[tokio::test]
async fn tool_event_types_require_exact_unambiguous_call_origin() {
    for (sampled_ids, child_ids, expected) in [
        (
            vec!["command-1", "control-1"],
            vec![],
            json!("model_tool_call"),
        ),
        (
            vec![],
            vec!["command-1", "control-1"],
            json!("inner_tool_call"),
        ),
        (vec![], vec![], json!(null)),
        (
            vec!["command-1", "control-1"],
            vec!["command-1", "control-1"],
            json!(null),
        ),
        (vec!["other-call"], vec!["other-child"], json!(null)),
    ] {
        let mut reducer = AnalyticsReducer::default();
        let mut events = Vec::new();
        ingest_review_prerequisites(&mut reducer, &mut events).await;
        ingest_code_mode_facts(
            &mut reducer,
            &mut events,
            [
                sampling_response("turn-1", "response-1", &sampled_ids),
                CodeModeToolCallFact::CellStarted {
                    thread_id: "thread-1".into(),
                    turn_id: "turn-1".into(),
                    call_id: "exec-1".into(),
                    cell_id: "cell-1".into(),
                },
            ],
        )
        .await;
        ingest_code_mode_facts(
            &mut reducer,
            &mut events,
            child_ids
                .into_iter()
                .map(|call_id| CodeModeToolCallFact::ChildStarted {
                    thread_id: "thread-1".into(),
                    turn_id: "turn-1".into(),
                    call_id: call_id.into(),
                    cell_id: "cell-1".into(),
                }),
        )
        .await;
        ingest_completed_command_execution_item(&mut reducer, &mut events, "thread-1", "command-1")
            .await;
        reducer
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::ControlToolCall(ControlToolCallFact {
                    thread_id: "thread-1".into(),
                    turn_id: "turn-1".into(),
                    turn_metadata: test_turn_metadata(/*root_turn_id*/ None),
                    call_id: "control-1".into(),
                    cell_id: Some("cell-1".into()),
                    tool_name: "view_image".into(),
                    started_at_ms: 1_000,
                    completed_at_ms: 1_042,
                    status: ControlToolCallStatus::Completed,
                })),
                &mut events,
            )
            .await;
        reducer.flush(&mut events);
        let payload = serde_json::to_value(&events).expect("serialize tool events");
        assert_eq!(
            payload
                .as_array()
                .expect("events")
                .iter()
                .map(|event| json!({
                    "event": event["event_type"],
                    "type": event["event_params"]["tool_event_type"],
                    "status": event["event_params"]["terminal_status"],
                    "duration": event["event_params"]["duration_ms"],
                }))
                .collect::<Vec<_>>(),
            vec![
                json!({"event": "codex_command_execution_event", "type": expected, "status": "completed", "duration": 42}),
                json!({"event": "codex_control_tool_call_event", "type": expected, "status": "completed", "duration": 42}),
            ],
        );
    }
}

#[tokio::test]
async fn tool_event_emitted_before_sampling_evidence_keeps_unknown_origin() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    ingest_review_prerequisites(&mut reducer, &mut events).await;
    ingest_completed_command_execution_item(&mut reducer, &mut events, "thread-1", "call-1").await;
    assert_eq!(events.len(), 1);
    ingest_code_mode_facts(
        &mut reducer,
        &mut events,
        [sampling_response("turn-1", "response-1", &["call-1"])],
    )
    .await;
    reducer.flush(&mut events);
    let payload = serde_json::to_value(&events).expect("serialize tool events");
    assert_eq!(
        payload
            .as_array()
            .expect("events")
            .iter()
            .map(|event| json!({
                "item": event["event_params"]["item_id"],
                "type": event["event_params"]["tool_event_type"],
                "origin": event["event_params"]["originating_response_id"],
            }))
            .collect::<Vec<_>>(),
        vec![json!({"item": "call-1", "type": null, "origin": null})],
    );
}

#[tokio::test]
async fn mcp_elicitation_classification_survives_turn_completion_and_preserves_call_grain() {
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

    let mut items = Vec::new();
    for (item_id, connector_id, elicitation_type) in [
        ("auth", "calendar", Some(ElicitationType::AuthOrLink)),
        ("retry", "calendar", None),
        ("denied", "drive", Some(ElicitationType::Approval)),
    ] {
        let item = ThreadItem::McpToolCall {
            id: item_id.to_string(),
            server: "server".to_string(),
            tool: "search".to_string(),
            status: McpToolCallStatus::Completed,
            arguments: json!({ "token": "synthetic-private-input" }),
            app_context: Some(McpToolCallAppContext {
                connector_id: connector_id.to_string(),
                link_id: None,
                resource_uri: None,
                app_name: None,
                action_name: None,
            }),
            mcp_app_resource_uri: None,
            mcp_app_ui: None,
            plugin_id: None,
            read_only_hint: None,
            result: None,
            error: None,
            duration_ms: Some(2),
        };
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
                &mut events,
            )
            .await;
        if let Some(elicitation_type) = elicitation_type {
            reducer
                .ingest(
                    AnalyticsFact::Custom(CustomAnalyticsFact::McpToolCallElicitation(
                        McpToolCallElicitation {
                            thread_id: "thread-2".to_string(),
                            turn_id: "turn-2".to_string(),
                            item_id: item_id.to_string(),
                            elicitation_type,
                        },
                    )),
                    &mut events,
                )
                .await;
        }
        items.push(item);
    }
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
    for item in items {
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
                &mut events,
            )
            .await;
    }

    let payload = serde_json::to_value(&events).expect("serialize analytics events");
    let classifications = payload
        .as_array()
        .expect("analytics events array")
        .iter()
        .filter(|event| event["event_type"] == "codex_mcp_tool_call_event")
        .map(|event| {
            json!({
                "item_id": event["event_params"]["item_id"],
                "connector_id": event["event_params"]["connector_id"],
                "elicitation_type": event["event_params"].get("elicitation_type")
                    .expect("elicitation_type must be present"),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        classifications,
        vec![
            json!({"item_id": "auth", "connector_id": "calendar", "elicitation_type": "auth_or_link"}),
            json!({"item_id": "retry", "connector_id": "calendar", "elicitation_type": null}),
            json!({"item_id": "denied", "connector_id": "drive", "elicitation_type": "approval"}),
        ]
    );
    assert!(!payload.to_string().contains("synthetic-private-input"));
}

#[tokio::test]
async fn reducer_ingests_artifact_operation_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::ArtifactOperation(
                ArtifactOperationInput {
                    tracking: test_tracking_context("thread-1", "turn-1"),
                    operation: ArtifactOperation {
                        item_id: "call-1".to_string(),
                        lifecycle: ArtifactOperationLifecycle::Started,
                        occurred_at_ms: 1_786_000_000_000,
                        plugin_id: "presentations@openai-primary-runtime".to_string(),
                        script_path: "skills/presentations/container_tools/mark_artifact_operation_started.mjs".to_string(),
                        skill: "presentations".to_string(),
                        artifact_type: "presentation".to_string(),
                        operation_kind: "create".to_string(),
                        expected_output_count: 2,
                        output_format: "pptx".to_string(),
                        execution_backend: "unified_exec".to_string(),
                    },
                },
            )),
            &mut events,
        )
        .await;

    assert!(events[0].can_send_with_api_key_auth());
    assert_eq!(
        serde_json::to_value(events).expect("serialize events"),
        json!([{
            "event_type": "codex_artifact_operation",
            "event_params": {
                "thread_id": "thread-1",
                "turn_id": "turn-1",
                "item_id": "call-1",
                "lifecycle": "started",
                "occurred_at_ms": 1_786_000_000_000_u64,
                "product_client_id": TEST_PRODUCT_CLIENT_ID,
                "runtime": serde_json::to_value(current_runtime_metadata())
                    .expect("serialize runtime metadata"),
                "model_slug": "gpt-5",
                "plugin_id": "presentations@openai-primary-runtime",
                "script_path": "skills/presentations/container_tools/mark_artifact_operation_started.mjs",
                "skill": "presentations",
                "artifact_type": "presentation",
                "operation_kind": "create",
                "expected_output_count": 2,
                "output_format": "pptx",
                "execution_backend": "unified_exec"
            }
        }])
    );
}

#[tokio::test]
async fn image_generation_events_preserve_transparent_background_metadata() {
    for (status, transparent_background) in [
        ("completed", Some(true)),
        ("completed", Some(false)),
        ("completed", None),
        ("failed", None),
    ] {
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

        let item = ThreadItem::ImageGeneration(ImageGenerationItem {
            id: "image-1".to_string(),
            status: status.to_string(),
            revised_prompt: None,
            result: "ok".to_string(),
            transparent_background,
            failure: None,
            saved_path: None,
            imagegen_request_id: None,
            generation_id: None,
        });

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

        let event = out
            .iter()
            .find(|event| matches!(event, TrackEventRequest::ImageGeneration(_)))
            .expect("image generation event should be emitted");
        let payload = serde_json::to_value(event).expect("serialize image generation event");

        assert_eq!(
            payload["event_params"].get("transparent_background"),
            Some(&json!(transparent_background))
        );
        assert_eq!(payload["event_params"]["terminal_status"], json!(status));
    }
}
