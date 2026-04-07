use crate::client::AnalyticsEventsQueue;
use crate::events::AppServerRpcTransport;
use crate::events::CodexAppMentionedEventRequest;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexAppUsedEventRequest;
use crate::events::CodexPluginEventRequest;
use crate::events::CodexPluginUsedEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::CodexTurnEventRequest;
use crate::events::CodexTurnSteerEventRequest;
use crate::events::ThreadInitializationMode;
use crate::events::ThreadInitializedEvent;
use crate::events::ThreadInitializedEventParams;
use crate::events::TrackEventRequest;
use crate::events::codex_app_metadata;
use crate::events::codex_plugin_metadata;
use crate::events::codex_plugin_used_metadata;
use crate::events::codex_turn_steer_event_params;
use crate::events::subagent_thread_started_event_request;
use crate::facts::AnalyticsFact;
use crate::facts::AppInvocation;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CodexTurnSteerEvent;
use crate::facts::CustomAnalyticsFact;
use crate::facts::InvocationType;
use crate::facts::PluginState;
use crate::facts::PluginStateChangedInput;
use crate::facts::PluginUsedInput;
use crate::facts::SkillInvocation;
use crate::facts::SkillInvokedInput;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::TrackEventsContext;
use crate::facts::TurnResolvedConfigFact;
use crate::facts::TurnStatus;
use crate::facts::TurnSteerInput;
use crate::facts::TurnSteerRejectionReason;
use crate::facts::TurnSteerResult;
use crate::facts::TurnSubmissionType;
use crate::facts::TurnTokenUsageFact;
use crate::reducer::AnalyticsReducer;
use crate::reducer::normalize_path_for_skill_id;
use crate::reducer::skill_id_for_local_skill;
use codex_app_server_protocol::ApprovalsReviewer as AppServerApprovalsReviewer;
use codex_app_server_protocol::AskForApproval as AppServerAskForApproval;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeCapabilities;
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
use codex_app_server_protocol::UserInput;
use codex_login::default_client::DEFAULT_ORIGINATOR;
use codex_login::default_client::originator;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use codex_plugin::PluginId;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ModeKind;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
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
        forked_from_id: None,
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
                UserInput::Text {
                    text: "hello".to_string(),
                    text_elements: vec![],
                },
                UserInput::Image {
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
                started_at: None,
                completed_at: None,
                duration_ms: None,
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
            started_at: Some(455),
            completed_at: None,
            duration_ms: None,
        },
    })
}

fn sample_turn_token_usage_fact(thread_id: &str, turn_id: &str) -> TurnTokenUsageFact {
    TurnTokenUsageFact {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        token_usage: TokenUsage {
            total_tokens: 321,
            input_tokens: 123,
            cached_input_tokens: 45,
            output_tokens: 140,
            reasoning_output_tokens: 13,
        },
    }
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
            started_at: None,
            completed_at: Some(456),
            duration_ms: Some(1234),
        },
    })
}

