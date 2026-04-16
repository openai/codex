use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::McpServerStartupState;
use codex_app_server_protocol::McpServerStatusUpdatedNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SandboxMode;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config::set_project_trust_level;
use codex_exec_server::ExecServerRuntimePaths;
use codex_git_utils::resolve_root_git_project_for_trust;
use codex_login::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::analytics::assert_basic_thread_initialized_event;
use super::analytics::mount_analytics_capture;
use super::analytics::thread_initialized_event;
use super::analytics::wait_for_analytics_payload;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";
const CODEX_EXEC_SERVER_CWD_ENV_VAR: &str = "CODEX_EXEC_SERVER_CWD";
const EXEC_SERVER_CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(25);

#[tokio::test]
async fn thread_start_creates_thread_and_emits_started() -> Result<()> {
    // Provide a mock server and config so model wiring is valid.
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    // Start server and initialize.
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Start a v2 thread with an explicit model override.
    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1".to_string()),
            ..Default::default()
        })
        .await?;

    // Expect a proper JSON-RPC response with a thread id.
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let resp_result = resp.result.clone();
    let ThreadStartResponse {
        thread,
        model_provider,
        ..
    } = to_response::<ThreadStartResponse>(resp)?;
    assert!(!thread.id.is_empty(), "thread id should not be empty");
    assert!(
        thread.preview.is_empty(),
        "new threads should start with an empty preview"
    );
    assert_eq!(model_provider, "mock_provider");
    assert!(
        thread.created_at > 0,
        "created_at should be a positive UNIX timestamp"
    );
    assert!(
        !thread.ephemeral,
        "new persistent threads should not be ephemeral"
    );
    assert_eq!(thread.status, ThreadStatus::Idle);
    let thread_path = thread.path.clone().expect("thread path should be present");
    assert!(thread_path.is_absolute(), "thread path should be absolute");
    assert!(
        !thread_path.exists(),
        "fresh thread rollout should not be materialized until first user message"
    );

    // Wire contract: thread title field is `name`, serialized as null when unset.
    let thread_json = resp_result
        .get("thread")
        .and_then(Value::as_object)
        .expect("thread/start result.thread must be an object");
    assert_eq!(
        thread_json.get("name"),
        Some(&Value::Null),
        "new threads should serialize `name: null`"
    );
    assert_eq!(
        thread_json.get("ephemeral").and_then(Value::as_bool),
        Some(false),
        "new persistent threads should serialize `ephemeral: false`"
    );
    assert_eq!(thread.name, None);

    // A corresponding thread/started notification should arrive.
    let deadline = tokio::time::Instant::now() + DEFAULT_READ_TIMEOUT;
    let notif = loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = timeout(remaining, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notif) = message else {
            continue;
        };
        if notif.method == "thread/status/changed" {
            let status_changed: ThreadStatusChangedNotification =
                serde_json::from_value(notif.params.expect("params must be present"))?;
            if status_changed.thread_id == thread.id {
                anyhow::bail!(
                    "thread/start should introduce the thread without a preceding thread/status/changed"
                );
            }
            continue;
        }
        if notif.method == "thread/started" {
            break notif;
        }
    };
    let started_params = notif.params.clone().expect("params must be present");
    let started_thread_json = started_params
        .get("thread")
        .and_then(Value::as_object)
        .expect("thread/started params.thread must be an object");
    assert_eq!(
        started_thread_json.get("name"),
        Some(&Value::Null),
        "thread/started should serialize `name: null` for new threads"
    );
    assert_eq!(
        started_thread_json
            .get("ephemeral")
            .and_then(Value::as_bool),
        Some(false),
        "thread/started should serialize `ephemeral: false` for new persistent threads"
    );
    let started: ThreadStartedNotification =
        serde_json::from_value(notif.params.expect("params must be present"))?;
    assert_eq!(started.thread, thread);

    Ok(())
}

#[tokio::test]
async fn thread_start_accepts_explicit_local_environment_when_default_is_remote() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(CODEX_EXEC_SERVER_URL_ENV_VAR, Some("ws://127.0.0.1:1"))],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            environment_ids: Some(vec!["local".to_string()]),
            ..Default::default()
        })
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(resp)?;

    assert!(!thread.id.is_empty(), "thread id should not be empty");
    Ok(())
}

