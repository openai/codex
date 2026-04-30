use std::borrow::Cow;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use anyhow::Result;
use anyhow::bail;
use app_test_support::ChatGptAuthFixture;
use app_test_support::DEFAULT_CLIENT_NAME;
use app_test_support::McpProcess;
use app_test_support::start_analytics_events_server;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::http::header::AUTHORIZATION;
use axum::routing::get;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppSummary;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use flate2::Compression;
use flate2::write::GzEncoder;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::Meta;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use wiremock::Match;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

// Plugin install tests wait on connector discovery after the install response path
// starts, which is noticeably slower on Windows CI.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const REMOTE_PLUGIN_ID: &str = "plugins~Plugin_00000000000000000000000000000000";
const TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS: &str =
    "CODEX_TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS";

#[tokio::test]
async fn plugin_install_rejects_relative_marketplace_paths() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "plugin/install",
            Some(serde_json::json!({
                "marketplacePath": "relative-marketplace.json",
                "pluginName": "missing-plugin",
            })),
        )
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("Invalid request"));
    Ok(())
}

#[tokio::test]
async fn plugin_install_rejects_missing_install_source() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: None,
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("requires exactly one of marketplacePath or remoteMarketplaceName")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_rejects_multiple_install_sources() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                codex_home.path().join("marketplace.json"),
            )?),
            remote_marketplace_name: Some("openai-curated".to_string()),
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("requires exactly one of marketplacePath or remoteMarketplaceName")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_rejects_remote_marketplace_when_remote_plugin_is_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: None,
            remote_marketplace_name: Some("chatgpt-global".to_string()),
            plugin_name: "plugins~Plugin_22222222222222222222222222222222".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("remote plugin install is not enabled")
    );
    assert!(err.error.message.contains("chatgpt-global"));
    Ok(())
}

