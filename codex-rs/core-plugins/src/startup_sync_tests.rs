use super::*;
use crate::git_policy::REPOSITORY_LOCAL_GIT_ENVIRONMENT_VARIABLES;
use crate::test_support::RecordingHttpClientSelector;
use crate::test_support::recorded_http_client_urls;
use pretty_assertions::assert_eq;
use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::Barrier;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header_exists;
use wiremock::matchers::method;
use wiremock::matchers::path;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const TEST_CURATED_PLUGIN_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

#[tokio::test]
async fn github_http_routes_repository_ref_and_zipball_urls() {
    let server = MockServer::start().await;
    let sha = TEST_CURATED_PLUGIN_SHA;
    let zipball = b"archive".to_vec();
    mount_github_repo_and_ref(&server, sha).await;
    mount_github_zipball(&server, sha, zipball.clone()).await;
    let api_base_url = server.uri();
    let repo_url = format!("{api_base_url}/repos/openai/plugins");
    let ref_url = format!("{repo_url}/git/ref/heads/main");
    let zipball_url = format!("{repo_url}/zipball/{sha}");
    let (http_clients, selected_urls) = RecordingHttpClientSelector::new();
    let http_clients = StartupSyncHttpClient::route_aware(http_clients);

    let remote_sha = fetch_curated_repo_remote_sha(&http_clients, &api_base_url)
        .await
        .expect("remote SHA request should succeed");
    let downloaded_zipball = fetch_curated_repo_zipball(&http_clients, &api_base_url, &remote_sha)
        .await
        .expect("zipball request should succeed");

    assert_eq!(remote_sha, sha);
    assert_eq!(downloaded_zipball, zipball);
    assert_eq!(
        recorded_http_client_urls(&selected_urls),
        vec![repo_url, ref_url, zipball_url]
    );
}

#[tokio::test]
async fn backup_archive_routes_metadata_and_backend_supplied_download_urls() {
    let metadata_server = MockServer::start().await;
    let download_server = MockServer::start().await;
    let download_url = format!(
        "{}/files/curated-plugins.zip?sig=signed",
        download_server.uri()
    );
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/export/curated"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"download_url": download_url.clone()})),
        )
        .expect(1)
        .mount(&metadata_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/files/curated-plugins.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"archive".to_vec()))
        .expect(1)
        .mount(&download_server)
        .await;
    let metadata_url = format!(
        "{}/backend-api/plugins/export/curated",
        metadata_server.uri()
    );
    let (http_clients, selected_urls) = RecordingHttpClientSelector::new();
    let http_clients = StartupSyncHttpClient::route_aware(http_clients);

    let body = fetch_curated_repo_backup_archive_zip(&http_clients, &metadata_url)
        .await
        .expect("backup archive download should succeed");

    assert_eq!(body, b"archive");
    assert_eq!(
        recorded_http_client_urls(&selected_urls),
        vec![metadata_url, download_url]
    );
}

#[test]
fn git_command_sanitizes_ambient_repository_environment() {
    let command = git_command(Path::new("git"));

    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            OsStr::new("-c"),
            OsStr::new(codex_git_utils::SAFE_BARE_REPOSITORY_CONFIG),
        ]
    );

    for name in REPOSITORY_LOCAL_GIT_ENVIRONMENT_VARIABLES {
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == OsStr::new(name))
                .map(|(_, value)| value),
            Some(None),
            "{name} should be removed from startup sync Git commands"
        );
    }
}

#[cfg(unix)]
#[test]
fn pretrust_git_sync_ignores_repository_local_transport_config() {
    let fixture = tempdir().expect("tempdir");
    let codex_home = fixture.path().join("codex-home");
    let repository = fixture.path().join("untrusted-project");
    let marker = fixture.path().join("transport-config-ran");
    std::fs::create_dir_all(&codex_home).expect("create Codex home");
    std::fs::create_dir_all(&repository).expect("create repository");
    run_git(&repository, &["init", "--quiet"]);

    let transport = repository.join("synthetic-transport.sh");
    write_executable_script(
        &transport,
        &format!(
            "#!/bin/sh\nprintf ran > '{}'\nprintf '{}\\tHEAD\\n'\n",
            marker.display(),
            TEST_CURATED_PLUGIN_SHA
        ),
    );
    run_git(
        &repository,
        &["config", "--local", "protocol.ext.allow", "always"],
    );
    let rewrite_key = format!("url.ext::{} %S .insteadOf", transport.display());
    run_git(
        &repository,
        &["config", "--local", &rewrite_key, OPENAI_PLUGINS_GIT_URL],
    );

    let global_config = fixture.path().join("global-gitconfig");
    std::fs::write(
        &global_config,
        format!(
            "[url \"file://{}/\"]\n\tinsteadOf = https://github.com/\n",
            fixture.path().join("missing-remotes").display()
        ),
    )
    .expect("write global Git config");
    let git_wrapper = fixture.path().join("git-from-untrusted-repository.sh");
    write_executable_script(
        &git_wrapper,
        &format!(
            "#!/bin/sh\ncd '{}' || exit 1\nGIT_CONFIG_GLOBAL='{}' GIT_CONFIG_SYSTEM=/dev/null GIT_TERMINAL_PROMPT=0 exec git \"$@\"\n",
            repository.display(),
            global_config.display()
        ),
    );

    let err = sync_openai_plugins_repo_via_git(&codex_home, &git_wrapper)
        .expect_err("isolated probe should use the missing global-config remote");

    assert!(err.contains("git ls-remote curated plugins repo"));
    assert!(
        !marker.exists(),
        "pre-trust sync must not execute repository-local transport configuration"
    );
}

