use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInterface;
use codex_app_server_protocol::PluginShareDeleteResponse;
use codex_app_server_protocol::PluginShareListItem;
use codex_app_server_protocol::PluginShareListResponse;
use codex_app_server_protocol::PluginShareSaveResponse;
use codex_app_server_protocol::PluginSource;
use codex_app_server_protocol::PluginSummary;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_utils_absolute_path::AbsolutePathBuf;
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
                    id: "plugins_123".to_string(),
                    name: "demo-plugin".to_string(),
                    source: PluginSource::Remote,
                    installed: true,
                    enabled: true,
                    install_policy: PluginInstallPolicy::Available,
                    auth_policy: PluginAuthPolicy::OnUse,
                    availability: codex_app_server_protocol::PluginAvailability::Available,
                    interface: Some(expected_plugin_interface()),
                },
                share_url: "https://chatgpt.example/plugins/share/share-key-1".to_string(),
                local_plugin_path: Some(expected_plugin_path),
            }],
        }
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
                    id: "plugins_123".to_string(),
                    name: "demo-plugin".to_string(),
                    source: PluginSource::Remote,
                    installed: true,
                    enabled: true,
                    install_policy: PluginInstallPolicy::Available,
                    auth_policy: PluginAuthPolicy::OnUse,
                    availability: codex_app_server_protocol::PluginAvailability::Available,
                    interface: Some(expected_plugin_interface()),
                },
                share_url: "https://chatgpt.example/plugins/share/share-key-1".to_string(),
                local_plugin_path: None,
            }],
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
                    id: "plugins_123".to_string(),
                    name: "demo-plugin".to_string(),
                    source: PluginSource::Remote,
                    installed: true,
                    enabled: true,
                    install_policy: PluginInstallPolicy::Available,
                    auth_policy: PluginAuthPolicy::OnUse,
                    availability: codex_app_server_protocol::PluginAvailability::Available,
                    interface: Some(expected_plugin_interface()),
                },
                share_url: "https://chatgpt.example/plugins/share/share-key-1".to_string(),
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

fn remote_plugin_json(plugin_id: &str) -> serde_json::Value {
    json!({
        "id": plugin_id,
        "name": "demo-plugin",
        "scope": "WORKSPACE",
        "share_url": "https://chatgpt.example/plugins/share/share-key-1",
        "installation_policy": "AVAILABLE",
        "authentication_policy": "ON_USE",
        "release": {
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
