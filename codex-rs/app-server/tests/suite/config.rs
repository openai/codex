use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpOAuthCredentialsStoreMode;
use codex_app_server_protocol::McpServerConfig as ProtocolMcpServerConfig;
use codex_app_server_protocol::McpServerTransportConfig as ProtocolMcpServerTransportConfig;
use codex_app_server_protocol::Profile;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxSettings;
use codex_app_server_protocol::Tools;
use codex_app_server_protocol::UpdateConfigParams;
use codex_app_server_protocol::UpdateConfigResponse;
use codex_app_server_protocol::UserSavedConfig;
use codex_core::config::ConfigToml;
use codex_core::protocol::AskForApproval;
use codex_protocol::config_types::ForcedLoginMethod;
use codex_protocol::config_types::ReasoningEffort;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::Verbosity;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn create_config_toml(codex_home: &Path) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        r#"
model = "gpt-5-codex"
approval_policy = "on-request"
sandbox_mode = "workspace-write"
model_reasoning_summary = "detailed"
model_reasoning_effort = "high"
model_verbosity = "medium"
profile = "test"
forced_chatgpt_workspace_id = "12345678-0000-0000-0000-000000000000"
forced_login_method = "chatgpt"
mcp_oauth_credentials_store = "keyring"

[sandbox_workspace_write]
writable_roots = ["/tmp"]
network_access = true
exclude_tmpdir_env_var = true
exclude_slash_tmp = true

[tools]
web_search = false
view_image = true

[profiles.test]
model = "gpt-4o"
approval_policy = "on-request"
model_reasoning_effort = "high"
model_reasoning_summary = "detailed"
model_verbosity = "medium"
model_provider = "openai"
chatgpt_base_url = "https://api.chatgpt.com"

[mcp_servers.docs]
command = "codex-docs"
args = ["serve"]
env_vars = ["DOCS_TOKEN"]
cwd = "/tmp/docs"
startup_timeout_sec = 12.5
tool_timeout_sec = 42.0
enabled = false
enabled_tools = ["read_docs"]
disabled_tools = ["delete_docs"]

[mcp_servers.docs.env]
PLAN = "gold"

[mcp_servers.issues]
url = "https://example.com/mcp"
bearer_token_env_var = "MCP_TOKEN"
startup_timeout_sec = 30.0
tool_timeout_sec = 15.0

[mcp_servers.issues.http_headers]
"X-Test" = "42"

[mcp_servers.issues.env_http_headers]
"X-Token" = "TOKEN_ENV"
"#,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn get_config_toml_parses_all_fields() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_get_config_request().await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let config: UserSavedConfig = to_response(resp)?;
    let expected = UserSavedConfig {
        approval_policy: Some(AskForApproval::OnRequest),
        sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        sandbox_settings: Some(SandboxSettings {
            writable_roots: vec!["/tmp".into()],
            network_access: Some(true),
            exclude_tmpdir_env_var: Some(true),
            exclude_slash_tmp: Some(true),
        }),
        forced_chatgpt_workspace_id: Some("12345678-0000-0000-0000-000000000000".into()),
        forced_login_method: Some(ForcedLoginMethod::Chatgpt),
        model: Some("gpt-5-codex".into()),
        model_reasoning_effort: Some(ReasoningEffort::High),
        model_reasoning_summary: Some(ReasoningSummary::Detailed),
        model_verbosity: Some(Verbosity::Medium),
        tools: Some(Tools {
            web_search: Some(false),
            view_image: Some(true),
        }),
        mcp_servers: HashMap::from([
            (
                "docs".into(),
                ProtocolMcpServerConfig {
                    transport: ProtocolMcpServerTransportConfig::Stdio {
                        command: "codex-docs".into(),
                        args: vec!["serve".into()],
                        env: Some(HashMap::from([("PLAN".into(), "gold".into())])),
                        env_vars: vec!["DOCS_TOKEN".into()],
                        cwd: Some("/tmp/docs".into()),
                    },
                    enabled: false,
                    startup_timeout_sec: Some(12.5),
                    tool_timeout_sec: Some(42.0),
                    enabled_tools: Some(vec!["read_docs".into()]),
                    disabled_tools: Some(vec!["delete_docs".into()]),
                },
            ),
            (
                "issues".into(),
                ProtocolMcpServerConfig {
                    transport: ProtocolMcpServerTransportConfig::StreamableHttp {
                        url: "https://example.com/mcp".into(),
                        bearer_token_env_var: Some("MCP_TOKEN".into()),
                        http_headers: Some(HashMap::from([("X-Test".into(), "42".into())])),
                        env_http_headers: Some(HashMap::from([(
                            "X-Token".into(),
                            "TOKEN_ENV".into(),
                        )])),
                    },
                    enabled: true,
                    startup_timeout_sec: Some(30.0),
                    tool_timeout_sec: Some(15.0),
                    enabled_tools: None,
                    disabled_tools: None,
                },
            ),
        ]),
        mcp_oauth_credentials_store: Some(McpOAuthCredentialsStoreMode::Keyring),
        profile: Some("test".to_string()),
        profiles: HashMap::from([(
            "test".into(),
            Profile {
                model: Some("gpt-4o".into()),
                approval_policy: Some(AskForApproval::OnRequest),
                model_reasoning_effort: Some(ReasoningEffort::High),
                model_reasoning_summary: Some(ReasoningSummary::Detailed),
                model_verbosity: Some(Verbosity::Medium),
                model_provider: Some("openai".into()),
                chatgpt_base_url: Some("https://api.chatgpt.com".into()),
            },
        )]),
    };

    assert_eq!(config, expected);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_config_toml_empty() -> Result<()> {
    let codex_home = TempDir::new()?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp.send_get_config_request().await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let config: UserSavedConfig = to_response(resp)?;
    let expected = UserSavedConfig {
        approval_policy: None,
        sandbox_mode: None,
        sandbox_settings: None,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        model: None,
        model_reasoning_effort: None,
        model_reasoning_summary: None,
        model_verbosity: None,
        tools: None,
        mcp_servers: HashMap::new(),
        mcp_oauth_credentials_store: None,
        profile: None,
        profiles: HashMap::new(),
    };

    assert_eq!(config, expected);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn update_config_persists_all_fields() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let desired = sample_user_saved_config();

    let request_id = mcp
        .send_update_config_request(UpdateConfigParams {
            config: desired.clone(),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: UpdateConfigResponse = to_response(resp)?;
    assert_eq!(response.config, desired);

    let config_contents = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let config_toml: ConfigToml = toml::from_str(&config_contents)?;
    let persisted: UserSavedConfig = config_toml.into();
    assert_eq!(persisted, desired);

    let read_request_id = mcp.send_get_config_request().await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_request_id)),
    )
    .await??;
    let read_config: UserSavedConfig = to_response(read_resp)?;
    assert_eq!(read_config, desired);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn update_config_clears_missing_fields() -> Result<()> {
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let desired = empty_user_saved_config();
    let request_id = mcp
        .send_update_config_request(UpdateConfigParams {
            config: desired.clone(),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: UpdateConfigResponse = to_response(resp)?;
    assert_eq!(response.config, desired);

    let config_contents = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let config_toml: ConfigToml = toml::from_str(&config_contents)?;
    let persisted: UserSavedConfig = config_toml.into();
    assert_eq!(persisted, desired);

    let read_request_id = mcp.send_get_config_request().await?;
    let read_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(read_request_id)),
    )
    .await??;
    let read_config: UserSavedConfig = to_response(read_resp)?;
    assert_eq!(read_config, desired);
    Ok(())
}

