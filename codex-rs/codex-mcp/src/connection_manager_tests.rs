use super::*;
use crate::elicitation::ElicitationRequestManager;
use crate::elicitation::ElicitationReviewRequest;
use crate::elicitation::ElicitationReviewer;
use crate::elicitation::McpElicitationState;
use crate::elicitation::elicitation_is_rejected_by_policy;
use crate::rmcp_client::AsyncManagedClient;
use crate::rmcp_client::ManagedClient;
use crate::rmcp_client::StartupOutcomeError;
use crate::server::EffectiveMcpServer;
use crate::server::McpElicitationRuntimeMetadata;
use crate::server::McpServerMetadata;
use crate::server::McpServerOrigin;
use crate::server::McpServerRuntimeMetadata;
use crate::server::McpToolApprovalPersistence;
use crate::server::McpToolRuntimeMetadata;
use crate::tools::ToolFilter;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use crate::tools::normalize_tools_for_model_with_prefix;
use codex_config::Constrained;
use codex_config::McpServerConfig;
use codex_config::McpServerToolConfig;
use codex_config::McpToolApproval;
use codex_config::types::ApprovalsReviewer;
use codex_config::types::AuthKeyringBackendKind;
use codex_exec_server::EnvironmentManager;
use codex_protocol::ToolName;
use codex_protocol::mcp::RequestId as ProtocolRequestId;
use codex_protocol::mcp_approval_meta::McpToolSource;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::GranularApprovalConfig;
use futures::FutureExt;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationCapability;
use rmcp::model::JsonObject;
use rmcp::model::NumberOrString;
use rmcp::model::Tool;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Notify;

struct CapturingElicitationReviewer {
    requests: async_channel::Sender<ElicitationReviewRequest>,
    release: Arc<Notify>,
}

impl ElicitationReviewer for CapturingElicitationReviewer {
    fn review(
        &self,
        request: ElicitationReviewRequest,
    ) -> BoxFuture<'static, anyhow::Result<Option<ElicitationResponse>>> {
        let requests = self.requests.clone();
        let release = Arc::clone(&self.release);
        Box::pin(async move {
            requests.send(request).await?;
            release.notified().await;
            Ok(Some(ElicitationResponse {
                action: ElicitationAction::Decline,
                content: None,
                meta: None,
            }))
        })
    }
}

fn create_test_tool(server_name: &str, tool_name: &str) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: tool_name.to_string(),
        callable_namespace: server_name.to_string(),
        namespace_description: None,
        namespace_title: None,
        search_aliases: Vec::new(),
        tool: Tool::new(
            tool_name.to_string(),
            format!("Test tool: {tool_name}"),
            Arc::new(JsonObject::default()),
        ),
        plugin_display_names: Vec::new(),
    }
}

fn model_tool_names(tools: &[ToolInfo]) -> HashSet<ToolName> {
    tools
        .iter()
        .map(ToolInfo::canonical_tool_name)
        .collect::<HashSet<_>>()
}

fn model_tool_name_len(name: &ToolName) -> usize {
    name.namespace
        .as_deref()
        .map_or(0, |namespace| namespace.len() + "__".len())
        + name.name.len()
}

fn is_code_mode_compatible_tool_name(name: &ToolName) -> bool {
    name.namespace
        .as_deref()
        .into_iter()
        .chain(std::iter::once(name.name.as_str()))
        .flat_map(str::chars)
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn test_http_server(url: &str) -> EffectiveMcpServer {
    EffectiveMcpServer::configured(
        serde_json::from_value(serde_json::json!({
            "url": url,
            "startup_timeout_sec": 1,
        }))
        .expect("valid test HTTP MCP server"),
    )
}

async fn start_pending_http_endpoint() -> (
    String,
    tokio::sync::oneshot::Receiver<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind pending MCP endpoint");
    let address = listener.local_addr().expect("pending MCP address");
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept MCP connection");
        let _ = accepted_tx.send(());
        let _socket = socket;
        std::future::pending::<()>().await;
    });
    (format!("http://{address}/mcp"), accepted_rx, task)
}

async fn test_reconciled_manager(
    servers: &HashMap<String, EffectiveMcpServer>,
    previous: Option<&McpConnectionManager>,
    environment_manager: Arc<EnvironmentManager>,
    auth_revision: u64,
    tx_event: async_channel::Sender<Event>,
) -> McpConnectionManager {
    test_reconciled_manager_with_auth(
        servers,
        previous,
        environment_manager,
        auth_revision,
        tx_event,
        /*auth*/ None,
    )
    .await
}

async fn test_reconciled_manager_with_auth(
    servers: &HashMap<String, EffectiveMcpServer>,
    previous: Option<&McpConnectionManager>,
    environment_manager: Arc<EnvironmentManager>,
    auth_revision: u64,
    tx_event: async_channel::Sender<Event>,
    auth: Option<&CodexAuth>,
) -> McpConnectionManager {
    let refresh = previous.map_or(
        McpConnectionRefresh::Restart,
        McpConnectionRefresh::ReuseUnchanged,
    );
    McpConnectionManager::new_with_refresh(
        servers,
        McpConnectionManagerInput {
            store_mode: OAuthCredentialsStoreMode::default(),
            keyring_backend_kind: AuthKeyringBackendKind::default(),
            auth_entries: HashMap::new(),
            approval_policy: &Constrained::allow_any(AskForApproval::OnRequest),
            submit_id: "test-reconcile".to_string(),
            tx_event,
            startup_cancellation_token: CancellationToken::new(),
            initial_permission_profile: PermissionProfile::default(),
            runtime_context: McpRuntimeContext::new(environment_manager, PathBuf::from("/tmp")),
            prefix_mcp_tool_names: true,
            client_elicitation_capability: ElicitationCapability::default(),
            supports_openai_form_elicitation: false,
            tool_plugin_provenance: ToolPluginProvenance::default(),
            auth_snapshot: McpAuthSnapshot::new(auth, auth_revision),
            elicitation_reviewer: None,
        },
        refresh,
    )
    .await
}

async fn next_startup_complete(rx: &async_channel::Receiver<Event>) -> McpStartupCompleteEvent {
    loop {
        let event = rx.recv().await.expect("startup event");
        if let EventMsg::McpStartupComplete(complete) = event.msg {
            return complete;
        }
    }
}
#[test]
fn elicitation_granular_policy_defaults_to_prompting() {
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::OnRequest
    ));
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::UnlessTrusted
    ));
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Granular(
        GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: false,
        }
    )));
}

#[test]
fn elicitation_granular_policy_respects_never_and_config() {
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Never));
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Granular(
        GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: false,
        }
    )));
}

