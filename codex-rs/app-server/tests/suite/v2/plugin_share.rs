use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInterface;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::PluginShareCheckoutResponse;
use codex_app_server_protocol::PluginShareContext;
use codex_app_server_protocol::PluginShareDeleteResponse;
use codex_app_server_protocol::PluginShareDiscoverability;
use codex_app_server_protocol::PluginShareListItem;
use codex_app_server_protocol::PluginShareListResponse;
use codex_app_server_protocol::PluginSharePrincipal;
use codex_app_server_protocol::PluginSharePrincipalRole;
use codex_app_server_protocol::PluginSharePrincipalType;
use codex_app_server_protocol::PluginShareSaveResponse;
use codex_app_server_protocol::PluginShareUpdateTargetsResponse;
use codex_app_server_protocol::PluginSource;
use codex_app_server_protocol::PluginSummary;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use flate2::Compression;
use flate2::write::GzEncoder;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS: &str =
    "CODEX_TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS";

#[tokio::test]
async fn plugin_share_save_uploads_local_plugin() -> Result<()> {
    let codex_home = TempDir::new()?;
    let plugin_root = TempDir::new()?;
    let plugin_path = write_test_plugin(plugin_root.path(), "demo-plugin")?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    write_corrupt_plugin_share_local_path_mapping(codex_home.path())?;

    Mock::given(method("POST"))
        .and(path("/backend-api/public/plugins/workspace/upload-url"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "file_id": "file_123",
            "upload_url": format!("{}/upload/file_123", server.uri()),
            "etag": "\"upload_etag_123\"",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload/file_123"))
        .and(header("x-ms-blob-type", "BlockBlob"))
        .and(header("content-type", "application/gzip"))
        .respond_with(ResponseTemplate::new(201).insert_header("etag", "\"blob_etag_123\""))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/backend-api/public/plugins/workspace"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .and(body_json(json!({
            "file_id": "file_123",
            "etag": "\"upload_etag_123\"",
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "plugin_id": "plugins_123",
            "share_url": "https://chatgpt.example/plugins/share/share-key-1",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let expected_plugin_path = AbsolutePathBuf::try_from(plugin_path.clone())?;
    let request_id = mcp
        .send_raw_request(
            "plugin/share/save",
            Some(json!({
                "pluginPath": expected_plugin_path.clone(),
            })),
        )
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginShareSaveResponse = to_response(response)?;

    assert_eq!(
        response,
        PluginShareSaveResponse {
            remote_plugin_id: "plugins_123".to_string(),
            share_url: "https://chatgpt.example/plugins/share/share-key-1".to_string(),
        }
    );

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/workspace/created"))
        .and(query_param("limit", "200"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [remote_plugin_json("plugins_123")],
            "pagination": empty_pagination_json(),
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("scope", "WORKSPACE"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [installed_remote_plugin_json("plugins_123")],
            "pagination": empty_pagination_json(),
        })))
        .expect(1)
        .mount(&server)
        .await;

    let request_id = mcp
        .send_raw_request("plugin/share/list", Some(json!({})))
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginShareListResponse = to_response(response)?;

    assert_eq!(
        response,
        PluginShareListResponse {
            data: vec![PluginShareListItem {
                plugin: PluginSummary {
                    id: "demo-plugin@workspace-shared-with-me".to_string(),
                    remote_plugin_id: Some("plugins_123".to_string()),
                    local_version: None,
                    name: "demo-plugin".to_string(),
                    share_context: Some(expected_share_context("plugins_123")),
                    source: PluginSource::Remote,
                    installed: true,
                    enabled: true,
                    install_policy: PluginInstallPolicy::Available,
                    auth_policy: PluginAuthPolicy::OnUse,
                    availability: codex_app_server_protocol::PluginAvailability::Available,
                    interface: Some(expected_plugin_interface()),
                    keywords: Vec::new(),
                },
                local_plugin_path: Some(expected_plugin_path),
            }],
        }
    );
    Ok(())
}

#[tokio::test]
async fn plugin_share_save_forwards_access_policy() -> Result<()> {
    let codex_home = TempDir::new()?;
    let plugin_root = TempDir::new()?;
    let plugin_path = write_test_plugin(plugin_root.path(), "demo-plugin")?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    Mock::given(method("POST"))
        .and(path("/backend-api/public/plugins/workspace/upload-url"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "file_id": "file_123",
            "upload_url": format!("{}/upload/file_123", server.uri()),
            "etag": "\"upload_etag_123\"",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload/file_123"))
        .respond_with(ResponseTemplate::new(201).insert_header("etag", "\"blob_etag_123\""))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/backend-api/public/plugins/workspace"))
        .and(body_json(json!({
            "file_id": "file_123",
            "etag": "\"upload_etag_123\"",
            "discoverability": "UNLISTED",
            "share_targets": [
                {
                    "principal_type": "user",
                    "principal_id": "user-1",
                    "role": "editor",
                },
                {
                    "principal_type": "workspace",
                    "principal_id": "account-123",
                    "role": "reader",
                },
            ],
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "plugin_id": "plugins_123",
            "share_url": "https://chatgpt.example/plugins/share/share-key-1",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let expected_plugin_path = AbsolutePathBuf::try_from(plugin_path)?;
    let request_id = mcp
        .send_raw_request(
            "plugin/share/save",
            Some(json!({
                "pluginPath": expected_plugin_path,
                "discoverability": "UNLISTED",
                "shareTargets": [
                    {
                        "principalType": "user",
                        "principalId": "user-1",
                        "role": "editor",
                    },
                ],
            })),
        )
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginShareSaveResponse = to_response(response)?;

    assert_eq!(
        response,
        PluginShareSaveResponse {
            remote_plugin_id: "plugins_123".to_string(),
            share_url: "https://chatgpt.example/plugins/share/share-key-1".to_string(),
        }
    );
    Ok(())
}

#[tokio::test]
async fn plugin_share_save_rejects_listed_discoverability() -> Result<()> {
    let codex_home = TempDir::new()?;
    let plugin_root = TempDir::new()?;
    let plugin_path = write_test_plugin(plugin_root.path(), "demo-plugin")?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let request_id = mcp
        .send_raw_request(
            "plugin/share/save",
            Some(json!({
                "pluginPath": AbsolutePathBuf::try_from(plugin_path)?,
                "discoverability": "LISTED",
            })),
        )
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "discoverability LISTED is not supported for plugin/share/save; use UNLISTED or PRIVATE"
    );
    Ok(())
}

#[tokio::test]
async fn plugin_share_save_rejects_when_plugin_sharing_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let plugin_root = TempDir::new()?;
    let plugin_path = write_test_plugin(plugin_root.path(), "demo-plugin")?;
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{}/backend-api"

[features]
plugins = true
remote_plugin = true
plugin_sharing = false
"#,
            server.uri()
        ),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let request_id = mcp
        .send_raw_request(
            "plugin/share/save",
            Some(json!({
                "pluginPath": AbsolutePathBuf::try_from(plugin_path)?,
            })),
        )
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert_eq!(error.error.message, "plugin sharing is disabled");
    assert!(
        server
            .received_requests()
            .await
            .expect("wiremock should record requests")
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn plugin_share_rejects_workspace_targets_from_client() -> Result<()> {
    let codex_home = TempDir::new()?;
    let plugin_root = TempDir::new()?;
    let plugin_path = write_test_plugin(plugin_root.path(), "demo-plugin")?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let request_id = mcp
        .send_raw_request(
            "plugin/share/save",
            Some(json!({
                "pluginPath": AbsolutePathBuf::try_from(plugin_path)?,
                "discoverability": "UNLISTED",
                "shareTargets": [
                    {
                        "principalType": "workspace",
                        "principalId": "account-123",
                        "role": "reader",
                    },
                ],
            })),
        )
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "shareTargets cannot include workspace principals; use discoverability UNLISTED for workspace link access"
    );

    let request_id = mcp
        .send_raw_request(
            "plugin/share/updateTargets",
            Some(json!({
                "remotePluginId": "plugins_123",
                "discoverability": "UNLISTED",
                "shareTargets": [
                    {
                        "principalType": "workspace",
                        "principalId": "account-123",
                        "role": "reader",
                    },
                ],
            })),
        )
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "shareTargets cannot include workspace principals; use discoverability UNLISTED for workspace link access"
    );
    Ok(())
}

#[tokio::test]
async fn plugin_share_save_rejects_access_policy_for_existing_plugin() -> Result<()> {
    let codex_home = TempDir::new()?;
    let plugin_root = TempDir::new()?;
    let plugin_path = write_test_plugin(plugin_root.path(), "demo-plugin")?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let request_id = mcp
        .send_raw_request(
            "plugin/share/save",
            Some(json!({
                "pluginPath": AbsolutePathBuf::try_from(plugin_path)?,
                "remotePluginId": "plugins_123",
                "discoverability": "PRIVATE",
                "shareTargets": [
                    {
                        "principalType": "user",
                        "principalId": "user-1",
                        "role": "reader",
                    },
                ],
            })),
        )
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "discoverability and shareTargets are only supported when creating a plugin share; use plugin/share/updateTargets to update share settings"
    );
    Ok(())
}

#[tokio::test]
async fn plugin_share_list_returns_created_workspace_plugins() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/workspace/created"))
        .and(query_param("limit", "200"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [remote_plugin_json("plugins_123")],
            "pagination": empty_pagination_json(),
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("scope", "WORKSPACE"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [installed_remote_plugin_json("plugins_123")],
            "pagination": empty_pagination_json(),
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let request_id = mcp
        .send_raw_request("plugin/share/list", Some(json!({})))
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginShareListResponse = to_response(response)?;

    assert_eq!(
        response,
        PluginShareListResponse {
            data: vec![PluginShareListItem {
                plugin: PluginSummary {
                    id: "demo-plugin@workspace-shared-with-me".to_string(),
                    remote_plugin_id: Some("plugins_123".to_string()),
                    local_version: None,
                    name: "demo-plugin".to_string(),
                    share_context: Some(expected_share_context("plugins_123")),
                    source: PluginSource::Remote,
                    installed: true,
                    enabled: true,
                    install_policy: PluginInstallPolicy::Available,
                    auth_policy: PluginAuthPolicy::OnUse,
                    availability: codex_app_server_protocol::PluginAvailability::Available,
                    interface: Some(expected_plugin_interface()),
                    keywords: Vec::new(),
                },
                local_plugin_path: None,
            }],
        }
    );
    Ok(())
}

#[tokio::test]
async fn plugin_share_checkout_adds_personal_marketplace_entry() -> Result<()> {
    let codex_home = TempDir::new()?;
    let home = TempDir::new()?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let bundle_url = mount_remote_plugin_bundle(
        &server,
        "demo-plugin",
        remote_plugin_bundle_tar_gz_bytes("demo-plugin")?,
    )
    .await;
    mount_remote_plugin_detail_with_bundle(
        &server,
        "plugins_123",
        "demo-plugin",
        &bundle_url,
        "WORKSPACE",
    )
    .await;
    mount_empty_remote_installed_plugins(&server, "WORKSPACE").await;

    let home_env = home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home_env.as_str())),
            ("USERPROFILE", Some(home_env.as_str())),
            (TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS, Some("1")),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "plugin/share/checkout",
            Some(json!({
                "remotePluginId": "plugins_123",
            })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginShareCheckoutResponse = to_response(response)?;

    let plugin_path = AbsolutePathBuf::try_from(home.path().join("plugins/demo-plugin"))?;
    let marketplace_path =
        AbsolutePathBuf::try_from(home.path().join(".agents/plugins/marketplace.json"))?;
    assert_eq!(
        response,
        PluginShareCheckoutResponse {
            remote_plugin_id: "plugins_123".to_string(),
            plugin_id: "demo-plugin@codex-curated".to_string(),
            plugin_name: "demo-plugin".to_string(),
            plugin_path: plugin_path.clone(),
            marketplace_name: "codex-curated".to_string(),
            marketplace_path: marketplace_path.clone(),
            remote_version: Some("1.2.3".to_string()),
        }
    );
    assert!(
        plugin_path
            .as_path()
            .join(".codex-plugin/plugin.json")
            .is_file()
    );

    let marketplace: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(marketplace_path.as_path())?)?;
    assert_eq!(
        marketplace,
        json!({
            "name": "codex-curated",
            "interface": {
                "displayName": "Personal",
            },
            "plugins": [
                {
                    "name": "demo-plugin",
                    "source": {
                        "source": "local",
                        "path": "./plugins/demo-plugin",
                    },
                    "policy": {
                        "installation": "AVAILABLE",
                        "authentication": "ON_USE",
                    },
                },
            ],
        })
    );

    let mapping: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        codex_home
            .path()
            .join(".tmp/plugin-share-local-paths-v1.json"),
    )?)?;
    assert_eq!(
        mapping,
        json!({
            "localPluginPathsByRemotePluginId": {
                "plugins_123": plugin_path.clone(),
            },
        })
    );

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            marketplace_kinds: Some(vec![
                codex_app_server_protocol::PluginListMarketplaceKind::Local,
            ]),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;
    assert_eq!(response.marketplaces.len(), 1);
    assert_eq!(response.marketplaces[0].name, "codex-curated");
    assert_eq!(response.marketplaces[0].plugins[0].name, "demo-plugin");
    assert_eq!(
        response.marketplaces[0].plugins[0]
            .share_context
            .as_ref()
            .map(|context| context.remote_plugin_id.as_str()),
        Some("plugins_123")
    );

    std::fs::write(plugin_path.as_path().join("local-edit.txt"), "keep")?;
    let request_id = mcp
        .send_raw_request(
            "plugin/share/checkout",
            Some(json!({
                "remotePluginId": "plugins_123",
            })),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginShareCheckoutResponse = to_response(response)?;
    assert_eq!(response.plugin_path, plugin_path);
    assert_eq!(
        std::fs::read_to_string(plugin_path.as_path().join("local-edit.txt"))?,
        "keep"
    );

    Ok(())
}

#[tokio::test]
async fn plugin_share_checkout_rejects_non_share_remote_plugin() -> Result<()> {
    let codex_home = TempDir::new()?;
    let home = TempDir::new()?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let bundle_url = format!("{}/bundles/global-plugin.tar.gz", server.uri());
    mount_remote_plugin_detail_with_bundle(
        &server,
        "plugins_global",
        "global-plugin",
        &bundle_url,
        "GLOBAL",
    )
    .await;
    mount_empty_remote_installed_plugins(&server, "GLOBAL").await;

    let home_env = home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home_env.as_str())),
            ("USERPROFILE", Some(home_env.as_str())),
            (TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS, Some("1")),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "plugin/share/checkout",
            Some(json!({
                "remotePluginId": "plugins_global",
            })),
        )
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert!(
        error
            .error
            .message
            .contains("not available for plugin/share/checkout")
    );
    assert!(!home.path().join("plugins/global-plugin").exists());

    Ok(())
}

