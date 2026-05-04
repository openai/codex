use super::*;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_app_server_protocol::PluginInterface;
use codex_login::CodexAuth;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;
use wiremock::matchers::query_param_is_missing;

fn test_config(server: &MockServer) -> RemotePluginServiceConfig {
    RemotePluginServiceConfig {
        chatgpt_base_url: format!("{}/backend-api", server.uri()),
    }
}

fn test_auth() -> CodexAuth {
    CodexAuth::create_dummy_chatgpt_auth_for_testing()
}

fn write_file(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("file should have a parent")).unwrap();
    fs::write(path, contents).unwrap();
}

fn write_test_plugin(root: &Path, plugin_name: &str) -> PathBuf {
    let plugin_path = root.join(plugin_name);
    write_file(
        &plugin_path.join(".codex-plugin/plugin.json"),
        &format!(r#"{{"name":"{plugin_name}"}}"#),
    );
    write_file(
        &plugin_path.join("skills/example/SKILL.md"),
        "# Example\n\nA test skill.\n",
    );
    plugin_path
}

fn write_plugin_share_local_path_mapping(
    codex_home: &Path,
    remote_plugin_id: &str,
    plugin_path: &AbsolutePathBuf,
) {
    write_file(
        &codex_home.join(".tmp/plugin-share-local-paths-v1.json"),
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "localPluginPathsByRemotePluginId": {
                    remote_plugin_id: plugin_path,
                },
            }))
            .unwrap()
        ),
    );
}

fn archive_file_entries(archive_bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(archive_bytes);
    let mut archive = tar::Archive::new(decoder);
    archive
        .entries()
        .unwrap()
        .filter_map(|entry| {
            let mut entry = entry.unwrap();
            if !entry.header().entry_type().is_file() {
                return None;
            }
            let path = entry.path().unwrap().to_string_lossy().into_owned();
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).unwrap();
            Some((path, contents))
        })
        .collect()
}