#[tokio::test]
async fn ordinary_clone_rejects_tracked_embedded_bare_repository() {
    let temp_dir = tempdir().expect("create temporary directory");
    let source = temp_dir.path().join("source");
    let clone = temp_dir.path().join("clone");
    let nested_source = source.join("nested");
    std::fs::create_dir_all(nested_source.join("objects")).expect("create nested object directory");
    std::fs::create_dir_all(nested_source.join("refs"))
        .expect("create nested references directory");

    let run_setup_git = |cwd: &Path, args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("run repository setup Git command");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };

    run_setup_git(&source, &["init", "--quiet"]);
    std::fs::write(nested_source.join("HEAD"), "ref: refs/heads/main\n")
        .expect("write tracked nested HEAD");
    std::fs::write(
        nested_source.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\tworktree = .\n\tfsmonitor = ./payload.sh\n",
    )
    .expect("write tracked nested Git configuration");
    std::fs::write(nested_source.join("objects/.keep"), "").expect("track nested object directory");
    std::fs::write(nested_source.join("refs/.keep"), "")
        .expect("track nested references directory");
    std::fs::write(
        nested_source.join("payload.sh"),
        "#!/bin/sh\nprintf ran > \"$0.ran\"\n",
    )
    .expect("write tracked filesystem monitor");

    run_setup_git(&source, &["add", "--all"]);
    run_setup_git(&source, &["add", "--chmod=+x", "nested/payload.sh"]);
    run_setup_git(
        &source,
        &[
            "-c",
            "user.name=Codex Tests",
            "-c",
            "user.email=codex-tests@example.com",
            "commit",
            "--quiet",
            "-m",
            "track embedded Git repository",
        ],
    );
    let clone_output = Command::new("git")
        .arg("clone")
        .arg(&source)
        .arg(&clone)
        .output()
        .expect("clone repository normally");
    assert!(
        clone_output.status.success(),
        "ordinary git clone failed: {}",
        String::from_utf8_lossy(&clone_output.stderr)
    );

    let nested = clone.join("nested");
    let marker = nested.join("payload.sh.ran");
    let vulnerable = Command::new("git")
        .args(["status", "--short"])
        .current_dir(&nested)
        .output()
        .expect("run unguarded Git against tracked embedded repository");
    assert!(
        vulnerable.status.success(),
        "unguarded Git should discover the tracked embedded repository: {}",
        String::from_utf8_lossy(&vulnerable.stderr)
    );
    assert!(
        marker.exists(),
        "unguarded Git should execute the tracked helper"
    );
    std::fs::remove_file(&marker).expect("remove unguarded execution marker");

    let guarded = git_command(Path::new("git"))
        .args(["status", "--short"])
        .current_dir(&nested)
        .output()
        .expect("run guarded startup Git against tracked embedded repository");
    assert!(
        !guarded.status.success(),
        "startup Git should reject the repository"
    );
    assert!(
        !marker.exists(),
        "startup Git must reject the repository before executing its helper"
    );

    let apply_error = codex_git_utils::apply_git_patch(&codex_git_utils::ApplyGitRequest {
        cwd: nested.clone(),
        diff: String::new(),
        revert: false,
        preflight: true,
    })
    .expect_err("patch root discovery should reject the tracked embedded repository");
    assert!(apply_error.to_string().contains("not a git repository"));
    assert!(codex_git_utils::collect_git_info(&nested).await.is_none());
    assert!(codex_git_utils::git_diff_to_remote(&nested).await.is_none());
    assert!(
        !marker.exists(),
        "Rust-owned Git inspection must not execute the tracked helper"
    );

    let explicit_git_dir = git_command(Path::new("git"))
        .arg("--git-dir")
        .arg(&nested)
        .args(["rev-parse", "--git-dir"])
        .current_dir(&clone)
        .output()
        .expect("run Git with an explicitly selected bare repository");
    assert!(
        explicit_git_dir.status.success(),
        "--git-dir must continue to permit an explicitly selected repository: {}",
        String::from_utf8_lossy(&explicit_git_dir.stderr)
    );

    let explicit_environment = Command::new("git")
        .args(["-c", codex_git_utils::SAFE_BARE_REPOSITORY_CONFIG])
        .args(["rev-parse", "--git-dir"])
        .env("GIT_DIR", &nested)
        .current_dir(&clone)
        .output()
        .expect("run Git with explicitly selected GIT_DIR");
    assert!(
        explicit_environment.status.success(),
        "GIT_DIR must continue to permit an explicitly selected repository: {}",
        String::from_utf8_lossy(&explicit_environment.stderr)
    );
}