fn sample_turn_resolved_config(turn_id: &str) -> TurnResolvedConfigFact {
    TurnResolvedConfigFact {
        turn_id: turn_id.to_string(),
        thread_id: "thread-2".to_string(),
        num_input_images: 1,
        submission_type: Some(TurnSubmissionType::Default),
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

fn sample_app_server_client_metadata() -> CodexAppServerClientMetadata {
    CodexAppServerClientMetadata {
        product_client_id: "codex-tui".to_string(),
        client_name: Some("codex-tui".to_string()),
        client_version: Some("1.0.0".to_string()),
        rpc_transport: AppServerRpcTransport::Stdio,
        experimental_api_enabled: None,
    }
}

fn sample_runtime_metadata() -> CodexRuntimeMetadata {
    CodexRuntimeMetadata {
        codex_rs_version: "0.1.0".to_string(),
        runtime_os: "macos".to_string(),
        runtime_os_version: "15.3.1".to_string(),
        runtime_arch: "aarch64".to_string(),
    }
}

async fn ingest_turn_prerequisites(
    reducer: &mut AnalyticsReducer,
    out: &mut Vec<TrackEventRequest>,
    include_initialize: bool,
    include_resolved_config: bool,
    include_started: bool,
    include_token_usage: bool,
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
                    product_client_id: "codex-tui".to_string(),
                    runtime: CodexRuntimeMetadata {
                        codex_rs_version: "0.1.0".to_string(),
                        runtime_os: "macos".to_string(),
                        runtime_os_version: "15.3.1".to_string(),
                        runtime_arch: "aarch64".to_string(),
                    },
                    rpc_transport: AppServerRpcTransport::Stdio,
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
                request: Box::new(sample_turn_start_request("thread-2", /*request_id*/ 3)),
            },
            out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Response {
                connection_id: 7,
                response: Box::new(sample_turn_start_response("turn-2", /*request_id*/ 3)),
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

    if include_token_usage {
        reducer
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::TurnTokenUsage(Box::new(
                    sample_turn_token_usage_fact("thread-2", "turn-2"),
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

    let path = normalize_path_for_skill_id(
        /*repo_url*/ None,
        /*repo_root*/ None,
        skill_path.as_path(),
    );
    let expected = expected_absolute_path(&skill_path);

    assert_eq!(path, expected);
}

#[test]
fn normalize_path_for_skill_id_admin_scoped_uses_absolute_path() {
    let skill_path = PathBuf::from("/etc/codex/skills/doc/SKILL.md");

    let path = normalize_path_for_skill_id(
        /*repo_url*/ None,
        /*repo_root*/ None,
        skill_path.as_path(),
    );
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
fn thread_initialized_event_serializes_expected_shape() {
    let event = TrackEventRequest::ThreadInitialized(ThreadInitializedEvent {
        event_type: "codex_thread_initialized",
        event_params: ThreadInitializedEventParams {
            thread_id: "thread-0".to_string(),
            app_server_client: CodexAppServerClientMetadata {
                product_client_id: DEFAULT_ORIGINATOR.to_string(),
                client_name: Some("codex-tui".to_string()),
                client_version: Some("1.0.0".to_string()),
                rpc_transport: AppServerRpcTransport::Stdio,
                experimental_api_enabled: Some(true),
            },
            runtime: CodexRuntimeMetadata {
                codex_rs_version: "0.1.0".to_string(),
                runtime_os: "macos".to_string(),
                runtime_os_version: "15.3.1".to_string(),
                runtime_arch: "aarch64".to_string(),
            },
            model: "gpt-5".to_string(),
            ephemeral: true,
            thread_source: Some("user"),
            initialization_mode: ThreadInitializationMode::New,
            subagent_source: None,
            parent_thread_id: None,
            created_at: 1,
        },
    });

    let payload = serde_json::to_value(&event).expect("serialize thread initialized event");

    assert_eq!(
        payload,
        json!({
            "event_type": "codex_thread_initialized",
            "event_params": {
                "thread_id": "thread-0",
                "app_server_client": {
                    "product_client_id": DEFAULT_ORIGINATOR,
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
                "model": "gpt-5",
                "ephemeral": true,
                "thread_source": "user",
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
                    /*ephemeral*/ false,
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
                    capabilities: Some(InitializeCapabilities {
                        experimental_api: false,
                        opt_out_notification_methods: None,
                    }),
                },
                product_client_id: DEFAULT_ORIGINATOR.to_string(),
                runtime: CodexRuntimeMetadata {
                    codex_rs_version: "0.99.0".to_string(),
                    runtime_os: "linux".to_string(),
                    runtime_os_version: "24.04".to_string(),
                    runtime_arch: "x86_64".to_string(),
                },
                rpc_transport: AppServerRpcTransport::Websocket,
            },
            &mut events,
        )
        .await;
    assert!(events.is_empty(), "initialize should not publish by itself");

    reducer
        .ingest(
            AnalyticsFact::Response {
                connection_id: 7,
                response: Box::new(sample_thread_resume_response(
                    "thread-1", /*ephemeral*/ true, "gpt-5",
                )),
            },
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_type"], "codex_thread_initialized");
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["product_client_id"],
        DEFAULT_ORIGINATOR
    );
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["client_name"],
        "codex-tui"
    );
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["client_version"],
        "1.0.0"
    );
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["rpc_transport"],
        "websocket"
    );
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["experimental_api_enabled"],
        false
    );
    assert_eq!(
        payload[0]["event_params"]["runtime"]["codex_rs_version"],
        "0.99.0"
    );
    assert_eq!(payload[0]["event_params"]["runtime"]["runtime_os"], "linux");
    assert_eq!(
        payload[0]["event_params"]["runtime"]["runtime_os_version"],
        "24.04"
    );
    assert_eq!(
        payload[0]["event_params"]["runtime"]["runtime_arch"],
        "x86_64"
    );
    assert_eq!(payload[0]["event_params"]["initialization_mode"], "resumed");
    assert_eq!(payload[0]["event_params"]["thread_source"], "user");
    assert_eq!(payload[0]["event_params"]["subagent_source"], json!(null));
    assert_eq!(payload[0]["event_params"]["parent_thread_id"], json!(null));
}

#[test]
fn subagent_thread_started_review_serializes_expected_shape() {
    let event = TrackEventRequest::ThreadInitialized(subagent_thread_started_event_request(
        SubAgentThreadStartedInput {
            thread_id: "thread-review".to_string(),
            product_client_id: "codex-tui".to_string(),
            client_name: "codex-tui".to_string(),
            client_version: "1.0.0".to_string(),
            model: "gpt-5".to_string(),
            ephemeral: false,
            subagent_source: SubAgentSource::Review,
            created_at: 123,
        },
    ));

    let payload = serde_json::to_value(&event).expect("serialize review subagent event");
    assert_eq!(payload["event_params"]["thread_source"], "subagent");
    assert_eq!(
        payload["event_params"]["app_server_client"]["product_client_id"],
        "codex-tui"
    );
    assert_eq!(
        payload["event_params"]["app_server_client"]["client_name"],
        "codex-tui"
    );
    assert_eq!(
        payload["event_params"]["app_server_client"]["client_version"],
        "1.0.0"
    );
    assert_eq!(
        payload["event_params"]["app_server_client"]["rpc_transport"],
        "in_process"
    );
    assert_eq!(payload["event_params"]["created_at"], 123);
    assert_eq!(payload["event_params"]["initialization_mode"], "new");
    assert_eq!(payload["event_params"]["subagent_source"], "review");
    assert_eq!(payload["event_params"]["parent_thread_id"], json!(null));
}

#[test]
fn subagent_thread_started_thread_spawn_serializes_parent_thread_id() {
    let parent_thread_id =
        codex_protocol::ThreadId::from_string("11111111-1111-1111-1111-111111111111")
            .expect("valid thread id");
    let event = TrackEventRequest::ThreadInitialized(subagent_thread_started_event_request(
        SubAgentThreadStartedInput {
            thread_id: "thread-spawn".to_string(),
            product_client_id: "codex-tui".to_string(),
            client_name: "codex-tui".to_string(),
            client_version: "1.0.0".to_string(),
            model: "gpt-5".to_string(),
            ephemeral: true,
            subagent_source: SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            },
            created_at: 124,
        },
    ));

    let payload = serde_json::to_value(&event).expect("serialize thread spawn subagent event");
    assert_eq!(payload["event_params"]["thread_source"], "subagent");
    assert_eq!(payload["event_params"]["subagent_source"], "thread_spawn");
    assert_eq!(
        payload["event_params"]["parent_thread_id"],
        "11111111-1111-1111-1111-111111111111"
    );
}

#[test]
fn subagent_thread_started_memory_consolidation_serializes_expected_shape() {
    let event = TrackEventRequest::ThreadInitialized(subagent_thread_started_event_request(
        SubAgentThreadStartedInput {
            thread_id: "thread-memory".to_string(),
            product_client_id: "codex-tui".to_string(),
            client_name: "codex-tui".to_string(),
            client_version: "1.0.0".to_string(),
            model: "gpt-5".to_string(),
            ephemeral: false,
            subagent_source: SubAgentSource::MemoryConsolidation,
            created_at: 125,
        },
    ));

    let payload =
        serde_json::to_value(&event).expect("serialize memory consolidation subagent event");
    assert_eq!(
        payload["event_params"]["subagent_source"],
        "memory_consolidation"
    );
    assert_eq!(payload["event_params"]["parent_thread_id"], json!(null));
}

#[test]
fn subagent_thread_started_other_serializes_expected_shape() {
    let event = TrackEventRequest::ThreadInitialized(subagent_thread_started_event_request(
        SubAgentThreadStartedInput {
            thread_id: "thread-guardian".to_string(),
            product_client_id: "codex-tui".to_string(),
            client_name: "codex-tui".to_string(),
            client_version: "1.0.0".to_string(),
            model: "gpt-5".to_string(),
            ephemeral: false,
            subagent_source: SubAgentSource::Other("guardian".to_string()),
            created_at: 126,
        },
    ));

    let payload = serde_json::to_value(&event).expect("serialize other subagent event");
    assert_eq!(payload["event_params"]["subagent_source"], "guardian");
}

#[tokio::test]
async fn subagent_thread_started_publishes_without_initialize() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::SubAgentThreadStarted(
                SubAgentThreadStartedInput {
                    thread_id: "thread-review".to_string(),
                    product_client_id: "codex-tui".to_string(),
                    client_name: "codex-tui".to_string(),
                    client_version: "1.0.0".to_string(),
                    model: "gpt-5".to_string(),
                    ephemeral: false,
                    subagent_source: SubAgentSource::Review,
                    created_at: 127,
                },
            )),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_type"], "codex_thread_initialized");
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["product_client_id"],
        "codex-tui"
    );
    assert_eq!(payload[0]["event_params"]["thread_source"], "subagent");
    assert_eq!(payload[0]["event_params"]["subagent_source"], "review");
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

#[tokio::test]
async fn reducer_ingests_skill_invoked_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let tracking = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
    };
    let skill_path = PathBuf::from("/Users/abc/.codex/skills/doc/SKILL.md");
    let expected_skill_id = skill_id_for_local_skill(
        /*repo_url*/ None,
        /*repo_root*/ None,
        skill_path.as_path(),
        "doc",
    );

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::SkillInvoked(SkillInvokedInput {
                tracking,
                invocations: vec![SkillInvocation {
                    skill_name: "doc".to_string(),
                    skill_scope: codex_protocol::protocol::SkillScope::User,
                    skill_path,
                    invocation_type: InvocationType::Explicit,
                }],
            })),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        payload,
        json!([{
            "event_type": "skill_invocation",
            "skill_id": expected_skill_id,
            "skill_name": "doc",
            "event_params": {
                "product_client_id": originator().value,
                "skill_scope": "user",
                "repo_url": null,
                "thread_id": "thread-1",
                "invoke_type": "explicit",
                "model_slug": "gpt-5"
            }
        }])
    );
}