#[tokio::test]
async fn plugin_share_checkout_cleans_up_path_when_marketplace_update_fails() -> Result<()> {
    let codex_home = TempDir::new()?;
    let home = TempDir::new()?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let marketplace_path = home.path().join(".agents/plugins/marketplace.json");
    std::fs::create_dir_all(
        marketplace_path
            .parent()
            .expect("marketplace path has parent"),
    )?;
    std::fs::write(
        &marketplace_path,
        serde_json::to_string_pretty(&json!({
            "name": "codex-curated",
            "plugins": [
                {
                    "name": "demo-plugin",
                    "source": {
                        "source": "local",
                        "path": "./other/demo-plugin",
                    },
                },
            ],
        }))?,
    )?;

    let bundle_url = mount_remote_plugin_bundle(
        &server,
        "demo-plugin",
        remote_plugin_bundle_tar_gz_bytes("demo-plugin")?,
    )
    .await;
    mount_remote_plugin_detail_with_bundle(
        &server,
        "plugins_123",
        "demo-plugin",
        &bundle_url,
        "WORKSPACE",
    )
    .await;
    mount_empty_remote_installed_plugins(&server, "WORKSPACE").await;

    let home_env = home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home_env.as_str())),
            ("USERPROFILE", Some(home_env.as_str())),
            (TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS, Some("1")),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "plugin/share/checkout",
            Some(json!({
                "remotePluginId": "plugins_123",
            })),
        )
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32600);
    assert!(
        error
            .error
            .message
            .contains("marketplace already contains plugin `demo-plugin`")
    );
    assert!(!home.path().join("plugins/demo-plugin").exists());
    assert!(
        !codex_home
            .path()
            .join(".tmp/plugin-share-local-paths-v1.json")
            .exists()
    );

    Ok(())
}