fn write_file(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("file should have a parent")).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn write_curated_plugin(root: &Path, plugin_name: &str) {
    let plugin_root = root.join("plugins").join(plugin_name);
    write_file(
        &plugin_root.join(".codex-plugin/plugin.json"),
        &format!(r#"{{"name":"{plugin_name}"}}"#),
    );
}

fn write_openai_curated_marketplace(root: &Path, plugin_names: &[&str]) {
    let plugins = plugin_names
        .iter()
        .map(|plugin_name| {
            format!(
                r#"{{
      "name": "{plugin_name}",
      "source": {{
        "source": "local",
        "path": "./plugins/{plugin_name}"
      }}
    }}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    write_file(
        &root.join(".agents/plugins/marketplace.json"),
        &format!(
            r#"{{
  "name": "openai-curated",
  "plugins": [
{plugins}
  ]
}}"#
        ),
    );
    for plugin_name in plugin_names {
        write_curated_plugin(root, plugin_name);
    }
}

fn write_curated_plugin_sha(codex_home: &Path) {
    write_file(
        &codex_home.join(".tmp/plugins.sha"),
        &format!("{TEST_CURATED_PLUGIN_SHA}\n"),
    );
}

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

#[cfg(unix)]
fn write_executable_script(path: &Path, contents: &str) {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(path, contents).expect("write script");
    #[cfg(unix)]
    {
        let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("chmod");
    }
}