#[tokio::test]
async fn thread_start_uses_remote_default_cwd_when_request_omits_cwd() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;
    let exec_server = TestExecServer::spawn().await?;

    let remote_default_cwd = TempDir::new()?;
    let remote_default_cwd = remote_default_cwd.path().to_string_lossy().into_owned();

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            (
                CODEX_EXEC_SERVER_URL_ENV_VAR,
                Some(exec_server.websocket_url.as_str()),
            ),
            (
                CODEX_EXEC_SERVER_CWD_ENV_VAR,
                Some(remote_default_cwd.as_str()),
            ),
        ],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(resp)?;

    assert_eq!(thread.cwd.as_path(), Path::new(&remote_default_cwd));
    Ok(())
}

#[tokio::test]
async fn thread_start_explicit_cwd_overrides_remote_default_cwd() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;
    let exec_server = TestExecServer::spawn().await?;

    let remote_default_cwd = TempDir::new()?;
    let remote_default_cwd = remote_default_cwd.path().to_string_lossy().into_owned();
    let requested_cwd = TempDir::new()?;
    let requested_cwd = requested_cwd.path().to_string_lossy().into_owned();

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            (
                CODEX_EXEC_SERVER_URL_ENV_VAR,
                Some(exec_server.websocket_url.as_str()),
            ),
            (
                CODEX_EXEC_SERVER_CWD_ENV_VAR,
                Some(remote_default_cwd.as_str()),
            ),
        ],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(requested_cwd.clone()),
            ..Default::default()
        })
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(resp)?;

    assert_eq!(thread.cwd.as_path(), Path::new(&requested_cwd));
    Ok(())
}

#[tokio::test]
async fn thread_start_rejects_explicit_remote_environment_when_not_configured() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            environment_ids: Some(vec!["remote".to_string()]),
            ..Default::default()
        })
        .await?;

    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(req_id)),
    )
    .await??;

    assert!(
        err.error
            .message
            .contains("remote environment is not configured"),
        "unexpected error message: {}",
        err.error.message
    );
    Ok(())
}

#[tokio::test]
async fn thread_start_rejects_explicit_environment_when_disabled() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[(CODEX_EXEC_SERVER_URL_ENV_VAR, Some("none"))],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            environment_ids: Some(vec!["local".to_string()]),
            ..Default::default()
        })
        .await?;

    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(req_id)),
    )
    .await??;

    assert!(
        err.error
            .message
            .contains("environments are disabled for this session"),
        "unexpected error message: {}",
        err.error.message
    );
    Ok(())
}

#[tokio::test]
async fn thread_start_response_includes_loaded_instruction_sources() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;
    let global_agents_path = codex_home.path().join("AGENTS.md");
    std::fs::write(&global_agents_path, "global instructions")?;
    let workspace = TempDir::new()?;
    let project_agents_path = workspace.path().join("AGENTS.md");
    std::fs::write(&project_agents_path, "project instructions")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().display().to_string()),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse {
        instruction_sources,
        ..
    } = to_response::<ThreadStartResponse>(response)?;

    let instruction_sources = instruction_sources
        .into_iter()
        .map(normalize_path_for_comparison)
        .collect::<Vec<_>>();
    let expected_instruction_sources = vec![
        std::fs::canonicalize(global_agents_path)?,
        std::fs::canonicalize(project_agents_path)?,
    ]
    .into_iter()
    .map(normalize_path_for_comparison)
    .collect::<Vec<_>>();

    assert_eq!(instruction_sources, expected_instruction_sources);

    Ok(())
}