#[tokio::test]
async fn reducer_ingests_app_and_plugin_facts() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    let tracking = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
    };

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::AppMentioned(AppMentionedInput {
                tracking: tracking.clone(),
                mentions: vec![AppInvocation {
                    connector_id: Some("calendar".to_string()),
                    app_name: Some("Calendar".to_string()),
                    invocation_type: Some(InvocationType::Explicit),
                }],
            })),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::AppUsed(AppUsedInput {
                tracking: tracking.clone(),
                app: AppInvocation {
                    connector_id: Some("drive".to_string()),
                    app_name: Some("Drive".to_string()),
                    invocation_type: Some(InvocationType::Implicit),
                },
            })),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::PluginUsed(PluginUsedInput {
                tracking,
                plugin: sample_plugin_metadata(),
            })),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 3);
    assert_eq!(payload[0]["event_type"], "codex_app_mentioned");
    assert_eq!(payload[1]["event_type"], "codex_app_used");
    assert_eq!(payload[2]["event_type"], "codex_plugin_used");
}

#[tokio::test]
async fn reducer_ingests_plugin_state_changed_fact() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::PluginStateChanged(
                PluginStateChangedInput {
                    plugin: sample_plugin_metadata(),
                    state: PluginState::Disabled,
                },
            )),
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(
        payload,
        json!([{
            "event_type": "codex_plugin_disabled",
            "event_params": {
                "plugin_id": "sample@test",
                "plugin_name": "sample",
                "marketplace_name": "test",
                "has_skills": true,
                "mcp_server_count": 2,
                "connector_ids": ["calendar", "drive"],
                "product_client_id": originator().value
            }
        }])
    );
}

