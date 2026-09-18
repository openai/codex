//! Shared analytics test facts, fixtures, and reducer setup.

use crate::events::AppServerRpcTransport;
use crate::events::CodexAppServerClientMetadata;
use crate::events::CodexRuntimeMetadata;
use crate::events::TrackEventRequest;
use crate::facts::AnalyticsFact;
use crate::facts::CodeModeToolCallFact;
use crate::facts::CustomAnalyticsFact;
use crate::facts::TrackEventsContext;
use crate::facts::TurnAnalyticsMetadata;
use crate::facts::TurnProfile;
use crate::facts::TurnProfileFact;
use crate::facts::TurnResolvedConfigFact;
use crate::facts::TurnTokenUsageFact;
use crate::reducer::AnalyticsReducer;
use codex_app_server_protocol::ApprovalsReviewer as AppServerApprovalsReviewer;
use codex_app_server_protocol::AskForApproval as AppServerAskForApproval;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ImageReference;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxPolicy as AppServerSandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionSource as AppServerSessionSource;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSource as AppServerThreadSource;
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
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use codex_plugin::PluginId;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::ModeKind;
use codex_protocol::models::PermissionProfile as CorePermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

pub(super) const TEST_PRODUCT_CLIENT_ID: &str = "codex_work_desktop";

pub(super) struct TestTurnMetadata {
    pub(super) root_turn_id: Mutex<Option<String>>,
}

impl TurnAnalyticsMetadata for TestTurnMetadata {
    fn root_turn_id(&self) -> Option<String> {
        self.root_turn_id.lock().expect("root turn ID").clone()
    }

    fn turn_trigger(&self) -> Option<String> {
        None
    }

    fn codex_turn_source(&self) -> Option<String> {
        None
    }
}

pub(super) fn test_turn_metadata(root_turn_id: Option<&str>) -> Arc<TestTurnMetadata> {
    Arc::new(TestTurnMetadata {
        root_turn_id: Mutex::new(root_turn_id.map(str::to_string)),
    })
}

pub(super) fn test_tracking_context(thread_id: &str, turn_id: &str) -> TrackEventsContext {
    TrackEventsContext {
        model_slug: "gpt-5".to_string(),
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        product_client_id: TEST_PRODUCT_CLIENT_ID.to_string(),
    }
}

pub(super) fn sample_thread_with_metadata(
    thread_id: &str,
    ephemeral: bool,
    source: AppServerSessionSource,
    thread_source: Option<AppServerThreadSource>,
    parent_thread_id: Option<String>,
) -> Thread {
    Thread {
        originator: None,
        environments: None,
        id: thread_id.to_string(),
        extra: None,
        session_id: format!("session-{thread_id}"),
        forked_from_id: None,
        parent_thread_id,
        preview: "first prompt".to_string(),
        ephemeral,
        section: None,
        section_entered_at: None,
        project_id: None,
        daybreak_enabled: None,
        history_mode: Default::default(),
        model_provider: "openai".to_string(),
        model: None,
        reasoning_effort: None,
        created_at: 1,
        updated_at: 2,
        recency_at: Some(2),
        status: AppServerThreadStatus::Idle,
        path: None,
        cwd: test_path_buf("/tmp").abs(),
        cli_version: "0.0.0".to_string(),
        source,
        can_accept_direct_input: None,
        thread_source,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: None,
        turns: Vec::new(),
    }
}