#[cfg(windows)]
fn normalize_path_for_comparison(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let path = path.display().to_string();
    PathBuf::from(path.strip_prefix(r"\\?\").unwrap_or(&path))
}

#[cfg(not(windows))]
fn normalize_path_for_comparison(path: impl AsRef<Path>) -> PathBuf {
    path.as_ref().to_path_buf()
}

struct TestExecServer {
    websocket_url: String,
    task: JoinHandle<()>,
}

impl TestExecServer {
    async fn spawn() -> Result<Self> {
        let bind_addr = TcpListener::bind("127.0.0.1:0")?.local_addr()?;
        let websocket_url = format!("ws://{bind_addr}");
        let runtime_paths = ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?;
        let listen_url = websocket_url.clone();
        let task = tokio::spawn(async move {
            codex_exec_server::run_main(&listen_url, runtime_paths)
                .await
                .expect("test exec-server should run");
        });
        wait_for_exec_server(websocket_url.as_str()).await?;
        Ok(Self {
            websocket_url,
            task,
        })
    }
}

impl Drop for TestExecServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn wait_for_exec_server(websocket_url: &str) -> Result<()> {
    let deadline = Instant::now() + DEFAULT_READ_TIMEOUT;
    loop {
        match connect_async(websocket_url).await {
            Ok((mut websocket, _)) => {
                websocket.close(None).await?;
                return Ok(());
            }
            Err(err) if Instant::now() < deadline => {
                let is_connection_refused = matches!(
                    err,
                    tokio_tungstenite::tungstenite::Error::Io(ref io_err)
                        if io_err.kind() == std::io::ErrorKind::ConnectionRefused
                );
                if is_connection_refused {
                    sleep(EXEC_SERVER_CONNECT_RETRY_INTERVAL).await;
                    continue;
                }
                return Err(err.into());
            }
            Err(err) => return Err(err.into()),
        }
    }
}

#[tokio::test]
async fn thread_start_tracks_thread_initialized_analytics() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &server.uri(),
        &server.uri(),
        /*general_analytics_enabled*/ true,
    )?;
    mount_analytics_capture(&server, codex_home.path()).await?;

    let mut mcp = McpProcess::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(resp)?;

    let payload = wait_for_analytics_payload(&server, DEFAULT_READ_TIMEOUT).await?;
    assert_eq!(payload["events"].as_array().expect("events array").len(), 1);
    let event = thread_initialized_event(&payload)?;
    assert_basic_thread_initialized_event(event, &thread.id, "mock-model", "new");
    Ok(())
}

#[tokio::test]
async fn thread_start_does_not_track_thread_initialized_analytics_without_feature() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &server.uri(),
        &server.uri(),
        /*general_analytics_enabled*/ false,
    )?;
    mount_analytics_capture(&server, codex_home.path()).await?;

    let mut mcp = McpProcess::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let _ = to_response::<ThreadStartResponse>(resp)?;

    assert_no_thread_initialized_analytics(&server, Duration::from_millis(250)).await?;
    Ok(())
}

async fn assert_no_thread_initialized_analytics(
    server: &MockServer,
    wait_duration: Duration,
) -> Result<()> {
    tokio::time::sleep(wait_duration).await;
    let requests = server.received_requests().await.unwrap_or_default();
    for request in requests.iter().filter(|request| {
        request.method == "POST" && request.url.path() == "/codex/analytics-events/events"
    }) {
        let payload: Value = serde_json::from_slice(&request.body)?;
        assert!(
            thread_initialized_event(&payload).is_err(),
            "thread analytics should be gated off when general_analytics is disabled; payload={payload}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn thread_start_respects_project_config_from_cwd() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let workspace = TempDir::new()?;
    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        r#"
model_reasoning_effort = "high"
"#,
    )?;
    set_project_trust_level(codex_home.path(), workspace.path(), TrustLevel::Trusted)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse {
        reasoning_effort, ..
    } = to_response::<ThreadStartResponse>(resp)?;

    assert_eq!(reasoning_effort, Some(ReasoningEffort::High));
    Ok(())
}

#[tokio::test]
async fn thread_start_accepts_flex_service_tier() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            service_tier: Some(Some(ServiceTier::Flex)),
            ..Default::default()
        })
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse { service_tier, .. } = to_response::<ThreadStartResponse>(resp)?;

    assert_eq!(service_tier, Some(ServiceTier::Flex));
    Ok(())
}

#[tokio::test]
async fn thread_start_accepts_metrics_service_name() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            service_name: Some("my_app_server_client".to_string()),
            ..Default::default()
        })
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(resp)?;
    assert!(!thread.id.is_empty(), "thread id should not be empty");

    Ok(())
}

#[tokio::test]
async fn thread_start_ephemeral_remains_pathless() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.1".to_string()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;

    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let resp_result = resp.result.clone();
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(resp)?;
    assert!(
        thread.ephemeral,
        "ephemeral threads should be marked explicitly"
    );
    assert_eq!(
        thread.path, None,
        "ephemeral threads should not expose a path"
    );
    let thread_json = resp_result
        .get("thread")
        .and_then(Value::as_object)
        .expect("thread/start result.thread must be an object");
    assert_eq!(
        thread_json.get("ephemeral").and_then(Value::as_bool),
        Some(true),
        "ephemeral threads should serialize `ephemeral: true`"
    );

    Ok(())
}

#[tokio::test]
async fn thread_start_fails_when_required_mcp_server_fails_to_initialize() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_with_required_broken_mcp(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;

    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(req_id)),
    )
    .await??;

    assert!(
        err.error
            .message
            .contains("required MCP servers failed to initialize"),
        "unexpected error message: {}",
        err.error.message
    );
    assert!(
        err.error.message.contains("required_broken"),
        "unexpected error message: {}",
        err.error.message
    );

    Ok(())
}

