//! Thread lifecycle, lineage, originator, and metadata event tests.

use crate::events::AppServerRpcTransport;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexRuntimeMetadata;
use crate::events::ThreadInitializedEvent;
use crate::events::ThreadInitializedEventParams;
use crate::events::TrackEventRequest;
use crate::events::subagent_thread_started_event_request;
use crate::facts::AnalyticsFact;
use crate::facts::CodeModeToolCallFact;
use crate::facts::CodeModeToolCallStatus;
use crate::facts::CodexCompactionEvent;
use crate::facts::CompactionImplementation;
use crate::facts::CompactionPhase;
use crate::facts::CompactionReason;
use crate::facts::CompactionStatus;
use crate::facts::CompactionStrategy;
use crate::facts::CompactionTrigger;
use crate::facts::CustomAnalyticsFact;
use crate::facts::SubAgentThreadStartedInput;
use crate::facts::ThreadInitializationMode;
use crate::facts::TurnResolvedConfigFact;
use crate::reducer::AnalyticsReducer;
use crate::tests::support::TEST_PRODUCT_CLIENT_ID;
use crate::tests::support::ingest_code_mode_facts;
use crate::tests::support::ingest_complete_child_turn;
use crate::tests::support::ingest_completed_command_execution_item;
use crate::tests::support::ingest_initialize;
use crate::tests::support::ingest_review_prerequisites;
use crate::tests::support::sample_command_execution_item;
use crate::tests::support::sample_initialize_fact;
use crate::tests::support::sample_thread_resume_response;
use crate::tests::support::sample_thread_resume_response_with_source;
use crate::tests::support::sample_thread_start_response;
use crate::tests::support::sample_turn_resolved_config;
use crate::tests::support::sample_turn_start_request;
use crate::tests::support::sample_turn_start_response;
use crate::tests::support::sample_turn_started_notification;
use crate::tests::support::test_turn_metadata;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionSource as AppServerSessionSource;
use codex_app_server_protocol::ThreadArchivedNotification;
use codex_app_server_protocol::ThreadSource as AppServerThreadSource;
use codex_app_server_protocol::ThreadUnarchivedNotification;
use codex_login::default_client::DEFAULT_ORIGINATOR;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadSource;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn thread_initialized_event_serializes_expected_shape() {
    let event = TrackEventRequest::ThreadInitialized(ThreadInitializedEvent {
        event_type: "codex_thread_initialized",
        event_params: ThreadInitializedEventParams {
            thread_id: "thread-0".to_string(),
            session_id: "session-thread-0".to_string(),
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
            is_worktree: Some(true),
            thread_source: Some(ThreadSource::Feature("automation".to_string())),
            initialization_mode: ThreadInitializationMode::New,
            subagent_source: None,
            parent_thread_id: None,
            forked_from_thread_id: None,
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
                "session_id": "session-thread-0",
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
                "is_worktree": true,
                "thread_source": "automation",
                "initialization_mode": "new",
                "subagent_source": null,
                "parent_thread_id": null,
                "forked_from_thread_id": null,
                "created_at": 1
            }
        })
    );
}