#[tokio::test]
async fn disabled_permissions_auto_accept_elicitation_with_empty_form_schema() {
    let manager = ElicitationRequestManager::new(
        AskForApproval::Never,
        PermissionProfile::Disabled,
        /*reviewer*/ None,
    );
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), tx_event);

    let response = sender(
        NumberOrString::Number(1),
        codex_rmcp_client::Elicitation::Mcp(
            CreateElicitationRequestParams::FormElicitationParams {
                meta: None,
                message: "Confirm?".to_string(),
                requested_schema: rmcp::model::ElicitationSchema::builder()
                    .build()
                    .expect("schema should build"),
            },
        ),
    )
    .await
    .expect("elicitation should auto accept");

    assert_eq!(
        response,
        ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(serde_json::json!({})),
            meta: None,
        }
    );
}

#[tokio::test]
async fn disabled_permissions_do_not_auto_accept_elicitation_with_requested_fields() {
    let manager = ElicitationRequestManager::new(
        AskForApproval::Never,
        PermissionProfile::Disabled,
        /*reviewer*/ None,
    );
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), tx_event);

    let response = sender(
        NumberOrString::Number(1),
        codex_rmcp_client::Elicitation::Mcp(
            CreateElicitationRequestParams::FormElicitationParams {
                meta: None,
                message: "What should I say?".to_string(),
                requested_schema: rmcp::model::ElicitationSchema::builder()
                    .required_property(
                        "message",
                        rmcp::model::PrimitiveSchema::String(rmcp::model::StringSchema::new()),
                    )
                    .build()
                    .expect("schema should build"),
            },
        ),
    )
    .await
    .expect("elicitation should auto decline");

    assert_eq!(
        response,
        ElicitationResponse {
            action: ElicitationAction::Decline,
            content: None,
            meta: None,
        }
    );
}

#[tokio::test]
async fn replacement_routes_same_upstream_elicitation_ids_to_their_origin() -> anyhow::Result<()> {
    let mut old_manager =
        McpConnectionManager::new_uninitialized(/*prefix_mcp_tool_names*/ true);
    let mut new_manager =
        McpConnectionManager::new_uninitialized(/*prefix_mcp_tool_names*/ true);
    let elicitation_state = McpElicitationState::default();
    old_manager.elicitation_state = elicitation_state.clone();
    new_manager.elicitation_state = elicitation_state.clone();
    let old_requests = ElicitationRequestManager::new_with_state(
        AskForApproval::OnRequest,
        PermissionProfile::default(),
        /*reviewer*/ None,
        McpElicitationRuntimeMetadata::default(),
        elicitation_state.clone(),
    );
    let new_requests = ElicitationRequestManager::new_with_state(
        AskForApproval::OnRequest,
        PermissionProfile::default(),
        /*reviewer*/ None,
        McpElicitationRuntimeMetadata::default(),
        elicitation_state,
    );
    old_manager
        .elicitation_requests
        .insert("same".to_string(), old_requests.clone());
    new_manager
        .elicitation_requests
        .insert("same".to_string(), new_requests.clone());
    let request = || {
        codex_rmcp_client::Elicitation::Mcp(CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: "Confirm?".to_string(),
            requested_schema: rmcp::model::ElicitationSchema::builder()
                .build()
                .expect("schema should build"),
        })
    };
    let request_id = NumberOrString::Number(7);
    let (old_tx, old_rx) = async_channel::unbounded();
    let old_task = tokio::spawn(old_requests.make_sender("same".to_string(), old_tx)(
        request_id.clone(),
        request(),
    ));
    let old_event = old_rx.recv().await.expect("old elicitation event");
    let (new_tx, new_rx) = async_channel::unbounded();
    let new_task = tokio::spawn(new_requests.make_sender("same".to_string(), new_tx)(
        request_id.clone(),
        request(),
    ));
    let new_event = new_rx.recv().await.expect("new elicitation event");
    let elicitation_id = |event: Event| {
        let EventMsg::ElicitationRequest(request) = event.msg else {
            panic!("expected elicitation request event");
        };
        match request.id {
            ProtocolRequestId::String(value) => NumberOrString::String(Arc::from(value)),
            ProtocolRequestId::Integer(value) => NumberOrString::Number(value),
        }
    };
    let old_id = elicitation_id(old_event);
    let new_id = elicitation_id(new_event);
    assert_ne!(old_id, new_id, "Codex-facing IDs must be generation-safe");

    let accepted = ElicitationResponse {
        action: ElicitationAction::Accept,
        content: Some(serde_json::json!({"generation": "new"})),
        meta: None,
    };
    new_manager
        .resolve_elicitation("same".to_string(), new_id, accepted.clone())
        .await
        .expect("resolve current generation");
    assert_eq!(new_task.await.expect("new elicitation task")?, accepted);
    assert!(
        !old_task.is_finished(),
        "new response must not resolve old request"
    );

    new_manager
        .resolve_elicitation(
            "same".to_string(),
            old_id,
            ElicitationResponse {
                action: ElicitationAction::Decline,
                content: None,
                meta: None,
            },
        )
        .await
        .expect("latest manager routes to the old generation");
    assert_eq!(
        old_task.await.expect("old elicitation task")?.action,
        ElicitationAction::Decline
    );
    Ok(())
}

