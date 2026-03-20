use super::*;
use crate::auth::CodexAuth;
use crate::config::CONFIG_TOML_FILE;
use crate::plugins::test_support::TEST_CURATED_PLUGIN_SHA;
use crate::plugins::test_support::write_curated_plugin_sha;
use crate::plugins::test_support::write_file;
use crate::plugins::test_support::write_openai_curated_marketplace;
use pretty_assertions::assert_eq;
use std::io::Write;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[test]
fn curated_plugins_repo_path_uses_codex_home_tmp_dir() {
    let tmp = tempdir().expect("tempdir");
    assert_eq!(
        curated_plugins_repo_path(tmp.path()),
        tmp.path().join(".tmp/plugins")
    );
}

#[test]
fn read_curated_plugins_sha_reads_trimmed_sha_file() {
    let tmp = tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".tmp")).expect("create tmp");
    std::fs::write(tmp.path().join(".tmp/plugins.sha"), "abc123\n").expect("write sha");

    assert_eq!(
        read_curated_plugins_sha(tmp.path()).as_deref(),
        Some("abc123")
    );
}

#[tokio::test]
async fn sync_openai_plugins_repo_downloads_zipball_and_records_sha() {
    let tmp = tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let sha = "0123456789abcdef0123456789abcdef01234567";

    Mock::given(method("GET"))
        .and(path("/repos/openai/plugins"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"default_branch":"main"}"#))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/openai/plugins/git/ref/heads/main"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!(r#"{{"object":{{"sha":"{sha}"}}}}"#)),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/openai/plugins/zipball/{sha}")))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/zip")
                .set_body_bytes(curated_repo_zipball_bytes(sha)),
        )
        .mount(&server)
        .await;

    let server_uri = server.uri();
    let tmp_path = tmp.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        sync_openai_plugins_repo_with_api_base_url(tmp_path.as_path(), &server_uri)
    })
    .await
    .expect("sync task should join")
    .expect("sync should succeed");

    let repo_path = curated_plugins_repo_path(tmp.path());
    assert!(repo_path.join(".agents/plugins/marketplace.json").is_file());
    assert!(
        repo_path
            .join("plugins/gmail/.codex-plugin/plugin.json")
            .is_file()
    );
    assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
}

#[tokio::test]
async fn sync_openai_plugins_repo_skips_archive_download_when_sha_matches() {
    let tmp = tempdir().expect("tempdir");
    let repo_path = curated_plugins_repo_path(tmp.path());
    std::fs::create_dir_all(repo_path.join(".agents/plugins")).expect("create repo");
    std::fs::write(
        repo_path.join(".agents/plugins/marketplace.json"),
        r#"{"name":"openai-curated","plugins":[]}"#,
    )
    .expect("write marketplace");
    std::fs::create_dir_all(tmp.path().join(".tmp")).expect("create tmp");
    let sha = "fedcba9876543210fedcba9876543210fedcba98";
    std::fs::write(tmp.path().join(".tmp/plugins.sha"), format!("{sha}\n")).expect("write sha");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/openai/plugins"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"default_branch":"main"}"#))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/openai/plugins/git/ref/heads/main"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!(r#"{{"object":{{"sha":"{sha}"}}}}"#)),
        )
        .mount(&server)
        .await;

    let server_uri = server.uri();
    let tmp_path = tmp.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        sync_openai_plugins_repo_with_api_base_url(tmp_path.as_path(), &server_uri)
    })
    .await
    .expect("sync task should join")
    .expect("sync should succeed");

    assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
    assert!(repo_path.join(".agents/plugins/marketplace.json").is_file());
}

#[tokio::test]
async fn startup_remote_plugin_sync_writes_marker_and_reconciles_state() {
    let tmp = tempdir().expect("tempdir");
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["linear"]);
    write_curated_plugin_sha(tmp.path());
    write_file(
        &tmp.path().join(CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[plugins."linear@openai-curated"]
enabled = false
"#,
    );

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/list"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"[
  {"id":"1","name":"linear","marketplace_name":"openai-curated","version":"1.0.0","enabled":true}
]"#,
        ))
        .mount(&server)
        .await;

    let mut config = crate::plugins::test_support::load_plugins_config(tmp.path()).await;
    config.chatgpt_base_url = format!("{}/backend-api/", server.uri());
    let manager = Arc::new(PluginsManager::new(tmp.path().to_path_buf()));
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());

    start_startup_remote_plugin_sync_once(
        Arc::clone(&manager),
        tmp.path().to_path_buf(),
        config,
        auth_manager,
    );

    let marker_path = tmp.path().join(STARTUP_REMOTE_PLUGIN_SYNC_MARKER_FILE);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if marker_path.is_file() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("marker should be written");

    assert!(
        tmp.path()
            .join(format!(
                "plugins/cache/openai-curated/linear/{TEST_CURATED_PLUGIN_SHA}"
            ))
            .is_dir()
    );
    let config =
        std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("config should exist");
    assert!(config.contains(r#"[plugins."linear@openai-curated"]"#));
    assert!(config.contains("enabled = true"));

    let marker_contents = std::fs::read_to_string(marker_path).expect("marker should be readable");
    assert_eq!(marker_contents, "ok\n");
}

fn curated_repo_zipball_bytes(sha: &str) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default();
    let root = format!("openai-plugins-{sha}");
    writer
        .start_file(format!("{root}/.agents/plugins/marketplace.json"), options)
        .expect("start marketplace entry");
    writer
        .write_all(
            br#"{
  "name": "openai-curated",
  "plugins": [
    {
      "name": "gmail",
      "source": {
        "source": "local",
        "path": "./plugins/gmail"
      }
    }
  ]
}"#,
        )
        .expect("write marketplace");
    writer
        .start_file(
            format!("{root}/plugins/gmail/.codex-plugin/plugin.json"),
            options,
        )
        .expect("start plugin manifest entry");
    writer
        .write_all(br#"{"name":"gmail"}"#)
        .expect("write plugin manifest");

    writer.finish().expect("finish zip writer").into_inner()
}
