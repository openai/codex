use super::AnalyticsEventsClient;
use super::AnalyticsEventsDestination;
use super::AnalyticsEventsQueue;
use super::AnalyticsEventsQueueMessage;
use super::InternalToolInputQueue; // copybara:strip-for-public
#[cfg(debug_assertions)]
use super::capture_track_events_request;
#[cfg(debug_assertions)]
use super::send_track_events;
#[cfg(debug_assertions)]
use super::send_track_events_request;
use super::track_event_request_batches;
#[cfg(debug_assertions)]
use crate::events::AppServerRpcTransport;
use crate::events::CodexAcceptedLineFingerprintsEventParams;
use crate::events::CodexAcceptedLineFingerprintsEventRequest;
#[cfg(debug_assertions)]
use crate::events::CodexAppServerClientMetadata;
#[cfg(debug_assertions)]
use crate::events::CodexMcpToolCallEventParams;
#[cfg(debug_assertions)]
use crate::events::CodexMcpToolCallEventRequest;
#[cfg(debug_assertions)]
use crate::events::CodexPluginMeasurementEventParams;
#[cfg(debug_assertions)]
use crate::events::CodexPluginMeasurementEventRequest;
#[cfg(debug_assertions)]
use crate::events::CodexPluginMetadata;
#[cfg(debug_assertions)]
use crate::events::CodexPluginUsedEventRequest;
#[cfg(debug_assertions)]
use crate::events::CodexPluginUsedMetadata;
#[cfg(debug_assertions)]
use crate::events::CodexRuntimeMetadata;
#[cfg(debug_assertions)]
use crate::events::CodexToolItemEventBase;
#[cfg(debug_assertions)]
use crate::events::FinalApprovalOutcome;
use crate::events::SkillInvocationEventParams;
use crate::events::SkillInvocationEventRequest;
#[cfg(debug_assertions)]
use crate::events::ThreadArchiveAction;
#[cfg(debug_assertions)]
use crate::events::ThreadArchiveEvent;
#[cfg(debug_assertions)]
use crate::events::ThreadArchiveEventParams;
#[cfg(debug_assertions)]
use crate::events::ToolItemTerminalStatus;
use crate::events::TrackEventRequest;
#[cfg(debug_assertions)]
use crate::events::codex_artifact_operation_event_request;
use crate::facts::AnalyticsFact;
#[cfg(debug_assertions)]
use crate::facts::ArtifactOperation;
#[cfg(debug_assertions)]
use crate::facts::ArtifactOperationLifecycle;
use crate::facts::CustomAnalyticsFact;
use crate::facts::InvocationType;
use crate::facts::PluginMeasurementRow;
use crate::facts::PluginMeasurementsInput;
#[cfg(debug_assertions)]
use crate::facts::TrackEventsContext;
use crate::reducer::MAX_PLUGIN_MEASUREMENTS_PER_BATCH;
use codex_app_server_protocol::ApprovalsReviewer as AppServerApprovalsReviewer;
use codex_app_server_protocol::AskForApproval as AppServerAskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::CommandExecutionOutputDeltaNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy as AppServerSandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionSource as AppServerSessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadArchivedNotification;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus as AppServerThreadStatus;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnDiffUpdatedNotification;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
#[cfg(debug_assertions)]
use codex_login::AuthManager;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::HashSet;
#[cfg(debug_assertions)]
use std::fs;
#[cfg(debug_assertions)]
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock; // copybara:strip-for-public
#[cfg(debug_assertions)]
use std::time::SystemTime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

#[cfg(debug_assertions)]
impl AnalyticsEventsClient {
    pub(crate) fn new_for_capture_file(auth_manager: Arc<AuthManager>, path: PathBuf) -> Self {
        // copybara:strip-for-public begin
        let internal_tool_input = InternalToolInputQueue {
            auth_manager: Arc::clone(&auth_manager),
            destination: AnalyticsEventsDestination::CaptureFile { path: path.clone() },
            queue: Arc::new(OnceLock::new()),
        };
        // copybara:strip-for-public end
        Self {
            queue: Some(AnalyticsEventsQueue::new(
                auth_manager,
                AnalyticsEventsDestination::CaptureFile { path },
            )),
            // copybara:strip-for-public begin
            internal_tool_input: Some(internal_tool_input),
            // copybara:strip-for-public end
        }
    }
}

fn sample_accepted_line_fingerprint_event(thread_id: &str) -> TrackEventRequest {
    TrackEventRequest::AcceptedLineFingerprints(Box::new(
        CodexAcceptedLineFingerprintsEventRequest {
            event_type: "codex_accepted_line_fingerprints",
            event_params: CodexAcceptedLineFingerprintsEventParams {
                event_type: "codex.accepted_line_fingerprints",
                turn_id: "turn-1".to_string(),
                thread_id: thread_id.to_string(),
                product_surface: Some("codex".to_string()),
                model_slug: Some("gpt-5.1-codex".to_string()),
                completed_at: 1,
                repo_hash: None,
                accepted_added_lines: 1,
                accepted_deleted_lines: 0,
                line_fingerprints: [],
            },
        },
    ))
}