#[tokio::test]
async fn same_name_replacement_keeps_pending_elicitation_runtime_metadata_generation_local()
-> anyhow::Result<()> {
    let runtime_metadata = |reviewer, source_id: &str| {
        let metadata = McpServerRuntimeMetadata::default()
            .with_approvals_reviewer(reviewer)
            .with_tool(
                "raw_tool",
                McpToolRuntimeMetadata::default()
                    .with_approval_source(
                        McpToolSource::new(
                            source_id,
                            format!("Source {source_id}"),
                            /*description*/ None,
                        )
                        .expect("valid source"),
                    )
                    .with_search_aliases(["routed_tool"]),
            );
        McpElicitationRuntimeMetadata::from(&metadata)
    };
    let request = || {
        codex_rmcp_client::Elicitation::Mcp(CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: "Confirm?".to_string(),
            requested_schema: rmcp::model::ElicitationSchema::builder()
                .build()
                .expect("schema should build"),
        })
    };
    let release = Arc::new(Notify::new());
    let (old_request_tx, old_request_rx) = async_channel::bounded(1);
    let old_requests = ElicitationRequestManager::new_with_state(
        AskForApproval::OnRequest,
        PermissionProfile::default(),
        Some(Arc::new(CapturingElicitationReviewer {
            requests: old_request_tx,
            release: Arc::clone(&release),
        })),
        runtime_metadata(ApprovalsReviewer::AutoReview, "old-source"),
        McpElicitationState::default(),
    );
    let (old_events, _old_events_rx) = async_channel::unbounded();
    let old_task = tokio::spawn(old_requests.make_sender("same".to_string(), old_events)(
        NumberOrString::Number(1),
        request(),
    ));
    let old_request = old_request_rx.recv().await?;
    assert!(
        !old_task.is_finished(),
        "old elicitation must remain pending"
    );

    let (new_request_tx, new_request_rx) = async_channel::bounded(1);
    let new_requests = ElicitationRequestManager::new_with_state(
        AskForApproval::OnRequest,
        PermissionProfile::default(),
        Some(Arc::new(CapturingElicitationReviewer {
            requests: new_request_tx,
            release: Arc::clone(&release),
        })),
        runtime_metadata(ApprovalsReviewer::User, "new-source"),
        McpElicitationState::default(),
    );
    let (new_events, _new_events_rx) = async_channel::unbounded();
    let new_task = tokio::spawn(new_requests.make_sender("same".to_string(), new_events)(
        NumberOrString::Number(2),
        request(),
    ));
    let new_request = new_request_rx.recv().await?;

    let review_context = |request: &ElicitationReviewRequest| {
        (
            request.server_runtime_metadata.approvals_reviewer(),
            request
                .server_runtime_metadata
                .approval_source_by_name_or_alias("routed_tool")
                .map(|source| source.id().to_string()),
        )
    };
    assert_eq!(
        review_context(&old_request),
        (
            Some(ApprovalsReviewer::AutoReview),
            Some("old-source".to_string())
        )
    );
    assert_eq!(
        review_context(&new_request),
        (
            Some(ApprovalsReviewer::User),
            Some("new-source".to_string())
        )
    );

    release.notify_waiters();
    assert_eq!(old_task.await??.action, ElicitationAction::Decline);
    assert_eq!(new_task.await??.action, ElicitationAction::Decline);
    Ok(())
}