#[cfg(unix)]
fn run_git(repo: &Path, args: &[&str]) -> std::process::Output {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

async fn mount_github_repo_and_ref(server: &MockServer, sha: &str) {
    Mock::given(method("GET"))
        .and(path("/repos/openai/plugins"))
        .and(header_exists("user-agent"))
        .and(header_exists("originator"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"default_branch":"main"}"#))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/repos/openai/plugins/git/ref/heads/main"))
        .and(header_exists("user-agent"))
        .and(header_exists("originator"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!(r#"{{"object":{{"sha":"{sha}"}}}}"#)),
        )
        .mount(server)
        .await;
}

async fn mount_github_zipball(server: &MockServer, sha: &str, bytes: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/openai/plugins/zipball/{sha}")))
        .and(header_exists("user-agent"))
        .and(header_exists("originator"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/zip")
                .set_body_bytes(bytes),
        )
        .mount(server)
        .await;
}

async fn mount_export_archive(server: &MockServer, bytes: Vec<u8>) -> String {
    let export_api_url = format!("{}/backend-api/plugins/export/curated", server.uri());
    Mock::given(method("GET"))
        .and(path("/backend-api/plugins/export/curated"))
        .and(header_exists("user-agent"))
        .and(header_exists("originator"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!(
            r#"{{"download_url":"{}/files/curated-plugins.zip"}}"#,
            server.uri()
        )))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/files/curated-plugins.zip"))
        .and(header_exists("user-agent"))
        .and(header_exists("originator"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/zip")
                .set_body_bytes(bytes),
        )
        .mount(server)
        .await;
    export_api_url
}

async fn run_sync_with_transport_overrides(
    codex_home: PathBuf,
    git_binary: impl Into<String>,
    api_base_url: impl Into<String>,
    backup_archive_api_url: impl Into<String>,
) -> Result<String, String> {
    let git_binary = git_binary.into();
    let api_base_url = api_base_url.into();
    let backup_archive_api_url = backup_archive_api_url.into();
    tokio::task::spawn_blocking(move || {
        let git_binary = PathBuf::from(git_binary);
        sync_openai_plugins_repo_with_transport_overrides(
            codex_home.as_path(),
            Some(git_binary.as_path()),
            &api_base_url,
            &backup_archive_api_url,
            &crate::test_support::test_http_client_factory(),
        )
    })
    .await
    .expect("sync task should join")
}

async fn run_sync_without_git(
    codex_home: PathBuf,
    api_base_url: impl Into<String>,
    backup_archive_api_url: impl Into<String>,
) -> Result<String, String> {
    let api_base_url = api_base_url.into();
    let backup_archive_api_url = backup_archive_api_url.into();
    tokio::task::spawn_blocking(move || {
        sync_openai_plugins_repo_with_transport_overrides(
            codex_home.as_path(),
            /*git_binary*/ None,
            &api_base_url,
            &backup_archive_api_url,
            &crate::test_support::test_http_client_factory(),
        )
    })
    .await
    .expect("sync task should join")
}

async fn run_http_sync(
    codex_home: PathBuf,
    api_base_url: impl Into<String>,
) -> Result<String, String> {
    let api_base_url = api_base_url.into();
    tokio::task::spawn_blocking(move || {
        sync_openai_plugins_repo_via_http(
            codex_home.as_path(),
            &api_base_url,
            &crate::test_support::test_http_client_factory(),
        )
    })
    .await
    .expect("sync task should join")
}

fn assert_curated_gmail_repo(repo_path: &Path) {
    assert!(repo_path.join(".agents/plugins/marketplace.json").is_file());
    assert!(
        repo_path
            .join("plugins/gmail/.codex-plugin/plugin.json")
            .is_file()
    );
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
fn concurrent_syncs_serialize_fetches_without_skipping_remote_checks() {
    let tmp = tempdir().expect("tempdir");
    let bin_dir = tempfile::Builder::new()
        .prefix("fake-git-")
        .tempdir()
        .expect("tempdir");
    let git_path = bin_dir.path().join("git");
    let invocation_log = bin_dir.path().join("invocations.log");
    let sha = "0123456789abcdef0123456789abcdef01234567";

    write_executable_script(
        &git_path,
        &format!(
            r#"#!/bin/sh
if [ "$1" = "-c" ] && [ "$2" = "safe.bareRepository=explicit" ]; then shift 2; fi
printf '%s\n' "$*" >> '{}'
if [ "$1" = "ls-remote" ]; then
  sleep 1
  printf '%s\tHEAD\n' "{sha}"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "init" ]; then
  mkdir -p "$2/.git"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "fetch" ]; then
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "reset" ]; then
  mkdir -p "$2/.agents/plugins" "$2/plugins/gmail/.codex-plugin"
  cat > "$2/.agents/plugins/marketplace.json" <<'EOF'
{{"name":"openai-curated","plugins":[{{"name":"gmail","source":{{"source":"local","path":"./plugins/gmail"}}}}]}}
EOF
  printf '%s\n' '{{"name":"gmail"}}' > "$2/plugins/gmail/.codex-plugin/plugin.json"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "clean" ]; then
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "rev-parse" ] && [ "$4" = "HEAD" ]; then
  printf '%s\n' "{sha}"
  exit 0
fi
echo "unexpected git invocation: $@" >&2
exit 1
"#,
            invocation_log.display()
        ),
    );

    let barrier = Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let run_sync = || {
            barrier.wait();
            sync_openai_plugins_repo_with_transport_overrides(
                tmp.path(),
                Some(git_path.as_path()),
                "http://127.0.0.1:9",
                "http://127.0.0.1:9/backend-api/plugins/export/curated",
                &crate::test_support::test_http_client_factory(),
            )
        };
        let first = scope.spawn(run_sync);
        let second = scope.spawn(run_sync);
        [
            first.join().expect("first sync thread"),
            second.join().expect("second sync thread"),
        ]
    });

    assert_eq!(results, [Ok(sha.to_string()), Ok(sha.to_string())]);
    let repo_path = curated_plugins_repo_path(tmp.path());
    assert!(repo_path.join(".git").is_dir());
    assert_curated_gmail_repo(&repo_path);
    assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
    let invocations = std::fs::read_to_string(invocation_log).expect("read invocation log");
    assert_eq!(
        invocations
            .lines()
            .filter(|invocation| invocation.starts_with("ls-remote "))
            .count(),
        2
    );
    assert_eq!(
        invocations
            .lines()
            .filter(|invocation| invocation.contains(" fetch --depth 1 --no-tags "))
            .count(),
        1
    );
    assert!(
        !invocations
            .lines()
            .any(|invocation| invocation.split_whitespace().any(|arg| arg == "clone"))
    );
}