#[tokio::test]
async fn thread_initialized_classifies_validated_linked_worktrees() {
    let root = std::env::temp_dir().join(format!(
        "codex-analytics-worktree-{}",
        codex_protocol::ThreadId::new()
    ));
    let primary = root.join("primary");
    let linked = root.join("linked");
    let admin = primary.join(".git/worktrees/linked");
    std::fs::create_dir_all(&admin).expect("worktree administrative directory");
    std::fs::create_dir_all(&linked).expect("linked checkout");
    std::fs::write(primary.join(".git/HEAD"), "ref: refs/heads/main\n")
        .expect("primary repository HEAD");
    std::fs::write(admin.join("commondir"), "../..\n").expect("common directory");
    std::fs::write(
        admin.join("gitdir"),
        format!("{}\n", linked.join(".git").display()),
    )
    .expect("linked checkout backlink");
    std::fs::write(
        linked.join(".git"),
        format!("gitdir: {}\n", admin.display()),
    )
    .expect("linked checkout git file");

    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();
    ingest_initialize(&mut reducer, &mut events).await;
    for (cwd, expected) in [
        (primary.as_path(), json!(false)),
        (linked.as_path(), json!(true)),
        (root.as_path(), serde_json::Value::Null),
    ] {
        events.clear();
        let mut response =
            sample_thread_start_response("thread-1", /*ephemeral*/ false, "gpt-5");
        let ClientResponsePayload::ThreadStart(start) = &mut response else {
            panic!("expected thread/start response");
        };
        start.thread.cwd = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(cwd)
            .expect("absolute checkout path");
        reducer
            .ingest(
                AnalyticsFact::ClientResponse {
                    connection_id: 7,
                    request_id: RequestId::Integer(1),
                    response: Box::new(response),
                    thread_originator: None,
                },
                &mut events,
            )
            .await;
        let payload = serde_json::to_value(&events).expect("serialize thread event");
        assert_eq!(payload[0]["event_params"]["is_worktree"], expected);
    }

    std::fs::remove_dir_all(root).expect("remove test checkout");
}

#[tokio::test]
async fn initialize_caches_client_and_thread_lifecycle_publishes_once_initialized() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(1),
                response: Box::new(sample_thread_start_response(
                    "thread-no-client",
                    /*ephemeral*/ false,
                    "gpt-5",
                )),
                thread_originator: None,
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
                        explicit_gateway_oauth: false,
                        experimental_api: false,
                        request_attestation: false,
                        opt_out_notification_methods: None,
                        mcp_server_openai_form_elicitation: false,
                        extensions: None,
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
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(2),
                response: Box::new(sample_thread_resume_response(
                    "thread-1", /*ephemeral*/ true, "gpt-5",
                )),
                thread_originator: None,
            },
            &mut events,
        )
        .await;

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 1);
    assert_eq!(payload[0]["event_type"], "codex_thread_initialized");
    assert_eq!(payload[0]["event_params"]["session_id"], "session-thread-1");
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
}

