use super::AnalyticsEventsQueue;
use super::AnalyticsFact;
use super::AnalyticsReducer;
use super::AppInvocation;
use super::CodexAppMentionedEventRequest;
use super::CodexAppUsedEventRequest;
use super::CodexPluginEventRequest;
use super::CodexPluginUsedEventRequest;
use super::CodexTurnEventRequest;
use super::CompletedTurnState;
use super::CustomAnalyticsFact;
use super::InitializationMode;
use super::InvocationType;
use super::ThreadInitializedInput;
use super::TrackEventRequest;
use super::TrackEventsContext;
use super::TurnResolvedConfigFact;
use super::TurnStatus;
use super::codex_app_metadata;
use super::codex_plugin_metadata;
use super::codex_plugin_used_metadata;
use super::codex_turn_event_params;
use super::normalize_path_for_skill_id;
use super::thread_initialized_event_request;
use codex_app_server_protocol::ApprovalsReviewer as AppServerApprovalsReviewer;
use codex_app_server_protocol::AskForApproval as AppServerAskForApproval;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy as AppServerSandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionSource as AppServerSessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus as AppServerThreadStatus;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnError as AppServerTurnError;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus as AppServerTurnStatus;
use codex_app_server_protocol::UserInput as AppServerUserInput;
use codex_login::default_client::originator;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use codex_plugin::PluginId;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;

fn sample_thread(thread_id: &str, ephemeral: bool) -> Thread {
    Thread {
        id: thread_id.to_string(),
        preview: "first prompt".to_string(),
        ephemeral,
        model_provider: "openai".to_string(),
        created_at: 1,
        updated_at: 2,
        status: AppServerThreadStatus::Idle,
        path: None,
        cwd: PathBuf::from("/tmp"),
        cli_version: "0.0.0".to_string(),
        source: AppServerSessionSource::Exec,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: None,
        turns: Vec::new(),
    }
}

fn sample_thread_start_response(thread_id: &str, ephemeral: bool, model: &str) -> ClientResponse {
    ClientResponse::ThreadStart {
        request_id: RequestId::Integer(1),
        response: ThreadStartResponse {
            thread: sample_thread(thread_id, ephemeral),
            model: model.to_string(),
            model_provider: "openai".to_string(),
            service_tier: None,
            cwd: PathBuf::from("/tmp"),
            approval_policy: AppServerAskForApproval::OnFailure,
            approvals_reviewer: AppServerApprovalsReviewer::User,
            sandbox: AppServerSandboxPolicy::DangerFullAccess,
            reasoning_effort: None,
        },
    }
}

fn sample_thread_resume_response(thread_id: &str, ephemeral: bool, model: &str) -> ClientResponse {
    ClientResponse::ThreadResume {
        request_id: RequestId::Integer(2),
        response: ThreadResumeResponse {
            thread: sample_thread(thread_id, ephemeral),
            model: model.to_string(),
            model_provider: "openai".to_string(),
            service_tier: None,
            cwd: PathBuf::from("/tmp"),
            approval_policy: AppServerAskForApproval::OnFailure,
            approvals_reviewer: AppServerApprovalsReviewer::User,
            sandbox: AppServerSandboxPolicy::DangerFullAccess,
            reasoning_effort: None,
        },
    }
}

fn sample_turn_start_request(thread_id: &str, request_id: i64) -> ClientRequest {
    ClientRequest::TurnStart {
        request_id: RequestId::Integer(request_id),
        params: TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![
                AppServerUserInput::Text {
                    text: "hello".to_string(),
                    text_elements: vec![],
                },
                AppServerUserInput::Image {
                    url: "https://example.com/a.png".to_string(),
                },
            ],
            ..Default::default()
        },
    }
}

fn sample_turn_start_response(turn_id: &str, request_id: i64) -> ClientResponse {
    ClientResponse::TurnStart {
        request_id: RequestId::Integer(request_id),
        response: codex_app_server_protocol::TurnStartResponse {
            turn: Turn {
                id: turn_id.to_string(),
                items: vec![],
                status: AppServerTurnStatus::InProgress,
                error: None,
            },
        },
    }
}

fn sample_turn_started_notification(thread_id: &str, turn_id: &str) -> ServerNotification {
    ServerNotification::TurnStarted(TurnStartedNotification {
        thread_id: thread_id.to_string(),
        turn: Turn {
            id: turn_id.to_string(),
            items: vec![],
            status: AppServerTurnStatus::InProgress,
            error: None,
        },
    })
}