#[cfg(unix)]
#[test]
fn sync_openai_plugins_repo_via_git_succeeds_with_local_rewritten_remote() {
    let tmp = tempdir().expect("tempdir");
    let repo_root = tempfile::Builder::new()
        .prefix("curated-repo-success-")
        .tempdir()
        .expect("tempdir");
    let work_repo = repo_root.path().join("work/plugins");
    let remote_repo = repo_root.path().join("remotes/openai/plugins.git");
    std::fs::create_dir_all(work_repo.join(".agents/plugins")).expect("create marketplace dir");
    std::fs::create_dir_all(work_repo.join("plugins/gmail/.codex-plugin"))
        .expect("create plugin dir");
    std::fs::write(
        work_repo.join(".agents/plugins/marketplace.json"),
        r#"{"name":"openai-curated","plugins":[{"name":"gmail","source":{"source":"local","path":"./plugins/gmail"}}]}"#,
    )
    .expect("write marketplace");
    std::fs::write(
        work_repo.join("plugins/gmail/.codex-plugin/plugin.json"),
        r#"{"name":"gmail"}"#,
    )
    .expect("write plugin manifest");

    run_git(&work_repo, &["init"]);
    run_git(&work_repo, &["add", "."]);
    run_git(
        &work_repo,
        &[
            "-c",
            "user.name=Codex Test",
            "-c",
            "user.email=codex@example.com",
            "commit",
            "-m",
            "init",
        ],
    );

    std::fs::create_dir_all(remote_repo.parent().expect("remote parent"))
        .expect("create remote parent");
    let clone_status = Command::new("git")
        .arg("clone")
        .arg("--bare")
        .arg(&work_repo)
        .arg(&remote_repo)
        .status()
        .expect("run git clone --bare");
    assert!(clone_status.success());

    let sha_output = run_git(&work_repo, &["rev-parse", "HEAD"]);
    let sha = String::from_utf8_lossy(&sha_output.stdout)
        .trim()
        .to_string();

    let git_config_path = repo_root.path().join("git-rewrite.conf");
    std::fs::write(
        &git_config_path,
        format!(
            "[url \"file://{}/\"]\n    insteadOf = https://github.com/\n",
            repo_root.path().join("remotes").display()
        ),
    )
    .expect("write git config");

    let bin_dir = tempfile::Builder::new()
        .prefix("git-rewrite-wrapper-")
        .tempdir()
        .expect("tempdir");
    let git_wrapper = bin_dir.path().join("git");
    let invocation_log = bin_dir.path().join("invocations.log");
    write_executable_script(
        &git_wrapper,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nGIT_CONFIG_GLOBAL='{}' exec git \"$@\"\n",
            invocation_log.display(),
            git_config_path.display()
        ),
    );

    let synced_sha = sync_openai_plugins_repo_via_git(tmp.path(), &git_wrapper)
        .expect("git sync should succeed");

    assert_eq!(synced_sha, sha);
    assert_curated_gmail_repo(&curated_plugins_repo_path(tmp.path()));
    assert_eq!(
        read_curated_plugins_sha(tmp.path()).as_deref(),
        Some(sha.as_str())
    );
    assert!(!has_plugins_clone_dirs(tmp.path()));

    let first_sync_invocation_count = std::fs::read_to_string(&invocation_log)
        .expect("read first sync invocations")
        .lines()
        .count();
    let first_sync_invocations =
        std::fs::read_to_string(&invocation_log).expect("read first sync invocations");
    assert!(
        first_sync_invocations
            .lines()
            .any(|invocation| invocation.contains(" fetch --depth 1 --no-tags "))
    );
    assert!(
        !first_sync_invocations
            .lines()
            .any(|invocation| invocation.split_whitespace().any(|arg| arg == "clone"))
    );
    write_openai_curated_marketplace(&work_repo, &["gmail", "linear"]);
    run_git(&work_repo, &["add", "."]);
    run_git(
        &work_repo,
        &[
            "-c",
            "user.name=Codex Test",
            "-c",
            "user.email=codex@example.com",
            "commit",
            "-m",
            "update",
        ],
    );
    let branch_output = run_git(&work_repo, &["symbolic-ref", "--short", "HEAD"]);
    let branch = String::from_utf8_lossy(&branch_output.stdout)
        .trim()
        .to_string();
    let remote_repo = remote_repo.to_str().expect("utf8 remote repo");
    let push_ref = format!("HEAD:refs/heads/{branch}");
    run_git(&work_repo, &["push", remote_repo, &push_ref]);
    let updated_sha_output = run_git(&work_repo, &["rev-parse", "HEAD"]);
    let updated_sha = String::from_utf8_lossy(&updated_sha_output.stdout)
        .trim()
        .to_string();

    let synced_sha = sync_openai_plugins_repo_via_git(tmp.path(), &git_wrapper)
        .expect("incremental git sync should succeed");

    assert_eq!(synced_sha, updated_sha);
    assert!(
        curated_plugins_repo_path(tmp.path())
            .join("plugins/linear/.codex-plugin/plugin.json")
            .is_file()
    );
    assert_eq!(
        read_curated_plugins_sha(tmp.path()).as_deref(),
        Some(updated_sha.as_str())
    );
    assert!(
        !curated_plugins_repo_path(tmp.path())
            .join(".git/objects/info/alternates")
            .exists()
    );
    let invocation_log_contents =
        std::fs::read_to_string(&invocation_log).expect("read sync invocations");
    let incremental_sync_invocations = invocation_log_contents
        .lines()
        .skip(first_sync_invocation_count)
        .collect::<Vec<_>>();
    let curated_repo_path = curated_plugins_repo_path(tmp.path());
    assert!(incremental_sync_invocations.iter().any(|invocation| {
        invocation.contains(&format!(" -C {} fetch ", curated_repo_path.display()))
            && invocation.contains(" https://github.com/openai/plugins.git ")
            && invocation.contains(updated_sha.as_str())
            && invocation.ends_with(CURATED_PLUGINS_FETCH_REF)
    }));
    assert!(incremental_sync_invocations.iter().any(|invocation| {
        invocation.contains(" fetch --depth 1 --no-tags ")
            && invocation.contains(&format!(" {} ", curated_repo_path.display()))
            && invocation.ends_with(&format!(
                "{CURATED_PLUGINS_FETCH_REF}:{CURATED_PLUGINS_FETCH_REF}"
            ))
    }));
    assert!(
        incremental_sync_invocations
            .iter()
            .any(|invocation| invocation.ends_with(" init"))
    );
    assert!(
        !incremental_sync_invocations
            .iter()
            .any(|invocation| invocation.split_whitespace().any(|arg| arg == "clone"))
    );
    assert!(!incremental_sync_invocations.iter().any(|invocation| {
        invocation.contains(&format!(" -C {} reset ", curated_repo_path.display()))
            || invocation.contains(&format!(" -C {} clean ", curated_repo_path.display()))
    }));
    assert!(!has_plugins_clone_dirs(tmp.path()));

    let unchanged_sync_invocation_count = invocation_log_contents.lines().count();
    let synced_sha = sync_openai_plugins_repo_via_git(tmp.path(), &git_wrapper)
        .expect("unchanged git sync should succeed");

    assert_eq!(synced_sha, updated_sha);
    let invocation_log = std::fs::read_to_string(&invocation_log).expect("read sync invocations");
    let unchanged_sync_invocations = invocation_log
        .lines()
        .skip(unchanged_sync_invocation_count)
        .collect::<Vec<_>>();
    assert!(
        unchanged_sync_invocations
            .iter()
            .any(|invocation| invocation.contains(" ls-remote "))
    );
    assert!(
        !unchanged_sync_invocations
            .iter()
            .any(|invocation| invocation.contains(" fetch "))
    );
}

