use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use wiremock::MockServer;

use super::mcp_tool::TEST_SERVER_NAME;
use super::mcp_tool::TEST_TOOL_NAME;
use super::mcp_tool::start_mcp_server;

const API_CURATED_PLUGIN_NAME: &str = "api-plugin";
const REFRESH_PROBE_SERVER_NAME: &str = "refresh-probe";
const GITHUB_PLUGINS_GIT_URL: &str = "https://github.com/openai/plugins.git";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

struct CuratedMcpSyncFixture {
    mcp: TestAppServer,
    sync_barrier: PathBuf,
    mcp_server_handle: JoinHandle<()>,
    _fixture_root: TempDir,
    _responses_server: MockServer,
}

impl CuratedMcpSyncFixture {
    async fn set_up() -> Result<Self> {
        let responses_server = responses::start_mock_server().await;
        let (mcp_server_url, mcp_server_handle) = start_mcp_server().await?;
        let fixture_root = TempDir::new()?;
        let codex_home = fixture_root.path().join("codex-home");
        let curated_repo = fixture_root.path().join("curated-repo");
        let git_wrapper_dir = fixture_root.path().join("bin");
        let git_config = fixture_root.path().join("gitconfig");
        let sync_barrier = fixture_root.path().join("allow-curated-sync");
        std::fs::create_dir_all(&codex_home)?;
        std::fs::create_dir_all(&curated_repo)?;
        std::fs::create_dir_all(&git_wrapper_dir)?;

        write_curated_marketplace(&curated_repo, "marketplace.json", "openai-curated", &[])?;
        write_curated_marketplace(
            &curated_repo,
            "api_marketplace.json",
            "openai-api-curated",
            &[API_CURATED_PLUGIN_NAME],
        )?;
        write_plugin(
            &curated_repo.join("plugins").join(API_CURATED_PLUGIN_NAME),
            API_CURATED_PLUGIN_NAME,
            TEST_SERVER_NAME,
            &mcp_server_url,
        )?;

        let real_git =
            find_executable_on_path("git").context("find git for curated sync fixture")?;
        run_git(&real_git, &curated_repo, &["init", "-b", "main"])?;
        run_git(
            &real_git,
            &curated_repo,
            &["config", "user.email", "codex-tests@openai.com"],
        )?;
        run_git(
            &real_git,
            &curated_repo,
            &["config", "user.name", "Codex Tests"],
        )?;
        run_git(&real_git, &curated_repo, &["add", "."])?;
        run_git(
            &real_git,
            &curated_repo,
            &["commit", "-m", "test curated plugins"],
        )?;

        let curated_repo_url = format!("file://{}", curated_repo.display());
        let rewrite_key = format!("url.{curated_repo_url}.insteadOf");
        run_git(
            &real_git,
            &curated_repo,
            &[
                "config",
                "--file",
                git_config
                    .to_str()
                    .context("git config path should be UTF-8")?,
                &rewrite_key,
                GITHUB_PLUGINS_GIT_URL,
            ],
        )?;

        let git_wrapper = git_wrapper_dir.join("git");
        std::fs::write(
            &git_wrapper,
            r#"#!/bin/sh
if [ "$1" = "ls-remote" ]; then
  while [ ! -f "$CURATED_SYNC_BARRIER" ]; do
    sleep 0.01
  done
fi
exec "$REAL_GIT" "$@"
"#,
        )?;
        let mut wrapper_permissions = std::fs::metadata(&git_wrapper)?.permissions();
        wrapper_permissions.set_mode(0o755);
        std::fs::set_permissions(&git_wrapper, wrapper_permissions)?;

        MockResponsesConfig::new(&responses_server.uri())
            .enable_feature(Feature::Plugins)
            .with_root_config(&format!(
                r#"chatgpt_base_url = "{}/backend-api/""#,
                responses_server.uri()
            ))
            .with_extra_config(&format!(
                r#"[plugins."{API_CURATED_PLUGIN_NAME}@openai-api-curated"]
enabled = true

[mcp_servers.{REFRESH_PROBE_SERVER_NAME}]
url = "{mcp_server_url}/mcp""#
            ))
            .write(&codex_home)?;
        write_chatgpt_auth(
            &codex_home,
            ChatGptAuthFixture::new("chatgpt-token")
                .account_id("account-123")
                .chatgpt_user_id("user-123")
                .chatgpt_account_id("account-123"),
            AuthCredentialsStoreMode::File,
        )?;

        let inherited_path = std::env::var_os("PATH").context("PATH should be set")?;
        let child_path = std::env::join_paths(
            std::iter::once(git_wrapper_dir).chain(std::env::split_paths(&inherited_path)),
        )?;
        let child_path = child_path.to_string_lossy();
        let real_git = real_git.to_string_lossy();
        let git_config = git_config.to_string_lossy();
        let sync_barrier_env = sync_barrier.to_string_lossy();
        let mcp = TestAppServer::builder()
            .with_codex_home(&codex_home)
            .with_env_overrides(&[
                ("PATH", Some(child_path.as_ref())),
                ("REAL_GIT", Some(real_git.as_ref())),
                ("GIT_CONFIG_GLOBAL", Some(git_config.as_ref())),
                ("CURATED_SYNC_BARRIER", Some(sync_barrier_env.as_ref())),
            ])
            .build_initialized_with_timeout(DEFAULT_TIMEOUT)
            .await?;

        Ok(Self {
            mcp,
            sync_barrier,
            mcp_server_handle,
            _fixture_root: fixture_root,
            _responses_server: responses_server,
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn existing_thread_loads_api_curated_mcp_after_auth_switch_sync() -> Result<()> {
    let mut fixture = CuratedMcpSyncFixture::set_up().await?;
    let thread_id = fixture
        .mcp
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    // Establish the existing thread's MCP runtime while curated Git sync is still blocked.
    wait_for_mcp_ready(&mut fixture.mcp, REFRESH_PROBE_SERVER_NAME).await?;

    let request_id = fixture
        .mcp
        .send_login_account_api_key_request("sk-test-key")
        .await?;
    let response: LoginAccountResponse =
        timeout(DEFAULT_TIMEOUT, fixture.mcp.read_response(request_id)).await??;
    assert_eq!(response, LoginAccountResponse::ApiKey {});
    timeout(
        DEFAULT_TIMEOUT,
        fixture
            .mcp
            .read_stream_until_notification_message("account/updated"),
    )
    .await??;

    // The account-change refresh has completed without the API-curated bundle on disk.
    wait_for_mcp_ready(&mut fixture.mcp, REFRESH_PROBE_SERVER_NAME).await?;

    // Let sync materialize the bundle; its completion callback must refresh this same thread.
    std::fs::write(&fixture.sync_barrier, "continue")?;
    wait_for_mcp_ready(&mut fixture.mcp, TEST_SERVER_NAME).await?;

    let response: McpServerToolCallResponse = fixture
        .mcp
        .request(|request_id| ClientRequest::McpServerToolCall {
            request_id,
            params: McpServerToolCallParams {
                thread_id: thread_id.clone(),
                server: TEST_SERVER_NAME.to_string(),
                tool: TEST_TOOL_NAME.to_string(),
                arguments: Some(json!({"message": "available after sync"})),
                meta: None,
            },
        })
        .await?;
    assert_eq!(
        response.structured_content,
        Some(json!({
            "echoed": "available after sync",
            "threadId": thread_id,
        }))
    );

    fixture.mcp_server_handle.abort();
    let _ = fixture.mcp_server_handle.await;
    Ok(())
}

async fn wait_for_mcp_ready(mcp: &mut TestAppServer, server_name: &str) -> Result<()> {
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_matching_notification(
            "mcpServer/startupStatus/updated ready",
            |notification| {
                notification.method == "mcpServer/startupStatus/updated"
                    && notification
                        .params
                        .as_ref()
                        .and_then(|params| params.get("name"))
                        .and_then(serde_json::Value::as_str)
                        == Some(server_name)
                    && notification
                        .params
                        .as_ref()
                        .and_then(|params| params.get("status"))
                        .and_then(serde_json::Value::as_str)
                        == Some("ready")
            },
        ),
    )
    .await??;
    Ok(())
}

fn write_curated_marketplace(
    repo_root: &Path,
    manifest_name: &str,
    marketplace_name: &str,
    plugin_names: &[&str],
) -> Result<()> {
    let manifest_root = repo_root.join(".agents/plugins");
    std::fs::create_dir_all(&manifest_root)?;
    let plugins = plugin_names
        .iter()
        .map(|plugin_name| {
            json!({
                "name": plugin_name,
                "source": {
                    "source": "local",
                    "path": format!("./plugins/{plugin_name}"),
                },
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        manifest_root.join(manifest_name),
        serde_json::to_vec_pretty(&json!({
            "name": marketplace_name,
            "plugins": plugins,
        }))?,
    )?;
    Ok(())
}

fn write_plugin(
    root: &Path,
    plugin_name: &str,
    server_name: &str,
    mcp_server_url: &str,
) -> Result<()> {
    std::fs::create_dir_all(root.join(".codex-plugin"))?;
    std::fs::write(
        root.join(".codex-plugin/plugin.json"),
        serde_json::to_vec_pretty(&json!({"name": plugin_name}))?,
    )?;
    std::fs::write(
        root.join(".mcp.json"),
        serde_json::to_vec_pretty(&json!({
            "mcpServers": {
                server_name: {
                    "type": "http",
                    "url": format!("{mcp_server_url}/mcp"),
                },
            },
        }))?,
    )?;
    Ok(())
}

fn find_executable_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn run_git(git: &Path, cwd: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new(git).current_dir(cwd).args(args).output()?;
    ensure!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