#[tokio::test]
async fn plugin_install_writes_remote_plugin_to_cloud_and_cache() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    let installed_path = codex_home
        .path()
        .join("plugins/cache/chatgpt-global/linear/1.2.3");
    let bundle_url = mount_remote_plugin_bundle(
        &server,
        /*status_code*/ 200,
        remote_plugin_bundle_tar_gz_bytes("linear")?,
    )
    .await;
    configure_remote_plugin_test(codex_home.path(), &server)?;
    mount_remote_plugin_detail(&server, REMOTE_PLUGIN_ID, "1.2.3", Some(&bundle_url)).await;
    mount_empty_remote_installed_plugins(&server).await;
    mount_remote_plugin_install_after_cache_write(
        &server,
        REMOTE_PLUGIN_ID,
        installed_path.join(".codex-plugin/plugin.json"),
    )
    .await;

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS, Some("1"))],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = send_remote_plugin_install_request(&mut mcp, REMOTE_PLUGIN_ID).await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginInstallResponse = to_response(response)?;

    assert_eq!(
        response,
        PluginInstallResponse {
            auth_policy: PluginAuthPolicy::OnUse,
            apps_needing_auth: Vec::new(),
        }
    );
    wait_for_remote_plugin_request_count(
        &server,
        "POST",
        &format!("/ps/plugins/{REMOTE_PLUGIN_ID}/install"),
        /*expected_count*/ 1,
    )
    .await?;
    wait_for_remote_plugin_request_count(
        &server,
        "GET",
        "/bundles/linear.tar.gz",
        /*expected_count*/ 1,
    )
    .await?;
    assert!(installed_path.join(".codex-plugin/plugin.json").is_file());
    assert!(installed_path.join("skills/plan-work/SKILL.md").is_file());
    assert!(
        !codex_home
            .path()
            .join(format!(
                "plugins/cache/chatgpt-global/{REMOTE_PLUGIN_ID}/1.2.3"
            ))
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_rejects_missing_remote_bundle_url() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    configure_remote_plugin_test(codex_home.path(), &server)?;
    mount_remote_plugin_detail(
        &server,
        REMOTE_PLUGIN_ID,
        "1.2.3",
        /*bundle_download_url*/ None,
    )
    .await;
    mount_empty_remote_installed_plugins(&server).await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = send_remote_plugin_install_request(&mut mcp, REMOTE_PLUGIN_ID).await?;
    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32603);
    assert!(
        err.error
            .message
            .contains("backend did not return a download URL")
    );
    wait_for_remote_plugin_request_count(
        &server,
        "POST",
        &format!("/ps/plugins/{REMOTE_PLUGIN_ID}/install"),
        /*expected_count*/ 0,
    )
    .await?;
    assert!(
        !codex_home
            .path()
            .join("plugins/cache/chatgpt-global/linear")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_rejects_plain_http_remote_bundle_url() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    let bundle_url = format!("{}/bundles/linear.tar.gz", server.uri());
    configure_remote_plugin_test(codex_home.path(), &server)?;
    mount_remote_plugin_detail(&server, REMOTE_PLUGIN_ID, "1.2.3", Some(&bundle_url)).await;
    mount_empty_remote_installed_plugins(&server).await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = send_remote_plugin_install_request(&mut mcp, REMOTE_PLUGIN_ID).await?;
    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32603);
    assert!(
        err.error
            .message
            .contains("unsupported download URL scheme")
    );
    wait_for_remote_plugin_request_count(
        &server,
        "POST",
        &format!("/ps/plugins/{REMOTE_PLUGIN_ID}/install"),
        /*expected_count*/ 0,
    )
    .await?;
    assert!(
        !codex_home
            .path()
            .join("plugins/cache/chatgpt-global/linear")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_rejects_invalid_remote_release_version() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    configure_remote_plugin_test(codex_home.path(), &server)?;
    mount_remote_plugin_detail(
        &server,
        REMOTE_PLUGIN_ID,
        "../1.2.3",
        Some("https://127.0.0.1:1/bundles/linear.tar.gz"),
    )
    .await;
    mount_empty_remote_installed_plugins(&server).await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = send_remote_plugin_install_request(&mut mcp, REMOTE_PLUGIN_ID).await?;
    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32603);
    assert!(err.error.message.contains("invalid release version"));
    wait_for_remote_plugin_request_count(
        &server,
        "POST",
        &format!("/ps/plugins/{REMOTE_PLUGIN_ID}/install"),
        /*expected_count*/ 0,
    )
    .await?;
    assert!(
        !codex_home
            .path()
            .join("plugins/cache/chatgpt-global/linear")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_rejects_invalid_remote_plugin_name() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_remote_plugin_catalog_config(codex_home.path(), "https://example.invalid/backend-api/")?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: None,
            remote_marketplace_name: Some("chatgpt-global".to_string()),
            plugin_name: "linear/../../oops".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("invalid remote plugin id"));
    assert!(
        err.error
            .message
            .contains("only ASCII letters, digits, `_`, `-`, and `~` are allowed")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_rejects_when_workspace_codex_plugins_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let server = MockServer::start().await;
    write_plugins_enabled_config_with_base_url(
        codex_home.path(),
        &format!("{}/backend-api/", server.uri()),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123")
            .plan_type("team"),
        AuthCredentialsStoreMode::File,
    )?;
    write_plugin_marketplace(
        repo_root.path(),
        "debug",
        "sample-plugin",
        "./sample-plugin",
        /*install_policy*/ None,
        /*auth_policy*/ None,
    )?;
    write_plugin_source(repo_root.path(), "sample-plugin", &[])?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    Mock::given(method("GET"))
        .and(path("/backend-api/accounts/account-123/settings"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"beta_settings":{"enable_plugins":false}}"#),
        )
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(
        err.error
            .message
            .contains("Codex plugins are disabled for this workspace")
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_returns_invalid_request_for_missing_marketplace_file() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(AbsolutePathBuf::try_from(
                codex_home.path().join("missing-marketplace.json"),
            )?),
            remote_marketplace_name: None,
            plugin_name: "missing-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("marketplace file"));
    assert!(err.error.message.contains("does not exist"));
    Ok(())
}

#[tokio::test]
async fn plugin_install_returns_invalid_request_for_not_available_plugin() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    write_plugin_marketplace(
        repo_root.path(),
        "debug",
        "sample-plugin",
        "./sample-plugin",
        Some("NOT_AVAILABLE"),
        /*auth_policy*/ None,
    )?;
    write_plugin_source(repo_root.path(), "sample-plugin", &[])?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("not available for install"));
    Ok(())
}

#[tokio::test]
async fn plugin_install_returns_invalid_request_for_disallowed_product_plugin() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    std::fs::write(
        repo_root.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample-plugin",
      "source": {
        "source": "local",
        "path": "./sample-plugin"
      },
      "policy": {
        "products": ["CHATGPT"]
      }
    }
  ]
}"#,
    )?;
    write_plugin_source(repo_root.path(), "sample-plugin", &[])?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp =
        McpProcess::new_with_args(codex_home.path(), &["--session-source", "atlas"]).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("not available for install"));
    Ok(())
}

