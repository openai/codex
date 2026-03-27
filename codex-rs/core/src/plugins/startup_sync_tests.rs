use super::*;
use crate::auth::CodexAuth;
use crate::config::CONFIG_TOML_FILE;
use crate::plugins::test_support::TEST_CURATED_PLUGIN_SHA;
use crate::plugins::test_support::write_curated_plugin_sha;
use crate::plugins::test_support::write_file;
use crate::plugins::test_support::write_openai_curated_marketplace;
use pretty_assertions::assert_eq;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

fn has_plugins_clone_dirs(codex_home: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(codex_home.join(".tmp")) else {
        return false;
    };

    entries.flatten().any(|entry| {
        let path = entry.path();
        path.is_dir()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("plugins-clone-"))
    })
}

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

#[cfg(unix)]
#[test]
fn remove_stale_curated_repo_temp_dirs_removes_only_matching_directories() {
    use std::os::unix::ffi::OsStrExt;
    use std::time::SystemTime;

    fn set_dir_mtime(path: &Path, age: Duration) -> Result<(), Box<dyn std::error::Error>> {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
        let modified_at = now.saturating_sub(age);
        let tv_sec = i64::try_from(modified_at.as_secs())?;
        let ts = libc::timespec { tv_sec, tv_nsec: 0 };
        let times = [ts, ts];
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        let result = unsafe { libc::utimensat(libc::AT_FDCWD, c_path.as_ptr(), times.as_ptr(), 0) };
        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }

    let tmp = tempdir().expect("tempdir");
    let parent = tmp.path().join(".tmp");
    let stale_clone_dir = parent.join("plugins-clone-stale");
    let fresh_clone_dir = parent.join("plugins-clone-fresh");
    let unrelated_dir = parent.join("plugins-cache");

    std::fs::create_dir_all(&stale_clone_dir).expect("create stale clone dir");
    std::fs::create_dir_all(&fresh_clone_dir).expect("create fresh clone dir");
    std::fs::create_dir_all(&unrelated_dir).expect("create unrelated dir");
    set_dir_mtime(
        &stale_clone_dir,
        CURATED_PLUGINS_STALE_TEMP_DIR_MAX_AGE + Duration::from_secs(60),
    )
    .expect("age stale clone dir");
    set_dir_mtime(&fresh_clone_dir, Duration::ZERO).expect("age fresh clone dir");

    remove_stale_curated_repo_temp_dirs(&parent, CURATED_PLUGINS_STALE_TEMP_DIR_MAX_AGE);

    assert!(!stale_clone_dir.exists());
    assert!(fresh_clone_dir.is_dir());
    assert!(unrelated_dir.is_dir());
}

#[cfg(unix)]
#[test]
fn sync_openai_plugins_repo_prefers_git_when_available() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempdir().expect("tempdir");
    let bin_dir = tempfile::Builder::new()
        .prefix("fake-git-")
        .tempdir()
        .expect("tempdir");
    let git_path = bin_dir.path().join("git");
    let sha = "0123456789abcdef0123456789abcdef01234567";

    std::fs::write(
        &git_path,
        format!(
            r#"#!/bin/sh
if [ "$1" = "ls-remote" ]; then
  printf '%s\tHEAD\n' "{sha}"
  exit 0
fi
if [ "$1" = "clone" ]; then
  dest="$5"
  mkdir -p "$dest/.git" "$dest/.agents/plugins" "$dest/plugins/gmail/.codex-plugin"
  cat > "$dest/.agents/plugins/marketplace.json" <<'EOF'
{{"name":"openai-curated","plugins":[{{"name":"gmail","source":{{"source":"local","path":"./plugins/gmail"}}}}]}}
EOF
  printf '%s\n' '{{"name":"gmail"}}' > "$dest/plugins/gmail/.codex-plugin/plugin.json"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "rev-parse" ] && [ "$4" = "HEAD" ]; then
  printf '%s\n' "{sha}"
  exit 0
fi
echo "unexpected git invocation: $@" >&2
exit 1
"#
        ),
    )
    .expect("write fake git");
    let mut permissions = std::fs::metadata(&git_path)
        .expect("metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&git_path, permissions).expect("chmod");

    let synced_sha = sync_openai_plugins_repo_with_transport_overrides(
        tmp.path(),
        git_path.to_str().expect("utf8 path"),
        "http://127.0.0.1:9",
    )
    .expect("git sync should succeed");

    assert_eq!(synced_sha, sha);
    assert!(curated_plugins_repo_path(tmp.path()).join(".git").is_dir());
    assert!(
        curated_plugins_repo_path(tmp.path())
            .join(".agents/plugins/marketplace.json")
            .is_file()
    );
    assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
}

