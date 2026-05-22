use super::*;
use crate::codex_apps::CodexAppsToolsCacheKey;
use crate::mcp::ToolPluginProvenance;
use crate::runtime::McpRuntimeContext;
use crate::server::EffectiveMcpServer;
use codex_config::Constrained;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::EnvironmentManager;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use rmcp::model::ElicitationCapability;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn no_local_runtime_fails_local_stdio_but_keeps_local_http_server() {
    let approval_policy = Constrained::allow_any(AskForApproval::OnFailure);
    let (tx_event, rx_event) = async_channel::unbounded();
    drop(rx_event);
    let codex_home = tempdir().expect("tempdir");
    let mcp_servers = HashMap::from([
        (
            "stdio".to_string(),
            EffectiveMcpServer::configured(McpServerConfig {
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

    let (manager, cancel_token) = McpConnectionManager::new(
        &mcp_servers,
        OAuthCredentialsStoreMode::default(),
        HashMap::new(),
        &approval_policy,
        String::new(),
        tx_event,
        PermissionProfile::default(),
        McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments()),
            PathBuf::from("/tmp"),
        ),
        codex_home.path().to_path_buf(),
        CodexAppsToolsCacheKey {
            account_id: None,
            chatgpt_user_id: None,
            is_workspace_account: false,
        },
        /*host_owned_codex_apps_enabled*/ false,
        ElicitationCapability::default(),
        ToolPluginProvenance::default(),
        /*auth*/ None,
        /*elicitation_reviewer*/ None,
    )
    .await;

    assert!(manager.client_pool.has_server("stdio"));
    assert!(manager.client_pool.has_server("http"));
    assert!(
        !manager
            .wait_for_server_ready("stdio", Duration::from_millis(10))
            .await
    );
    let failures = manager
        .required_startup_failures(&["stdio".to_string()])
        .await;
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].server, "stdio");
    assert_eq!(
        failures[0].error,
        "local stdio MCP server `stdio` requires a local environment"
    );
    cancel_token.cancel();
}