#[tokio::test]
async fn plugin_install_tracks_analytics_event() -> Result<()> {
    let analytics_server = start_analytics_events_server().await?;
    let codex_home = TempDir::new()?;
    write_analytics_config(codex_home.path(), &analytics_server.uri())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let repo_root = TempDir::new()?;
    write_plugin_marketplace(
        repo_root.path(),
        "debug",
        "sample-plugin",
        "./sample-plugin",
        /*install_policy*/ None,
        /*auth_policy*/ None,
    )?;
    write_plugin_source(repo_root.path(), "sample-plugin", &[])?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginInstallResponse = to_response(response)?;
    assert_eq!(response.apps_needing_auth, Vec::<AppSummary>::new());

    let payload = timeout(DEFAULT_TIMEOUT, async {
        loop {
            let Some(requests) = analytics_server.received_requests().await else {
                tokio::time::sleep(Duration::from_millis(25)).await;
                continue;
            };
            if let Some(request) = requests.iter().find(|request| {
                request.method == "POST" && request.url.path() == "/codex/analytics-events/events"
            }) {
                break request.body.clone();
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await?;
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("analytics payload");
    assert_eq!(
        payload,
        json!({
            "events": [{
                "event_type": "codex_plugin_installed",
                "event_params": {
                    "plugin_id": "sample-plugin@debug",
                    "plugin_name": "sample-plugin",
                    "marketplace_name": "debug",
                    "has_skills": false,
                    "mcp_server_count": 0,
                    "connector_ids": [],
                    "product_client_id": DEFAULT_CLIENT_NAME,
                }
            }]
        })
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_errors_when_remote_bundle_download_fails() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    let bundle_url = mount_remote_plugin_bundle(
        &server,
        /*status_code*/ 503,
        b"bundle temporarily unavailable".to_vec(),
    )
    .await;
    configure_remote_plugin_test(codex_home.path(), &server)?;
    mount_remote_plugin_detail(&server, REMOTE_PLUGIN_ID, "1.2.3", Some(&bundle_url)).await;
    mount_empty_remote_installed_plugins(&server).await;
    mount_remote_plugin_install(&server, REMOTE_PLUGIN_ID).await;

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS, Some("1"))],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = send_remote_plugin_install_request(&mut mcp, REMOTE_PLUGIN_ID).await?;
    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32603);
    assert!(err.error.message.contains("failed with status 503"));
    wait_for_remote_plugin_request_count(
        &server,
        "GET",
        "/bundles/linear.tar.gz",
        /*expected_count*/ 1,
    )
    .await?;
    wait_for_remote_plugin_request_count(
        &server,
        "POST",
        &format!("/ps/plugins/{REMOTE_PLUGIN_ID}/install"),
        /*expected_count*/ 0,
    )
    .await?;
    assert!(
        !codex_home
            .path()
            .join("plugins/cache/chatgpt-global/linear")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn plugin_install_returns_apps_needing_auth() -> Result<()> {
    let connectors = vec![
        AppInfo {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            description: Some("Alpha connector".to_string()),
            logo_url: Some("https://example.com/alpha.png".to_string()),
            logo_url_dark: None,
            distribution_channel: Some("featured".to_string()),
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        },
        AppInfo {
            id: "beta".to_string(),
            name: "Beta".to_string(),
            description: Some("Beta connector".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        },
    ];
    let tools = vec![connector_tool("beta", "Beta App")?];
    let (server_url, server_handle) = start_apps_server(connectors, tools).await?;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server_url)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let repo_root = TempDir::new()?;
    write_plugin_marketplace(
        repo_root.path(),
        "debug",
        "sample-plugin",
        "./sample-plugin",
        /*install_policy*/ None,
        /*auth_policy*/ None,
    )?;
    write_plugin_source(repo_root.path(), "sample-plugin", &["alpha", "beta"])?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginInstallResponse = to_response(response)?;

    assert_eq!(
        response,
        PluginInstallResponse {
            auth_policy: PluginAuthPolicy::OnInstall,
            apps_needing_auth: vec![AppSummary {
                id: "alpha".to_string(),
                name: "Alpha".to_string(),
                description: Some("Alpha connector".to_string()),
                install_url: Some("https://chatgpt.com/apps/alpha/alpha".to_string()),
                needs_auth: true,
            }],
        }
    );

    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}

#[tokio::test]
async fn plugin_install_filters_disallowed_apps_needing_auth() -> Result<()> {
    let connectors = vec![AppInfo {
        id: "alpha".to_string(),
        name: "Alpha".to_string(),
        description: Some("Alpha connector".to_string()),
        logo_url: Some("https://example.com/alpha.png".to_string()),
        logo_url_dark: None,
        distribution_channel: Some("featured".to_string()),
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: None,
        is_accessible: false,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }];
    let (server_url, server_handle) = start_apps_server(connectors, Vec::new()).await?;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server_url)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let repo_root = TempDir::new()?;
    write_plugin_marketplace(
        repo_root.path(),
        "debug",
        "sample-plugin",
        "./sample-plugin",
        /*install_policy*/ None,
        Some("ON_USE"),
    )?;
    write_plugin_source(
        repo_root.path(),
        "sample-plugin",
        &["alpha", "asdk_app_6938a94a61d881918ef32cb999ff937c"],
    )?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginInstallResponse = to_response(response)?;

    assert_eq!(
        response,
        PluginInstallResponse {
            auth_policy: PluginAuthPolicy::OnUse,
            apps_needing_auth: vec![AppSummary {
                id: "alpha".to_string(),
                name: "Alpha".to_string(),
                description: Some("Alpha connector".to_string()),
                install_url: Some("https://chatgpt.com/apps/alpha/alpha".to_string()),
                needs_auth: true,
            }],
        }
    );

    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}

#[tokio::test]
async fn plugin_install_makes_bundled_mcp_servers_available_to_followup_requests() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nplugins = true\n",
    )?;
    let repo_root = TempDir::new()?;
    write_plugin_marketplace(
        repo_root.path(),
        "debug",
        "sample-plugin",
        "./sample-plugin",
        /*install_policy*/ None,
        /*auth_policy*/ None,
    )?;
    write_plugin_source(repo_root.path(), "sample-plugin", &[])?;
    std::fs::write(
        repo_root.path().join("sample-plugin/.mcp.json"),
        r#"{
  "mcpServers": {
    "sample-mcp": {
      "command": "echo"
    }
  }
}"#,
    )?;
    let marketplace_path =
        AbsolutePathBuf::try_from(repo_root.path().join(".agents/plugins/marketplace.json"))?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            plugin_name: "sample-plugin".to_string(),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginInstallResponse = to_response(response)?;
    assert_eq!(response.apps_needing_auth, Vec::<AppSummary>::new());
    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(!config.contains("[mcp_servers.sample-mcp]"));
    assert!(!config.contains("command = \"echo\""));

    let request_id = mcp
        .send_raw_request(
            "mcpServer/oauth/login",
            Some(json!({
                "name": "sample-mcp",
            })),
        )
        .await?;
    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert_eq!(
        err.error.message,
        "OAuth login is only supported for streamable HTTP servers."
    );
    Ok(())
}