#[test]
fn turn_event_serializes_expected_shape() {
    let event = TrackEventRequest::TurnEvent(Box::new(CodexTurnEventRequest {
        event_type: "codex_turn_event",
        event_params: crate::events::CodexTurnEventParams {
            thread_id: "thread-2".to_string(),
            turn_id: "turn-2".to_string(),
            product_client_id: "codex-tui".to_string(),
            submission_type: Some(TurnSubmissionType::Default),
            model: Some("gpt-5".to_string()),
            model_provider: "openai".to_string(),
            sandbox_policy: Some("read_only"),
            reasoning_effort: Some("high".to_string()),
            reasoning_summary: Some("detailed".to_string()),
            service_tier: "flex".to_string(),
            approval_policy: "on-request".to_string(),
            approvals_reviewer: "guardian_subagent".to_string(),
            sandbox_network_access: true,
            collaboration_mode: Some("plan"),
            personality: Some("pragmatic".to_string()),
            num_input_images: 2,
            is_first_turn: true,
            status: Some(TurnStatus::Completed),
            turn_error: None,
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
            output_tokens: None,
            reasoning_output_tokens: None,
            total_tokens: None,
            duration_ms: Some(1234),
            started_at: Some(455),
            completed_at: Some(456),
        },
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
                "submission_type": "default",
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

#[test]
fn turn_steer_event_serializes_expected_shape() {
    let tracking = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-2".to_string(),
        turn_id: "turn-2".to_string(),
    };
    let event = TrackEventRequest::TurnSteer(CodexTurnSteerEventRequest {
        event_type: "codex_turn_steer_event",
        event_params: codex_turn_steer_event_params(
            sample_app_server_client_metadata(),
            sample_runtime_metadata(),
            &tracking,
            CodexTurnSteerEvent {
                expected_turn_id: Some("turn-2".to_string()),
                accepted_turn_id: Some("turn-2".to_string()),
                num_input_images: 2,
                result: TurnSteerResult::Accepted,
                rejection_reason: None,
                created_at: 1_716_000_123,
            },
        ),
    });

    let payload = serde_json::to_value(&event).expect("serialize turn steer event");

    assert_eq!(payload["event_type"], json!("codex_turn_steer_event"));
    assert_eq!(payload["event_params"]["thread_id"], json!("thread-2"));
    assert_eq!(payload["event_params"]["expected_turn_id"], json!("turn-2"));
    assert_eq!(payload["event_params"]["accepted_turn_id"], json!("turn-2"));
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
    assert_eq!(payload["event_params"]["num_input_images"], json!(2));
    assert_eq!(payload["event_params"]["result"], json!("accepted"));
    assert_eq!(payload["event_params"]["rejection_reason"], json!(null));
    assert_eq!(payload["event_params"]["started_at"], json!(1_716_000_123));
    assert!(payload["event_params"].get("created_at").is_none());
}

#[test]
fn rejected_turn_steer_event_serializes_expected_shape() {
    let tracking = TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: "thread-3".to_string(),
        turn_id: "turn-3".to_string(),
    };
    let event = TrackEventRequest::TurnSteer(CodexTurnSteerEventRequest {
        event_type: "codex_turn_steer_event",
        event_params: codex_turn_steer_event_params(
            sample_app_server_client_metadata(),
            sample_runtime_metadata(),
            &tracking,
            CodexTurnSteerEvent {
                expected_turn_id: Some("turn-expected".to_string()),
                accepted_turn_id: None,
                num_input_images: 1,
                result: TurnSteerResult::Rejected,
                rejection_reason: Some(TurnSteerRejectionReason::ExpectedTurnMismatch),
                created_at: 1_716_000_124,
            },
        ),
    });

    let payload = serde_json::to_value(&event).expect("serialize rejected turn steer event");

    assert_eq!(payload["event_type"], json!("codex_turn_steer_event"));
    assert_eq!(payload["event_params"]["thread_id"], json!("thread-3"));
    assert_eq!(
        payload["event_params"]["expected_turn_id"],
        json!("turn-expected")
    );
    assert_eq!(payload["event_params"]["accepted_turn_id"], json!(null));
    assert_eq!(
        payload["event_params"]["app_server_client"]["product_client_id"],
        json!("codex-tui")
    );
    assert_eq!(
        payload["event_params"]["runtime"]["codex_rs_version"],
        json!("0.1.0")
    );
    assert!(payload["event_params"].get("product_client_id").is_none());
    assert_eq!(payload["event_params"]["num_input_images"], json!(1));
    assert_eq!(payload["event_params"]["result"], json!("rejected"));
    assert_eq!(
        payload["event_params"]["rejection_reason"],
        json!("expected_turn_mismatch")
    );
    assert_eq!(payload["event_params"]["started_at"], json!(1_716_000_124));
    assert!(payload["event_params"].get("created_at").is_none());
}

#[tokio::test]
async fn turn_steer_uses_connection_metadata() {
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
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnSteer(TurnSteerInput {
                tracking: TrackEventsContext {
                    model_slug: "gpt-5".to_string(),
                    thread_id: "thread-2".to_string(),
                    turn_id: "turn-2".to_string(),
                },
                turn_steer: CodexTurnSteerEvent {
                    expected_turn_id: Some("turn-2".to_string()),
                    accepted_turn_id: Some("turn-2".to_string()),
                    num_input_images: 2,
                    result: TurnSteerResult::Accepted,
                    rejection_reason: None,
                    created_at: 1_716_000_125,
                },
            })),
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn steer event");
    assert_eq!(payload["event_type"], json!("codex_turn_steer_event"));
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
}

#[tokio::test]
async fn rejected_turn_steer_uses_thread_connection_metadata() {
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
                product_client_id: "codex-tui".to_string(),
                runtime: sample_runtime_metadata(),
                rpc_transport: AppServerRpcTransport::Stdio,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Response {
                connection_id: 7,
                response: Box::new(sample_thread_start_response(
                    "thread-2", /*ephemeral*/ false, "gpt-5",
                )),
            },
            &mut out,
        )
        .await;
    out.clear();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnSteer(TurnSteerInput {
                tracking: TrackEventsContext {
                    model_slug: String::new(),
                    thread_id: "thread-2".to_string(),
                    turn_id: String::new(),
                },
                turn_steer: CodexTurnSteerEvent {
                    expected_turn_id: None,
                    accepted_turn_id: None,
                    num_input_images: 1,
                    result: TurnSteerResult::Rejected,
                    rejection_reason: Some(TurnSteerRejectionReason::NoActiveTurn),
                    created_at: 1_716_000_126,
                },
            })),
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn steer event");
    assert_eq!(payload["event_type"], json!("codex_turn_steer_event"));
    assert_eq!(
        payload["event_params"]["app_server_client"]["product_client_id"],
        json!("codex-tui")
    );
    assert_eq!(
        payload["event_params"]["runtime"]["codex_rs_version"],
        json!("0.1.0")
    );
    assert_eq!(payload["event_params"]["result"], json!("rejected"));
    assert_eq!(
        payload["event_params"]["rejection_reason"],
        json!("no_active_turn")
    );
}