fn sample_skill_track_event(thread_id: &str, plugin_id: Option<&str>) -> TrackEventRequest {
    TrackEventRequest::SkillInvocation(SkillInvocationEventRequest {
        event_type: "skill_invocation",
        skill_id: format!("skill-{thread_id}"),
        skill_name: "doc".to_string(),
        event_params: SkillInvocationEventParams {
            product_client_id: None,
            skill_scope: None,
            plugin_id: plugin_id.map(str::to_string),
            remote_plugin_id: None,
            repo_url: None,
            thread_id: Some(thread_id.to_string()),
            turn_id: Some("turn-1".to_string()),
            invoke_type: Some(InvocationType::Explicit),
            model_slug: Some("gpt-5.1-codex".to_string()),
        },
    })
}

#[cfg(debug_assertions)]
fn sample_artifact_operation_event(thread_id: &str) -> TrackEventRequest {
    TrackEventRequest::ArtifactOperation(codex_artifact_operation_event_request(
        TrackEventsContext {
            model_slug: "gpt-5.1-codex".to_string(),
            thread_id: thread_id.to_string(),
            turn_id: "turn-1".to_string(),
            product_client_id: "codex_desktop".to_string(),
        },
        ArtifactOperation {
            item_id: format!("item-{thread_id}"),
            lifecycle: ArtifactOperationLifecycle::Started,
            occurred_at_ms: 1,
            plugin_id: "presentations@openai-primary-runtime".to_string(),
            script_path: "skills/presentations/container_tools/mark_artifact_operation_started.mjs"
                .to_string(),
            skill: "presentations".to_string(),
            artifact_type: "presentation".to_string(),
            operation_kind: "create".to_string(),
            expected_output_count: 1,
            output_format: "pptx".to_string(),
            execution_backend: "unified_exec".to_string(),
        },
    ))
}

fn sample_regular_track_event(thread_id: &str) -> TrackEventRequest {
    sample_skill_track_event(thread_id, /*plugin_id*/ None)
}

#[cfg(debug_assertions)]
fn sample_mcp_tool_call_event(thread_id: &str, plugin_id: Option<&str>) -> TrackEventRequest {
    TrackEventRequest::McpToolCall(CodexMcpToolCallEventRequest {
        event_type: "codex_mcp_tool_call_event",
        event_params: CodexMcpToolCallEventParams {
            base: CodexToolItemEventBase {
                thread_id: thread_id.to_string(),
                session_id: format!("session-{thread_id}"),
                turn_id: "turn-1".to_string(),
                item_id: format!("item-{thread_id}"),
                cell_id: None,
                parent_call_id: None,
                originating_response_id: None,
                subsequent_response_id: None,
                app_server_client: CodexAppServerClientMetadata {
                    product_client_id: "codex_desktop".to_string(),
                    client_name: None,
                    client_version: None,
                    rpc_transport: AppServerRpcTransport::InProcess,
                    experimental_api_enabled: None,
                },
                runtime: CodexRuntimeMetadata {
                    codex_rs_version: "0.0.0".to_string(),
                    runtime_os: "test".to_string(),
                    runtime_os_version: "test".to_string(),
                    runtime_arch: "test".to_string(),
                },
                thread_source: None,
                subagent_source: None,
                parent_thread_id: None,
                tool_name: "search".to_string(),
                started_at_ms: 1,
                completed_at_ms: 2,
                duration_ms: Some(1),
                execution_duration_ms: Some(1),
                review_count: 0,
                guardian_review_count: 0,
                user_review_count: 0,
                final_approval_outcome: FinalApprovalOutcome::NotNeeded,
                terminal_status: ToolItemTerminalStatus::Completed,
                failure_kind: None,
                requested_additional_permissions: false,
                requested_network_access: false,
            },
            mcp_server_name: "sample".to_string(),
            mcp_tool_name: "search".to_string(),
            mcp_error_present: false,
            plugin_id: plugin_id.map(str::to_string),
            connector_id: None,
        },
    })
}

#[cfg(debug_assertions)]
fn sample_plugin_used_track_event(thread_id: &str, plugin_id: Option<&str>) -> TrackEventRequest {
    TrackEventRequest::PluginUsed(CodexPluginUsedEventRequest {
        event_type: "codex_plugin_used",
        event_params: CodexPluginUsedMetadata {
            plugin: CodexPluginMetadata {
                plugin_id: plugin_id.map(str::to_string),
                remote_plugin_id: None,
                plugin_name: Some("sample".to_string()),
                marketplace_name: Some("test".to_string()),
                has_skills: Some(true),
                mcp_server_count: Some(1),
                connector_ids: Some(vec!["calendar".to_string()]),
                product_client_id: Some("codex_desktop".to_string()),
            },
            mcp_server_names: Some(vec!["mcp-1".to_string()]),
            thread_id: Some(thread_id.to_string()),
            turn_id: Some("turn-1".to_string()),
            model_slug: Some("gpt-5.1-codex".to_string()),
        },
    })
}

#[cfg(debug_assertions)]
fn unique_capture_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "codex-analytics-{name}-{}-{nonce}.jsonl",
        std::process::id()
    ))
}