#[derive(Clone)]
struct AppsServerState {
    response: Arc<StdMutex<serde_json::Value>>,
}

#[derive(Clone)]
struct PluginInstallMcpServer {
    tools: Arc<StdMutex<Vec<Tool>>>,
}

impl ServerHandler for PluginInstallMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        let tools = self.tools.clone();
        async move {
            let tools = tools
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            Ok(ListToolsResult {
                tools,
                next_cursor: None,
                meta: None,
            })
        }
    }
}

async fn start_apps_server(
    connectors: Vec<AppInfo>,
    tools: Vec<Tool>,
) -> Result<(String, JoinHandle<()>)> {
    let state = Arc::new(AppsServerState {
        response: Arc::new(StdMutex::new(
            json!({ "apps": connectors, "next_token": null }),
        )),
    });
    let tools = Arc::new(StdMutex::new(tools));

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let mcp_service = StreamableHttpService::new(
        {
            let tools = tools.clone();
            move || {
                Ok(PluginInstallMcpServer {
                    tools: tools.clone(),
                })
            }
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = Router::new()
        .route("/connectors/directory/list", get(list_directory_connectors))
        .route(
            "/connectors/directory/list_workspace",
            get(list_directory_connectors),
        )
        .with_state(state)
        .nest_service("/api/codex/apps", mcp_service);

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((format!("http://{addr}"), handle))
}

async fn list_directory_connectors(
    State(state): State<Arc<AppsServerState>>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let bearer_ok = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "Bearer chatgpt-token");
    let account_ok = headers
        .get("chatgpt-account-id")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "account-123");
    let external_logos_ok = uri
        .query()
        .is_some_and(|query| query.split('&').any(|pair| pair == "external_logos=true"));

    if !bearer_ok || !account_ok {
        Err(StatusCode::UNAUTHORIZED)
    } else if !external_logos_ok {
        Err(StatusCode::BAD_REQUEST)
    } else {
        let response = state
            .response
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        Ok(Json(response))
    }
}