#[test]
fn test_normalize_tools_short_non_duplicated_names() {
    let tools = vec![
        create_test_tool("server1", "tool1"),
        create_test_tool("server1", "tool2"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(
        model_tool_names(&model_tools),
        HashSet::from([
            ToolName::namespaced("mcp__server1", "tool1"),
            ToolName::namespaced("mcp__server1", "tool2")
        ])
    );
}

#[test]
fn test_normalize_tools_duplicated_names_skipped() {
    let tools = vec![
        create_test_tool("server1", "duplicate_tool"),
        create_test_tool("server1", "duplicate_tool"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    // Only the first tool should remain, the second is skipped
    assert_eq!(
        model_tool_names(&model_tools),
        HashSet::from([ToolName::namespaced("mcp__server1", "duplicate_tool")])
    );
}

#[test]
fn test_normalize_tools_long_names_same_server() {
    let server_name = "my_server";

    let tools = vec![
        create_test_tool(
            server_name,
            "extremely_lengthy_function_name_that_absolutely_surpasses_all_reasonable_limits",
        ),
        create_test_tool(
            server_name,
            "yet_another_extremely_lengthy_function_name_that_absolutely_surpasses_all_reasonable_limits",
        ),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 2);

    let names = model_tool_names(&model_tools);

    assert!(names.iter().all(|name| model_tool_name_len(name) == 64));
    assert!(
        names
            .iter()
            .all(|name| name.namespace.as_deref() == Some("mcp__my_server"))
    );
    assert!(
        names.iter().all(is_code_mode_compatible_tool_name),
        "model-visible names must be code-mode compatible: {names:?}"
    );
}

#[test]
fn test_normalize_tools_sanitizes_invalid_characters() {
    let tools = vec![create_test_tool("server.one", "tool.two-three")];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 1);
    let tool = model_tools.into_iter().next().expect("one tool");
    let model_name = tool.canonical_tool_name();
    assert_eq!(
        model_name,
        ToolName::namespaced("mcp__server_one", "tool_two_three")
    );
    assert_eq!(
        ToolName::namespaced(tool.callable_namespace.clone(), tool.callable_name.clone()),
        model_name
    );
    // The callable parts are sanitized for model-visible tool calls, but the raw
    // MCP name is preserved for the actual MCP call.
    assert_eq!(tool.server_name, "server.one");
    assert_eq!(tool.callable_namespace, "mcp__server_one");
    assert_eq!(tool.callable_name, "tool_two_three");
    assert_eq!(tool.tool.name, "tool.two-three");

    assert!(
        is_code_mode_compatible_tool_name(&model_name),
        "model-visible name must be code-mode compatible: {model_name:?}"
    );
}

#[test]
fn test_normalize_tools_keeps_hyphenated_mcp_tools_callable() {
    let tools = vec![create_test_tool("music-studio", "get-strudel-guide")];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 1);
    let tool = model_tools.into_iter().next().expect("one tool");
    assert_eq!(
        tool.canonical_tool_name(),
        ToolName::namespaced("mcp__music_studio", "get_strudel_guide")
    );
    assert_eq!(tool.callable_namespace, "mcp__music_studio");
    assert_eq!(tool.callable_name, "get_strudel_guide");
    assert_eq!(tool.tool.name, "get-strudel-guide");
}

#[test]
fn test_normalize_tools_disambiguates_sanitized_namespace_collisions() {
    let tools = vec![
        create_test_tool("basic-server", "lookup"),
        create_test_tool("basic_server", "query"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 2);
    let mut namespaces = model_tools
        .iter()
        .map(|tool| tool.callable_namespace.as_str())
        .collect::<Vec<_>>();
    namespaces.sort();
    namespaces.dedup();
    assert_eq!(namespaces.len(), 2);

    let raw_servers = model_tools
        .iter()
        .map(|tool| tool.server_name.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(raw_servers, HashSet::from(["basic-server", "basic_server"]));
    let model_names = model_tool_names(&model_tools);
    assert!(
        model_names.iter().all(is_code_mode_compatible_tool_name),
        "model-visible names must be code-mode compatible: {model_names:?}"
    );
}

#[test]
fn test_normalize_tools_disambiguates_shared_callable_namespace_across_servers() {
    let mut first = create_test_tool("server_one", "lookup");
    first.callable_namespace = "shared".to_string();
    let mut second = create_test_tool("server_two", "query");
    second.callable_namespace = "shared".to_string();

    let model_tools =
        normalize_tools_for_model_with_prefix([first, second], /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 2);
    assert_eq!(
        model_tools
            .iter()
            .map(|tool| tool.server_name.as_str())
            .collect::<HashSet<_>>(),
        HashSet::from(["server_one", "server_two"])
    );
    assert_eq!(
        model_tools
            .iter()
            .map(|tool| tool.callable_namespace.as_str())
            .collect::<HashSet<_>>()
            .len(),
        2,
        "distinct routing identities must not collapse when their model namespaces match"
    );
}

#[test]
fn test_normalize_tools_disambiguates_sanitized_tool_name_collisions() {
    let tools = vec![
        create_test_tool("server", "tool-name"),
        create_test_tool("server", "tool_name"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true);

    assert_eq!(model_tools.len(), 2);
    let raw_tool_names = model_tools
        .iter()
        .map(|tool| tool.tool.name.to_string())
        .collect::<HashSet<_>>();
    assert_eq!(
        raw_tool_names,
        HashSet::from(["tool-name".to_string(), "tool_name".to_string()])
    );
    let callable_tool_names = model_tools
        .iter()
        .map(|tool| tool.callable_name.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(callable_tool_names.len(), 2);
}

#[test]
fn tool_filter_allows_by_default() {
    let filter = ToolFilter::default();

    assert!(filter.allows("any"));
}

#[test]
fn tool_filter_applies_enabled_list() {
    let filter = ToolFilter {
        enabled: Some(HashSet::from(["allowed".to_string()])),
        disabled: HashSet::new(),
    };

    assert!(filter.allows("allowed"));
    assert!(!filter.allows("denied"));
}

#[test]
fn tool_filter_applies_disabled_list() {
    let filter = ToolFilter {
        enabled: None,
        disabled: HashSet::from(["blocked".to_string()]),
    };

    assert!(!filter.allows("blocked"));
    assert!(filter.allows("open"));
}

#[test]
fn tool_filter_applies_enabled_then_disabled() {
    let filter = ToolFilter {
        enabled: Some(HashSet::from(["keep".to_string(), "remove".to_string()])),
        disabled: HashSet::from(["remove".to_string()]),
    };

    assert!(filter.allows("keep"));
    assert!(!filter.allows("remove"));
    assert!(!filter.allows("unknown"));
}

#[test]
fn filter_tools_applies_per_server_filters() {
    let server1_tools = vec![
        create_test_tool("server1", "tool_a"),
        create_test_tool("server1", "tool_b"),
    ];
    let server2_tools = vec![create_test_tool("server2", "tool_a")];
    let server1_filter = ToolFilter {
        enabled: Some(HashSet::from(["tool_a".to_string(), "tool_b".to_string()])),
        disabled: HashSet::from(["tool_b".to_string()]),
    };
    let server2_filter = ToolFilter {
        enabled: None,
        disabled: HashSet::from(["tool_a".to_string()]),
    };

    let filtered: Vec<_> = filter_tools(server1_tools, &server1_filter)
        .into_iter()
        .chain(filter_tools(server2_tools, &server2_filter))
        .collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].server_name, "server1");
    assert_eq!(filtered[0].callable_name, "tool_a");
}

#[test]
fn normalize_tools_accepts_canonical_namespaced_tool_names() {
    let tools = normalize_tools_for_model_with_prefix(
        vec![create_test_tool("rmcp", "echo")],
        /*prefix_mcp_tool_names*/ false,
    );
    let tool = tools
        .iter()
        .find(|tool| tool.canonical_tool_name() == ToolName::namespaced("rmcp", "echo"))
        .expect("split MCP tool namespace and name should resolve");

    let expected = ("rmcp", "rmcp", "echo", "echo");
    assert_eq!(
        (
            tool.server_name.as_str(),
            tool.callable_namespace.as_str(),
            tool.callable_name.as_str(),
            tool.tool.name.as_ref(),
        ),
        expected
    );
}

#[test]
fn normalize_tools_applies_legacy_mcp_prefix_by_default() {
    let tools = normalize_tools_for_model_with_prefix(
        vec![create_test_tool("rmcp", "echo")],
        /*prefix_mcp_tool_names*/ true,
    );
    let tool = tools
        .iter()
        .find(|tool| tool.canonical_tool_name() == ToolName::namespaced("mcp__rmcp", "echo"))
        .expect("legacy-prefixed MCP tool name should resolve");

    let expected = ("rmcp", "mcp__rmcp", "echo", "echo");
    assert_eq!(
        (
            tool.server_name.as_str(),
            tool.callable_namespace.as_str(),
            tool.callable_name.as_str(),
            tool.tool.name.as_ref(),
        ),
        expected
    );
}

#[tokio::test]
async fn list_all_tools_blocks_while_client_is_pending() {
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let mut manager = McpConnectionManager::new_uninitialized(/*prefix_mcp_tool_names*/ true);
    manager.clients.insert(
        "rmcp".to_string(),
        AsyncManagedClient::for_test(pending_client, CancellationToken::new()),
    );

    let timeout_result =
        tokio::time::timeout(Duration::from_millis(10), manager.list_all_tools()).await;
    assert!(timeout_result.is_err());
}

#[tokio::test]
async fn shutdown_cancels_pending_tool_listing() {
    let cancel_token = CancellationToken::new();
    let cancel_token_for_startup = cancel_token.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let pending_client = async move {
        let _ = started_tx.send(());
        cancel_token_for_startup.cancelled().await;
        Err(StartupOutcomeError::Cancelled)
    }
    .boxed()
    .shared();
    let mut manager = McpConnectionManager::new_uninitialized(/*prefix_mcp_tool_names*/ true);
    manager.clients.insert(
        "rmcp".to_string(),
        AsyncManagedClient::for_test(pending_client, cancel_token),
    );
    let manager = Arc::new(manager);
    let manager_for_list = Arc::clone(&manager);
    let list_task = tokio::spawn(async move { manager_for_list.list_all_tools().await });

    started_rx.await.expect("tool listing should start");
    tokio::time::timeout(Duration::from_secs(1), manager.shutdown())
        .await
        .expect("shutdown should cancel speculative tool listing");
    let tools = list_task.await.expect("tool listing task should not panic");
    assert!(tools.is_empty());
}

#[tokio::test]
async fn shutdown_continues_after_caller_is_aborted() {
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (completed_tx, completed_rx) = tokio::sync::oneshot::channel();
    let release = Arc::new(tokio::sync::Notify::new());
    let release_for_client = Arc::clone(&release);
    let blocking_client = async move {
        let _ = started_tx.send(());
        release_for_client.notified().await;
        let _ = completed_tx.send(());
        Err(StartupOutcomeError::Cancelled)
    }
    .boxed()
    .shared();
    let mut manager = McpConnectionManager::new_uninitialized(/*prefix_mcp_tool_names*/ true);
    manager.clients.insert(
        "rmcp".to_string(),
        AsyncManagedClient::for_test(blocking_client, CancellationToken::new()),
    );
    let manager = Arc::new(manager);
    let shutdown_task = tokio::spawn({
        let manager = Arc::clone(&manager);
        async move { manager.shutdown().await }
    });

    started_rx.await.expect("client shutdown should start");
    shutdown_task.abort();
    let shutdown_error = shutdown_task
        .await
        .expect_err("caller shutdown task should be aborted");
    assert!(shutdown_error.is_cancelled());
    release.notify_one();

    tokio::time::timeout(Duration::from_secs(1), completed_rx)
        .await
        .expect("client shutdown should survive caller cancellation")
        .expect("client shutdown completion sender should stay alive");
}

#[test]
fn server_metadata_is_added_to_listed_tools() {
    let server_name = "docs";
    let mut manager = McpConnectionManager::new_uninitialized(/*prefix_mcp_tool_names*/ true);
    manager.server_metadata.insert(
        server_name.to_string(),
        McpServerMetadata {
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            pollutes_memory: true,
            origin: Some(McpServerOrigin::StreamableHttp(
                "https://docs.example".to_string(),
            )),
            supports_parallel_tool_calls: true,
            default_tools_approval_mode: None,
            tool_approval_modes: HashMap::new(),
            tool_runtime_metadata: HashMap::new(),
            trusts_tool_input: false,
            trusts_approval_context: false,
            sandbox_state_source: super::McpSandboxStateSource::PrimaryTurnEnvironment,
            approvals_reviewer: None,
            _runtime_owner: None,
        },
    );
    let tool = manager.with_server_metadata(create_test_tool(server_name, "search"));
    assert_eq!(tool.server_name, server_name);
    assert_eq!(tool.callable_namespace, server_name);
    assert!(tool.supports_parallel_tool_calls);
    assert_eq!(tool.server_origin.as_deref(), Some("https://docs.example"));
    assert_eq!(
        manager.server_sandbox_state_source(server_name),
        super::McpSandboxStateSource::PrimaryTurnEnvironment
    );
}

#[test]
fn server_metadata_preserves_tool_approval_policy() {
    let mut config: McpServerConfig = serde_json::from_value(serde_json::json!({
        "url": "https://docs.example/mcp"
    }))
    .expect("valid MCP config");
    config.environment_id = "remote".to_string();
    config.default_tools_approval_mode = Some(McpToolApproval::Prompt);
    config.tools.insert(
        "search".to_string(),
        McpServerToolConfig {
            approval_mode: Some(McpToolApproval::Approve),
        },
    );
    let metadata = McpServerMetadata::from(&EffectiveMcpServer::configured(config));

    assert_eq!(metadata.environment_id, "remote");
    assert_eq!(metadata.tool_approval_mode("read"), McpToolApproval::Prompt);
    assert_eq!(
        metadata.tool_approval_mode("search"),
        McpToolApproval::Approve
    );
}

#[test]
fn runtime_metadata_alias_lookup_is_exact_unique_and_unambiguous() {
    let source = |id: &str, aliases: &[&str]| {
        McpToolRuntimeMetadata::default()
            .with_approval_source(
                McpToolSource::new(id, format!("Source {id}"), /*description*/ None)
                    .expect("valid source"),
            )
            .with_search_aliases(aliases.iter().copied())
    };
    let server = EffectiveMcpServer::configured(
        serde_json::from_value(serde_json::json!({"url": "https://example.com/mcp"}))
            .expect("valid server"),
    )
    .with_runtime_metadata(
        McpServerRuntimeMetadata::default().with_tools(HashMap::from([
            (
                "first".to_string(),
                source("raw-first", &["raw-first", "duplicate"]),
            ),
            ("shared".to_string(), source("raw-exact", &["raw-exact"])),
            (
                "second".to_string(),
                source("raw-second", &["raw-second", "duplicate"]),
            ),
        ])),
    );
    let runtime_metadata = McpElicitationRuntimeMetadata::from(server.runtime_metadata());

    assert_eq!(
        runtime_metadata
            .approval_source_by_name_or_alias("shared")
            .map(McpToolSource::id),
        Some("raw-exact")
    );
    assert_eq!(
        runtime_metadata
            .approval_source_by_name_or_alias("raw-first")
            .map(McpToolSource::id),
        Some("raw-first")
    );
    assert!(
        runtime_metadata
            .approval_source_by_name_or_alias("duplicate")
            .is_none(),
        "ambiguous aliases must not select arbitrary runtime metadata"
    );
    assert!(
        runtime_metadata
            .approval_source_by_name_or_alias("missing")
            .is_none()
    );
}

#[tokio::test]
async fn no_local_runtime_fails_local_stdio_but_keeps_local_http_server() {
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let (tx_event, rx_event) = async_channel::unbounded();
    drop(rx_event);
    let mcp_servers = HashMap::from([
        (
            "stdio".to_string(),
            EffectiveMcpServer::configured(McpServerConfig {
                auth: Default::default(),
                transport: McpServerTransportConfig::Stdio {
                    command: "echo".to_string(),
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            }),
        ),
        (
            "http".to_string(),
            EffectiveMcpServer::configured(McpServerConfig {
                auth: Default::default(),
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "http://127.0.0.1:1".to_string(),
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            }),
        ),
    ]);

    let cancel_token = CancellationToken::new();
    let manager = McpConnectionManager::new(
        &mcp_servers,
        McpConnectionManagerInput {
            store_mode: OAuthCredentialsStoreMode::default(),
            keyring_backend_kind: AuthKeyringBackendKind::default(),
            auth_entries: HashMap::new(),
            approval_policy: &approval_policy,
            submit_id: String::new(),
            tx_event,
            startup_cancellation_token: cancel_token.clone(),
            initial_permission_profile: PermissionProfile::default(),
            runtime_context: McpRuntimeContext::new(
                Arc::new(EnvironmentManager::without_environments()),
                PathBuf::from("/tmp"),
            ),
            prefix_mcp_tool_names: true,
            client_elicitation_capability: ElicitationCapability::default(),
            supports_openai_form_elicitation: false,
            tool_plugin_provenance: ToolPluginProvenance::default(),
            auth_snapshot: McpAuthSnapshot::new(/*auth*/ None, /*revision*/ 0),
            elicitation_reviewer: None,
        },
    )
    .await;

    assert!(manager.clients.contains_key("stdio"));
    assert!(manager.clients.contains_key("http"));
    assert!(
        !manager
            .wait_for_server_ready("stdio", Duration::from_millis(10))
            .await
    );
    let error = match manager
        .clients
        .get("stdio")
        .expect("stdio client")
        .client()
        .await
    {
        Ok(_) => panic!("local stdio MCP startup should fail"),
        Err(error) => error,
    };
    assert_eq!(
        startup_outcome_error_message(error),
        "local stdio MCP server `stdio` requires a local environment"
    );
    cancel_token.cancel();
}

#[tokio::test]
async fn reconcile_reuses_equivalent_runtime_metadata_and_restarts_elicitation_changes() {
    let environment_manager = Arc::new(EnvironmentManager::without_environments());
    let (url, accepted, endpoint) = start_pending_http_endpoint().await;
    let (tx, _rx) = async_channel::unbounded();
    let runtime_metadata = |reviewer: Option<ApprovalsReviewer>, source_id: &str| {
        let tool = McpToolRuntimeMetadata::default()
            .with_approval_persistence(McpToolApprovalPersistence::new(|| async { Ok(()) }))
            .with_approval_source(
                McpToolSource::new(source_id, "Source", /*description*/ None)
                    .expect("valid source"),
            );
        let metadata = McpServerRuntimeMetadata::default().with_tool("tool", tool);
        match reviewer {
            Some(reviewer) => metadata.with_approvals_reviewer(reviewer),
            None => metadata,
        }
    };
    let first_server = test_http_server(&url)
        .with_runtime_owner(Arc::new("first owner"))
        .with_runtime_metadata(runtime_metadata(/*reviewer*/ None, "source"));
    let first = test_reconciled_manager(
        &HashMap::from([("runtime".to_string(), first_server)]),
        /*previous*/ None,
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
    )
    .await;
    accepted
        .await
        .expect("first client reaches pending endpoint");

    let second_server = test_http_server(&url)
        .with_runtime_owner(Arc::new("second owner"))
        .with_runtime_metadata(runtime_metadata(/*reviewer*/ None, "source"));
    let second = test_reconciled_manager(
        &HashMap::from([("runtime".to_string(), second_server)]),
        Some(&first),
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
    )
    .await;

    assert!(
        first.clients["runtime"].same_instance(&second.clients["runtime"]),
        "retention-only owner changes must not reconnect an identical MCP endpoint"
    );
    let reviewer_changed = test_reconciled_manager(
        &HashMap::from([(
            "runtime".to_string(),
            test_http_server(&url).with_runtime_metadata(runtime_metadata(
                Some(ApprovalsReviewer::AutoReview),
                "source",
            )),
        )]),
        Some(&second),
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
    )
    .await;
    assert!(
        !second.clients["runtime"].same_instance(&reviewer_changed.clients["runtime"]),
        "elicitation policy changes must bind a new client to the new generation"
    );
    let source_changed = test_reconciled_manager(
        &HashMap::from([(
            "runtime".to_string(),
            test_http_server(&url).with_runtime_metadata(runtime_metadata(
                Some(ApprovalsReviewer::AutoReview),
                "other-source",
            )),
        )]),
        Some(&reviewer_changed),
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
    )
    .await;
    assert!(
        !reviewer_changed.clients["runtime"].same_instance(&source_changed.clients["runtime"]),
        "elicitation source changes must bind a new client to the new generation"
    );
    let (tx_strict, _rx_strict) = async_channel::unbounded();
    let strict = test_reconciled_manager(
        &HashMap::from([("runtime".to_string(), test_http_server(&url))]),
        /*previous*/ None,
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx_strict,
    )
    .await;
    assert!(
        !second.clients["runtime"].same_instance(&strict.clients["runtime"]),
        "strict refresh must restart an unchanged registration"
    );
    first.shutdown_superseded_by(&second).await;
    second.shutdown_superseded_by(&reviewer_changed).await;
    reviewer_changed
        .shutdown_superseded_by(&source_changed)
        .await;
    source_changed.shutdown().await;
    strict.shutdown().await;
    endpoint.abort();
    let _ = endpoint.await;
}

#[tokio::test]
async fn reconcile_restarts_clients_after_environment_instance_replacement() {
    let first_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind first environment endpoint");
    let environment_manager = Arc::new(
        EnvironmentManager::create_for_tests(
            Some(format!(
                "ws://{}",
                first_listener
                    .local_addr()
                    .expect("first environment address")
            )),
            /*local_runtime_paths*/ None,
        )
        .await,
    );
    let mut config: McpServerConfig = serde_json::from_value(serde_json::json!({
        "url": "http://example.invalid/mcp",
        "startup_timeout_sec": 60,
    }))
    .expect("valid remote HTTP MCP server");
    config.environment_id = "remote".to_string();
    let (stable_url, stable_accepted, stable_endpoint) = start_pending_http_endpoint().await;
    let servers = HashMap::from([
        ("remote".to_string(), EffectiveMcpServer::configured(config)),
        ("stable".to_string(), test_http_server(&stable_url)),
    ]);
    let (tx, _rx) = async_channel::unbounded();
    let first = test_reconciled_manager(
        &servers,
        /*previous*/ None,
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
    )
    .await;
    stable_accepted
        .await
        .expect("unrelated local client reaches its endpoint");
    let first_environment = environment_manager
        .get_environment("remote")
        .expect("first environment");

    let second_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind replacement environment endpoint");
    environment_manager
        .upsert_environment(
            "remote".to_string(),
            format!(
                "ws://{}",
                second_listener
                    .local_addr()
                    .expect("replacement environment address")
            ),
            /*connect_timeout*/ None,
        )
        .expect("replace environment");
    let replacement_environment = environment_manager
        .get_environment("remote")
        .expect("replacement environment");
    assert!(!Arc::ptr_eq(&first_environment, &replacement_environment));

    let second = test_reconciled_manager(
        &servers,
        Some(&first),
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx,
    )
    .await;
    assert!(
        !first.clients["remote"].same_instance(&second.clients["remote"]),
        "an environment replacement must relaunch its MCP clients"
    );
    assert!(
        first.clients["stable"].same_instance(&second.clients["stable"]),
        "an unrelated environment replacement must preserve local MCP clients"
    );

    first.shutdown_superseded_by(&second).await;
    second.shutdown().await;
    stable_endpoint.abort();
    let _ = stable_endpoint.await;
}

#[tokio::test]
async fn reconcile_replaces_changed_server_and_preserves_unrelated_client() {
    let environment_manager = Arc::new(EnvironmentManager::without_environments());
    let (old_changed_url, old_changed_accepted, old_changed_endpoint) =
        start_pending_http_endpoint().await;
    let (stable_url, stable_accepted, stable_endpoint) = start_pending_http_endpoint().await;
    let initial_servers = HashMap::from([
        ("changed".to_string(), test_http_server(&old_changed_url)),
        ("stable".to_string(), test_http_server(&stable_url)),
    ]);
    let (tx, rx) = async_channel::unbounded();
    let first = test_reconciled_manager(
        &initial_servers,
        /*previous*/ None,
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
    )
    .await;
    old_changed_accepted
        .await
        .expect("old changed client reaches pending endpoint");
    stable_accepted
        .await
        .expect("stable client reaches pending endpoint");
    let (new_changed_url, new_changed_accepted, new_changed_endpoint) =
        start_pending_http_endpoint().await;
    let next_servers = HashMap::from([
        ("changed".to_string(), test_http_server(&new_changed_url)),
        ("stable".to_string(), initial_servers["stable"].clone()),
    ]);
    let second = test_reconciled_manager(
        &next_servers,
        Some(&first),
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx,
    )
    .await;
    new_changed_accepted
        .await
        .expect("new changed client reaches pending endpoint");

    assert!(first.clients["stable"].same_instance(&second.clients["stable"]));
    assert!(!first.clients["changed"].same_instance(&second.clients["changed"]));

    first.shutdown_superseded_by(&second).await;
    second.cancel_startup();
    let complete = tokio::time::timeout(Duration::from_secs(3), next_startup_complete(&rx))
        .await
        .expect("mixed reconcile startup round completes");
    let reported = complete
        .ready
        .into_iter()
        .chain(complete.cancelled)
        .chain(complete.failed.into_iter().map(|failure| failure.server))
        .collect::<HashSet<_>>();
    assert_eq!(
        reported,
        HashSet::from(["changed".to_string(), "stable".to_string()])
    );

    second.shutdown().await;
    for endpoint in [old_changed_endpoint, stable_endpoint, new_changed_endpoint] {
        endpoint.abort();
        let _ = endpoint.await;
    }
}

#[tokio::test]
async fn auth_revision_only_replaces_chatgpt_authenticated_servers() {
    let environment_manager = Arc::new(EnvironmentManager::without_environments());
    let (chatgpt_url, chatgpt_accepted, chatgpt_endpoint) = start_pending_http_endpoint().await;
    let (ordinary_url, ordinary_accepted, ordinary_endpoint) = start_pending_http_endpoint().await;
    let mut chatgpt_config: McpServerConfig = serde_json::from_value(serde_json::json!({
        "url": chatgpt_url,
        "startup_timeout_sec": 1,
    }))
    .expect("valid ChatGPT-authenticated test server");
    chatgpt_config.auth = McpServerAuth::ChatGpt;
    let chatgpt = EffectiveMcpServer::configured(chatgpt_config);
    let servers = HashMap::from([
        ("chatgpt".to_string(), chatgpt),
        ("ordinary".to_string(), test_http_server(&ordinary_url)),
    ]);
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let (tx, _rx) = async_channel::unbounded();
    let first = test_reconciled_manager_with_auth(
        &servers,
        /*previous*/ None,
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
        Some(&auth),
    )
    .await;
    chatgpt_accepted
        .await
        .expect("ChatGPT-authenticated client reaches pending endpoint");
    ordinary_accepted
        .await
        .expect("ordinary client reaches pending endpoint");
    let second = test_reconciled_manager_with_auth(
        &servers,
        Some(&first),
        Arc::clone(&environment_manager),
        /*auth_revision*/ 2,
        tx,
        Some(&auth),
    )
    .await;

    assert!(!first.clients["chatgpt"].same_instance(&second.clients["chatgpt"]));
    assert!(first.clients["ordinary"].same_instance(&second.clients["ordinary"]));
    first.shutdown_superseded_by(&second).await;
    second.shutdown().await;
    for endpoint in [chatgpt_endpoint, ordinary_endpoint] {
        endpoint.abort();
        let _ = endpoint.await;
    }
}

#[tokio::test]
async fn reconcile_restarts_a_terminally_failed_client() {
    let environment_manager = Arc::new(EnvironmentManager::without_environments());
    let mut config: McpServerConfig = serde_json::from_value(serde_json::json!({
        "url": "http://unused.invalid/mcp",
        "startup_timeout_sec": 1,
    }))
    .expect("valid test HTTP MCP server");
    config.environment_id = "missing".to_string();
    let servers = HashMap::from([("failed".to_string(), EffectiveMcpServer::configured(config))]);
    let (tx, rx) = async_channel::unbounded();
    let first = test_reconciled_manager(
        &servers,
        /*previous*/ None,
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
    )
    .await;
    tokio::time::timeout(Duration::from_secs(3), next_startup_complete(&rx))
        .await
        .expect("failed startup round completes");
    assert!(matches!(
        first.clients["failed"].client().await,
        Err(StartupOutcomeError::Failed { .. })
    ));

    let second = test_reconciled_manager(
        &servers,
        Some(&first),
        environment_manager,
        /*auth_revision*/ 1,
        tx,
    )
    .await;
    assert!(
        !first.clients["failed"].same_instance(&second.clients["failed"]),
        "a completed startup failure must be retried"
    );
    first.shutdown_superseded_by(&second).await;
    second.shutdown().await;
}

#[tokio::test]
async fn reconcile_restarts_a_cancelled_client_before_the_future_completes() {
    let environment_manager = Arc::new(EnvironmentManager::without_environments());
    let (url, accepted, endpoint) = start_pending_http_endpoint().await;
    let servers = HashMap::from([("cancelled".to_string(), test_http_server(&url))]);
    let (tx, _rx) = async_channel::unbounded();
    let first = test_reconciled_manager(
        &servers,
        /*previous*/ None,
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
    )
    .await;
    accepted.await.expect("client reaches pending endpoint");
    first.cancel_startup();
    let second = test_reconciled_manager(
        &servers,
        Some(&first),
        environment_manager,
        /*auth_revision*/ 1,
        tx,
    )
    .await;
    assert!(
        !first.clients["cancelled"].same_instance(&second.clients["cancelled"]),
        "a cancelled startup must be retried before its future observes cancellation"
    );
    assert!(matches!(
        first.clients["cancelled"].client().await,
        Err(StartupOutcomeError::Cancelled)
    ));
    first.shutdown_superseded_by(&second).await;
    second.shutdown().await;
    endpoint.abort();
    let _ = endpoint.await;
}

#[tokio::test]
async fn cancel_startup_reaches_a_pending_reused_client() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind pending MCP endpoint");
    let address = listener.local_addr().expect("pending MCP address");
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let endpoint = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept MCP connection");
        let _ = accepted_tx.send(());
        let _socket = socket;
        std::future::pending::<()>().await;
    });
    let servers = HashMap::from([(
        "pending".to_string(),
        test_http_server(&format!("http://{address}/mcp")),
    )]);
    let environment_manager = Arc::new(EnvironmentManager::without_environments());
    let (tx, _rx) = async_channel::unbounded();
    let first = test_reconciled_manager(
        &servers,
        /*previous*/ None,
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx.clone(),
    )
    .await;
    accepted_rx.await.expect("client reaches pending endpoint");
    let second = test_reconciled_manager(
        &servers,
        Some(&first),
        Arc::clone(&environment_manager),
        /*auth_revision*/ 1,
        tx,
    )
    .await;
    assert!(first.clients["pending"].same_instance(&second.clients["pending"]));

    first.shutdown_superseded_by(&second).await;
    drop(first);
    assert!(
        !second.clients["pending"].startup_is_cancelled(),
        "superseded cleanup must not cancel a shared client"
    );
    second.cancel_startup();
    assert!(second.clients["pending"].startup_is_cancelled());
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), second.clients["pending"].client()).await,
        Ok(Err(StartupOutcomeError::Cancelled))
    ));
    second.shutdown().await;
    endpoint.abort();
    let _ = endpoint.await;
}