#[tokio::test]
async fn sync_openai_plugins_repo_falls_back_to_http_when_git_is_unavailable() {
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
    let synced_sha = tokio::task::spawn_blocking(move || {
        sync_openai_plugins_repo_with_transport_overrides(
            tmp_path.as_path(),
            "missing-git-for-test",
            &server_uri,
        )
    })
    .await
    .expect("sync task should join")
    .expect("fallback sync should succeed");

    let repo_path = curated_plugins_repo_path(tmp.path());
    assert_eq!(synced_sha, sha);
    assert!(repo_path.join(".agents/plugins/marketplace.json").is_file());
    assert!(
        repo_path
            .join("plugins/gmail/.codex-plugin/plugin.json")
            .is_file()
    );
    assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
}

#[cfg(unix)]
#[tokio::test]
async fn sync_openai_plugins_repo_falls_back_to_http_when_git_sync_fails() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempdir().expect("tempdir");
    let bin_dir = tempfile::Builder::new()
        .prefix("fake-git-fail-")
        .tempdir()
        .expect("tempdir");
    let git_path = bin_dir.path().join("git");
    let sha = "0123456789abcdef0123456789abcdef01234567";

    std::fs::write(
        &git_path,
        r#"#!/bin/sh
echo "simulated git failure" >&2
exit 1
"#,
    )
    .expect("write fake git");
    let mut permissions = std::fs::metadata(&git_path)
        .expect("metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&git_path, permissions).expect("chmod");

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
    let synced_sha = tokio::task::spawn_blocking(move || {
        sync_openai_plugins_repo_with_transport_overrides(
            tmp_path.as_path(),
            git_path.to_str().expect("utf8 path"),
            &server_uri,
        )
    })
    .await
    .expect("sync task should join")
    .expect("fallback sync should succeed");

    let repo_path = curated_plugins_repo_path(tmp.path());
    assert_eq!(synced_sha, sha);
    assert!(repo_path.join(".agents/plugins/marketplace.json").is_file());
    assert!(
        repo_path
            .join("plugins/gmail/.codex-plugin/plugin.json")
            .is_file()
    );
    assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
}

#[cfg(unix)]
#[test]
fn sync_openai_plugins_repo_via_git_cleans_up_staged_dir_on_clone_failure() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempdir().expect("tempdir");
    let bin_dir = tempfile::Builder::new()
        .prefix("fake-git-partial-fail-")
        .tempdir()
        .expect("tempdir");
    let git_path = bin_dir.path().join("git");
    let sha = "0123456789abcdef0123456789abcdef01234567";

    std::fs::write(
        &git_path,
        format!(
            r#"#!/bin/sh
if [ "$1" = "ls-remote" ]; then
  printf '%s\tHEAD\n' "{sha}"
  exit 0
fi
if [ "$1" = "clone" ]; then
  dest="$5"
  mkdir -p "$dest/.git"
  echo "fatal: early EOF" >&2
  exit 128
fi
echo "unexpected git invocation: $@" >&2
exit 1
"#
        ),
    )
    .expect("write fake git");
    let mut permissions = std::fs::metadata(&git_path)
        .expect("metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&git_path, permissions).expect("chmod");

    let err = sync_openai_plugins_repo_via_git(tmp.path(), git_path.to_str().expect("utf8 path"))
        .expect_err("git sync should fail");

    assert!(err.contains("fatal: early EOF"));
    assert!(!has_plugins_clone_dirs(tmp.path()));
}

#[tokio::test]
async fn sync_openai_plugins_repo_via_http_cleans_up_staged_dir_on_extract_failure() {
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
                .set_body_bytes(b"not a zip archive".to_vec()),
        )
        .mount(&server)
        .await;

    let server_uri = server.uri();
    let tmp_path = tmp.path().to_path_buf();
    let err = tokio::task::spawn_blocking(move || {
        sync_openai_plugins_repo_via_http(tmp_path.as_path(), &server_uri)
    })
    .await
    .expect("sync task should join")
    .expect_err("http sync should fail");

    assert!(err.contains("failed to open curated plugins zip archive"));
    assert!(!has_plugins_clone_dirs(tmp.path()));
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
        sync_openai_plugins_repo_with_transport_overrides(
            tmp_path.as_path(),
            "missing-git-for-test",
            &server_uri,
        )
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
    let cursor = std::io::Cursor::new(Vec::new());
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
