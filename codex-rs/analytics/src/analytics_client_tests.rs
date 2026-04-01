use crate::client::AnalyticsEventsQueue;
use crate::events::AppServerRpcTransport;
use crate::events::CodexAppMentionedEventRequest;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexAppUsedEventRequest;
use crate::events::CodexPluginEventRequest;
use crate::events::CodexPluginUsedEventRequest;
use crate::events::CodexRuntimeMetadata;
use crate::events::ThreadInitializationMode;
use crate::events::ThreadInitializedEvent;
use crate::events::ThreadInitializedEventParams;
use crate::events::TrackEventRequest;
use crate::events::codex_app_metadata;
use crate::events::codex_plugin_metadata;
use crate::events::codex_plugin_used_metadata;
use crate::events::subagent_thread_started_event_request;
use crate::facts::AnalyticsFact;
use crate::facts::AppInvocation;
use crate::facts::AppMentionedInput;
use crate::facts::AppUsedInput;
use crate::facts::CustomAnalyticsFact;
use crate::facts::InvocationType;
use crate::facts::PluginState;
use crate::facts::PluginStateChangedInput;
use crate::facts::PluginUsedInput;
use crate::facts::SkillInvocation;
use crate::facts::SkillInvokedInput;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::TrackEventsContext;
use crate::reducer::AnalyticsReducer;
use crate::reducer::normalize_path_for_skill_id;
use crate::reducer::skill_id_for_local_skill;
use codex_app_server_protocol::ApprovalsReviewer as AppServerApprovalsReviewer;
use codex_app_server_protocol::AskForApproval as AppServerAskForApproval;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientResponse;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy as AppServerSandboxPolicy;
use codex_app_server_protocol::SessionSource as AppServerSessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStatus as AppServerThreadStatus;
use codex_login::default_client::DEFAULT_ORIGINATOR;
use codex_login::default_client::originator;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use codex_plugin::PluginId;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::protocol::SubAgentSource;
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