#[tokio::test]
async fn dropping_last_manager_cancels_blocked_startup() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind blocked MCP endpoint");
    let address = listener.local_addr().expect("blocked MCP address");
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let endpoint = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("accept MCP connection");
        let _ = accepted_tx.send(());
        let _socket = socket;
        std::future::pending::<()>().await;
    });
    let servers = HashMap::from([(
        "blocked".to_string(),
        test_http_server(&format!("http://{address}/mcp")),
    )]);
    let (tx, _rx) = async_channel::unbounded();
    let manager = test_reconciled_manager(
        &servers,
        /*previous*/ None,
        Arc::new(EnvironmentManager::without_environments()),
        /*auth_revision*/ 1,
        tx,
    )
    .await;
    let startup = manager.clients["blocked"].client.clone();
    accepted_rx.await.expect("client reaches blocked endpoint");

    drop(manager);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), startup).await,
        Ok(Err(StartupOutcomeError::Cancelled))
    ));
    endpoint.abort();
    let _ = endpoint.await;
}

#[test]
fn elicitation_capability_uses_2025_06_18_shape_for_form_only_support() {
    let capability = Some(ElicitationCapability::default());
    assert_eq!(
        serde_json::to_value(capability).expect("serialize elicitation capability"),
        serde_json::json!({})
    );
}