fn empty_user_saved_config() -> UserSavedConfig {
    UserSavedConfig {
        approval_policy: None,
        sandbox_mode: None,
        sandbox_settings: None,
        forced_chatgpt_workspace_id: None,
        forced_login_method: None,
        model: None,
        model_reasoning_effort: None,
        model_reasoning_summary: None,
        model_verbosity: None,
        tools: None,
        mcp_servers: HashMap::new(),
        mcp_oauth_credentials_store: None,
        profile: None,
        profiles: HashMap::new(),
    }
}

fn sample_user_saved_config() -> UserSavedConfig {
    UserSavedConfig {
        approval_policy: Some(AskForApproval::OnRequest),
        sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        sandbox_settings: Some(SandboxSettings {
            writable_roots: vec!["/tmp".into()],
            network_access: Some(true),
            exclude_tmpdir_env_var: Some(true),
            exclude_slash_tmp: Some(true),
        }),
        forced_chatgpt_workspace_id: Some("12345678-0000-0000-0000-000000000000".into()),
        forced_login_method: Some(ForcedLoginMethod::Chatgpt),
        model: Some("gpt-5-codex".into()),
        model_reasoning_effort: Some(ReasoningEffort::High),
        model_reasoning_summary: Some(ReasoningSummary::Detailed),
        model_verbosity: Some(Verbosity::Medium),
        tools: Some(Tools {
            web_search: Some(false),
            view_image: Some(true),
        }),
        mcp_servers: HashMap::from([
            (
                "docs".into(),
                ProtocolMcpServerConfig {
                    transport: ProtocolMcpServerTransportConfig::Stdio {
                        command: "codex-docs".into(),
                        args: vec!["serve".into()],
                        env: Some(HashMap::from([("PLAN".into(), "gold".into())])),
                        env_vars: vec!["DOCS_TOKEN".into()],
                        cwd: Some("/tmp/docs".into()),
                    },
                    enabled: false,
                    startup_timeout_sec: Some(12.5),
                    tool_timeout_sec: Some(42.0),
                    enabled_tools: Some(vec!["read_docs".into()]),
                    disabled_tools: Some(vec!["delete_docs".into()]),
                },
            ),
            (
                "issues".into(),
                ProtocolMcpServerConfig {
                    transport: ProtocolMcpServerTransportConfig::StreamableHttp {
                        url: "https://example.com/mcp".into(),
                        bearer_token_env_var: Some("MCP_TOKEN".into()),
                        http_headers: Some(HashMap::from([("X-Test".into(), "42".into())])),
                        env_http_headers: Some(HashMap::from([(
                            "X-Token".into(),
                            "TOKEN_ENV".into(),
                        )])),
                    },
                    enabled: true,
                    startup_timeout_sec: Some(30.0),
                    tool_timeout_sec: Some(15.0),
                    enabled_tools: None,
                    disabled_tools: None,
                },
            ),
        ]),
        mcp_oauth_credentials_store: Some(McpOAuthCredentialsStoreMode::Keyring),
        profile: Some("test".into()),
        profiles: HashMap::from([(
            "test".into(),
            Profile {
                model: Some("gpt-4o".into()),
                approval_policy: Some(AskForApproval::OnRequest),
                model_reasoning_effort: Some(ReasoningEffort::High),
                model_reasoning_summary: Some(ReasoningSummary::Detailed),
                model_verbosity: Some(Verbosity::Medium),
                model_provider: Some("openai".into()),
                chatgpt_base_url: Some("https://api.chatgpt.com".into()),
            },
        )]),
    }
}