fn sample_turn_completed_notification(
    thread_id: &str,
    turn_id: &str,
    status: AppServerTurnStatus,
    codex_error_info: Option<codex_app_server_protocol::CodexErrorInfo>,
) -> ServerNotification {
    ServerNotification::TurnCompleted(TurnCompletedNotification {
        thread_id: thread_id.to_string(),
        turn: Turn {
            id: turn_id.to_string(),
            items: vec![],
            status,
            error: codex_error_info.map(|codex_error_info| AppServerTurnError {
                message: "turn failed".to_string(),
                codex_error_info: Some(codex_error_info),
                additional_details: None,
            }),
        },
    })
}

fn sample_turn_resolved_config(turn_id: &str) -> TurnResolvedConfigFact {
    TurnResolvedConfigFact {
        turn_id: turn_id.to_string(),
        thread_id: "thread-2".to_string(),
        num_input_images: 1,
        submission_type: None,
        model: "gpt-5".to_string(),
        model_provider: "openai".to_string(),
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        reasoning_effort: None,
        reasoning_summary: None,
        service_tier: None,
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::GuardianSubagent,
        sandbox_network_access: true,
        collaboration_mode: ModeKind::Plan,
        personality: None,
        is_first_turn: true,
    }
}

async fn ingest_turn_prerequisites(
    reducer: &mut AnalyticsReducer,
    out: &mut Vec<TrackEventRequest>,
    include_initialize: bool,
    include_resolved_config: bool,
    include_started: bool,
) {
    if include_initialize {
        reducer
            .ingest(
                AnalyticsFact::Initialize {
                    connection_id: 7,
                    params: InitializeParams {
                        client_info: ClientInfo {
                            name: "codex-tui".to_string(),
                            title: None,
                            version: "1.0.0".to_string(),
                        },
                        capabilities: None,
                    },
                },
                out,
            )
            .await;
    }

    reducer
        .ingest(
            AnalyticsFact::Request {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                request: Box::new(sample_turn_start_request("thread-2", 3)),
            },
            out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Response {
                connection_id: 7,
                response: Box::new(sample_turn_start_response("turn-2", 3)),
            },
            out,
        )
        .await;

    if include_resolved_config {
        reducer
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                    sample_turn_resolved_config("turn-2"),
                ))),
                out,
            )
            .await;
    }

    if include_started {
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                    "thread-2", "turn-2",
                ))),
                out,
            )
            .await;
    }
}

fn expected_absolute_path(path: &PathBuf) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn normalize_path_for_skill_id_repo_scoped_uses_relative_path() {
    let repo_root = PathBuf::from("/repo/root");
    let skill_path = PathBuf::from("/repo/root/.codex/skills/doc/SKILL.md");

    let path = normalize_path_for_skill_id(
        Some("https://example.com/repo.git"),
        Some(repo_root.as_path()),
        skill_path.as_path(),
    );

    assert_eq!(path, ".codex/skills/doc/SKILL.md");
}

#[test]
fn normalize_path_for_skill_id_user_scoped_uses_absolute_path() {
    let skill_path = PathBuf::from("/Users/abc/.codex/skills/doc/SKILL.md");

    let path = normalize_path_for_skill_id(None, None, skill_path.as_path());
    let expected = expected_absolute_path(&skill_path);

    assert_eq!(path, expected);
}

#[test]
fn normalize_path_for_skill_id_admin_scoped_uses_absolute_path() {
    let skill_path = PathBuf::from("/etc/codex/skills/doc/SKILL.md");

    let path = normalize_path_for_skill_id(None, None, skill_path.as_path());
    let expected = expected_absolute_path(&skill_path);

    assert_eq!(path, expected);
}

#[test]
fn normalize_path_for_skill_id_repo_root_not_in_skill_path_uses_absolute_path() {
    let repo_root = PathBuf::from("/repo/root");
    let skill_path = PathBuf::from("/other/path/.codex/skills/doc/SKILL.md");

    let path = normalize_path_for_skill_id(
        Some("https://example.com/repo.git"),
        Some(repo_root.as_path()),
        skill_path.as_path(),
    );
    let expected = expected_absolute_path(&skill_path);

    assert_eq!(path, expected);
}

#[test]
fn app_mentioned_event_serializes_expected_shape() {
    let tracking = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
    };
    let event = TrackEventRequest::AppMentioned(CodexAppMentionedEventRequest {
        event_type: "codex_app_mentioned",
        event_params: codex_app_metadata(
            &tracking,
            AppInvocation {
                connector_id: Some("calendar".to_string()),
                app_name: Some("Calendar".to_string()),
                invocation_type: Some(InvocationType::Explicit),
            },
        ),
    });

    let payload = serde_json::to_value(&event).expect("serialize app mentioned event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_app_mentioned",
            "event_params": {
                "connector_id": "calendar",
                "thread_id": "thread-1",
                "turn_id": "turn-1",
                "app_name": "Calendar",
                "product_client_id": originator().value,
                "invoke_type": "explicit",
                "model_slug": "gpt-5"
            }
        })
    );
}

