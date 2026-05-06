use super::AnalyticsEventsClient;
use super::AnalyticsEventsQueue;
use crate::facts::AnalyticsFact;
use codex_app_server_protocol::ApprovalsReviewer as AppServerApprovalsReviewer;
use codex_app_server_protocol::AskForApproval as AppServerAskForApproval;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::PermissionProfile as AppServerPermissionProfile;
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
use codex_protocol::models::PermissionProfile as CorePermissionProfile;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

fn client_with_receiver() -> (AnalyticsEventsClient, mpsc::Receiver<AnalyticsFact>) {
    let (sender, receiver) = mpsc::channel(8);
    let queue = AnalyticsEventsQueue {
        sender,
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
            input: Vec::new(),
            responsesapi_client_metadata: None,
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

fn sample_permission_profile() -> AppServerPermissionProfile {
    CorePermissionProfile::Disabled.into()
}

fn sample_thread_start_response() -> ClientResponsePayload {
    ClientResponsePayload::ThreadStart(ThreadStartResponse {
        thread: sample_thread("thread-1"),
        model: "gpt-5".to_string(),
        model_provider: "openai".to_string(),
        service_tier: None,
        cwd: test_path_buf("/tmp").abs(),
        instruction_sources: Vec::new(),
        approval_policy: AppServerAskForApproval::OnFailure,
        approvals_reviewer: AppServerApprovalsReviewer::User,
        sandbox: AppServerSandboxPolicy::DangerFullAccess,
        permission_profile: Some(sample_permission_profile()),
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
        instruction_sources: Vec::new(),
        approval_policy: AppServerAskForApproval::OnFailure,
        approvals_reviewer: AppServerApprovalsReviewer::User,
        sandbox: AppServerSandboxPolicy::DangerFullAccess,
        permission_profile: Some(sample_permission_profile()),
        active_permission_profile: None,
        reasoning_effort: None,
    })
}

fn sample_thread_fork_response() -> ClientResponsePayload {
    ClientResponsePayload::ThreadFork(ThreadForkResponse {
        thread: sample_thread("thread-3"),
        model: "gpt-5".to_string(),
        model_provider: "openai".to_string(),
        service_tier: None,
        cwd: test_path_buf("/tmp").abs(),
        instruction_sources: Vec::new(),
        approval_policy: AppServerAskForApproval::OnFailure,
        approvals_reviewer: AppServerApprovalsReviewer::User,
        sandbox: AppServerSandboxPolicy::DangerFullAccess,
        permission_profile: Some(sample_permission_profile()),
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
            Ok(AnalyticsFact::ClientRequest { .. })
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
            Ok(AnalyticsFact::ClientResponse { .. })
        ));
    }

    client.track_response(
        /*connection_id*/ 7,
        RequestId::Integer(6),
        ClientResponsePayload::ThreadArchive(ThreadArchiveResponse {}),
    );
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}