#[tokio::test]
async fn thread_start_emits_mcp_server_status_updated_notifications() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_with_optional_broken_mcp(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;

    let _: ThreadStartResponse = to_response(
        timeout(
            DEFAULT_READ_TIMEOUT,
            mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
        )
        .await??,
    )?;

    let starting = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_matching_notification(
            "mcpServer/startupStatus/updated starting",
            |notification| {
                notification.method == "mcpServer/startupStatus/updated"
                    && notification
                        .params
                        .as_ref()
                        .and_then(|params| params.get("name"))
                        .and_then(Value::as_str)
                        == Some("optional_broken")
                    && notification
                        .params
                        .as_ref()
                        .and_then(|params| params.get("status"))
                        .and_then(Value::as_str)
                        == Some("starting")
            },
        ),
    )
    .await??;
    let starting: ServerNotification = starting.try_into()?;
    let ServerNotification::McpServerStatusUpdated(starting) = starting else {
        anyhow::bail!("unexpected notification variant");
    };
    assert_eq!(
        starting,
        McpServerStatusUpdatedNotification {
            name: "optional_broken".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }
    );

    let failed = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_matching_notification(
            "mcpServer/startupStatus/updated failed",
            |notification| {
                notification.method == "mcpServer/startupStatus/updated"
                    && notification
                        .params
                        .as_ref()
                        .and_then(|params| params.get("name"))
                        .and_then(Value::as_str)
                        == Some("optional_broken")
                    && notification
                        .params
                        .as_ref()
                        .and_then(|params| params.get("status"))
                        .and_then(Value::as_str)
                        == Some("failed")
            },
        ),
    )
    .await??;
    let failed: ServerNotification = failed.try_into()?;
    let ServerNotification::McpServerStatusUpdated(failed) = failed else {
        anyhow::bail!("unexpected notification variant");
    };
    assert_eq!(failed.name, "optional_broken");
    assert_eq!(failed.status, McpServerStartupState::Failed);
    assert!(
        failed
            .error
            .as_deref()
            .is_some_and(|error| error.contains("MCP client for `optional_broken` failed to start")),
        "unexpected MCP startup error: {:?}",
        failed.error
    );

    Ok(())
}

#[tokio::test]
async fn thread_start_surfaces_cloud_requirements_load_errors() -> Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/config/requirements"))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header("content-type", "text/html")
                .set_body_string("<html>nope</html>"),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": { "code": "refresh_token_invalidated" }
        })))
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let model_server = create_mock_responses_server_repeating_assistant("Done").await;
    let chatgpt_base_url = format!("{}/backend-api", server.uri());
    create_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &model_server.uri(),
        &chatgpt_base_url,
        /*general_analytics_enabled*/ false,
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .refresh_token("stale-refresh-token")
            .plan_type("business")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123")
            .account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let refresh_token_url = format!("{}/oauth/token", server.uri());
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("OPENAI_API_KEY", None),
            (
                REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR,
                Some(refresh_token_url.as_str()),
            ),
        ],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;

    let err: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(req_id)),
    )
    .await??;

    assert!(
        err.error.message.contains("failed to load configuration"),
        "unexpected error message: {}",
        err.error.message
    );
    assert_eq!(
        err.error.data,
        Some(json!({
            "reason": "cloudRequirements",
            "errorCode": "Auth",
            "action": "relogin",
            "statusCode": 401,
            "detail": "Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again.",
        }))
    );

    Ok(())
}

#[tokio::test]
async fn thread_start_with_elevated_sandbox_trusts_project_and_followup_loads_project_config()
-> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let workspace = TempDir::new()?;
    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        r#"
model_reasoning_effort = "high"
"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let first_request = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().display().to_string()),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_request)),
    )
    .await??;

    let second_request = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().display().to_string()),
            ..Default::default()
        })
        .await?;
    let second_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_request)),
    )
    .await??;
    let ThreadStartResponse {
        approval_policy,
        reasoning_effort,
        ..
    } = to_response::<ThreadStartResponse>(second_response)?;

    assert_eq!(approval_policy, AskForApproval::OnRequest);
    assert_eq!(reasoning_effort, Some(ReasoningEffort::High));

    let config_toml = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let trusted_root = resolve_root_git_project_for_trust(workspace.path())
        .await
        .unwrap_or_else(|| workspace.path().to_path_buf());
    assert!(config_toml.contains(&persisted_trust_path(&trusted_root)));
    assert!(config_toml.contains("trust_level = \"trusted\""));

    Ok(())
}