fn remote_plugin_json(plugin_id: &str) -> serde_json::Value {
    json!({
        "id": plugin_id,
        "name": "demo-plugin",
        "scope": "WORKSPACE",
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

fn remote_plugin_json_with_share_url(
    plugin_id: &str,
    share_url: Option<&str>,
) -> serde_json::Value {
    let mut plugin = remote_plugin_json(plugin_id);
    let serde_json::Value::Object(fields) = &mut plugin else {
        unreachable!("plugin json should be an object");
    };
    fields.insert("share_url".to_string(), json!(share_url));
    plugin
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

#[tokio::test]
async fn save_remote_plugin_share_creates_workspace_plugin() {
    let codex_home = TempDir::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let plugin_path =
        AbsolutePathBuf::try_from(write_test_plugin(temp_dir.path(), "demo-plugin")).unwrap();
    let archive_size = archive_plugin_for_upload(plugin_path.as_path())
        .unwrap()
        .len();
    let server = MockServer::start().await;
    let config = test_config(&server);
    let auth = test_auth();

    Mock::given(method("POST"))
        .and(path("/backend-api/public/plugins/workspace/upload-url"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .and(body_json(json!({
            "filename": "demo-plugin.tar.gz",
            "mime_type": "application/gzip",
            "size_bytes": archive_size,
        })))
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
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
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

    let result = save_remote_plugin_share(
        &config,
        Some(&auth),
        codex_home.path(),
        &plugin_path,
        /*remote_plugin_id*/ None,
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        RemotePluginShareSaveResult {
            remote_plugin_id: "plugins_123".to_string(),
            share_url: Some("https://chatgpt.example/plugins/share/share-key-1".to_string()),
        }
    );
    assert_eq!(
        local_paths::load_plugin_share_local_paths(codex_home.path()).unwrap(),
        BTreeMap::from([("plugins_123".to_string(), plugin_path)])
    );

    let requests = server.received_requests().await.unwrap_or_default();
    let upload_request = requests
        .iter()
        .find(|request| request.method == "PUT" && request.url.path() == "/upload/file_123")
        .unwrap();
    let archive_files = archive_file_entries(&upload_request.body);
    assert_eq!(
        archive_files
            .get(".codex-plugin/plugin.json")
            .map(Vec::as_slice),
        Some(br#"{"name":"demo-plugin"}"#.as_slice())
    );
    assert_eq!(
        archive_files
            .get("skills/example/SKILL.md")
            .map(Vec::as_slice),
        Some(b"# Example\n\nA test skill.\n".as_slice())
    );
}

#[test]
fn archive_plugin_for_upload_rejects_archives_over_limit() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_path = write_test_plugin(temp_dir.path(), "demo-plugin");
    write_file(
        &plugin_path.join("large.txt"),
        &"0123456789abcdef".repeat(1024),
    );

    let err = archive_plugin_for_upload_with_limit(&plugin_path, /*max_bytes*/ 16)
        .expect_err("oversized plugin archive should fail");

    assert!(matches!(
        err,
        RemotePluginCatalogError::ArchiveTooLarge { .. }
    ));
}

#[test]
fn archive_plugin_for_upload_places_manifest_at_archive_root() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_path = write_test_plugin(temp_dir.path(), "demo-plugin");

    let archive_bytes = archive_plugin_for_upload(&plugin_path).unwrap();
    let archive_files = archive_file_entries(&archive_bytes);

    assert_eq!(
        archive_files.keys().cloned().collect::<Vec<_>>(),
        vec![
            ".codex-plugin/plugin.json".to_string(),
            "skills/example/SKILL.md".to_string()
        ]
    );
    assert_eq!(
        archive_files
            .get(".codex-plugin/plugin.json")
            .map(Vec::as_slice),
        Some(br#"{"name":"demo-plugin"}"#.as_slice())
    );
    assert_eq!(
        archive_files
            .get("skills/example/SKILL.md")
            .map(Vec::as_slice),
        Some(b"# Example\n\nA test skill.\n".as_slice())
    );
}

#[tokio::test]
async fn save_remote_plugin_share_updates_existing_workspace_plugin() {
    let codex_home = TempDir::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let plugin_path =
        AbsolutePathBuf::try_from(write_test_plugin(temp_dir.path(), "demo-plugin")).unwrap();
    let archive_size = archive_plugin_for_upload(plugin_path.as_path())
        .unwrap()
        .len();
    let server = MockServer::start().await;
    let config = test_config(&server);
    let auth = test_auth();

    Mock::given(method("POST"))
        .and(path("/backend-api/public/plugins/workspace/upload-url"))
        .and(body_json(json!({
            "filename": "demo-plugin.tar.gz",
            "mime_type": "application/gzip",
            "size_bytes": archive_size,
            "plugin_id": "plugins_123",
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "file_id": "file_456",
            "upload_url": format!("{}/upload/file_456", server.uri()),
            "etag": "\"upload_etag_456\"",
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload/file_456"))
        .respond_with(ResponseTemplate::new(201).insert_header("etag", "\"blob_etag_456\""))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/backend-api/public/plugins/workspace/plugins_123"))
        .and(body_json(json!({
            "file_id": "file_456",
            "etag": "\"upload_etag_456\"",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugin_id": "plugins_123",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = save_remote_plugin_share(
        &config,
        Some(&auth),
        codex_home.path(),
        &plugin_path,
        Some("plugins_123"),
    )
    .await
    .unwrap();

    assert_eq!(
        result,
        RemotePluginShareSaveResult {
            remote_plugin_id: "plugins_123".to_string(),
            share_url: None,
        }
    );
}

#[tokio::test]
async fn list_remote_plugin_shares_fetches_created_workspace_plugins() {
    let codex_home = TempDir::new().unwrap();
    let local_plugin_path =
        AbsolutePathBuf::try_from(codex_home.path().join("local-plugin")).unwrap();
    write_plugin_share_local_path_mapping(codex_home.path(), "plugins_123", &local_plugin_path);
    let server = MockServer::start().await;
    let config = test_config(&server);
    let auth = test_auth();

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/workspace/created"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .and(query_param(
            "limit",
            REMOTE_PLUGIN_LIST_PAGE_LIMIT.to_string(),
        ))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [remote_plugin_json_with_share_url(
                "plugins_123",
                Some("https://chatgpt.example/plugins/share/share-key-1"),
            )],
            "pagination": {
                "next_page_token": "page-2"
            },
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/workspace/created"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .and(query_param(
            "limit",
            REMOTE_PLUGIN_LIST_PAGE_LIMIT.to_string(),
        ))
        .and(query_param("pageToken", "page-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [remote_plugin_json_with_share_url("plugins_456", /*share_url*/ None)],
            "pagination": empty_pagination_json(),
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("scope", "WORKSPACE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [installed_remote_plugin_json("plugins_456")],
            "pagination": empty_pagination_json(),
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = list_remote_plugin_shares(&config, Some(&auth), codex_home.path())
        .await
        .unwrap();

    assert_eq!(
        result,
        vec![
            RemotePluginShareSummary {
                summary: RemotePluginSummary {
                    id: "plugins_123".to_string(),
                    name: "demo-plugin".to_string(),
                    installed: false,
                    enabled: false,
                    install_policy: PluginInstallPolicy::Available,
                    auth_policy: PluginAuthPolicy::OnUse,
                    availability: PluginAvailability::Available,
                    interface: Some(expected_plugin_interface()),
                },
                share_url: Some("https://chatgpt.example/plugins/share/share-key-1".to_string()),
                local_plugin_path: Some(local_plugin_path),
            },
            RemotePluginShareSummary {
                summary: RemotePluginSummary {
                    id: "plugins_456".to_string(),
                    name: "demo-plugin".to_string(),
                    installed: true,
                    enabled: true,
                    install_policy: PluginInstallPolicy::Available,
                    auth_policy: PluginAuthPolicy::OnUse,
                    availability: PluginAvailability::Available,
                    interface: Some(expected_plugin_interface()),
                },
                share_url: None,
                local_plugin_path: None,
            }
        ]
    );
}

#[tokio::test]
async fn delete_remote_plugin_share_deletes_workspace_plugin() {
    let codex_home = TempDir::new().unwrap();
    let local_plugin_path =
        AbsolutePathBuf::try_from(codex_home.path().join("local-plugin")).unwrap();
    write_plugin_share_local_path_mapping(codex_home.path(), "plugins_123", &local_plugin_path);
    let server = MockServer::start().await;
    let config = test_config(&server);
    let auth = test_auth();

    Mock::given(method("DELETE"))
        .and(path("/backend-api/public/plugins/workspace/plugins_123"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    delete_remote_plugin_share(&config, Some(&auth), codex_home.path(), "plugins_123")
        .await
        .unwrap();
    assert_eq!(
        local_paths::load_plugin_share_local_paths(codex_home.path()).unwrap(),
        BTreeMap::new()
    );
}