#[test]
fn elicitation_capability_advertises_url_support_when_enabled() {
    let capability = Some(ElicitationCapability {
        form: Some(rmcp::model::FormElicitationCapability::default()),
        url: Some(rmcp::model::UrlElicitationCapability::default()),
    });
    assert_eq!(
        serde_json::to_value(capability).expect("serialize elicitation capability"),
        serde_json::json!({
            "form": {},
            "url": {},
        })
    );
}

#[test]
fn mcp_init_error_display_prompts_for_github_pat() {
    let server_name = "github";
    let entry = McpAuthStatusEntry {
        config: Some(McpServerConfig {
            auth: Default::default(),
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://api.githubcopilot.com/mcp/".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            enabled: true,
            required: false,
            supports_parallel_tool_calls: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            default_tools_approval_mode: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth: None,
            oauth_resource: None,
            tools: HashMap::new(),
        }),
        auth_state: McpAuthState::Unsupported,
    };
    let err: StartupOutcomeError = anyhow::anyhow!("OAuth is unsupported").into();

    let display = mcp_init_error_display(server_name, Some(&entry), &err);

    let expected = format!(
        "GitHub MCP does not support OAuth. Log in by adding a personal access token (https://github.com/settings/personal-access-tokens) to your environment and config.toml:\n[mcp_servers.{server_name}]\nbearer_token_env_var = CODEX_GITHUB_PERSONAL_ACCESS_TOKEN"
    );

    assert_eq!(expected, display);
}

