use std::sync::Arc;
use std::time::Duration;

use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::McpManager;
use codex_core_plugins::PluginsManager;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use core_test_support::apps_test_server::AppsTestServer;
use serde_json::Value;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::MockServer;

use super::*;

const TEST_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn refreshes_codex_apps_tools_after_first_interval_and_periodically() {
    let codex_home = TempDir::new().expect("create Codex home");
    let server = MockServer::start().await;
    let apps_server = AppsTestServer::mount(&server)
        .await
        .expect("mount apps server");
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"chatgpt_base_url = "{}"
cli_auth_credentials_store = "file"
mcp_oauth_credentials_store = "file"

[features]
apps = true
"#,
            apps_server.chatgpt_base_url
        ),
    )
    .expect("write config");
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("access-chatgpt"),
        AuthCredentialsStoreMode::File,
    )
    .expect("write auth");

    let config_manager =
        ConfigManager::without_managed_config_for_tests(codex_home.path().to_path_buf());
    let runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        Some(std::env::current_exe().expect("resolve test executable")),
        /*codex_linux_sandbox_exe*/ None,
    )
    .expect("resolve exec server runtime paths");
    let environment_manager = Arc::new(
        EnvironmentManager::from_codex_home(codex_home.path().to_path_buf(), Some(runtime_paths))
            .await
            .expect("create environment manager"),
    );
    let plugins_manager = Arc::new(PluginsManager::new(codex_home.path().to_path_buf()));
    let mcp_manager = Arc::new(McpManager::new(plugins_manager));
    let worker = spawn_with_interval(
        config_manager,
        &environment_manager,
        &mcp_manager,
        TEST_REFRESH_INTERVAL,
    );

    wait_for_tools_list_request_count(&server, /*expected*/ 4).await;
    drop(worker);
    let request_count_after_shutdown = tools_list_request_count(&server).await;
    tokio::time::sleep(TEST_REFRESH_INTERVAL * 2).await;

    assert_eq!(
        tools_list_request_count(&server).await,
        request_count_after_shutdown
    );
}

async fn wait_for_tools_list_request_count(server: &MockServer, expected: usize) {
    timeout(TEST_TIMEOUT, async {
        while tools_list_request_count(server).await < expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("expected {expected} Codex Apps tools/list requests"));
}

async fn tools_list_request_count(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|request| {
            request.url.path() == "/api/codex/apps"
                && serde_json::from_slice::<Value>(&request.body)
                    .ok()
                    .and_then(|body| body.get("method").cloned())
                    .as_ref()
                    .and_then(Value::as_str)
                    == Some("tools/list")
        })
        .count()
}
