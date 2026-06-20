use std::sync::Arc;
use std::time::Duration;

use codex_core_plugins::PluginsManager;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;

const TEST_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::test]
async fn refreshes_remote_installed_plugins_immediately_and_periodically() {
    let codex_home = TempDir::new().expect("create Codex home");
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"chatgpt_base_url = "{}/backend-api/"

[features]
plugins = true
"#,
            server.uri()
        ),
    )
    .expect("write config");
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .respond_with(ResponseTemplate::new(200).set_body_string(empty_installed_plugins_body()))
        .mount(&server)
        .await;

    let plugins_manager = Arc::new(PluginsManager::new(codex_home.path().to_path_buf()));
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let config_manager =
        ConfigManager::without_managed_config_for_tests(codex_home.path().to_path_buf());
    let worker = spawn_with_interval(
        &plugins_manager,
        &auth_manager,
        config_manager,
        Arc::new(|| {}),
        TEST_REFRESH_INTERVAL,
    );

    wait_for_bundle_sync_request_count(&server, /*expected*/ 6).await;
    drop(worker);
    tokio::time::sleep(TEST_REFRESH_INTERVAL * 2).await;
    let request_count_after_shutdown = bundle_sync_request_count(&server).await;
    tokio::time::sleep(TEST_REFRESH_INTERVAL * 2).await;

    assert_eq!(
        bundle_sync_request_count(&server).await,
        request_count_after_shutdown
    );
}

async fn wait_for_bundle_sync_request_count(server: &MockServer, expected: usize) {
    timeout(TEST_TIMEOUT, async {
        while bundle_sync_request_count(server).await < expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("expected {expected} remote installed plugin bundle requests"));
}

async fn bundle_sync_request_count(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter(|request| {
            request.url.path() == "/backend-api/ps/plugins/installed"
                && request
                    .url
                    .query_pairs()
                    .any(|(key, value)| key == "includeDownloadUrls" && value == "true")
        })
        .count()
}

fn empty_installed_plugins_body() -> &'static str {
    r#"{
  "plugins": [],
  "pagination": {
    "limit": 50,
    "next_page_token": null
  }
}"#
}