#[tokio::test]
async fn turn_steer_does_not_emit_without_connection_metadata() {
    let mut reducer = AnalyticsReducer::default();
    let mut out = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnSteer(TurnSteerInput {
                tracking: TrackEventsContext {
                    model_slug: "gpt-5".to_string(),
                    thread_id: "thread-2".to_string(),
                    turn_id: "turn-2".to_string(),
                },
                turn_steer: CodexTurnSteerEvent {
                    expected_turn_id: Some("turn-2".to_string()),
                    accepted_turn_id: None,
                    num_input_images: 1,
                    result: TurnSteerResult::Rejected,
                    rejection_reason: Some(TurnSteerRejectionReason::NoActiveTurn),
                    created_at: 1_716_000_126,
                },
            })),
            &mut out,
        )
        .await;

    assert!(out.is_empty());
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
    assert_eq!(payload["event_params"]["turn_id"], json!("turn-2"));
    assert_eq!(
        payload["event_params"]["product_client_id"],
        json!("codex-tui")
    );
    assert_eq!(payload["event_params"]["num_input_images"], json!(1));
    assert_eq!(payload["event_params"]["status"], json!("completed"));
    assert_eq!(payload["event_params"]["steer_count"], json!(0));
    assert_eq!(payload["event_params"]["started_at"], json!(455));
    assert_eq!(payload["event_params"]["completed_at"], json!(456));
    assert_eq!(payload["event_params"]["duration_ms"], json!(1234));
    assert_eq!(payload["event_params"]["input_tokens"], json!(123));
    assert_eq!(payload["event_params"]["cached_input_tokens"], json!(45));
    assert_eq!(payload["event_params"]["output_tokens"], json!(140));
    assert_eq!(
        payload["event_params"]["reasoning_output_tokens"],
        json!(13)
    );
    assert_eq!(payload["event_params"]["total_tokens"], json!(321));
}