fn connector_tool(connector_id: &str, connector_name: &str) -> Result<Tool> {
    let schema: JsonObject = serde_json::from_value(json!({
        "type": "object",
        "additionalProperties": false
    }))?;
    let mut tool = Tool::new(
        Cow::Owned(format!("connector_{connector_id}")),
        Cow::Borrowed("Connector test tool"),
        Arc::new(schema),
    );
    tool.annotations = Some(ToolAnnotations::new().read_only(true));

    let mut meta = Meta::new();
    meta.0
        .insert("connector_id".to_string(), json!(connector_id));
    meta.0
        .insert("connector_name".to_string(), json!(connector_name));
    tool.meta = Some(meta);
    Ok(tool)
}

fn write_connectors_config(codex_home: &std::path::Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"
mcp_oauth_credentials_store = "file"

[features]
connectors = true
"#
        ),
    )
}

fn write_plugins_enabled_config_with_base_url(
    codex_home: &std::path::Path,
    base_url: &str,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"chatgpt_base_url = "{base_url}"

[features]
plugins = true
"#,
        ),
    )
}

fn write_analytics_config(codex_home: &std::path::Path, base_url: &str) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!("chatgpt_base_url = \"{base_url}\"\n"),
    )
}

fn write_remote_plugin_catalog_config(
    codex_home: &std::path::Path,
    base_url: &str,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"

[features]
plugins = true
remote_plugin = true
"#
        ),
    )
}

fn configure_remote_plugin_test(codex_home: &std::path::Path, server: &MockServer) -> Result<()> {
    write_remote_plugin_catalog_config(codex_home, &format!("{}/backend-api/", server.uri()))?;
    write_chatgpt_auth(
        codex_home,
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )
}

async fn mount_remote_plugin_bundle(
    server: &MockServer,
    status_code: u16,
    body: Vec<u8>,
) -> String {
    Mock::given(method("GET"))
        .and(path("/bundles/linear.tar.gz"))
        .respond_with(
            ResponseTemplate::new(status_code)
                .insert_header("content-type", "application/gzip")
                .set_body_bytes(body),
        )
        .mount(server)
        .await;
    format!("{}/bundles/linear.tar.gz", server.uri())
}

async fn mount_remote_plugin_detail(
    server: &MockServer,
    remote_plugin_id: &str,
    release_version: &str,
    bundle_download_url: Option<&str>,
) {
    let bundle_download_url_field = bundle_download_url
        .map(|url| format!(r#"    "bundle_download_url": "{url}","#))
        .unwrap_or_default();
    let detail_body = format!(
        r#"{{
  "id": "{remote_plugin_id}",
  "name": "linear",
  "scope": "GLOBAL",
  "installation_policy": "AVAILABLE",
  "authentication_policy": "ON_USE",
  "release": {{
    "version": "{release_version}",
{bundle_download_url_field}
    "display_name": "Linear",
    "description": "Track work in Linear",
    "app_ids": [],
    "interface": {{
      "short_description": "Plan and track work"
    }},
    "skills": []
  }}
}}"#
    );

    Mock::given(method("GET"))
        .and(path(format!("/backend-api/ps/plugins/{remote_plugin_id}")))
        .and(query_param("includeDownloadUrls", "true"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(detail_body))
        .mount(server)
        .await;
}

async fn mount_empty_remote_installed_plugins(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("scope", "GLOBAL"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{
  "plugins": [],
  "pagination": {
    "limit": 50,
    "next_page_token": null
  }
}"#,
        ))
        .mount(server)
        .await;
}

async fn mount_remote_plugin_install(server: &MockServer, remote_plugin_id: &str) {
    Mock::given(method("POST"))
        .and(path(format!(
            "/backend-api/ps/plugins/{remote_plugin_id}/install"
        )))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!(r#"{{"id":"{remote_plugin_id}","enabled":true}}"#)),
        )
        .mount(server)
        .await;
}

#[derive(Debug, Clone)]
struct CacheManifestExists {
    manifest_path: std::path::PathBuf,
}