pub(super) fn sample_thread_start_response(
    thread_id: &str,
    ephemeral: bool,
    model: &str,
) -> ClientResponsePayload {
    ClientResponsePayload::ThreadStart(ThreadStartResponse {
        disabled_plugin_ids: Vec::new(),
        thread: sample_thread_with_metadata(
            thread_id,
            ephemeral,
            AppServerSessionSource::Exec,
            Some(AppServerThreadSource::User),
            /*parent_thread_id*/ None,
        ),
        model: model.to_string(),
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

pub(super) fn sample_app_server_client_metadata() -> CodexAppServerClientMetadata {
    CodexAppServerClientMetadata {
        product_client_id: DEFAULT_ORIGINATOR.to_string(),
        client_name: Some("codex-tui".to_string()),
        client_version: Some("1.0.0".to_string()),
        rpc_transport: AppServerRpcTransport::Stdio,
        experimental_api_enabled: Some(true),
    }
}

pub(super) fn sample_runtime_metadata() -> CodexRuntimeMetadata {
    CodexRuntimeMetadata {
        codex_rs_version: "0.1.0".to_string(),
        runtime_os: "macos".to_string(),
        runtime_os_version: "15.3.1".to_string(),
        runtime_arch: "aarch64".to_string(),
    }
}

pub(super) fn sample_thread_resume_response(
    thread_id: &str,
    ephemeral: bool,
    model: &str,
) -> ClientResponsePayload {
    sample_thread_resume_response_with_source(
        thread_id,
        ephemeral,
        model,
        AppServerSessionSource::Exec,
        Some(AppServerThreadSource::User),
        /*parent_thread_id*/ None,
    )
}

pub(super) fn sample_thread_resume_response_with_source(
    thread_id: &str,
    ephemeral: bool,
    model: &str,
    source: AppServerSessionSource,
    thread_source: Option<AppServerThreadSource>,
    parent_thread_id: Option<String>,
) -> ClientResponsePayload {
    ClientResponsePayload::ThreadResume(ThreadResumeResponse {
        disabled_plugin_ids: Vec::new(),
        thread: sample_thread_with_metadata(
            thread_id,
            ephemeral,
            source,
            thread_source,
            parent_thread_id,
        ),
        model: model.to_string(),
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
        collaboration_mode: None,
        multi_agent_mode: Default::default(),
        initial_turns_page: None,
        turns_backwards_cursor: None,
        items_backwards_cursor: None,
    })
}

pub(super) fn sample_turn_start_request(thread_id: &str, request_id: i64) -> ClientRequest {
    ClientRequest::TurnStart {
        request_id: RequestId::Integer(request_id),
        params: TurnStartParams {
            thread_id: thread_id.to_string(),
            client_user_message_id: None,
            input: vec![
                UserInput::Text {
                    text: "hello".to_string(),
                    text_elements: vec![],
                },
                UserInput::Image {
                    image: ImageReference::Inline {
                        url: "https://example.com/a.png".to_string(),
                    },
                    detail: None,
                },
            ],
            ..Default::default()
        },
    }
}

pub(super) fn sample_turn_start_response(turn_id: &str) -> ClientResponsePayload {
    ClientResponsePayload::TurnStart(codex_app_server_protocol::TurnStartResponse {
        turn: Turn {
            id: turn_id.to_string(),
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            items: vec![],
            status: AppServerTurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        },
    })
}

pub(super) fn sample_turn_started_notification(
    thread_id: &str,
    turn_id: &str,
) -> ServerNotification {
    ServerNotification::TurnStarted(TurnStartedNotification {
        thread_id: thread_id.to_string(),
        turn: Turn {
            id: turn_id.to_string(),
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            items: vec![],
            status: AppServerTurnStatus::InProgress,
            error: None,
            started_at: Some(455),
            completed_at: None,
            duration_ms: None,
        },
    })
}

pub(super) fn sample_turn_token_usage_fact(thread_id: &str, turn_id: &str) -> TurnTokenUsageFact {
    TurnTokenUsageFact {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        token_usage: TokenUsage {
            total_tokens: 321,
            input_tokens: 123,
            cached_input_tokens: 45,
            cache_write_input_tokens: 7,
            output_tokens: 140,
            reasoning_output_tokens: 13,
            codex_rollout_budget_units: None,
        },
    }
}

pub(super) fn sample_turn_completed_notification(
    thread_id: &str,
    turn_id: &str,
    status: AppServerTurnStatus,
    codex_error_info: Option<codex_app_server_protocol::CodexErrorInfo>,
) -> ServerNotification {
    ServerNotification::TurnCompleted(TurnCompletedNotification {
        thread_id: thread_id.to_string(),
        turn: Turn {
            id: turn_id.to_string(),
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            items: vec![],
            status,
            error: codex_error_info.map(|codex_error_info| AppServerTurnError {
                misalignment: None,
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

pub(super) fn sample_turn_resolved_config(
    thread_id: &str,
    turn_id: &str,
) -> TurnResolvedConfigFact {
    TurnResolvedConfigFact {
        turn_id: turn_id.to_string(),
        thread_id: thread_id.to_string(),
        turn_metadata: test_turn_metadata(/*root_turn_id*/ None),
        active_plugin_ids_at_turn_start: None,
        num_input_images: 1,
        submission_type: None,
        ephemeral: false,
        session_source: SessionSource::Exec,
        model: "gpt-5".to_string(),
        model_provider: "openai".to_string(),
        permission_profile: CorePermissionProfile::read_only(),
        permission_profile_cwd: PathBuf::from("/tmp"),
        reasoning_effort: None,
        reasoning_summary: None,
        service_tier: None,
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::AutoReview,
        guardian_v2_enabled: false,
        sandbox_network_access: true,
        collaboration_mode: ModeKind::Plan,
        personality: None,
        workspace_kind: None,
        is_first_turn: true,
    }
}

pub(super) fn sample_turn_profile() -> TurnProfile {
    TurnProfile {
        before_first_sampling_ms: 100,
        sampling_ms: 700,
        compaction_ms: 40,
        between_sampling_overhead_ms: 50,
        tool_blocking_ms: 250,
        after_last_sampling_ms: 94,
        sampling_request_count: 2,
        sampling_retry_count: 1,
    }
}

pub(super) async fn ingest_initialize(
    reducer: &mut AnalyticsReducer,
    out: &mut Vec<TrackEventRequest>,
) {
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
            out,
        )
        .await;
}

pub(super) async fn ingest_turn_prerequisites(
    reducer: &mut AnalyticsReducer,
    out: &mut Vec<TrackEventRequest>,
    include_initialize: bool,
    include_resolved_config: bool,
    include_started: bool,
    include_token_usage: bool,
) {
    if include_initialize {
        ingest_initialize(reducer, out).await;
        reducer
            .ingest(
                AnalyticsFact::ClientResponse {
                    connection_id: 7,
                    request_id: RequestId::Integer(1),
                    response: Box::new(sample_thread_start_response(
                        "thread-2", /*ephemeral*/ false, "gpt-5",
                    )),
                    thread_originator: None,
                },
                out,
            )
            .await;
        out.clear();
    }

    reducer
        .ingest(
            AnalyticsFact::ClientRequest {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                request: Box::new(sample_turn_start_request("thread-2", /*request_id*/ 3)),
            },
            out,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(3),
                response: Box::new(sample_turn_start_response("turn-2")),
                thread_originator: None,
            },
            out,
        )
        .await;

    if include_resolved_config {
        reducer
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
                    sample_turn_resolved_config("thread-2", "turn-2"),
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

    reducer
        .ingest(
            AnalyticsFact::Custom(CustomAnalyticsFact::TurnProfile(Box::new(
                TurnProfileFact {
                    turn_id: "turn-2".to_string(),
                    profile: sample_turn_profile(),
                },
            ))),
            out,
        )
        .await;
}

pub(super) async fn ingest_review_prerequisites(
    reducer: &mut AnalyticsReducer,
    events: &mut Vec<TrackEventRequest>,
) {
    reducer
        .ingest(sample_initialize_fact(/*connection_id*/ 7), events)
        .await;
    reducer
        .ingest(
            AnalyticsFact::ClientResponse {
                connection_id: 7,
                request_id: RequestId::Integer(1),
                response: Box::new(sample_thread_start_response(
                    "thread-1", /*ephemeral*/ false, "gpt-5",
                )),
                thread_originator: None,
            },
            events,
        )
        .await;
    events.clear();
}

pub(super) async fn ingest_completed_command_execution_item(
    reducer: &mut AnalyticsReducer,
    events: &mut Vec<TrackEventRequest>,
    thread_id: &str,
    item_id: &str,
) {
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(sample_turn_started_notification(
                thread_id, "turn-1",
            ))),
            events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ItemStarted(
                ItemStartedNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    started_at_ms: 1_000,
                    item: sample_command_execution_item_with_id(
                        item_id,
                        CommandExecutionStatus::InProgress,
                        /*exit_code*/ None,
                        /*duration_ms*/ None,
                    ),
                },
            ))),
            events,
        )
        .await;
    reducer
        .ingest(
            AnalyticsFact::Notification(Box::new(ServerNotification::ItemCompleted(
                ItemCompletedNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    completed_at_ms: 1_042,
                    item: sample_command_execution_item_with_id(
                        item_id,
                        CommandExecutionStatus::Completed,
                        Some(0),
                        Some(42),
                    ),
                },
            ))),
            events,
        )
        .await;
}

pub(super) fn sample_initialize_fact(connection_id: u64) -> AnalyticsFact {
    AnalyticsFact::Initialize {
        connection_id,
        params: InitializeParams {
            client_info: ClientInfo {
                name: "codex-tui".to_string(),
                title: None,
                version: "1.0.0".to_string(),
            },
            capabilities: Some(InitializeCapabilities {
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
    }
}

pub(super) async fn ingest_complete_child_turn(
    reducer: &mut AnalyticsReducer,
    events: &mut Vec<TrackEventRequest>,
    thread_id: &str,
    turn_id: &str,
) {
    for fact in [
        AnalyticsFact::Custom(CustomAnalyticsFact::TurnResolvedConfig(Box::new(
            sample_turn_resolved_config(thread_id, turn_id),
        ))),
        AnalyticsFact::Custom(CustomAnalyticsFact::TurnProfile(Box::new(
            TurnProfileFact {
                turn_id: turn_id.to_string(),
                profile: sample_turn_profile(),
            },
        ))),
        AnalyticsFact::Notification(Box::new(sample_turn_completed_notification(
            thread_id,
            turn_id,
            AppServerTurnStatus::Completed,
            /*codex_error_info*/ None,
        ))),
    ] {
        reducer.ingest(fact, events).await;
    }
}

pub(super) fn sample_command_execution_item(
    status: CommandExecutionStatus,
    exit_code: Option<i32>,
    duration_ms: Option<i64>,
) -> ThreadItem {
    sample_command_execution_item_with_id("item-1", status, exit_code, duration_ms)
}

pub(super) fn sample_command_execution_item_with_id(
    id: &str,
    status: CommandExecutionStatus,
    exit_code: Option<i32>,
    duration_ms: Option<i64>,
) -> ThreadItem {
    ThreadItem::CommandExecution {
        model_context: None,
        id: id.to_string(),
        plugin_id: None,
        script_path: None,
        command: "echo hi".to_string(),
        cwd: test_path_buf("/tmp").abs().into(),
        process_id: Some("pid-1".to_string()),
        source: CommandExecutionSource::Agent,
        status,
        command_actions: Vec::new(),
        aggregated_output: None,
        exit_code,
        duration_ms,
    }
}

pub(super) async fn ingest_code_mode_facts(
    reducer: &mut AnalyticsReducer,
    events: &mut Vec<TrackEventRequest>,
    facts: impl IntoIterator<Item = CodeModeToolCallFact>,
) {
    for fact in facts {
        reducer
            .ingest(
                AnalyticsFact::Custom(CustomAnalyticsFact::CodeModeToolCall(fact)),
                events,
            )
            .await;
    }
}

pub(super) fn sample_plugin_metadata() -> PluginTelemetryMetadata {
    PluginTelemetryMetadata {
        plugin_id: Some(PluginId::parse("sample@test").expect("valid plugin id")),
        remote_plugin_id: None,
        capability_summary: Some(PluginCapabilitySummary {
            config_name: "sample@test".to_string(),
            display_name: "sample".to_string(),
            plugin_namespace: None,
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