#[tokio::test]
async fn sync_openai_plugins_repo_falls_back_to_http_when_git_is_unavailable() {
    let tmp = tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let sha = "0123456789abcdef0123456789abcdef01234567";

    mount_github_repo_and_ref(&server, sha).await;
    mount_github_zipball(&server, sha, curated_repo_zipball_bytes(sha)).await;

    let synced_sha = run_sync_with_transport_overrides(
        tmp.path().to_path_buf(),
        "missing-git-for-test",
        server.uri(),
        "http://127.0.0.1:9/backend-api/plugins/export/curated",
    )
    .await
    .expect("fallback sync should succeed");

    let repo_path = curated_plugins_repo_path(tmp.path());
    assert_eq!(synced_sha, sha);
    assert_curated_gmail_repo(&repo_path);
    assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
}

#[test]
fn apple_git_without_developer_tools_is_unavailable() {
    assert_eq!(
        macos_git_binary_from_path(
            PathBuf::from("/usr/bin/git"),
            /*apple_developer_tools_available*/ false,
        ),
        None
    );
    assert_eq!(
        macos_git_binary_from_path(
            PathBuf::from("/usr/bin/git"),
            /*apple_developer_tools_available*/ true,
        ),
        Some(PathBuf::from("/usr/bin/git"))
    );
    assert_eq!(
        macos_git_binary_from_path(
            PathBuf::from("/opt/homebrew/bin/git"),
            /*apple_developer_tools_available*/ false,
        ),
        Some(PathBuf::from("/opt/homebrew/bin/git"))
    );
}

#[tokio::test]
async fn sync_openai_plugins_repo_uses_http_without_git_transport() {
    let tmp = tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let sha = "0123456789abcdef0123456789abcdef01234567";

    mount_github_repo_and_ref(&server, sha).await;
    mount_github_zipball(&server, sha, curated_repo_zipball_bytes(sha)).await;

    let synced_sha = run_sync_without_git(
        tmp.path().to_path_buf(),
        server.uri(),
        "http://127.0.0.1:9/backend-api/plugins/export/curated",
    )
    .await
    .expect("HTTP sync should succeed");

    assert_eq!(synced_sha, sha);
    assert_curated_gmail_repo(&curated_plugins_repo_path(tmp.path()));
}

#[cfg(unix)]
#[tokio::test]
async fn sync_openai_plugins_repo_falls_back_to_http_when_git_sync_fails() {
    let tmp = tempdir().expect("tempdir");
    let bin_dir = tempfile::Builder::new()
        .prefix("fake-git-fail-")
        .tempdir()
        .expect("tempdir");
    let git_path = bin_dir.path().join("git");
    let sha = "0123456789abcdef0123456789abcdef01234567";

    write_executable_script(
        &git_path,
        r#"#!/bin/sh
echo "simulated git failure" >&2
exit 1
"#,
    );

    let server = MockServer::start().await;
    mount_github_repo_and_ref(&server, sha).await;
    mount_github_zipball(&server, sha, curated_repo_zipball_bytes(sha)).await;

    let synced_sha = run_sync_with_transport_overrides(
        tmp.path().to_path_buf(),
        git_path.to_str().expect("utf8 path"),
        server.uri(),
        "http://127.0.0.1:9/backend-api/plugins/export/curated",
    )
    .await
    .expect("fallback sync should succeed");

    let repo_path = curated_plugins_repo_path(tmp.path());
    assert_eq!(synced_sha, sha);
    assert_curated_gmail_repo(&repo_path);
    assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
}