impl Match for CacheManifestExists {
    fn matches(&self, _request: &Request) -> bool {
        self.manifest_path.is_file()
    }
}

async fn mount_remote_plugin_install_after_cache_write(
    server: &MockServer,
    remote_plugin_id: &str,
    manifest_path: std::path::PathBuf,
) {
    Mock::given(method("POST"))
        .and(path(format!(
            "/backend-api/ps/plugins/{remote_plugin_id}/install"
        )))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .and(CacheManifestExists { manifest_path })
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!(r#"{{"id":"{remote_plugin_id}","enabled":true}}"#)),
        )
        .mount(server)
        .await;
}

async fn send_remote_plugin_install_request(
    mcp: &mut McpProcess,
    remote_plugin_id: &str,
) -> Result<i64> {
    mcp.send_plugin_install_request(PluginInstallParams {
        marketplace_path: None,
        remote_marketplace_name: Some("chatgpt-global".to_string()),
        plugin_name: remote_plugin_id.to_string(),
    })
    .await
}

async fn wait_for_remote_plugin_request_count(
    server: &MockServer,
    method_name: &str,
    path_suffix: &str,
    expected_count: usize,
) -> Result<()> {
    timeout(DEFAULT_TIMEOUT, async {
        loop {
            let Some(requests) = server.received_requests().await else {
                bail!("wiremock did not record requests");
            };
            let request_count = requests
                .iter()
                .filter(|request| {
                    request.method == method_name && request.url.path().ends_with(path_suffix)
                })
                .count();
            if request_count == expected_count {
                return Ok::<(), anyhow::Error>(());
            }
            if request_count > expected_count {
                bail!(
                    "expected exactly {expected_count} {method_name} {path_suffix} requests, got {request_count}"
                );
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok(())
}

fn write_plugin_marketplace(
    repo_root: &std::path::Path,
    marketplace_name: &str,
    plugin_name: &str,
    source_path: &str,
    install_policy: Option<&str>,
    auth_policy: Option<&str>,
) -> std::io::Result<()> {
    let policy = if install_policy.is_some() || auth_policy.is_some() {
        let installation = install_policy
            .map(|installation| format!("\n        \"installation\": \"{installation}\""))
            .unwrap_or_default();
        let separator = if install_policy.is_some() && auth_policy.is_some() {
            ","
        } else {
            ""
        };
        let authentication = auth_policy
            .map(|authentication| {
                format!("{separator}\n        \"authentication\": \"{authentication}\"")
            })
            .unwrap_or_default();
        format!(",\n      \"policy\": {{{installation}{authentication}\n      }}")
    } else {
        String::new()
    };
    std::fs::create_dir_all(repo_root.join(".git"))?;
    std::fs::create_dir_all(repo_root.join(".agents/plugins"))?;
    std::fs::write(
        repo_root.join(".agents/plugins/marketplace.json"),
        format!(
            r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{
      "name": "{plugin_name}",
      "source": {{
        "source": "local",
        "path": "{source_path}"
      }}{policy}
    }}
  ]
}}"#
        ),
    )
}

fn write_plugin_source(
    repo_root: &std::path::Path,
    plugin_name: &str,
    app_ids: &[&str],
) -> Result<()> {
    let plugin_root = repo_root.join(plugin_name);
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(r#"{{"name":"{plugin_name}"}}"#),
    )?;

    let apps = app_ids
        .iter()
        .map(|app_id| ((*app_id).to_string(), json!({ "id": app_id })))
        .collect::<serde_json::Map<_, _>>();
    std::fs::write(
        plugin_root.join(".app.json"),
        serde_json::to_vec_pretty(&json!({ "apps": apps }))?,
    )?;
    Ok(())
}

fn remote_plugin_bundle_tar_gz_bytes(plugin_name: &str) -> Result<Vec<u8>> {
    let manifest = format!(r#"{{"name":"{plugin_name}"}}"#);
    let skill = "# Plan Work\n\nTrack work in Linear.\n";
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(encoder);
    for (path, contents, mode) in [
        (
            ".codex-plugin/plugin.json",
            manifest.as_bytes(),
            /*mode*/ 0o644,
        ),
        (
            "skills/plan-work/SKILL.md",
            skill.as_bytes(),
            /*mode*/ 0o644,
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        tar.append_data(&mut header, path, contents)?;
    }
    Ok(tar.into_inner()?.finish()?)
}