fn client_with_receiver() -> (
    AnalyticsEventsClient,
    mpsc::Receiver<AnalyticsEventsQueueMessage>,
) {
    let (sender, receiver) = mpsc::channel(8);
    let queue = AnalyticsEventsQueue {
        sender,
        app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
        plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
    };
    // copybara:replace-for-public begin
    (
        AnalyticsEventsClient {
            queue: Some(queue),
            internal_tool_input: None,
        },
        receiver,
    )
    // copybara:replace-for-public with
    // copybara:public (AnalyticsEventsClient { queue: Some(queue) }, receiver)
    // copybara:replace-for-public end
}

#[test]
#[cfg(debug_assertions)]
fn analytics_destination_uses_explicit_capture_file() {
    let capture_path = unique_capture_path("destination");
    let destination = AnalyticsEventsDestination::from_base_url_and_capture_file(
        "https://chatgpt.com/backend-api/".to_string(),
        Some(capture_path.clone()),
    );

    assert_eq!(
        destination,
        AnalyticsEventsDestination::CaptureFile {
            path: capture_path.clone()
        }
    );
    assert_eq!(
        fs::read_to_string(&capture_path).expect("read capture file"),
        ""
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(&capture_path)
            .expect("read capture file metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    fs::remove_file(capture_path).expect("remove capture file");
}

#[test]
fn analytics_destination_uses_http_without_capture_file() {
    let destination = AnalyticsEventsDestination::from_base_url_and_capture_file(
        "https://chatgpt.com/backend-api/".to_string(),
        /*capture_file*/ None,
    );

    assert_eq!(
        destination,
        AnalyticsEventsDestination::Http {
            url: "https://chatgpt.com/backend-api/codex/analytics-events/events".to_string()
        }
    );
}

#[test]
#[cfg(not(debug_assertions))]
fn analytics_destination_ignores_capture_file_in_release() {
    let destination = AnalyticsEventsDestination::from_base_url_and_capture_file(
        "https://chatgpt.com/backend-api/".to_string(),
        Some(std::path::PathBuf::from("ignored.jsonl")),
    );

    assert_eq!(
        destination,
        AnalyticsEventsDestination::Http {
            url: "https://chatgpt.com/backend-api/codex/analytics-events/events".to_string()
        }
    );
}

#[tokio::test]
#[cfg(debug_assertions)]
async fn capture_file_writes_exact_serialized_request() {
    let capture_path = unique_capture_path("single");
    let destination = AnalyticsEventsDestination::CaptureFile {
        path: capture_path.clone(),
    };
    let event = sample_regular_track_event("thread-1");
    let expected_event = serde_json::to_value(&event).expect("serialize expected event");
    let auth = codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing();

    send_track_events_request(&auth, &destination, vec![event]).await;

    let contents = fs::read_to_string(&capture_path).expect("read capture file");
    let lines = contents.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    let payload: serde_json::Value =
        serde_json::from_str(lines[0]).expect("parse captured payload");
    assert_eq!(payload, serde_json::json!({"events": [expected_event]}));

    fs::remove_file(capture_path).expect("remove capture file");
}

#[tokio::test]
#[cfg(debug_assertions)]
async fn capture_file_writes_final_batches_as_separate_lines() {
    let capture_path = unique_capture_path("batches");
    let destination = AnalyticsEventsDestination::CaptureFile {
        path: capture_path.clone(),
    };
    let auth = codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let events = vec![
        sample_regular_track_event("thread-1"),
        sample_accepted_line_fingerprint_event("thread-2"),
        sample_regular_track_event("thread-3"),
    ];

    for batch in track_event_request_batches(events) {
        send_track_events_request(&auth, &destination, batch).await;
    }

    let contents = fs::read_to_string(&capture_path).expect("read capture file");
    let payloads = contents
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("parse capture line"))
        .collect::<Vec<_>>();
    assert_eq!(payloads.len(), 3);
    assert_eq!(payloads[0]["events"][0]["skill_id"], "skill-thread-1");
    assert_eq!(
        payloads[1]["events"][0]["event_type"],
        "codex_accepted_line_fingerprints"
    );
    assert_eq!(payloads[2]["events"][0]["skill_id"], "skill-thread-3");

    fs::remove_file(capture_path).expect("remove capture file");
}

#[tokio::test]
#[cfg(debug_assertions)]
async fn api_key_auth_sends_only_plugin_events_to_codex_backend() {
    let capture_path = unique_capture_path("api-key-plugin-events");
    let destination = AnalyticsEventsDestination::CaptureFile {
        path: capture_path.clone(),
    };
    let auth_manager = codex_login::AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::from_api_key("sk-test"),
    );
    let plugin_measurement = |thread_id: &str, plugin_id: &str| {
        TrackEventRequest::PluginMeasurement(CodexPluginMeasurementEventRequest {
            event_type: "codex_plugin_measurement_event",
            event_params: CodexPluginMeasurementEventParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                plugin_id: plugin_id.to_string(),
                execution_id: "execution-1".to_string(),
                operation: "security_scan".to_string(),
                measurement_name: "findings".to_string(),
                number_value: 1.0,
                dimensions: None,
            },
        })
    };

    send_track_events(
        &auth_manager,
        &destination,
        vec![
            sample_regular_track_event("non-plugin-skill"),
            sample_mcp_tool_call_event("non-plugin-mcp", /*plugin_id*/ None),
            sample_plugin_used_track_event("non-plugin-used", /*plugin_id*/ None),
            plugin_measurement("non-plugin-measurement", /*plugin_id*/ ""),
            sample_accepted_line_fingerprint_event("other-event"),
            TrackEventRequest::ThreadArchive(ThreadArchiveEvent {
                event_type: "codex_thread_archive_event",
                event_params: ThreadArchiveEventParams {
                    thread_id: "non-plugin-thread-archive".to_string(),
                    action: ThreadArchiveAction::Archived,
                    occurred_at_ms: 1,
                },
            }),
            sample_plugin_used_track_event("plugin-used", Some("sample@test")),
            sample_skill_track_event("plugin-skill", Some("sample@test")),
            sample_mcp_tool_call_event("plugin-mcp", Some("sample@test")),
            sample_artifact_operation_event("plugin-artifact"),
            plugin_measurement("plugin-measurement", "sample@test"),
        ],
    )
    .await;

    let contents = fs::read_to_string(&capture_path).expect("read capture file");
    let lines = contents.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    let payload: serde_json::Value =
        serde_json::from_str(lines[0]).expect("parse captured payload");
    let events = payload["events"].as_array().expect("events array");
    for event in events {
        let event_params = event["event_params"].as_object().expect("event params");
        for server_owned_field in [
            "auth_mode",
            "api_organization_id",
            "api_project_id",
            "api_key_tracking_id",
        ] {
            assert!(!event_params.contains_key(server_owned_field));
        }
    }
    let delivered_events = events
        .iter()
        .map(|event| {
            serde_json::json!({
                "event_type": event["event_type"],
                "plugin_id": event["event_params"]["plugin_id"],
                "thread_id": event["event_params"]["thread_id"],
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        delivered_events,
        vec![
            serde_json::json!({
                "event_type": "codex_plugin_used",
                "plugin_id": "sample@test",
                "thread_id": "plugin-used",
            }),
            serde_json::json!({
                "event_type": "skill_invocation",
                "plugin_id": "sample@test",
                "thread_id": "plugin-skill",
            }),
            serde_json::json!({
                "event_type": "codex_mcp_tool_call_event",
                "plugin_id": "sample@test",
                "thread_id": "plugin-mcp",
            }),
            serde_json::json!({
                "event_type": "codex_artifact_operation",
                "plugin_id": "presentations@openai-primary-runtime",
                "thread_id": "plugin-artifact",
            }),
            serde_json::json!({
                "event_type": "codex_plugin_measurement_event",
                "plugin_id": "sample@test",
                "thread_id": "plugin-measurement",
            }),
        ]
    );

    fs::remove_file(capture_path).expect("remove capture file");
}

#[test]
#[cfg(debug_assertions)]
fn capture_write_failure_still_consumes_delivery() {
    let capture_path = unique_capture_path("missing-parent").join("events.jsonl");
    let destination = AnalyticsEventsDestination::CaptureFile { path: capture_path };
    let payload = crate::events::TrackEventsRequest {
        events: vec![sample_regular_track_event("thread-1")],
    };

    assert!(capture_track_events_request(&destination, &payload));
}

fn sample_turn_start_request() -> ClientRequest {
    ClientRequest::TurnStart {
        request_id: RequestId::Integer(1),
        params: TurnStartParams {
            thread_id: "thread-1".to_string(),
            client_user_message_id: None,
            input: Vec::new(),
            ..Default::default()
        },
    }
}

fn sample_turn_steer_request() -> ClientRequest {
    ClientRequest::TurnSteer {
        request_id: RequestId::Integer(2),
        params: TurnSteerParams {
            thread_id: "thread-1".to_string(),
            expected_turn_id: "turn-1".to_string(),
            client_user_message_id: None,
            input: Vec::new(),
            responsesapi_client_metadata: None,
            additional_context: None,
        },
    }
}

fn sample_turn_interrupt_request(turn_id: &str) -> ClientRequest {
    ClientRequest::TurnInterrupt {
        request_id: RequestId::Integer(3),
        params: TurnInterruptParams {
            thread_id: "thread-1".to_string(),
            turn_id: turn_id.to_string(),
        },
    }
}

fn sample_turn_interrupt_response() -> ClientResponsePayload {
    ClientResponsePayload::TurnInterrupt(TurnInterruptResponse {})
}

fn sample_thread_archive_request() -> ClientRequest {
    ClientRequest::ThreadArchive {
        request_id: RequestId::Integer(3),
        params: ThreadArchiveParams {
            thread_id: "thread-1".to_string(),
        },
    }
}

fn sample_thread(thread_id: &str) -> Thread {
    Thread {
        id: thread_id.to_string(),
        extra: None,
        session_id: format!("session-{thread_id}"),
        forked_from_id: None,
        parent_thread_id: None,
        preview: "first prompt".to_string(),
        ephemeral: false,
        section: None,
        section_entered_at: None,
        project_id: None,
        history_mode: Default::default(),
        model_provider: "openai".to_string(),
        created_at: 1,
        updated_at: 2,
        recency_at: Some(2),
        status: AppServerThreadStatus::Idle,
        path: None,
        cwd: test_path_buf("/tmp").abs(),
        cli_version: "0.0.0".to_string(),
        source: AppServerSessionSource::Exec,
        can_accept_direct_input: None,
        thread_source: None,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: None,
        turns: Vec::new(),
    }
}

fn sample_thread_start_response() -> ClientResponsePayload {
    ClientResponsePayload::ThreadStart(ThreadStartResponse {
        thread: sample_thread("thread-1"),
        model: "gpt-5".to_string(),
        model_provider: "openai".to_string(),
        service_tier: None,
        cwd: test_path_buf("/tmp").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_sources: Vec::new(),
        approval_policy: AppServerAskForApproval::OnRequest,
        approvals_reviewer: AppServerApprovalsReviewer::User,
        sandbox: AppServerSandboxPolicy::DangerFullAccess,
        active_permission_profile: None,
        reasoning_effort: None,
        multi_agent_mode: Default::default(),
    })
}

fn sample_thread_resume_response() -> ClientResponsePayload {
    ClientResponsePayload::ThreadResume(ThreadResumeResponse {
        thread: sample_thread("thread-2"),
        model: "gpt-5".to_string(),
        model_provider: "openai".to_string(),
        service_tier: None,
        cwd: test_path_buf("/tmp").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_sources: Vec::new(),
        approval_policy: AppServerAskForApproval::OnRequest,
        approvals_reviewer: AppServerApprovalsReviewer::User,
        sandbox: AppServerSandboxPolicy::DangerFullAccess,
        active_permission_profile: None,
        reasoning_effort: None,
        multi_agent_mode: Default::default(),
        initial_turns_page: None,
        turns_backwards_cursor: None,
        items_backwards_cursor: None,
    })
}

fn sample_thread_fork_response() -> ClientResponsePayload {
    ClientResponsePayload::ThreadFork(ThreadForkResponse {
        thread: sample_thread("thread-3"),
        model: "gpt-5".to_string(),
        model_provider: "openai".to_string(),
        service_tier: None,
        cwd: test_path_buf("/tmp").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_sources: Vec::new(),
        approval_policy: AppServerAskForApproval::OnRequest,
        approvals_reviewer: AppServerApprovalsReviewer::User,
        sandbox: AppServerSandboxPolicy::DangerFullAccess,
        active_permission_profile: None,
        reasoning_effort: None,
        multi_agent_mode: Default::default(),
    })
}

fn sample_turn_start_response() -> ClientResponsePayload {
    ClientResponsePayload::TurnStart(TurnStartResponse {
        turn: Turn {
            id: "turn-1".to_string(),
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            items: Vec::new(),
            status: AppServerTurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        },
    })
}

fn sample_turn_steer_response() -> ClientResponsePayload {
    ClientResponsePayload::TurnSteer(TurnSteerResponse {
        turn_id: "turn-2".to_string(),
    })
}

#[test]
fn track_plugin_measurements_rejects_unbounded_inputs_before_queueing() {
    let (client, mut receiver) = client_with_receiver();
    let measurements = |row_count| PluginMeasurementsInput {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        item_id: "item-1".to_string(),
        plugin_id: "sample@openai-curated".to_string(),
        execution_id: "execution-1".to_string(),
        operation: "security_scan".to_string(),
        rows: vec![
            PluginMeasurementRow {
                measurement_name: "finding_count".to_string(),
                number_value: 1.0,
                dimensions: BTreeMap::new(),
            };
            row_count
        ],
    };

    client.track_plugin_measurements(measurements(MAX_PLUGIN_MEASUREMENTS_PER_BATCH + 1));
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));

    let mut oversized_operation = measurements(1);
    oversized_operation.operation = "o".repeat(65);
    client.track_plugin_measurements(oversized_operation);
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));

    let mut mixed_rows = measurements(4);
    mixed_rows.rows[0].measurement_name = "m".repeat(65);
    mixed_rows.rows[1]
        .dimensions
        .insert("d".repeat(65), "valid".to_string());
    mixed_rows.rows[2]
        .dimensions
        .insert("valid".to_string(), "v".repeat(65));
    client.track_plugin_measurements(mixed_rows);
    assert!(matches!(
        receiver.try_recv(),
        Ok(AnalyticsEventsQueueMessage::Fact(fact))
            if matches!(
                fact.as_ref(),
                AnalyticsFact::Custom(CustomAnalyticsFact::PluginMeasurements(input))
                    if input.rows.len() == 1
                        && input.rows[0].measurement_name == "finding_count"
            )
    ));

    client.track_plugin_measurements(measurements(MAX_PLUGIN_MEASUREMENTS_PER_BATCH));
    assert!(matches!(
        receiver.try_recv(),
        Ok(AnalyticsEventsQueueMessage::Fact(fact))
            if matches!(
                fact.as_ref(),
                AnalyticsFact::Custom(CustomAnalyticsFact::PluginMeasurements(input))
                    if input.rows.len() == MAX_PLUGIN_MEASUREMENTS_PER_BATCH
            )
    ));
}