#[tokio::test]
async fn thread_originator_overrides_shared_connection_across_thread_events() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(sample_initialize_fact(/*connection_id*/ 7), &mut events)
        .await;
    for (request_id, thread_id, thread_originator) in [
        (1, "thread-work", Some(TEST_PRODUCT_CLIENT_ID.to_string())),
        (2, "thread-default", None),
    ] {
        reducer
            .ingest(
                AnalyticsFact::ClientResponse {
                    connection_id: 7,
                    request_id: RequestId::Integer(request_id),
                    response: Box::new(sample_thread_start_response(
                        thread_id, /*ephemeral*/ false, "gpt-5",
                    )),
                    thread_originator,
                },
                &mut events,
            )
            .await;
    }

    let initialized = serde_json::to_value(&events).expect("serialize thread events");
    assert_eq!(
        initialized
            .as_array()
            .expect("thread events")
            .iter()
            .map(|event| {
                json!({
                    "thread_id": event["event_params"]["thread_id"],
                    "app_server_client": event["event_params"]["app_server_client"],
                })
            })
            .collect::<Vec<_>>(),
        vec![
            json!({
                "thread_id": "thread-work",
                "app_server_client": {
                    "product_client_id": TEST_PRODUCT_CLIENT_ID,
                    "client_name": "codex-tui",
                    "client_version": "1.0.0",
                    "rpc_transport": "websocket",
                    "experimental_api_enabled": false,
                },
            }),
            json!({
                "thread_id": "thread-default",
                "app_server_client": {
                    "product_client_id": DEFAULT_ORIGINATOR,
                    "client_name": "codex-tui",
                    "client_version": "1.0.0",
                    "rpc_transport": "websocket",
                    "experimental_api_enabled": false,
                },
            }),
        ]
    );

    events.clear();
    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                request: Box::new(sample_turn_start_request(
                    "thread-work",
                    /*request_id*/ 3,
                )),
            },
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                response: Box::new(sample_turn_start_response("turn-1")),
                thread_originator: None,
            },
            &mut events,
        )
        .await;
    ingest_completed_command_execution_item(&mut reducer, &mut events, "thread-work", "item-work")
        .await;
    ingest_complete_child_turn(&mut reducer, &mut events, "thread-work", "turn-1").await;
    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::Compaction(Box::new(
                CodexCompactionEvent {
                    thread_id: "thread-work".to_string(),
                    turn_id: "turn-compact".to_string(),
                    trigger: CompactionTrigger::Manual,
                    reason: CompactionReason::UserRequested,
                    implementation: CompactionImplementation::Responses,
                    phase: CompactionPhase::StandaloneTurn,
                    strategy: CompactionStrategy::Memento,
                    status: CompactionStatus::Completed,
                    codex_error_kind: None,
                    codex_error_http_status_code: None,
                    usage_limit_window_minutes: None,
                    active_context_tokens_before: 131_000,
                    active_context_tokens_after: 64_000,
                    retained_image_count: None,
                    compaction_summary_tokens: None,
                    cached_input_tokens: None,
                    cache_write_input_tokens: None,
                    started_at: 100,
                    completed_at: 101,
                    duration_ms: Some(1200),
                },
            ))),
            &mut events,
        )
        .await;

    let lifecycle = serde_json::to_value(&events).expect("serialize lifecycle events");
    assert_eq!(
        lifecycle
            .as_array()
            .expect("lifecycle events")
            .iter()
            .map(|event| {
                json!({
                    "event_type": event["event_type"],
                    "product_client_id":
                        event["event_params"]["app_server_client"]["product_client_id"],
                })
            })
            .collect::<Vec<_>>(),
        vec![
            json!({
                "event_type": "codex_command_execution_event",
                "product_client_id": TEST_PRODUCT_CLIENT_ID,
            }),
            json!({
                "event_type": "codex_turn_event",
                "product_client_id": TEST_PRODUCT_CLIENT_ID,
            }),
            json!({
                "event_type": "codex_compaction_event",
                "product_client_id": TEST_PRODUCT_CLIENT_ID,
            }),
        ]
    );

    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(4),
                response: Box::new(sample_thread_resume_response_with_source(
                    "thread-private-source",
                    /*ephemeral*/ false,
                    "gpt-5",
                    AppServerSessionSource::Exec,
                    Some(AppServerThreadSource::Feature(
                        "private customer feature label".to_string(),
                    )),
                    Some("019ee5cf-4d15-77a2-8023-01a9f79b6e7d".to_string()),
                )),
                thread_originator: Some(TEST_PRODUCT_CLIENT_ID.to_string()),
            },
            &mut events,
        )
        .await;
    events.clear();

    for notification in [
        ServerNotification::ThreadArchived(ThreadArchivedNotification {
            thread_id: "thread-work".to_string(),
        }),
        ServerNotification::ThreadUnarchived(ThreadUnarchivedNotification {
            thread_id: "thread-default".to_string(),
        }),
        ServerNotification::ThreadArchived(ThreadArchivedNotification {
            thread_id: "thread-private-source".to_string(),
        }),
        ServerNotification::ThreadUnarchived(ThreadUnarchivedNotification {
            thread_id: "thread-without-context".to_string(),
        }),
    ] {
        reducer
            .ingest(
                AnalyticsFact::Notification(Box::new(notification)),
                &mut events,
            )
            .await;
    }

    let mut archives = serde_json::to_value(&events).expect("serialize archive events");
    for event in archives.as_array_mut().expect("archive events") {
        assert!(event["event_params"]["occurred_at_ms"].is_u64());
        event["event_params"]
            .as_object_mut()
            .expect("archive event params")
            .remove("occurred_at_ms");
    }
    assert_eq!(
        archives,
        json!([
            {
                "event_type": "codex_thread_archive_event",
                "event_params": {
                    "thread_id": "thread-work",
                    "action": "archived",
                    "app_server_client": initialized[0]["event_params"]["app_server_client"],
                    "runtime": initialized[0]["event_params"]["runtime"],
                    "thread_source": "user",
                },
            },
            {
                "event_type": "codex_thread_archive_event",
                "event_params": {
                    "thread_id": "thread-default",
                    "action": "unarchived",
                    "app_server_client": initialized[1]["event_params"]["app_server_client"],
                    "runtime": initialized[1]["event_params"]["runtime"],
                    "thread_source": "user",
                },
            },
            {
                "event_type": "codex_thread_archive_event",
                "event_params": {
                    "thread_id": "thread-private-source",
                    "action": "archived",
                    "app_server_client": initialized[0]["event_params"]["app_server_client"],
                    "runtime": initialized[0]["event_params"]["runtime"],
                    "parent_thread_id": "019ee5cf-4d15-77a2-8023-01a9f79b6e7d",
                },
            },
            {
                "event_type": "codex_thread_archive_event",
                "event_params": {
                    "thread_id": "thread-without-context",
                    "action": "unarchived",
                },
            }
        ])
    );
}