#[tokio::test]
async fn plugin_share_update_targets_updates_share_targets() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    Mock::given(method("PUT"))
        .and(path("/backend-api/ps/plugins/plugins_123/shares"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .and(body_json(json!({
            "discoverability": "UNLISTED",
            "targets": [
                {
                    "principal_type": "user",
                    "principal_id": "user-1",
                    "role": "editor",
                },
                {
                    "principal_type": "workspace",
                    "principal_id": "account-123",
                    "role": "reader",
                },
            ],
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "principals": [
                {
                    "principal_type": "user",
                    "principal_id": "owner-1",
                    "role": "owner",
                    "name": "Owner",
                },
                {
                    "principal_type": "user",
                    "principal_id": "user-1",
                    "role": "editor",
                    "name": "Gavin",
                },
                {
                    "principal_type": "workspace",
                    "principal_id": "account-123",
                    "role": "reader",
                    "name": "Workspace",
                },
            ],
            "discoverability": "UNLISTED",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let request_id = mcp
        .send_raw_request(
            "plugin/share/updateTargets",
            Some(json!({
                "remotePluginId": "plugins_123",
                "discoverability": "UNLISTED",
                "shareTargets": [
                    {
                        "principalType": "user",
                        "principalId": "user-1",
                        "role": "editor",
                    },
                ],
            })),
        )
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginShareUpdateTargetsResponse = to_response(response)?;

    assert_eq!(
        response,
        PluginShareUpdateTargetsResponse {
            principals: vec![
                PluginSharePrincipal {
                    principal_type: PluginSharePrincipalType::User,
                    principal_id: "owner-1".to_string(),
                    role: PluginSharePrincipalRole::Owner,
                    name: "Owner".to_string(),
                },
                PluginSharePrincipal {
                    principal_type: PluginSharePrincipalType::User,
                    principal_id: "user-1".to_string(),
                    role: PluginSharePrincipalRole::Editor,
                    name: "Gavin".to_string(),
                },
                PluginSharePrincipal {
                    principal_type: PluginSharePrincipalType::Workspace,
                    principal_id: "account-123".to_string(),
                    role: PluginSharePrincipalRole::Reader,
                    name: "Workspace".to_string(),
                },
            ],
            discoverability: codex_app_server_protocol::PluginShareDiscoverability::Unlisted,
        }
    );
    Ok(())
}

#[tokio::test]
async fn plugin_share_delete_removes_created_workspace_plugin() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    write_remote_plugin_config(codex_home.path(), &format!("{}/backend-api", server.uri()))?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    let local_plugin_path = AbsolutePathBuf::try_from(codex_home.path().join("local-plugin"))?;
    write_plugin_share_local_path_mapping(codex_home.path(), "plugins_123", &local_plugin_path)?;

    Mock::given(method("DELETE"))
        .and(path("/backend-api/public/plugins/workspace/plugins_123"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;
    let request_id = mcp
        .send_raw_request(
            "plugin/share/delete",
            Some(json!({
                "remotePluginId": "plugins_123",
            })),
        )
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginShareDeleteResponse = to_response(response)?;

    assert_eq!(response, PluginShareDeleteResponse {});

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/workspace/created"))
        .and(query_param("limit", "200"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [remote_plugin_json("plugins_123")],
            "pagination": empty_pagination_json(),
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("scope", "WORKSPACE"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [installed_remote_plugin_json("plugins_123")],
            "pagination": empty_pagination_json(),
        })))
        .expect(1)
        .mount(&server)
        .await;

    let request_id = mcp
        .send_raw_request("plugin/share/list", Some(json!({})))
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginShareListResponse = to_response(response)?;

    assert_eq!(
        response,
        PluginShareListResponse {
            data: vec![PluginShareListItem {
                plugin: PluginSummary {
                    id: "demo-plugin@workspace-shared-with-me".to_string(),
                    remote_plugin_id: Some("plugins_123".to_string()),
                    local_version: None,
                    name: "demo-plugin".to_string(),
                    share_context: Some(expected_share_context("plugins_123")),
                    source: PluginSource::Remote,
                    installed: true,
                    enabled: true,
                    install_policy: PluginInstallPolicy::Available,
                    auth_policy: PluginAuthPolicy::OnUse,
                    availability: codex_app_server_protocol::PluginAvailability::Available,
                    interface: Some(expected_plugin_interface()),
                    keywords: Vec::new(),
                },
                local_plugin_path: None,
            }],
        }
    );
    Ok(())
}

fn write_remote_plugin_config(codex_home: &Path, base_url: &str) -> std::io::Result<()> {
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

async fn mount_remote_plugin_bundle(
    server: &MockServer,
    plugin_name: &str,
    body: Vec<u8>,
) -> String {
    let bundle_path = format!("/bundles/{plugin_name}.tar.gz");
    Mock::given(method("GET"))
        .and(path(bundle_path.clone()))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/gzip")
                .set_body_bytes(body),
        )
        .expect(1)
        .mount(server)
        .await;
    format!("{}{}", server.uri(), bundle_path)
}

async fn mount_remote_plugin_detail_with_bundle(
    server: &MockServer,
    remote_plugin_id: &str,
    plugin_name: &str,
    bundle_url: &str,
    scope: &str,
) {
    Mock::given(method("GET"))
        .and(path(format!("/backend-api/ps/plugins/{remote_plugin_id}")))
        .and(query_param("includeDownloadUrls", "true"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": remote_plugin_id,
            "name": plugin_name,
            "scope": scope,
            "discoverability": "PRIVATE",
            "share_url": "https://chatgpt.example/plugins/share/share-key-1",
            "share_principals": [
                {
                    "principal_type": "user",
                    "principal_id": "user-owner__account-123",
                    "role": "owner",
                    "name": "Owner",
                },
            ],
            "installation_policy": "AVAILABLE",
            "authentication_policy": "ON_USE",
            "release": {
                "version": "1.2.3",
                "bundle_download_url": bundle_url,
                "display_name": "Demo Plugin",
                "description": "Demo plugin description",
                "interface": {
                    "short_description": "A demo plugin",
                    "capabilities": ["Read", "Write"],
                },
                "skills": [],
            },
        })))
        .mount(server)
        .await;
}

async fn mount_empty_remote_installed_plugins(server: &MockServer, scope: &str) {
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("scope", scope))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [],
            "pagination": {
                "next_page_token": null,
            },
        })))
        .mount(server)
        .await;
}

