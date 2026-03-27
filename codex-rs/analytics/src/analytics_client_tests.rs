use super::AnalyticsEventsQueue;
use super::AnalyticsFact;
use super::AnalyticsReducer;
use super::AppInvocation;
use super::CodexAppMentionedEventRequest;
use super::CodexAppUsedEventRequest;
use super::CodexPluginEventRequest;
use super::CodexPluginUsedEventRequest;
use super::InitializationMode;
use super::InvocationType;
use super::ThreadInitializedInput;
use super::TrackEventRequest;
use super::TrackEventsContext;
use super::codex_app_metadata;
use super::codex_plugin_metadata;
use super::codex_plugin_used_metadata;
use super::normalize_path_for_skill_id;
use super::thread_initialized_event_request;
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
use codex_login::default_client::originator;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use codex_plugin::PluginId;
use codex_plugin::PluginTelemetryMetadata;
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
            thread_source: SessionSource::Exec,
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
                    capabilities: Some(InitializeCapabilities {
                        experimental_api: false,
                        opt_out_notification_methods: None,
                    }),
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
    assert_eq!(payload[0]["event_params"]["client_name"], "codex-tui");
    assert_eq!(payload[0]["event_params"]["client_version"], "1.0.0");
    assert_eq!(
        payload[0]["event_params"]["experimental_api_enabled"],
        false
    );
    assert_eq!(payload[0]["event_params"]["initialization_mode"], "resumed");
    assert_eq!(payload[0]["event_params"]["thread_source"], "user");
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