#[test]
fn mcp_init_error_display_prompts_for_login_when_auth_required() {
    let server_name = "example";
    let err: StartupOutcomeError = anyhow::anyhow!("Auth required for server").into();

    let display = mcp_init_error_display(server_name, /*entry*/ None, &err);

    let expected = format!(
        "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
    );

    assert_eq!(expected, display);
}

#[test]
fn mcp_startup_failure_reason_requires_existing_oauth_and_auth_failure() {
    for (auth_state, is_authentication_required, expected) in [
        (
            Some(McpAuthState::LoggedOut(
                McpLoginRequirement::Reauthentication,
            )),
            true,
            Some(McpStartupFailureReason::ReauthenticationRequired),
        ),
        (
            Some(McpAuthState::LoggedOut(
                McpLoginRequirement::Reauthentication,
            )),
            false,
            None,
        ),
        (
            Some(McpAuthState::LoggedOut(McpLoginRequirement::Login)),
            true,
            None,
        ),
        (Some(McpAuthState::Unsupported), true, None),
        (Some(McpAuthState::BearerToken), true, None),
        (Some(McpAuthState::OAuth), true, None),
        (None, true, None),
    ] {
        let entry = auth_state.map(|auth_state| McpAuthStatusEntry {
            config: None,
            auth_state,
        });
        let error = StartupOutcomeError::Failed {
            error: "startup failed".to_string(),
            is_authentication_required,
        };

        assert_eq!(
            mcp_startup_failure_reason(entry.as_ref(), &error),
            expected,
            "auth_state={auth_state:?}, is_authentication_required={is_authentication_required}"
        );
    }
}