#[test]
fn subagent_thread_started_review_serializes_expected_shape() {
    let event = TrackEventRequest::ThreadInitialized(subagent_thread_started_event_request(
        SubAgentThreadStartedInput {
            session_id: "session-root".to_string(),
            thread_id: "thread-review".to_string(),
            parent_thread_id: None,
            forked_from_thread_id: None,
            product_client_id: "codex-tui".to_string(),
            client_name: Some("codex-tui".to_string()),
            client_version: Some("1.0.0".to_string()),
            model: "gpt-5".to_string(),
            ephemeral: false,
            thread_source: Some(ThreadSource::Subagent),
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
    assert_eq!(
        payload["event_params"]["forked_from_thread_id"],
        json!(null)
    );
}

#[test]
fn subagent_thread_started_thread_spawn_serializes_thread_lineage() {
    let parent_thread_id =
        codex_protocol::ThreadId::from_string("11111111-1111-1111-1111-111111111111")
            .expect("valid thread id");
    let forked_from_thread_id =
        codex_protocol::ThreadId::from_string("22222222-2222-4222-8222-222222222222")
            .expect("valid thread id");
    let event = TrackEventRequest::ThreadInitialized(subagent_thread_started_event_request(
        SubAgentThreadStartedInput {
            session_id: "session-root".to_string(),
            thread_id: "thread-spawn".to_string(),
            parent_thread_id: Some(parent_thread_id.to_string()),
            forked_from_thread_id: Some(forked_from_thread_id.to_string()),
            product_client_id: "codex-tui".to_string(),
            client_name: Some("codex-tui".to_string()),
            client_version: Some("1.0.0".to_string()),
            model: "gpt-5".to_string(),
            ephemeral: true,
            thread_source: Some(ThreadSource::Subagent),
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
    assert_eq!(payload["event_params"]["thread_id"], "thread-spawn");
    assert_eq!(payload["event_params"]["thread_source"], "subagent");
    assert_eq!(payload["event_params"]["subagent_source"], "thread_spawn");
    assert_eq!(
        payload["event_params"]["parent_thread_id"],
        "11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(
        payload["event_params"]["forked_from_thread_id"],
        "22222222-2222-4222-8222-222222222222"
    );
    assert_eq!(payload["event_params"]["session_id"], "session-root");
}

#[test]
fn subagent_thread_started_memory_consolidation_serializes_expected_shape() {
    let event = TrackEventRequest::ThreadInitialized(subagent_thread_started_event_request(
        SubAgentThreadStartedInput {
            session_id: "session-root".to_string(),
            thread_id: "thread-memory".to_string(),
            parent_thread_id: None,
            forked_from_thread_id: None,
            product_client_id: "codex-tui".to_string(),
            client_name: Some("codex-tui".to_string()),
            client_version: Some("1.0.0".to_string()),
            model: "gpt-5".to_string(),
            ephemeral: false,
            thread_source: Some(ThreadSource::Subagent),
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
            session_id: "session-root".to_string(),
            thread_id: "thread-guardian".to_string(),
            parent_thread_id: None,
            forked_from_thread_id: None,
            product_client_id: "codex-tui".to_string(),
            client_name: Some("codex-tui".to_string()),
            client_version: Some("1.0.0".to_string()),
            model: "gpt-5".to_string(),
            ephemeral: false,
            thread_source: Some(ThreadSource::GuardianReview),
            subagent_source: SubAgentSource::Other("guardian".to_string()),
            created_at: 126,
        },
    ));

    let payload = serde_json::to_value(&event).expect("serialize other subagent event");
    assert_eq!(payload["event_params"]["thread_source"], "guardian_review");
    assert_eq!(payload["event_params"]["subagent_source"], "guardian");
    assert_eq!(payload["event_params"]["parent_thread_id"], json!(null));
}

#[test]
fn subagent_thread_started_other_serializes_explicit_parent_thread_id() {
    let parent_thread_id =
        codex_protocol::ThreadId::from_string("33333333-3333-4333-8333-333333333333")
            .expect("valid thread id");
    let event = TrackEventRequest::ThreadInitialized(subagent_thread_started_event_request(
        SubAgentThreadStartedInput {
            session_id: "session-root".to_string(),
            thread_id: "thread-guardian".to_string(),
            parent_thread_id: Some(parent_thread_id.to_string()),
            forked_from_thread_id: None,
            product_client_id: "codex-tui".to_string(),
            client_name: Some("codex-tui".to_string()),
            client_version: Some("1.0.0".to_string()),
            model: "gpt-5".to_string(),
            ephemeral: false,
            thread_source: Some(ThreadSource::GuardianReview),
            subagent_source: SubAgentSource::Other("guardian".to_string()),
            created_at: 126,
        },
    ));

    let payload = serde_json::to_value(&event).expect("serialize auto-review subagent event");
    assert_eq!(payload["event_params"]["subagent_source"], "guardian");
    assert_eq!(
        payload["event_params"]["parent_thread_id"],
        "33333333-3333-4333-8333-333333333333"
    );
}

#[tokio::test]
async fn subagent_thread_started_publishes_without_initialize() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::SubAgentThreadStarted(
                SubAgentThreadStartedInput {
                    session_id: "session-root".to_string(),
                    thread_id: "thread-review".to_string(),
                    parent_thread_id: None,
                    forked_from_thread_id: None,
                    product_client_id: "codex-tui".to_string(),
                    client_name: Some("codex-tui".to_string()),
                    client_version: Some("1.0.0".to_string()),
                    model: "gpt-5".to_string(),
                    ephemeral: false,
                    thread_source: Some(ThreadSource::Subagent),
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

#[tokio::test]
async fn subagent_tool_items_inherit_parent_connection_metadata() {
    let mut reducer = AnalyticsReducer::default();
    let mut events = Vec::new();

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::SubAgentThreadStarted(
                SubAgentThreadStartedInput {
                    session_id: "session-thread-1".to_string(),
                    thread_id: "thread-subagent".to_string(),
                    parent_thread_id: Some("thread-1".to_string()),
                    forked_from_thread_id: None,
                    product_client_id: "codex-tui".to_string(),
                    client_name: Some("codex-tui".to_string()),
                    client_version: Some("1.0.0".to_string()),
                    model: "gpt-5".to_string(),
                    ephemeral: false,
                    thread_source: Some(ThreadSource::Subagent),
                    subagent_source: SubAgentSource::Review,
                    created_at: 128,
                },
            )),
            &mut events,
        )
        .await;
    ingest_review_prerequisites(&mut reducer, &mut events).await;
    for (thread_id, turn_id, root_turn_id) in [
        ("thread-1", "turn-parent", "parent-current-root"),
        ("thread-subagent", "turn-subagent", "child-causal-root"),
    ] {
        reducer
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                    TurnResolvedConfigFact {
                        turn_metadata: test_turn_metadata(Some(root_turn_id)),
                        ..sample_turn_resolved_config(thread_id, turn_id)
                    },
                ))),
                &mut events,
            )
            .await;
    }
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                "thread-subagent",
                "turn-subagent",
            ))),
            &mut events,
        )
        .await;

    ingest_code_mode_facts(
        &mut reducer,
        &mut events,
        [CodeModeToolCallFact::SamplingResponseCompleted {
            thread_id: "thread-subagent".into(),
            turn_id: "turn-subagent".into(),
            response_id: "response-subagent".into(),
            tool_call_ids: vec!["item-1".into(), "exec-1".into()],
        }],
    )
    .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ItemStarted(
                ItemStartedNotification {
                    thread_id: "thread-subagent".to_string(),
                    turn_id: "turn-subagent".to_string(),
                    started_at_ms: 1_000,
                    item: sample_command_execution_item(
                        CommandExecutionStatus::InProgress,
                        /*exit_code*/ None,
                        /*duration_ms*/ None,
                    ),
                },
            ))),
            &mut events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ItemCompleted(
                ItemCompletedNotification {
                    thread_id: "thread-subagent".to_string(),
                    turn_id: "turn-subagent".to_string(),
                    completed_at_ms: 1_042,
                    item: sample_command_execution_item(
                        CommandExecutionStatus::Completed,
                        Some(0),
                        Some(42),
                    ),
                },
            ))),
            &mut events,
        )
        .await;

    ingest_code_mode_facts(
        &mut reducer,
        &mut events,
        [CodeModeToolCallFact::Completed {
            thread_id: "thread-subagent".into(),
            turn_id: "turn-subagent".into(),
            turn_metadata: test_turn_metadata(Some("child-causal-root")),
            call_id: "exec-1".into(),
            cell_id: None,
            tool_name: "exec".into(),
            started_at_ms: 1_000,
            completed_at_ms: 1_042,
            status: CodeModeToolCallStatus::Completed,
        }],
    )
    .await;
    reducer.flush(&mut events);

    let payload = serde_json::to_value(&events).expect("serialize events");
    assert_eq!(payload.as_array().expect("events array").len(), 2);
    assert_eq!(
        payload
            .as_array()
            .expect("events array")
            .iter()
            .map(|event| json!({
                "turn_id": event["event_params"]["turn_id"],
                "root_turn_id": event["event_params"]["root_turn_id"],
                "tool_event_type": event["event_params"]["tool_event_type"],
            }))
            .collect::<Vec<_>>(),
        vec![
            json!({"turn_id": "turn-subagent", "root_turn_id": "child-causal-root", "tool_event_type": "model_tool_call"}),
            json!({"turn_id": "turn-subagent", "root_turn_id": "child-causal-root", "tool_event_type": "model_tool_call"}),
        ]
    );
    assert_eq!(payload[0]["event_type"], "codex_command_execution_event");
    assert_eq!(payload[0]["event_params"]["thread_id"], "thread-subagent");
    assert_eq!(payload[0]["event_params"]["session_id"], "session-thread-1");
    assert_eq!(payload[0]["event_params"]["thread_source"], "subagent");
    assert_eq!(payload[0]["event_params"]["subagent_source"], "review");
    assert_eq!(payload[0]["event_params"]["parent_thread_id"], "thread-1");
    assert_eq!(
        payload[0]["event_params"]["app_server_client"]["client_name"],
        "codex-tui"
    );
    assert_eq!(payload[1]["event_type"], "codex_dynamic_tool_call_event");
    assert_eq!(payload[1]["event_params"]["parent_thread_id"], "thread-1");
}