#[cfg(unix)]
#[test]
fn sync_openai_plugins_repo_via_git_cleans_up_staged_dir_on_fetch_failure() {
    let tmp = tempdir().expect("tempdir");
    let bin_dir = tempfile::Builder::new()
        .prefix("fake-git-partial-fail-")
        .tempdir()
        .expect("tempdir");
    let git_path = bin_dir.path().join("git");
    let sha = "0123456789abcdef0123456789abcdef01234567";

    write_executable_script(
        &git_path,
        &format!(
            r#"#!/bin/sh
if [ "$1" = "-c" ] && [ "$2" = "safe.bareRepository=explicit" ]; then shift 2; fi
if [ "$1" = "ls-remote" ]; then
  printf '%s\tHEAD\n' "{sha}"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "init" ]; then
  mkdir -p "$2/.git"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "fetch" ]; then
  echo "fatal: early EOF" >&2
  exit 128
fi
echo "unexpected git invocation: $@" >&2
exit 1
"#
        ),
    );

    let err =
        sync_openai_plugins_repo_via_git(tmp.path(), &git_path).expect_err("git sync should fail");

    assert!(err.contains("fatal: early EOF"));
    assert!(!has_plugins_clone_dirs(tmp.path()));
}

#[cfg(unix)]
#[test]
fn sync_openai_plugins_repo_via_git_preserves_existing_snapshot_on_validation_failure() {
    let tmp = tempdir().expect("tempdir");
    let repo_path = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&repo_path, &["gmail"]);
    std::fs::create_dir_all(repo_path.join(".git")).expect("create git dir");
    write_curated_plugin_sha(tmp.path());

    let bin_dir = tempfile::Builder::new()
        .prefix("fake-git-invalid-update-")
        .tempdir()
        .expect("tempdir");
    let git_path = bin_dir.path().join("git");
    let remote_sha = "fedcba9876543210fedcba9876543210fedcba98";

    write_executable_script(
        &git_path,
        &format!(
            r#"#!/bin/sh
if [ "$1" = "-c" ] && [ "$2" = "safe.bareRepository=explicit" ]; then shift 2; fi
if [ "$1" = "ls-remote" ]; then
  printf '%s\tHEAD\n' "{remote_sha}"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$2" = "{}" ] && [ "$3" = "rev-parse" ]; then
  printf '%s\n' "{TEST_CURATED_PLUGIN_SHA}"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$2" = "{}" ] && [ "$3" = "fetch" ]; then
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "init" ]; then
  mkdir -p "$2/.git"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "fetch" ]; then
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "reset" ]; then
  mkdir -p "$2/plugins/linear/.codex-plugin"
  printf '%s\n' '{{"name":"linear"}}' > "$2/plugins/linear/.codex-plugin/plugin.json"
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "clean" ]; then
  exit 0
fi
if [ "$1" = "-C" ] && [ "$3" = "rev-parse" ]; then
  printf '%s\n' "{remote_sha}"
  exit 0
fi
echo "unexpected git invocation: $@" >&2
exit 1
"#,
            repo_path.display(),
            repo_path.display(),
        ),
    );

    let err = sync_openai_plugins_repo_via_git(tmp.path(), &git_path)
        .expect_err("invalid staged checkout should fail");

    assert!(err.contains("curated plugins archive missing marketplace manifest"));
    assert_curated_gmail_repo(&repo_path);
    assert!(!repo_path.join("plugins/linear").exists());
    assert_eq!(
        read_curated_plugins_sha(tmp.path()).as_deref(),
        Some(TEST_CURATED_PLUGIN_SHA)
    );
    assert!(!has_plugins_clone_dirs(tmp.path()));
}

#[tokio::test]
async fn sync_openai_plugins_repo_via_http_cleans_up_staged_dir_on_extract_failure() {
    let tmp = tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let sha = "0123456789abcdef0123456789abcdef01234567";

    mount_github_repo_and_ref(&server, sha).await;
    mount_github_zipball(&server, sha, b"not a zip archive".to_vec()).await;

    let err = run_http_sync(tmp.path().to_path_buf(), server.uri())
        .await
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
    mount_github_repo_and_ref(&server, sha).await;

    run_sync_with_transport_overrides(
        tmp.path().to_path_buf(),
        "missing-git-for-test",
        server.uri(),
        "http://127.0.0.1:9/backend-api/plugins/export/curated",
    )
    .await
    .expect("sync should succeed");

    assert_eq!(read_curated_plugins_sha(tmp.path()).as_deref(), Some(sha));
    assert!(repo_path.join(".agents/plugins/marketplace.json").is_file());
}

