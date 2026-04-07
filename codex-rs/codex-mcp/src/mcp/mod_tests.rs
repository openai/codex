use super::*;
use codex_config::Constrained;
use codex_login::CodexAuth;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn test_mcp_config(codex_home: PathBuf) -> McpConfig {
    McpConfig {
        chatgpt_base_url: "https://chatgpt.com".to_string(),
        codex_home,
        mcp_oauth_credentials_store_mode: OAuthCredentialsStoreMode::default(),
        mcp_oauth_callback_port: None,
        mcp_oauth_callback_url: None,
        skill_mcp_dependency_install_enabled: true,
        approval_policy: Constrained::allow_any(AskForApproval::OnFailure),
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: false,
        apps_enabled: false,
        configured_mcp_servers: HashMap::new(),
        plugin_capability_summaries: Vec::new(),
    }
}

fn make_tool_info(server_name: &str, tool_name: &str, tool_namespace: &str) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_string(),
        tool_name: tool_name.to_string(),
        tool_namespace: tool_namespace.to_string(),
        tool: rmcp::model::Tool {
            name: tool_name.to_string().into(),
            title: None,
            description: None,
            input_schema: Arc::new(JsonObject::default()),
            output_schema: None,
            annotations: None,
            execution: None,
            icons: None,
            meta: None,
        },
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
        connector_description: None,
    }
}

#[test]
fn qualified_mcp_tool_name_prefix_sanitizes_server_names_without_lowercasing() {
    assert_eq!(
        qualified_mcp_tool_name_prefix("Some-Server"),
        "mcp__Some_Server__".to_string()
    );
}

#[test]
fn mcp_server_status_tool_name_preserves_hyphenated_mcp_tool_names() {
    let tool_info = make_tool_info("music-studio", "play-live-pattern", "music-studio");

    assert_eq!(
        mcp_server_status_tool_name(&tool_info),
        "play-live-pattern".to_string()
    );
}

#[test]
fn mcp_server_status_tool_name_includes_codex_apps_connector_namespace() {
    let tool_info = make_tool_info(
        CODEX_APPS_MCP_SERVER_NAME,
        "_property_search",
        "mcp__codex_apps__zillow",
    );

    assert_eq!(
        mcp_server_status_tool_name(&tool_info),
        "zillow_property_search".to_string()
    );
}

#[test]
fn tool_plugin_provenance_collects_app_and_mcp_sources() {
    let provenance = ToolPluginProvenance::from_capability_summaries(&[
        PluginCapabilitySummary {
            display_name: "alpha-plugin".to_string(),
            app_connector_ids: vec![AppConnectorId("connector_example".to_string())],
            mcp_server_names: vec!["alpha".to_string()],
            ..PluginCapabilitySummary::default()
        },
        PluginCapabilitySummary {
            display_name: "beta-plugin".to_string(),
            app_connector_ids: vec![
                AppConnectorId("connector_example".to_string()),
                AppConnectorId("connector_gmail".to_string()),
            ],
            mcp_server_names: vec!["beta".to_string()],
            ..PluginCapabilitySummary::default()
        },
    ]);

    assert_eq!(
        provenance,
        ToolPluginProvenance {
            plugin_display_names_by_connector_id: HashMap::from([
                (
                    "connector_example".to_string(),
                    vec!["alpha-plugin".to_string(), "beta-plugin".to_string()],
                ),
                (
                    "connector_gmail".to_string(),
                    vec!["beta-plugin".to_string()],
                ),
            ]),
            plugin_display_names_by_mcp_server_name: HashMap::from([
                ("alpha".to_string(), vec!["alpha-plugin".to_string()]),
                ("beta".to_string(), vec!["beta-plugin".to_string()]),
            ]),
        }
    );
}

#[test]
fn codex_apps_mcp_url_for_base_url_keeps_existing_paths() {
    assert_eq!(
        codex_apps_mcp_url_for_base_url("https://chatgpt.com/backend-api"),
        "https://chatgpt.com/backend-api/wham/apps"
    );
    assert_eq!(
        codex_apps_mcp_url_for_base_url("https://chat.openai.com"),
        "https://chat.openai.com/backend-api/wham/apps"
    );
    assert_eq!(
        codex_apps_mcp_url_for_base_url("http://localhost:8080/api/codex"),
        "http://localhost:8080/api/codex/apps"
    );
    assert_eq!(
        codex_apps_mcp_url_for_base_url("http://localhost:8080"),
        "http://localhost:8080/api/codex/apps"
    );
}

#[test]
fn codex_apps_mcp_url_uses_legacy_codex_apps_path() {
    let config = test_mcp_config(PathBuf::from("/tmp"));

    assert_eq!(
        codex_apps_mcp_url(&config),
        "https://chatgpt.com/backend-api/wham/apps"
    );
}

#[test]
fn codex_apps_server_config_uses_legacy_codex_apps_path() {
    let mut config = test_mcp_config(PathBuf::from("/tmp"));
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    let mut servers = with_codex_apps_mcp(HashMap::new(), /*auth*/ None, &config);
    assert!(!servers.contains_key(CODEX_APPS_MCP_SERVER_NAME));

    config.apps_enabled = true;

    servers = with_codex_apps_mcp(servers, Some(&auth), &config);
    let server = servers
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .expect("codex apps should be present when apps is enabled");
    let url = match &server.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => url,
        _ => panic!("expected streamable http transport for codex apps"),
    };

    assert_eq!(url, "https://chatgpt.com/backend-api/wham/apps");
}

#[tokio::test]
async fn effective_mcp_servers_preserve_user_servers_and_add_codex_apps() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let mut config = test_mcp_config(codex_home.path().to_path_buf());
    config.apps_enabled = true;
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();

    config.configured_mcp_servers.insert(
        "sample".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://user.example/mcp".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );
    config.configured_mcp_servers.insert(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://docs.example/mcp".to_string(),
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );

    let effective = effective_mcp_servers(&config, Some(&auth));

    let sample = effective.get("sample").expect("user server should exist");
    let docs = effective
        .get("docs")
        .expect("configured server should exist");
    let codex_apps = effective
        .get(CODEX_APPS_MCP_SERVER_NAME)
        .expect("codex apps server should exist");

    match &sample.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://user.example/mcp");
        }
        other => panic!("expected streamable http transport, got {other:?}"),
    }
    match &docs.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://docs.example/mcp");
        }
        other => panic!("expected streamable http transport, got {other:?}"),
    }
    match &codex_apps.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://chatgpt.com/backend-api/wham/apps");
        }
        other => panic!("expected streamable http transport, got {other:?}"),
    }
}