fn remote_plugin_json(plugin_id: &str) -> serde_json::Value {
    json!({
        "id": plugin_id,
        "name": "demo-plugin",
        "scope": "WORKSPACE",
        "discoverability": "PRIVATE",
        "share_url": "https://chatgpt.example/plugins/share/share-key-1",
        "share_principals": [
            {
                "principal_type": "user",
                "principal_id": "user-owner__account-123",
                "role": "owner",
                "name": "Owner"
            },
            {
                "principal_type": "user",
                "principal_id": "user-reader__account-123",
                "role": "reader",
                "name": "Reader"
            }
        ],
        "installation_policy": "AVAILABLE",
        "authentication_policy": "ON_USE",
        "release": {
            "version": "0.1.0",
            "display_name": "Demo Plugin",
            "description": "Demo plugin description",
            "interface": {
                "short_description": "A demo plugin",
                "capabilities": ["Read", "Write"]
            },
            "skills": []
        }
    })
}

fn installed_remote_plugin_json(plugin_id: &str) -> serde_json::Value {
    let mut plugin = remote_plugin_json(plugin_id);
    let serde_json::Value::Object(fields) = &mut plugin else {
        unreachable!("plugin json should be an object");
    };
    fields.insert("enabled".to_string(), json!(true));
    fields.insert("disabled_skill_names".to_string(), json!([]));
    plugin
}