#[test]
fn mcp_init_error_display_reports_generic_errors() {
    let server_name = "custom";
    let entry = McpAuthStatusEntry {
        config: Some(McpServerConfig {
            auth: Default::default(),
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com".to_string(),
                bearer_token_env_var: Some("TOKEN".to_string()),
                http_headers: None,
                env_http_headers: None,
            },
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            enabled: true,
            required: false,
            supports_parallel_tool_calls: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            default_tools_approval_mode: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth: None,
            oauth_resource: None,
            tools: HashMap::new(),
        }),
        auth_state: McpAuthState::Unsupported,
    };
    let err: StartupOutcomeError = anyhow::anyhow!("boom").into();

    let display = mcp_init_error_display(server_name, Some(&entry), &err);

    let expected = format!("MCP client for `{server_name}` failed to start: {err:#}");

    assert_eq!(expected, display);
}

#[test]
fn mcp_init_error_display_includes_startup_timeout_hint() {
    let server_name = "slow";
    let err: StartupOutcomeError = anyhow::anyhow!("request timed out").into();

    let display = mcp_init_error_display(server_name, /*entry*/ None, &err);

    assert_eq!(
        "MCP client for `slow` timed out after 30 seconds. Add or adjust `startup_timeout_sec` in your config.toml:\n[mcp_servers.slow]\nstartup_timeout_sec = XX",
        display
    );
}
