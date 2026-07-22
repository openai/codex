use super::AnalyticsEventsClient;
use super::AnalyticsEventsDestination;
use super::AnalyticsEventsQueue;
use super::AnalyticsEventsQueueMessage;
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
use crate::events::ToolItemTerminalStatus;
use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::InvocationType;
use codex_app_server_protocol::ApprovalsReviewer as AppServerApprovalsReviewer;
use codex_app_server_protocol::AskForApproval as AppServerAskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy as AppServerSandboxPolicy;
use codex_app_server_protocol::SessionSource as AppServerSessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus as AppServerThreadStatus;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
use codex_app_server_protocol::TurnSteerParams;
use codex_app_server_protocol::TurnSteerResponse;
#[cfg(debug_assertions)]
use codex_login::AuthManager;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use std::collections::HashSet;
#[cfg(debug_assertions)]
use std::fs;
#[cfg(debug_assertions)]
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
#[cfg(debug_assertions)]
use std::time::SystemTime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

#[cfg(debug_assertions)]
impl AnalyticsEventsClient {
    pub(crate) fn new_for_capture_file(auth_manager: Arc<AuthManager>, path: PathBuf) -> Self {
        Self {
            queue: Some(AnalyticsEventsQueue::new(
                auth_manager,
                AnalyticsEventsDestination::CaptureFile { path },
            )),
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
                line_fingerprints: Vec::new(),
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
            repo_url: None,
            thread_id: Some(thread_id.to_string()),
            turn_id: Some("turn-1".to_string()),
            invoke_type: Some(InvocationType::Explicit),
            model_slug: Some("gpt-5.1-codex".to_string()),
        },
    })
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
    (AnalyticsEventsClient { queue: Some(queue) }, receiver)
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

    send_track_events(
        &auth_manager,
        &destination,
        vec![
            sample_regular_track_event("non-plugin-skill"),
            sample_mcp_tool_call_event("non-plugin-mcp", /*plugin_id*/ None),
            sample_plugin_used_track_event("non-plugin-used", /*plugin_id*/ None),
            sample_accepted_line_fingerprint_event("other-event"),
            sample_plugin_used_track_event("plugin-used", Some("sample@test")),
            sample_skill_track_event("plugin-skill", Some("sample@test")),
            sample_mcp_tool_call_event("plugin-mcp", Some("sample@test")),
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
        is_pinned: false,
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

    let ignored_request = sample_thread_archive_request();
    client.track_request(
        /*connection_id*/ 7,
        RequestId::Integer(3),
        &ignored_request,
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
    ] {
        client.track_response(/*connection_id*/ 7, request_id, response);
        assert!(matches!(
            receiver.try_recv(),
            Ok(AnalyticsEventsQueueMessage::Fact(input))
                if matches!(*input, AnalyticsFact::ClientResponse { .. })
        ));
    }

    client.track_response(
        /*connection_id*/ 7,
        RequestId::Integer(6),
        ClientResponsePayload::ThreadArchive(ThreadArchiveResponse {}),
    );
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
    AnalyticsEventsClient::disabled().flush().await;
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