#[test]
fn app_used_event_serializes_expected_shape() {
    let tracking = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-2".to_string(),
        turn_id: "turn-2".to_string(),
    };
    let event = TrackEventRequest::AppUsed(CodexAppUsedEventRequest {
        event_type: "codex_app_used",
        event_params: codex_app_metadata(
            &tracking,
            AppInvocation {
                connector_id: Some("drive".to_string()),
                app_name: Some("Google Drive".to_string()),
                invocation_type: Some(InvocationType::Implicit),
            },
        ),
    });

    let payload = serde_json::to_value(&event).expect("serialize app used event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_app_used",
            "event_params": {
                "connector_id": "drive",
                "thread_id": "thread-2",
                "turn_id": "turn-2",
                "app_name": "Google Drive",
                "product_client_id": originator().value,
                "invoke_type": "implicit",
                "model_slug": "gpt-5"
            }
        })
    );
}

#[test]
fn app_used_dedupe_is_keyed_by_turn_and_connector() {
    let (sender, _receiver) = mpsc::channel(1);
    let queue = AnalyticsEventsQueue {
        sender,
        app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
        plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
    };
    let app = AppInvocation {
        connector_id: Some("calendar".to_string()),
        app_name: Some("Calendar".to_string()),
        invocation_type: Some(InvocationType::Implicit),
    };

    let turn_1 = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
    };
    let turn_2 = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-1".to_string(),
        turn_id: "turn-2".to_string(),
    };

    assert_eq!(queue.should_enqueue_app_used(&turn_1, &app), true);
    assert_eq!(queue.should_enqueue_app_used(&turn_1, &app), false);
    assert_eq!(queue.should_enqueue_app_used(&turn_2, &app), true);
}

#[test]
fn turn_event_serializes_expected_shape() {
    let event = TrackEventRequest::TurnEvent(Box::new(CodexTurnEventRequest {
        event_type: "codex_turn_event",
        event_params: codex_turn_event_params(
            "codex-tui".to_string(),
            "thread-2".to_string(),
            "turn-2".to_string(),
            2,
            TurnResolvedConfigFact {
                turn_id: "turn-2".to_string(),
                thread_id: "thread-2".to_string(),
                num_input_images: 2,
                submission_type: None,
                model: "gpt-5".to_string(),
                model_provider: "openai".to_string(),
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                reasoning_effort: Some(ReasoningEffort::High),
                reasoning_summary: Some(ReasoningSummary::Detailed),
                service_tier: Some(ServiceTier::Flex),
                approval_policy: AskForApproval::OnRequest,
                approvals_reviewer: ApprovalsReviewer::GuardianSubagent,
                sandbox_network_access: true,
                collaboration_mode: ModeKind::Plan,
                personality: Some(Personality::Pragmatic),
                is_first_turn: true,
            },
            CompletedTurnState {
                status: Some(TurnStatus::Completed),
                turn_error: None,
                completed_at_secs: 456,
                duration_ms: Some(1234),
            },
            Some(455),
        ),
    }));

    let payload = serde_json::to_value(&event).expect("serialize turn event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_turn_event",
            "event_params": {
                "thread_id": "thread-2",
                "turn_id": "turn-2",
                "product_client_id": "codex-tui",
                "submission_type": null,
                "model": "gpt-5",
                "model_provider": "openai",
                "sandbox_policy": "read_only",
                "reasoning_effort": "high",
                "reasoning_summary": "detailed",
                "service_tier": "flex",
                "approval_policy": "on-request",
                "approvals_reviewer": "guardian_subagent",
                "sandbox_network_access": true,
                "collaboration_mode": "plan",
                "personality": "pragmatic",
                "num_input_images": 2,
                "is_first_turn": true,
                "status": "completed",
                "turn_error": null,
                "steer_count": null,
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
                "output_tokens": null,
                "reasoning_output_tokens": null,
                "total_tokens": null,
                "duration_ms": 1234,
                "started_at": 455,
                "completed_at": 456
            }
        })
    );
}

