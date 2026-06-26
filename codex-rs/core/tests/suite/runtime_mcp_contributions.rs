#![cfg(not(target_os = "windows"))]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use anyhow::Result;
use codex_config::McpServerAuth;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpServerRuntimeMetadata;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;

const PRIVATE_APPROVAL_CONTEXT_EMAIL: &str = "private-owner@example.com";

struct MutableMcpContributor {
    command: String,
    use_second_server: AtomicBool,
    revision: AtomicU64,
}

impl MutableMcpContributor {
    fn new(command: String) -> Self {
        Self {
            command,
            use_second_server: AtomicBool::new(false),
            revision: AtomicU64::new(0),
        }
    }

    fn publish_second_server(&self) {
        self.use_second_server.store(true, Ordering::Release);
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    fn server(&self) -> (&'static str, McpServerConfig) {
        let name = if self.use_second_server.load(Ordering::Acquire) {
            "second"
        } else {
            "first"
        };
        (
            name,
            McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: self.command.clone(),
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                auth: McpServerAuth::default(),
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
            },
        )
    }
}

impl McpServerContributor<Config> for MutableMcpContributor {
    fn id(&self) -> &'static str {
        "mutable-test"
    }

    fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let (name, config) = self.server();
            vec![McpServerContribution::Set {
                name: name.to_string(),
                config: Box::new(config),
            }]
        })
    }
}

struct PublishSecondServerTool {
    contributor: Arc<MutableMcpContributor>,
}

impl ToolExecutor<ToolCall> for PublishSecondServerTool {
    fn tool_name(&self) -> codex_tools::ToolName {
        codex_tools::ToolName::plain("publish_second_server")
    }

    fn spec(&self) -> codex_tools::ToolSpec {
        codex_tools::ToolSpec::Function(codex_tools::ResponsesApiTool {
            name: "publish_second_server".to_string(),
            description: "Publishes a replacement MCP server for this test.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: codex_tools::JsonSchema::default(),
            output_schema: None,
        })
    }

    fn handle(&self, _call: ToolCall) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
            self.contributor.publish_second_server();
            Ok(Box::new(codex_tools::JsonToolOutput::new(
                serde_json::json!({"published": true}),
            )) as Box<dyn codex_tools::ToolOutput>)
        })
    }
}

impl ToolContributor for PublishSecondServerTool {
    fn tools(
        &self,
        _session_store: &codex_extension_api::ExtensionData,
        _thread_store: &codex_extension_api::ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        vec![Arc::new(Self {
            contributor: Arc::clone(&self.contributor),
        })]
    }
}

struct TrustedApprovalContextContributor {
    command: String,
}

impl McpServerContributor<Config> for TrustedApprovalContextContributor {
    fn id(&self) -> &'static str {
        "trusted-approval-context-test"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move {
            let config = McpServerConfig {
                transport: McpServerTransportConfig::Stdio {
                    command: self.command.clone(),
                    args: Vec::new(),
                    env: Some(HashMap::from([(
                        "MCP_TEST_APPROVAL_CONTEXT_EMAIL".to_string(),
                        PRIVATE_APPROVAL_CONTEXT_EMAIL.to_string(),
                    )])),
                    env_vars: Vec::new(),
                    cwd: None,
                },
                auth: McpServerAuth::default(),
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
            };
            let server = EffectiveMcpServer::configured(config).with_runtime_metadata(
                McpServerRuntimeMetadata::default().with_trusted_approval_context(),
            );
            vec![McpServerContribution::SetEffective {
                name: "private-context".to_string(),
                server: Box::new(server),
            }]
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contributor_revision_replaces_the_ordinary_mcp_server_set() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let model_server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &model_server,
        vec![
            sse(vec![
                ev_response_created("response-1"),
                ev_function_call("publish", "publish_second_server", "{}"),
                ev_completed("response-1"),
            ]),
            sse(vec![
                ev_response_created("response-2"),
                ev_tool_search_call("search", &serde_json::json!({"query": "echo"})),
                ev_completed("response-2"),
            ]),
            sse(vec![
                ev_response_created("response-3"),
                ev_assistant_message("message-2", "second complete"),
                ev_completed("response-3"),
            ]),
        ],
    )
    .await;
    let contributor = Arc::new(MutableMcpContributor::new(stdio_server_bin()?));
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.mcp_server_contributor(contributor.clone());
    extensions.tool_contributor(Arc::new(PublishSecondServerTool {
        contributor: Arc::clone(&contributor),
    }));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build(&model_server)
        .await?;
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        wait_for_mcp_server(&test.codex, "first"),
    )
    .await??;

    test.submit_turn("replace the MCP server").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let second_tools = requests[2].tool_search_output("search");
    assert!(
        core_test_support::responses::namespace_child_tool(&second_tools, "mcp__first", "echo")
            .is_none()
    );
    assert!(
        core_test_support::responses::namespace_child_tool(&second_tools, "mcp__second", "echo")
            .is_some()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_approval_context_never_enters_model_requests_or_rollout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let model_server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &model_server,
        vec![
            sse(vec![
                ev_response_created("response-1"),
                ev_tool_search_call("search", &serde_json::json!({"query": "echo message"})),
                ev_completed("response-1"),
            ]),
            sse(vec![
                ev_response_created("response-2"),
                ev_assistant_message("message-2", "done"),
                ev_completed("response-2"),
            ]),
        ],
    )
    .await;
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.mcp_server_contributor(Arc::new(TrustedApprovalContextContributor {
        command: stdio_server_bin()?,
    }));
    let test = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build(&model_server)
        .await?;
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        wait_for_mcp_server(&test.codex, "private-context"),
    )
    .await??;

    test.submit_turn("find the echo tool").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2);
    let search_output = requests[1].tool_search_output("search");
    assert!(
        core_test_support::responses::namespace_child_tool(
            &search_output,
            "mcp__private_context",
            "echo",
        )
        .is_some(),
        "tool_search should expose the MCP tool without its private metadata: {search_output:?}"
    );
    for request in &requests {
        let serialized = serde_json::to_string(&request.body_json())?;
        assert!(!serialized.contains(PRIVATE_APPROVAL_CONTEXT_EMAIL));
        assert!(!serialized.contains(codex_protocol::mcp::MCP_APPROVAL_CONTEXT_META_KEY));
        assert!(
            !serialized
                .contains(codex_protocol::mcp::MCP_APPROVAL_CONTEXT_CONNECTED_ACCOUNT_EMAIL_KEY)
        );
    }

    test.codex.flush_rollout().await?;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let rollout = tokio::fs::read_to_string(rollout_path).await?;
    assert!(!rollout.contains(PRIVATE_APPROVAL_CONTEXT_EMAIL));
    assert!(!rollout.contains(codex_protocol::mcp::MCP_APPROVAL_CONTEXT_META_KEY));
    assert!(
        !rollout.contains(codex_protocol::mcp::MCP_APPROVAL_CONTEXT_CONNECTED_ACCOUNT_EMAIL_KEY)
    );

    Ok(())
}