#[tokio::test]
async fn thread_start_with_nested_git_cwd_trusts_repo_root() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let repo_root = TempDir::new()?;
    std::fs::create_dir(repo_root.path().join(".git"))?;
    let nested = repo_root.path().join("nested/project");
    std::fs::create_dir_all(&nested)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(nested.display().to_string()),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let config_toml = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let trusted_root = resolve_root_git_project_for_trust(&nested)
        .await
        .expect("git root should resolve");
    assert!(config_toml.contains(&persisted_trust_path(&trusted_root)));
    assert!(!config_toml.contains(&persisted_trust_path(&nested)));

    Ok(())
}

#[tokio::test]
async fn thread_start_with_read_only_sandbox_does_not_persist_project_trust() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let workspace = TempDir::new()?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().display().to_string()),
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let config_toml = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(!config_toml.contains("trust_level = \"trusted\""));
    assert!(!config_toml.contains(&workspace.path().display().to_string()));

    Ok(())
}

#[tokio::test]
async fn thread_start_skips_trust_write_when_project_is_already_trusted() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;

    let codex_home = TempDir::new()?;
    create_config_toml_without_approval_policy(codex_home.path(), &server.uri())?;

    let workspace = TempDir::new()?;
    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        r#"
model_reasoning_effort = "high"
"#,
    )?;
    set_project_trust_level(codex_home.path(), workspace.path(), TrustLevel::Trusted)?;
    let config_before = std::fs::read_to_string(codex_home.path().join("config.toml"))?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.path().display().to_string()),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse {
        approval_policy,
        reasoning_effort,
        ..
    } = to_response::<ThreadStartResponse>(response)?;

    assert_eq!(approval_policy, AskForApproval::OnRequest);
    assert_eq!(reasoning_effort, Some(ReasoningEffort::High));

    let config_after = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert_eq!(config_after, config_before);

    Ok(())
}

fn create_config_toml_without_approval_policy(
    codex_home: &Path,
    server_uri: &str,
) -> std::io::Result<()> {
    create_config_toml_with_optional_approval_policy(
        codex_home, server_uri, /*approval_policy*/ None,
    )
}

fn persisted_trust_path(project_path: &Path) -> String {
    let project_path =
        std::fs::canonicalize(project_path).unwrap_or_else(|_| project_path.to_path_buf());
    let project_path = project_path.display().to_string();

    if let Some(project_path) = project_path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{project_path}");
    }

    project_path
        .strip_prefix(r"\\?\")
        .unwrap_or(&project_path)
        .to_string()
}

fn create_config_toml_with_optional_approval_policy(
    codex_home: &Path,
    server_uri: &str,
    approval_policy: Option<&str>,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    let approval_policy = approval_policy
        .map(|policy| format!("approval_policy = \"{policy}\"\n"))
        .unwrap_or_default();
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
{approval_policy}sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}

fn create_config_toml_with_chatgpt_base_url(
    codex_home: &Path,
    server_uri: &str,
    chatgpt_base_url: &str,
    general_analytics_enabled: bool,
) -> std::io::Result<()> {
    let general_analytics_toml = if general_analytics_enabled {
        "\ngeneral_analytics = true".to_string()
    } else {
        "\ngeneral_analytics = false".to_string()
    };
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
chatgpt_base_url = "{chatgpt_base_url}"

model_provider = "mock_provider"

[features]
{general_analytics_toml}

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}

fn create_config_toml_with_required_broken_mcp(
    codex_home: &Path,
    server_uri: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[mcp_servers.required_broken]
{required_broken_transport}
required = true
"#,
            required_broken_transport = broken_mcp_transport_toml()
        ),
    )
}

fn create_config_toml_with_optional_broken_mcp(
    codex_home: &Path,
    server_uri: &str,
) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[mcp_servers.optional_broken]
{optional_broken_transport}
"#,
            optional_broken_transport = broken_mcp_transport_toml()
        ),
    )
}

#[cfg(target_os = "windows")]
fn broken_mcp_transport_toml() -> &'static str {
    r#"command = "cmd"
args = ["/C", "exit 1"]"#
}

#[cfg(not(target_os = "windows"))]
fn broken_mcp_transport_toml() -> &'static str {
    r#"command = "/bin/sh"
args = ["-c", "exit 1"]"#
}
