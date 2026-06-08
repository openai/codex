use super::AnalyticsEventsClient;
use super::AnalyticsEventsQueue;
use super::track_event_request_batches;
use crate::LocalAnalyticsRecord;
use crate::LocalAnalyticsRecordType;
use crate::LocalResponsesApiTransport;
use crate::events::AppServerRpcTransport;
use crate::events::CodexAcceptedLineFingerprintsEventParams;
use crate::events::CodexAcceptedLineFingerprintsEventRequest;
use crate::events::SkillInvocationEventParams;
use crate::events::SkillInvocationEventRequest;
use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::InvocationType;
use crate::local_responses::AnalyticsQueueInput;
use codex_app_server_protocol::ApprovalsReviewer as AppServerApprovalsReviewer;
use codex_app_server_protocol::AskForApproval as AppServerAskForApproval;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
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
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

static NEXT_TEST_PATH_ID: AtomicU64 = AtomicU64::new(0);

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

fn sample_regular_track_event(thread_id: &str) -> TrackEventRequest {
    TrackEventRequest::SkillInvocation(SkillInvocationEventRequest {
        event_type: "skill_invocation",
        skill_id: format!("skill-{thread_id}"),
        skill_name: "doc".to_string(),
        event_params: SkillInvocationEventParams {
            product_client_id: None,
            skill_scope: None,
            plugin_id: None,
            repo_url: None,
            thread_id: Some(thread_id.to_string()),
            turn_id: Some("turn-1".to_string()),
            invoke_type: Some(InvocationType::Explicit),
            model_slug: Some("gpt-5.1-codex".to_string()),
        },
    })
}

fn client_with_receiver() -> (AnalyticsEventsClient, mpsc::Receiver<AnalyticsQueueInput>) {
    let (sender, receiver) = mpsc::channel(8);
    let queue = AnalyticsEventsQueue {
        sender,
        local_sink: None,
        app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
        plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
    };
    (AnalyticsEventsClient { queue: Some(queue) }, receiver)
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
        session_id: format!("session-{thread_id}"),
        forked_from_id: None,
        parent_thread_id: None,
        preview: "first prompt".to_string(),
        ephemeral: false,
        model_provider: "openai".to_string(),
        created_at: 1,
        updated_at: 2,
        status: AppServerThreadStatus::Idle,
        path: None,
        cwd: test_path_buf("/tmp").abs(),
        cli_version: "0.0.0".to_string(),
        source: AppServerSessionSource::Exec,
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
        approval_policy: AppServerAskForApproval::OnFailure,
        approvals_reviewer: AppServerApprovalsReviewer::User,
        sandbox: AppServerSandboxPolicy::DangerFullAccess,
        active_permission_profile: None,
        reasoning_effort: None,
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
        approval_policy: AppServerAskForApproval::OnFailure,
        approvals_reviewer: AppServerApprovalsReviewer::User,
        sandbox: AppServerSandboxPolicy::DangerFullAccess,
        active_permission_profile: None,
        reasoning_effort: None,
        initial_turns_page: None,
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
        approval_policy: AppServerAskForApproval::OnFailure,
        approvals_reviewer: AppServerApprovalsReviewer::User,
        sandbox: AppServerSandboxPolicy::DangerFullAccess,
        active_permission_profile: None,
        reasoning_effort: None,
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

#[tokio::test]
async fn local_sink_reduces_events_when_backend_analytics_are_disabled() {
    let path = test_sink_path("backend-disabled");
    let client = AnalyticsEventsClient::new_with_local_sink_path(
        AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test")),
        "https://example.invalid".to_string(),
        /*analytics_enabled*/ Some(false),
        Some(path.clone()),
    );

    assert!(client.queue.is_some());
    client.track_initialize(
        /*connection_id*/ 7,
        InitializeParams {
            client_info: ClientInfo {
                name: "codex-tui".to_string(),
                title: None,
                version: "1.0.0".to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: false,
                request_attestation: false,
                opt_out_notification_methods: None,
            }),
        },
        "codex".to_string(),
        AppServerRpcTransport::Stdio,
    );
    client.track_response(
        /*connection_id*/ 7,
        RequestId::Integer(1),
        sample_thread_start_response(),
    );

    let records = wait_for_local_records(&path, 1).await;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].payload["event_type"], "codex_thread_initialized");
    assert_eq!(records[0].thread_id.as_deref(), Some("thread-1"));
}

#[tokio::test]
async fn local_responses_capture_writes_reduced_terminal_record() {
    let path = test_sink_path("local-responses");
    let client = AnalyticsEventsClient::new_with_local_sink_path(
        AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test")),
        "https://example.invalid".to_string(),
        Some(false),
        Some(path.clone()),
    );
    let capture = client.local_responses_api_call_capture(
        "session-1".to_string(),
        "thread-1".to_string(),
        "turn-1".to_string(),
    );
    let attempt = capture.start_attempt(LocalResponsesApiTransport::Http, &json!({"model": "gpt"}));
    attempt.record_completed("response-1", Some("request-1"), &None, &[]);

    let records = wait_for_local_records(&path, 1).await;
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].record_type,
        LocalAnalyticsRecordType::ResponsesApiCall
    );
    assert_eq!(records[0].session_id.as_deref(), Some("session-1"));
    assert_eq!(records[0].payload["status"], "completed");
    assert_eq!(records[0].payload["request_json"], json!({"model": "gpt"}));
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
            Ok(AnalyticsQueueInput::AnalyticsFact(
                AnalyticsFact::ClientRequest { .. }
            ))
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
            Ok(AnalyticsQueueInput::AnalyticsFact(
                AnalyticsFact::ClientResponse { .. }
            ))
        ));
    }

    client.track_response(
        /*connection_id*/ 7,
        RequestId::Integer(6),
        ClientResponsePayload::ThreadArchive(ThreadArchiveResponse {}),
    );
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

async fn wait_for_local_records(path: &PathBuf, min_records: usize) -> Vec<LocalAnalyticsRecord> {
    for _ in 0..100 {
        if let Ok(contents) = fs::read_to_string(path) {
            let records = contents
                .lines()
                .map(|line| serde_json::from_str::<LocalAnalyticsRecord>(line).expect("record"))
                .collect::<Vec<_>>();
            if records.len() >= min_records {
                return records;
            }
        }
        tokio::task::yield_now().await;
    }

    panic!("timed out waiting for {min_records} local analytics records");
}

fn test_sink_path(label: &str) -> PathBuf {
    let id = NEXT_TEST_PATH_ID.fetch_add(1, Ordering::Relaxed);
    let process_id = std::process::id();
    let dir =
        std::env::temp_dir().join(format!("codex-analytics-client-{process_id}-{label}-{id}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir.join("events.jsonl")
}