#[tokio::test]
async fn turn_lifecycle_emits_turn_event() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Initialize {
                connection_id: 7,
                params: InitializeParams {
                    client_info: ClientInfo {
                        name: "codex-tui".to_string(),
                        title: None,
                        version: "1.0.0".to_string(),
                    },
                    capabilities: None,
                },
            },
            &mut out,
        )
        .await;

    assert!(out.is_empty());

    reducer
        .ingest(
            AnalyticsFact::Request {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                request: Box::new(sample_turn_start_request("thread-2", 3)),
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Response {
                connection_id: 7,
                response: Box::new(sample_turn_start_response("turn-2", 3)),
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                TurnResolvedConfigFact {
                    turn_id: "turn-2".to_string(),
                    thread_id: "thread-2".to_string(),
                    num_input_images: 2,
                    submission_type: None,
                    model: "gpt-5".to_string(),
                    model_provider: "openai".to_string(),
                    sandbox_policy: SandboxPolicy::new_read_only_policy(),
                    reasoning_effort: Some(ReasoningEffort::High),
                    reasoning_summary: Some(ReasoningSummary::Detailed),
                    service_tier: Some(ServiceTier::Flex),
                    approval_policy: AskForApproval::OnRequest,
                    approvals_reviewer: ApprovalsReviewer::GuardianSubagent,
                    sandbox_network_access: true,
                    collaboration_mode: ModeKind::Plan,
                    personality: Some(Personality::Pragmatic),
                    is_first_turn: true,
                },
            ))),
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                "thread-2", "turn-2",
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
                None,
            ))),
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(payload["event_type"], json!("codex_turn_event"));
    assert_eq!(payload["event_params"]["thread_id"], json!("thread-2"));
    assert_eq!(payload["event_params"]["turn_id"], json!("turn-2"));
    assert_eq!(
        payload["event_params"]["product_client_id"],
        json!("codex-tui")
    );
    assert_eq!(payload["event_params"]["num_input_images"], json!(1));
    assert_eq!(payload["event_params"]["status"], json!("completed"));
    assert!(payload["event_params"]["started_at"].as_u64().is_some());
    assert!(payload["event_params"]["completed_at"].as_u64().is_some());
}

#[tokio::test]
async fn turn_does_not_emit_without_required_prerequisites() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(&mut reducer, &mut out, false, true, false).await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                None,
            ))),
            &mut out,
        )
        .await;
    assert!(out.is_empty());

    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(&mut reducer, &mut out, true, false, false).await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                None,
            ))),
            &mut out,
        )
        .await;
    assert!(out.is_empty());
}

#[tokio::test]
async fn turn_completed_without_started_notification_emits_null_started_at() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    ingest_turn_prerequisites(&mut reducer, &mut out, true, true, false).await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Completed,
                None,
            ))),
            &mut out,
        )
        .await;

    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(payload["event_params"]["started_at"], json!(null));
    assert_eq!(payload["event_params"]["duration_ms"], json!(null));
}

#[tokio::test]
async fn turn_completed_maps_completion_variants() {
    for (status, codex_error_info, expected_status, expected_turn_error) in [
        (
            AppServerTurnStatus::Failed,
            Some(codex_app_server_protocol::CodexErrorInfo::BadRequest),
            json!("failed"),
            Some(json!("bad_request")),
        ),
        (
            AppServerTurnStatus::Interrupted,
            None,
            json!("interrupted"),
            None,
        ),
    ] {
        let mut reducer = AnalyticsReducer::default();
        let mut out = Vec::new();

        ingest_turn_prerequisites(&mut reducer, &mut out, true, true, false).await;
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                    "thread-2",
                    "turn-2",
                    status,
                    codex_error_info,
                ))),
                &mut out,
            )
            .await;

        let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
        assert_eq!(payload["event_params"]["status"], expected_status);
        assert_eq!(
            payload["event_params"]["turn_error"],
            expected_turn_error.unwrap_or(json!(null))
        );
    }
}

#[test]
fn thread_initialized_event_serializes_expected_shape() {
    let event = TrackEventRequest::ThreadInitialized(thread_initialized_event_request(
        &super::ConnectionState {
            product_client_id: "codex-tui".to_string(),
            client_name: Some("codex-tui".to_string()),
            client_version: Some("1.0.0".to_string()),
            experimental_api_enabled: Some(true),
        },
        ThreadInitializedInput {
            connection_id: 1,
            thread_id: "thread-0".to_string(),
            model: "gpt-5".to_string(),
            ephemeral: true,
            session_source: SessionSource::Exec,
            initialization_mode: InitializationMode::New,
            created_at: 1,
        },
    ));

    let payload = serde_json::to_value(&event).expect("serialize thread initialized event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_thread_initialized",
            "event_params": {
                "thread_id": "thread-0",
                "product_client_id": "codex-tui",
                "client_name": "codex-tui",
                "client_version": "1.0.0",
                "experimental_api_enabled": true,
                "model": "gpt-5",
                "ephemeral": true,
                "session_source": "user",
                "initialization_mode": "new",
                "subagent_source": null,
                "parent_thread_id": null,
                "created_at": 1
            }
        })
    );
}