fn empty_pagination_json() -> serde_json::Value {
    json!({
        "next_page_token": null
    })
}

fn expected_plugin_interface() -> PluginInterface {
    PluginInterface {
        display_name: Some("Demo Plugin".to_string()),
        short_description: Some("A demo plugin".to_string()),
        long_description: None,
        developer_name: None,
        category: None,
        capabilities: vec!["Read".to_string(), "Write".to_string()],
        website_url: None,
        privacy_policy_url: None,
        terms_of_service_url: None,
        default_prompt: None,
        brand_color: None,
        composer_icon: None,
        composer_icon_url: None,
        logo: None,
        logo_url: None,
        screenshots: Vec::new(),
        screenshot_urls: Vec::new(),
    }
}

fn expected_share_context(plugin_id: &str) -> PluginShareContext {
    PluginShareContext {
        remote_plugin_id: plugin_id.to_string(),
        remote_version: Some("0.1.0".to_string()),
        discoverability: Some(PluginShareDiscoverability::Private),
        share_url: Some("https://chatgpt.example/plugins/share/share-key-1".to_string()),
        creator_account_user_id: None,
        creator_name: None,
        share_principals: Some(vec![
            PluginSharePrincipal {
                principal_type: PluginSharePrincipalType::User,
                principal_id: "user-owner__account-123".to_string(),
                role: PluginSharePrincipalRole::Owner,
                name: "Owner".to_string(),
            },
            PluginSharePrincipal {
                principal_type: PluginSharePrincipalType::User,
                principal_id: "user-reader__account-123".to_string(),
                role: PluginSharePrincipalRole::Reader,
                name: "Reader".to_string(),
            },
        ]),
    }
}

