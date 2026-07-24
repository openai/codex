use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_features::Feature;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::process::Command;
use wiremock::MockServer;

const PROXY_TEST_SUBPROCESS_ENV_VAR: &str = "CODEX_MCP_HTTP_PROXY_TEST_SUBPROCESS";
const TEST_NAME: &str = "suite::mcp_startup_refresh_http_proxy::local_mcp_startup_and_refresh_use_configured_http_client";
const SERVER_NAME: &str = "proxied_mcp";
const SERVER_URL: &str = "http://mcp-proxy.invalid/api/codex/ps/mcp";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_mcp_startup_and_refresh_use_configured_http_client() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if std::env::var_os(PROXY_TEST_SUBPROCESS_ENV_VAR).is_none() {
        let proxy = MockServer::start().await;
        let _apps_server = AppsTestServer::mount(&proxy).await?;
        let mut command = Command::new(std::env::current_exe()?);
        command.arg("--exact").arg(TEST_NAME);
        for &key in codex_network_proxy::PROXY_ENV_KEYS {
            command.env_remove(key);
        }
        command
            .env(PROXY_TEST_SUBPROCESS_ENV_VAR, "1")
            .env("HTTP_PROXY", proxy.uri())
            .env("http_proxy", proxy.uri())
            .env("NO_PROXY", codex_network_proxy::DEFAULT_NO_PROXY_VALUE)
            .env("no_proxy", codex_network_proxy::DEFAULT_NO_PROXY_VALUE);

        let output = command.output().await?;
        let requests = proxy
            .received_requests()
            .await
            .expect("mock proxy should record MCP requests");
        assert!(
            output.status.success(),
            "subprocess test `{TEST_NAME}` failed\nstdout:\n{}\nstderr:\n{}\nproxy requests:\n{requests:#?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let initialize_authorizations = requests
            .iter()
            .filter_map(|request| {
                let body = serde_json::from_slice::<Value>(&request.body).ok()?;
                (body.get("method").and_then(Value::as_str) == Some("initialize"))
                    .then(|| {
                        request
                            .headers
                            .get("authorization")
                            .and_then(|header| header.to_str().ok())
                            .map(str::to_string)
                    })
                    .flatten()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            initialize_authorizations,
            vec!["Bearer initial", "Bearer refreshed"]
        );
        return Ok(());
    }

    let responses_server = responses::start_mock_server().await;
    let fixture = test_codex()
        .with_config(|config| {
            if cfg!(target_os = "linux") {
                config
                    .features
                    .enable(Feature::RespectSystemProxy)
                    .expect("test config should allow the system proxy feature");
                config.respect_system_proxy = true;
            }
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                SERVER_NAME.to_string(),
                McpServerConfig {
                    auth: Default::default(),
                    transport: McpServerTransportConfig::StreamableHttp {
                        url: SERVER_URL.to_string(),
                        bearer_token_env_var: None,
                        http_headers: Some(HashMap::from([(
                            "Authorization".to_string(),
                            "Bearer initial".to_string(),
                        )])),
                        env_http_headers: None,
                    },
                    environment_id: DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test MCP servers should accept any configuration");
        })
        .build_with_auto_env(&responses_server)
        .await?;
    wait_for_mcp_server(&fixture.codex, SERVER_NAME).await?;

    let mut refreshed_config = fixture.config.clone();
    let mut servers = refreshed_config.mcp_servers.get().clone();
    let server = servers
        .get_mut(SERVER_NAME)
        .expect("configured MCP server should exist");
    let McpServerTransportConfig::StreamableHttp { http_headers, .. } = &mut server.transport
    else {
        unreachable!("test MCP server should use streamable HTTP");
    };
    *http_headers = Some(HashMap::from([(
        "Authorization".to_string(),
        "Bearer refreshed".to_string(),
    )]));
    refreshed_config
        .mcp_servers
        .set(servers)
        .expect("test MCP servers should accept the refreshed configuration");
    fixture.codex.refresh_runtime_config(refreshed_config).await;
    let result = fixture
        .codex
        .call_mcp_tool(
            SERVER_NAME,
            "calendar_create_event",
            Some(json!({
                "title": "Proxy refresh",
                "starts_at": "2026-07-23T12:00:00Z",
            })),
            /*meta*/ None,
        )
        .await?;
    assert_eq!(result.is_error, Some(false));
    Ok(())
}