#[tokio::test]
async fn accepted_steers_increment_turn_steer_count() {
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
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnSteer(TurnSteerInput {
                tracking: TrackEventsContext {
                    model_slug: "gpt-5".to_string(),
                    thread_id: "thread-2".to_string(),
                    turn_id: "turn-2".to_string(),
                },
                turn_steer: CodexTurnSteerEvent {
                    expected_turn_id: Some("turn-2".to_string()),
                    accepted_turn_id: Some("turn-2".to_string()),
                    num_input_images: 0,
                    result: TurnSteerResult::Accepted,
                    rejection_reason: None,
                    created_at: 1,
                },
            })),
            &mut out,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnSteer(TurnSteerInput {
                tracking: TrackEventsContext {
                    model_slug: "gpt-5".to_string(),
                    thread_id: "thread-2".to_string(),
                    turn_id: "turn-2".to_string(),
                },
                turn_steer: CodexTurnSteerEvent {
                    expected_turn_id: None,
                    accepted_turn_id: None,
                    num_input_images: 0,
                    result: TurnSteerResult::Rejected,
                    rejection_reason: Some(TurnSteerRejectionReason::NoActiveTurn),
                    created_at: 2,
                },
            })),
            &mut out,
        )
        .await;

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnSteer(TurnSteerInput {
                tracking: TrackEventsContext {
                    model_slug: "gpt-5".to_string(),
                    thread_id: "thread-2".to_string(),
                    turn_id: "turn-2".to_string(),
                },
                turn_steer: CodexTurnSteerEvent {
                    expected_turn_id: Some("turn-2".to_string()),
                    accepted_turn_id: Some("turn-2".to_string()),
                    num_input_images: 1,
                    result: TurnSteerResult::Accepted,
                    rejection_reason: None,
                    created_at: 3,
                },
            })),
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

    let turn_event = out
        .iter()
        .find(|event| matches!(event, TrackEventRequest::TurnEvent(_)))
        .expect("turn event should be emitted");
    let payload = serde_json::to_value(turn_event).expect("serialize turn event");
    assert_eq!(payload["event_params"]["steer_count"], json!(2));
}