#[tokio::test]
async fn sync_openai_plugins_repo_falls_back_to_export_archive_when_no_snapshot_exists() {
    let tmp = tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let export_sha = "1111111111111111111111111111111111111111";

    Mock::given(method("GET"))
        .and(path("/repos/openai/plugins"))
        .respond_with(ResponseTemplate::new(500).set_body_string("github repo lookup failed"))
        .mount(&server)
        .await;
    let export_api_url =
        mount_export_archive(&server, curated_repo_backup_archive_zip_bytes(export_sha)).await;

    let synced_sha = run_sync_with_transport_overrides(
        tmp.path().to_path_buf(),
        "missing-git-for-test",
        server.uri(),
        export_api_url,
    )
    .await
    .expect("export fallback sync should succeed");

    let repo_path = curated_plugins_repo_path(tmp.path());
    assert_eq!(synced_sha, export_sha);
    assert_curated_gmail_repo(&repo_path);
    assert_eq!(
        read_curated_plugins_sha(tmp.path()).as_deref(),
        Some(export_sha)
    );
}

#[tokio::test]
async fn sync_openai_plugins_repo_skips_export_archive_when_snapshot_exists() {
    let tmp = tempdir().expect("tempdir");
    let curated_root = curated_plugins_repo_path(tmp.path());
    write_openai_curated_marketplace(&curated_root, &["linear"]);
    write_curated_plugin_sha(tmp.path());

    let plugin_manifest_path = curated_root.join("plugins/linear/.codex-plugin/plugin.json");
    let original_manifest =
        std::fs::read_to_string(&plugin_manifest_path).expect("read existing plugin manifest");

    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/repos/openai/plugins"))
        .respond_with(ResponseTemplate::new(500).set_body_string("github repo lookup failed"))
        .mount(&server)
        .await;
    let export_api_url = mount_export_archive(
        &server,
        curated_repo_backup_archive_zip_bytes("2222222222222222222222222222222222222222"),
    )
    .await;

    let err = run_sync_with_transport_overrides(
        tmp.path().to_path_buf(),
        "missing-git-for-test",
        server.uri(),
        export_api_url,
    )
    .await
    .expect_err("existing snapshot should suppress export fallback");

    assert!(err.contains("export archive fallback skipped"));
    assert_eq!(
        std::fs::read_to_string(&plugin_manifest_path).expect("read plugin manifest after sync"),
        original_manifest
    );
    assert_eq!(
        read_curated_plugins_sha(tmp.path()).as_deref(),
        Some(TEST_CURATED_PLUGIN_SHA)
    );
}

#[test]
fn read_extracted_backup_archive_git_sha_reads_head_ref_from_extracted_repo() {
    let tmp = tempdir().expect("tempdir");
    let git_dir = tmp.path().join(".git/refs/heads");
    std::fs::create_dir_all(&git_dir).expect("create git ref dir");
    std::fs::write(tmp.path().join(".git/HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    std::fs::write(
        git_dir.join("main"),
        "3333333333333333333333333333333333333333\n",
    )
    .expect("write main ref");

    assert_eq!(
        read_extracted_backup_archive_git_sha(tmp.path())
            .expect("read extracted backup archive git sha"),
        Some("3333333333333333333333333333333333333333".to_string())
    );
}

#[test]
fn read_extracted_backup_archive_git_sha_rejects_non_refs_head_target() {
    let tmp = tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".git")).expect("create git dir");
    std::fs::write(tmp.path().join(".git/HEAD"), "ref: HEAD\n").expect("write HEAD");

    let err = read_extracted_backup_archive_git_sha(tmp.path())
        .expect_err("non-refs target should be rejected");

    assert!(err.contains("must stay under refs/"));
}

#[test]
fn read_extracted_backup_archive_git_sha_rejects_path_traversal_ref() {
    let tmp = tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".git")).expect("create git dir");
    std::fs::write(tmp.path().join(".git/HEAD"), "ref: refs/heads/../../evil\n")
        .expect("write HEAD");

    let err = read_extracted_backup_archive_git_sha(tmp.path())
        .expect_err("path traversal ref should be rejected");

    assert!(err.contains("invalid path components"));
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

fn curated_repo_backup_archive_zip_bytes(sha: &str) -> Vec<u8> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default();

    writer
        .start_file("plugins/.git/HEAD", options)
        .expect("start HEAD entry");
    writer
        .write_all(b"ref: refs/heads/main\n")
        .expect("write HEAD");
    writer
        .start_file("plugins/.git/refs/heads/main", options)
        .expect("start main ref entry");
    writer
        .write_all(format!("{sha}\n").as_bytes())
        .expect("write main ref");
    writer
        .start_file("plugins/.agents/plugins/marketplace.json", options)
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
        .start_file("plugins/plugins/gmail/.codex-plugin/plugin.json", options)
        .expect("start plugin manifest entry");
    writer
        .write_all(br#"{"name":"gmail"}"#)
        .expect("write plugin manifest");

    writer.finish().expect("finish zip writer").into_inner()
}