#[test]
fn track_request_only_enqueues_analytics_relevant_requests() {
    let (client, mut receiver) = client_with_receiver();

    for (request_id, request) in [
        (RequestId::Integer(1), sample_turn_start_request()),
        (RequestId::Integer(2), sample_turn_steer_request()),
    ] {
        client.track_request(/*connection_id*/ 7, request_id, &request);
        assert!(matches!(
            receiver.try_recv(),
            Ok(AnalyticsEventsQueueMessage::Fact(input))
                if matches!(*input, AnalyticsFact::ClientRequest { .. })
        ));
    }

    client.track_request(
        /*connection_id*/ 7,
        RequestId::Integer(3),
        &sample_turn_interrupt_request("turn-1"),
    );
    assert!(matches!(
        receiver.try_recv(),
        Ok(AnalyticsEventsQueueMessage::Fact(input))
            if matches!(
                *input,
                AnalyticsFact::ExplicitClientInterruptRequest {
                    ref turn_id,
                    requested_at_ms,
                    ..
                } if turn_id == "turn-1" && requested_at_ms > 0
            )
    ));

    let ignored_request = sample_thread_archive_request();
    client.track_request(
        /*connection_id*/ 7,
        RequestId::Integer(3),
        &ignored_request,
    );
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));

    client.track_request(
        /*connection_id*/ 7,
        RequestId::Integer(4),
        &sample_turn_interrupt_request(""),
    );
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn track_response_only_enqueues_analytics_relevant_responses() {
    let (client, mut receiver) = client_with_receiver();

    for (request_id, response) in [
        (RequestId::Integer(1), sample_thread_start_response()),
        (RequestId::Integer(2), sample_thread_resume_response()),
        (RequestId::Integer(3), sample_thread_fork_response()),
        (RequestId::Integer(4), sample_turn_start_response()),
        (RequestId::Integer(5), sample_turn_steer_response()),
        (RequestId::Integer(6), sample_turn_interrupt_response()),
    ] {
        client.track_response(/*connection_id*/ 7, request_id, &response);
        assert!(matches!(
            receiver.try_recv(),
            Ok(AnalyticsEventsQueueMessage::Fact(input))
                if matches!(*input, AnalyticsFact::ClientResponse { .. })
        ));
    }

    client.track_response(
        /*connection_id*/ 7,
        RequestId::Integer(7),
        &ClientResponsePayload::ThreadArchive(ThreadArchiveResponse {}),
    );
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

#[cfg(unix)]
#[test]
fn track_response_ignores_unserializable_thread_responses() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let (client, mut receiver) = client_with_receiver();
    let mut response = sample_thread_start_response();
    let ClientResponsePayload::ThreadStart(thread_start) = &mut response else {
        panic!("expected thread/start response");
    };
    thread_start.cwd = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(
        std::path::PathBuf::from(OsString::from_vec(vec![b'/', b'b', b'a', b'd', 0xff])),
    )
    .expect("non-UTF-8 Unix paths are valid absolute paths");

    client.track_response(/*connection_id*/ 7, RequestId::Integer(1), &response);

    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

#[tokio::test]
async fn flush_waits_for_preceding_fact_delivery() {
    let (client, mut receiver) = client_with_receiver();
    client.track_request(
        /*connection_id*/ 7,
        RequestId::Integer(1),
        &sample_turn_start_request(),
    );

    let flush = tokio::spawn(async move { client.flush().await });
    assert!(matches!(
        receiver.recv().await,
        Some(AnalyticsEventsQueueMessage::Fact(input))
            if matches!(*input, AnalyticsFact::ClientRequest { .. })
    ));
    let done_tx = match receiver.recv().await {
        Some(AnalyticsEventsQueueMessage::Flush(done_tx)) => done_tx,
        _ => panic!("expected analytics flush barrier"),
    };
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    assert!(!flush.is_finished());
    done_tx.send(()).expect("flush receiver should remain open");
    flush.await.expect("flush task should complete");
}

#[tokio::test]
async fn flush_is_noop_when_analytics_is_disabled() {
    let client = AnalyticsEventsClient::new(
        codex_login::AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
        "https://chatgpt.com/backend-api".to_string(),
        /*analytics_enabled*/ Some(false),
    );
    client.track_notification(&ServerNotification::ThreadArchived(
        ThreadArchivedNotification {
            thread_id: "thread-1".to_string(),
        },
    ));
    assert!(client.queue.is_none());
    client.flush().await;
}

#[test]
fn track_notification_only_enqueues_analytics_relevant_notifications() {
    let (client, mut receiver) = client_with_receiver();
    let tracked_payload = TurnDiffUpdatedNotification {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        diff: "diff".to_string(),
    };
    let tracked_notification = ServerNotification::TurnDiffUpdated(tracked_payload.clone());

    client.track_notification(&tracked_notification);

    let Ok(AnalyticsEventsQueueMessage::Fact(input)) = receiver.try_recv() else {
        panic!("expected analytics notification");
    };
    let AnalyticsFact::Notification(notification) = *input else {
        panic!("expected analytics notification fact");
    };
    let ServerNotification::TurnDiffUpdated(notification) = *notification else {
        panic!("expected turn diff notification");
    };
    assert_eq!(notification, tracked_payload);

    let ignored_notification =
        ServerNotification::CommandExecutionOutputDelta(CommandExecutionOutputDeltaNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            delta: "output".to_string(),
        });

    client.track_notification(&ignored_notification);
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn track_event_request_batches_only_isolates_accepted_line_fingerprint_events() {
    let batches = track_event_request_batches(vec![
        sample_regular_track_event("thread-1"),
        sample_regular_track_event("thread-2"),
        sample_accepted_line_fingerprint_event("thread-3"),
        sample_accepted_line_fingerprint_event("thread-4"),
        sample_regular_track_event("thread-5"),
        sample_regular_track_event("thread-6"),
    ]);

    assert_eq!(batches.len(), 4);
    assert_eq!(batches[0].len(), 2);
    assert_eq!(batches[1].len(), 1);
    assert_eq!(batches[2].len(), 1);
    assert_eq!(batches[3].len(), 2);
    assert!(batches[1][0].should_send_in_isolated_request());
    assert!(batches[2][0].should_send_in_isolated_request());
}
// copybara:strip-for-public begin
use crate::facts::InternalToolHookOutcome;
use crate::facts::InternalToolInputLog;
use crate::facts::InternalToolSourceKind;
use codex_login::CodexAuth;

const ALICE_TOKEN: &str = "e30.eyJlbWFpbCI6ImFsaWNlQG9wZW5haS5jb20iLCJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF91c2VyX2lkIjoiZW1wbG95ZWUtYWxpY2UiLCJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY2NvdW50LXRlc3QifX0.sig";
const ALICE_OTHER_USER_TOKEN: &str = "e30.eyJlbWFpbCI6ImFsaWNlQG9wZW5haS5jb20iLCJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF91c2VyX2lkIjoiZW1wbG95ZWUtb3RoZXItYWxpY2UifX0.sig";
const BOB_TOKEN: &str = "e30.eyJlbWFpbCI6ImJvYkBvcGVuYWkuY29tIiwiaHR0cHM6Ly9hcGkub3BlbmFpLmNvbS9hdXRoIjp7ImNoYXRncHRfdXNlcl9pZCI6ImVtcGxveWVlLWJvYiJ9fQ.sig";
const OUTSIDER_TOKEN: &str = "e30.eyJlbWFpbCI6Im91dHNpZGVyQGV4YW1wbGUuY29tIiwiaHR0cHM6Ly9hcGkub3BlbmFpLmNvbS9hdXRoIjp7ImNoYXRncHRfdXNlcl9pZCI6ImV4dGVybmFsLXVzZXIifX0.sig";

fn internal_tool_input_auth(access_token: &str, account_id: &str) -> CodexAuth {
    CodexAuth::from_external_chatgpt_tokens(
        access_token,
        account_id,
        /*chatgpt_plan_type*/ None,
    )
    .unwrap()
}

fn internal_tool_input() -> InternalToolInputLog {
    InternalToolInputLog {
        thread_id: "thread-123".to_string(),
        turn_id: Some("turn-123".to_string()),
        tool_call_id: "call-123".to_string(),
        item_id: Some("item-123".to_string()),
        tool_name: "exec_command".to_string(),
        namespace: Some("functions".to_string()),
        source_kind: InternalToolSourceKind::Direct,
        arguments_before_hooks: serde_json::json!({
            "commands": [{"command": "git", "subcommand": "status", "flags": ["--short"]}]
        }),
        hook_normalized_input: Some(serde_json::json!({
            "commands": [{"command": "curl", "flags": ["-H", "--data"]}]
        })),
        arguments_after_hooks: Some(serde_json::json!({})),
        hook_outcome: InternalToolHookOutcome::Unchanged,
        code_mode_cell_id: None,
        code_mode_runtime_tool_call_id: None,
    }
}

#[tokio::test]
async fn internal_tool_inputs_require_employee_auth_and_a_trusted_destination() {
    let employee = internal_tool_input_auth(ALICE_TOKEN, "account-test");
    let outsider = internal_tool_input_auth(OUTSIDER_TOKEN, "account-test");
    let mismatched = internal_tool_input_auth(ALICE_TOKEN, "other-workspace");
    let trusted = "https://chatgpt.com/backend-api";
    let untrusted = "https://chatgpt.com.attacker.invalid/backend-api";
    for (auth, analytics_enabled, base_url) in [
        (CodexAuth::from_api_key("sk-test"), Some(true), trusted),
        (outsider, Some(true), trusted),
        (mismatched, Some(true), trusted),
        (employee.clone(), Some(false), trusted),
        (employee.clone(), Some(true), untrusted),
    ] {
        let client = AnalyticsEventsClient::new(
            codex_login::AuthManager::from_auth_for_testing(auth),
            base_url.to_string(),
            analytics_enabled,
        );
        assert!(!client.is_internal_tool_input_logging_enabled());
    }

    let client = AnalyticsEventsClient::new(
        codex_login::AuthManager::from_auth_for_testing(employee.clone()),
        trusted.to_string(),
        /*analytics_enabled*/ None,
    );
    assert!(client.is_internal_tool_input_logging_enabled());
    assert!(
        !client
            .without_internal_tool_input_logging()
            .is_internal_tool_input_logging_enabled()
    );
    #[cfg(debug_assertions)]
    assert!(
        AnalyticsEventsClient::new(
            codex_login::AuthManager::from_auth_for_testing(employee),
            "http://127.0.0.1:1234".to_string(),
            /*analytics_enabled*/ None,
        )
        .is_internal_tool_input_logging_enabled()
    );
}

#[tokio::test]
async fn internal_tool_inputs_accept_only_bounded_metadata_in_an_isolated_queue() {
    let employee = internal_tool_input_auth(ALICE_TOKEN, "account-test");
    let (mut client, mut receiver) = client_with_receiver();
    let (internal_client, mut internal_receiver) = client_with_receiver();
    client.internal_tool_input = Some(InternalToolInputQueue {
        auth_manager: codex_login::AuthManager::from_auth_for_testing(employee),
        destination: AnalyticsEventsDestination::from_base_url(
            "https://chatgpt.com/backend-api".to_string(),
        ),
        queue: Arc::new(OnceLock::from(internal_client.queue.unwrap())),
    });

    for metadata in [
        serde_json::json!({"cmd": "Bearer private-canary"}),
        serde_json::json!({"commands": [{"command": "git", "flags": ["--token=private-canary"]}]}),
        serde_json::json!({"commands": [{"command": "git", "flags": []}], "password": "private-canary"}),
        serde_json::json!({"commands": vec![serde_json::json!({"command": "git", "flags": []}); 17]}),
        serde_json::json!({"commands": [{"command": "git", "flags": vec!["--short"; 33]}]}),
    ] {
        let mut rejected = internal_tool_input();
        rejected.arguments_before_hooks = metadata;
        client.track_internal_tool_input(rejected);
        assert!(matches!(
            internal_receiver.try_recv(),
            Err(TryRecvError::Empty)
        ));
    }
    let raw = serde_json::json!({"cmd": "Bearer private-canary"});
    let mut rejected = internal_tool_input();
    rejected.hook_normalized_input = Some(raw.clone());
    client.track_internal_tool_input(rejected);
    let mut rejected = internal_tool_input();
    rejected.arguments_after_hooks = Some(raw);
    client.track_internal_tool_input(rejected);
    assert!(matches!(
        internal_receiver.try_recv(),
        Err(TryRecvError::Empty)
    ));

    let input = internal_tool_input();
    for _ in 0..9 {
        client.track_internal_tool_input(input.clone());
    }
    assert_eq!(internal_receiver.len(), 8);
    assert!(!client.is_internal_tool_input_logging_enabled());
    let AnalyticsEventsQueueMessage::InternalFact(fact, identity) =
        internal_receiver.try_recv().unwrap()
    else {
        unreachable!("tool inputs must enqueue an analytics fact")
    };
    assert!(client.is_internal_tool_input_logging_enabled());
    let AnalyticsFact::Custom(CustomAnalyticsFact::InternalToolInput(queued)) = fact.as_ref()
    else {
        unreachable!("tool inputs must contain an internal diagnostic")
    };
    assert_eq!(queued.as_ref(), &input);
    assert_eq!(
        identity,
        ("account-test".to_string(), "employee-alice".to_string())
    );
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

#[tokio::test]
#[cfg(debug_assertions)]
async fn internal_tool_inputs_remain_bound_to_the_authenticated_employee() {
    use codex_login::AuthCredentialsStoreMode as Store;

    let employee = internal_tool_input_auth(ALICE_TOKEN, "account-test");
    let identity = super::internal_tool_input_identity(&employee).unwrap();
    let input = internal_tool_input();
    let path = unique_capture_path("internal-tool-input");
    let home = unique_capture_path("internal-tool-auth-home");
    let auth_manager =
        codex_login::AuthManager::from_auth_for_testing_with_home(employee, home.clone());
    let client =
        AnalyticsEventsClient::new_for_capture_file(Arc::clone(&auth_manager), path.clone());
    client.track_internal_tool_input(input.clone());
    client.flush().await;

    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 1);

    let queue = client
        .internal_tool_input
        .as_ref()
        .unwrap()
        .queue
        .get()
        .unwrap();
    let refreshed = ALICE_TOKEN.replace(".sig", ".refreshed");
    for (access, id, account, deliver) in [
        (BOB_TOKEN, BOB_TOKEN, "account-test", false),
        (ALICE_TOKEN, ALICE_TOKEN, "other-workspace", false),
        (OUTSIDER_TOKEN, ALICE_TOKEN, "account-test", false),
        (ALICE_OTHER_USER_TOKEN, ALICE_TOKEN, "account-test", false),
        (refreshed.as_str(), refreshed.as_str(), "account-test", true),
    ] {
        let mut stored: codex_login::AuthDotJson = serde_json::from_value(serde_json::json!({
            "auth_mode": "chatgptAuthTokens",
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": id,
                "access_token": access,
                "refresh_token": "",
                "account_id": account,
            },
        }))
        .unwrap();
        stored.last_refresh = Some(SystemTime::now().into());
        codex_login::save_auth(&home, &stored, Store::File, Default::default()).unwrap();
        auth_manager.reload().await;
        queue
            .sender
            .try_send(AnalyticsEventsQueueMessage::InternalFact(
                Box::new(input.clone().into()),
                identity.clone(),
            ))
            .unwrap();
        client.flush().await;
        assert_eq!(
            fs::read_to_string(&path).unwrap().lines().count(),
            1 + usize::from(deliver)
        );
    }

    fs::remove_file(home.join("auth.json")).unwrap();
    auth_manager.reload().await;
    queue
        .sender
        .try_send(AnalyticsEventsQueueMessage::InternalFact(
            Box::new(input.into()),
            identity,
        ))
        .unwrap();
    client.flush().await;
    assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);
    fs::remove_file(&path).unwrap();
    fs::remove_dir(home).unwrap();
}
// copybara:strip-for-public end