#[tokio::test]
async fn initialize_caches_client_and_thread_lifecycle_publishes_once_initialized() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Response {
                connection_id: 7,
                response: Box::new(sample_thread_start_response(
                    "thread-no-client",
                    false,
                    "gpt-5",
                )),
            },
            &mut events,
        )
        .await;
    assert!(events.is_empty(), "thread events should require initialize");

    reducer
        .ingest(
            AnalyticsFact::Initialize {
                connection_id: 7,
                params: InitializeParams {
                    client_info: ClientInfo {
                        name: "codex-tui".to_string(),
                        title: None,
                        version: "1.0.0".to_string(),
                    },
                    capabilities: None,
                },
            },
            &mut events,
        )
        .await;
    assert!(events.is_empty(), "initialize should not publish by itself");

    reducer
        .ingest(
            AnalyticsFact::Response {
                connection_id: 7,
                response: Box::new(sample_thread_resume_response("thread-1", true, "gpt-5")),
            },
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_type"], "codex_thread_initialized");
    assert_eq!(payload[0]["event_params"]["product_client_id"], "codex-tui");
    assert_eq!(payload[0]["event_params"]["initialization_mode"], "resumed");
    assert_eq!(payload[0]["event_params"]["session_source"], "user");
    assert_eq!(payload[0]["event_params"]["subagent_source"], json!(null));
    assert_eq!(payload[0]["event_params"]["parent_thread_id"], json!(null));
}

#[test]
fn plugin_used_event_serializes_expected_shape() {
    let tracking = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-3".to_string(),
        turn_id: "turn-3".to_string(),
    };
    let event = TrackEventRequest::PluginUsed(CodexPluginUsedEventRequest {
        event_type: "codex_plugin_used",
        event_params: codex_plugin_used_metadata(&tracking, sample_plugin_metadata()),
    });

    let payload = serde_json::to_value(&event).expect("serialize plugin used event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_plugin_used",
            "event_params": {
                "plugin_id": "sample@test",
                "plugin_name": "sample",
                "marketplace_name": "test",
                "has_skills": true,
                "mcp_server_count": 2,
                "connector_ids": ["calendar", "drive"],
                "product_client_id": originator().value,
                "thread_id": "thread-3",
                "turn_id": "turn-3",
                "model_slug": "gpt-5"
            }
        })
    );
}

#[test]
fn plugin_management_event_serializes_expected_shape() {
    let event = TrackEventRequest::PluginInstalled(CodexPluginEventRequest {
        event_type: "codex_plugin_installed",
        event_params: codex_plugin_metadata(sample_plugin_metadata()),
    });

    let payload = serde_json::to_value(&event).expect("serialize plugin installed event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_plugin_installed",
            "event_params": {
                "plugin_id": "sample@test",
                "plugin_name": "sample",
                "marketplace_name": "test",
                "has_skills": true,
                "mcp_server_count": 2,
                "connector_ids": ["calendar", "drive"],
                "product_client_id": originator().value
            }
        })
    );
}

#[test]
fn plugin_used_dedupe_is_keyed_by_turn_and_plugin() {
    let (sender, _receiver) = mpsc::channel(1);
    let queue = AnalyticsEventsQueue {
        sender,
        app_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
        plugin_used_emitted_keys: Arc::new(Mutex::new(HashSet::new())),
    };
    let plugin = sample_plugin_metadata();

    let turn_1 = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
    };
    let turn_2 = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-1".to_string(),
        turn_id: "turn-2".to_string(),
    };

    assert_eq!(queue.should_enqueue_plugin_used(&turn_1, &plugin), true);
    assert_eq!(queue.should_enqueue_plugin_used(&turn_1, &plugin), false);
    assert_eq!(queue.should_enqueue_plugin_used(&turn_2, &plugin), true);
}

fn sample_plugin_metadata() -> PluginTelemetryMetadata {
    PluginTelemetryMetadata {
        plugin_id: PluginId::parse("sample@test").expect("valid plugin id"),
        capability_summary: Some(PluginCapabilitySummary {
            config_name: "sample@test".to_string(),
            display_name: "sample".to_string(),
            description: None,
            has_skills: true,
            mcp_server_names: vec!["mcp-1".to_string(), "mcp-2".to_string()],
            app_connector_ids: vec![
                AppConnectorId("calendar".to_string()),
                AppConnectorId("drive".to_string()),
            ],
        }),
    }
}