#[tokio::test]
async fn queued_submission_type_emits_queued_turn_event() {
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
                product_client_id: "codex-tui".to_string(),
                runtime: CodexRuntimeMetadata {
                    codex_rs_version: "0.1.0".to_string(),
                    runtime_os: "macos".to_string(),
                    runtime_os_version: "15.3.1".to_string(),
                    runtime_arch: "aarch64".to_string(),
                },
                rpc_transport: AppServerRpcTransport::Stdio,
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Request {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                request: Box::new(sample_turn_start_request("thread-2", /*request_id*/ 3)),
            },
            &mut out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Response {
                connection_id: 7,
                response: Box::new(sample_turn_start_response("turn-2", /*request_id*/ 3)),
            },
            &mut out,
        )
        .await;
    let mut resolved_config = sample_turn_resolved_config("turn-2");
    resolved_config.submission_type = Some(TurnSubmissionType::Queued);
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                resolved_config,
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
                /*codex_error_info*/ None,
            ))),
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(payload["event_params"]["submission_type"], json!("queued"));
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
    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(
        payload["event_params"]["product_client_id"],
        json!(originator().value)
    );

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
}

#[tokio::test]
async fn turn_lifecycle_emits_interrupted_turn_event_without_error() {
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
            AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
                "thread-2",
                "turn-2",
                AppServerTurnStatus::Interrupted,
                /*codex_error_info*/ None,
            ))),
            &mut out,
        )
        .await;

    assert_eq!(out.len(), 1);
    let payload = serde_json::to_value(&out[0]).expect("serialize turn event");
    assert_eq!(payload["event_params"]["status"], json!("interrupted"));
    assert_eq!(payload["event_params"]["turn_error"], json!(null));
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
