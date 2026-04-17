use super::*;
use codex_config::Constrained;
use codex_login::CodexAuth;
use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use codex_protocol::protocol::AskForApproval;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;

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
        mcp_server_allowlist: None,
        plugin_capability_summaries: Vec::new(),
    }
}

fn http_mcp_server(url: &str) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        experimental_environment: None,
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
        oauth_resource: None,
        tools: HashMap::new(),
    }
}

fn make_tool(name: &str) -> Tool {
    Tool {
        name: name.to_string(),
        title: None,
        description: None,
        input_schema: serde_json::json!({"type": "object", "properties": {}}),
        output_schema: None,
        annotations: None,
        icons: None,
        meta: None,
    }
}

#[test]
fn split_qualified_tool_name_returns_server_and_tool() {
    assert_eq!(
        split_qualified_tool_name("mcp__alpha__do_thing"),
        Some(("alpha".to_string(), "do_thing".to_string()))
    );
}

#[test]
fn qualified_mcp_tool_name_prefix_sanitizes_server_names_without_lowercasing() {
    assert_eq!(
        qualified_mcp_tool_name_prefix("Some-Server"),
        "mcp__Some_Server__".to_string()
    );
}

#[test]
fn split_qualified_tool_name_rejects_invalid_names() {
    assert_eq!(split_qualified_tool_name("other__alpha__do_thing"), None);
    assert_eq!(split_qualified_tool_name("mcp__alpha__"), None);
}

#[test]
fn group_tools_by_server_strips_prefix_and_groups() {
    let mut tools = HashMap::new();
    tools.insert("mcp__alpha__do_thing".to_string(), make_tool("do_thing"));
    tools.insert(
        "mcp__alpha__nested__op".to_string(),
        make_tool("nested__op"),
    );
    tools.insert("mcp__beta__do_other".to_string(), make_tool("do_other"));

    let mut expected_alpha = HashMap::new();
    expected_alpha.insert("do_thing".to_string(), make_tool("do_thing"));
    expected_alpha.insert("nested__op".to_string(), make_tool("nested__op"));

    let mut expected_beta = HashMap::new();
    expected_beta.insert("do_other".to_string(), make_tool("do_other"));

    let mut expected = HashMap::new();
    expected.insert("alpha".to_string(), expected_alpha);
    expected.insert("beta".to_string(), expected_beta);

    assert_eq!(group_tools_by_server(&tools), expected);
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
        http_mcp_server("https://user.example/mcp"),
    );
    config.configured_mcp_servers.insert(
        "docs".to_string(),
        http_mcp_server("https://docs.example/mcp"),
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

#[tokio::test]
async fn effective_mcp_servers_respects_session_allowlist() {
    let mut config = test_mcp_config(PathBuf::from("/tmp"));
    config.apps_enabled = true;
    config.configured_mcp_servers.insert(
        "docs".to_string(),
        http_mcp_server("https://docs.example/mcp"),
    );
    config.configured_mcp_servers.insert(
        "linear".to_string(),
        http_mcp_server("https://linear.example/mcp"),
    );
    config.mcp_server_allowlist = Some(vec![
        "docs".to_string(),
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
    ]);

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let effective = effective_mcp_servers(&config, Some(&auth));

    assert_eq!(
        effective
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            "docs".to_string(),
            CODEX_APPS_MCP_SERVER_NAME.to_string(),
        ])
    );
}