fn write_test_plugin(root: &Path, plugin_name: &str) -> std::io::Result<PathBuf> {
    let plugin_path = root.join(plugin_name);
    write_file(
        &plugin_path.join(".codex-plugin/plugin.json"),
        &format!(r#"{{"name":"{plugin_name}"}}"#),
    )?;
    write_file(
        &plugin_path.join("skills/example/SKILL.md"),
        "# Example\n\nA test skill.\n",
    )?;
    Ok(plugin_path)
}

fn remote_plugin_bundle_tar_gz_bytes(plugin_name: &str) -> Result<Vec<u8>> {
    let manifest = format!(r#"{{"name":"{plugin_name}"}}"#);
    let skill = "# Example\n\nA test skill.\n";
    let encoder = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(encoder);
    for (path, contents, mode) in [
        (
            ".codex-plugin/plugin.json",
            manifest.as_bytes(),
            /*mode*/ 0o644,
        ),
        (
            "skills/example/SKILL.md",
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

fn write_corrupt_plugin_share_local_path_mapping(codex_home: &Path) -> std::io::Result<()> {
    write_file(
        &codex_home.join(".tmp/plugin-share-local-paths-v1.json"),
        "not-json",
    )
}

fn write_plugin_share_local_path_mapping(
    codex_home: &Path,
    remote_plugin_id: &str,
    plugin_path: &AbsolutePathBuf,
) -> std::io::Result<()> {
    let mut local_plugin_paths_by_remote_plugin_id = serde_json::Map::new();
    local_plugin_paths_by_remote_plugin_id.insert(
        remote_plugin_id.to_string(),
        serde_json::to_value(plugin_path).map_err(std::io::Error::other)?,
    );
    let contents = serde_json::to_string_pretty(&json!({
        "localPluginPathsByRemotePluginId": local_plugin_paths_by_remote_plugin_id,
    }))
    .map_err(std::io::Error::other)?;
    write_file(
        &codex_home.join(".tmp/plugin-share-local-paths-v1.json"),
        &format!("{contents}\n"),
    )
}

fn write_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Err(std::io::Error::other(format!(
            "file path `{}` should have a parent",
            path.display()
        )));
    };
    std::fs::create_dir_all(parent)?;
    std::fs::write(path, contents)
}
