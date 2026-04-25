//! Shared helpers for Streamable HTTP RMCP integration tests.
//!
//! This support module starts the test HTTP server, launches a real
//! `exec-server` when remote coverage is needed, and provides small helpers for
//! creating RMCP clients and asserting round-trip behavior.

// This support module is included by multiple integration-test crates. Each
// crate uses a different subset of the helpers, so dead-code warnings would
// otherwise depend on which test file compiled the module.
#![allow(dead_code)]

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context as _;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_exec_server::ExecServerClient;
use codex_exec_server::RemoteExecServerConnectArgs;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::RmcpClient;
use codex_utils_cargo_bin::CargoBinError;
use futures::FutureExt as _;
use pretty_assertions::assert_eq;
use rmcp::model::CallToolResult;
use rmcp::model::ClientCapabilities;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::sleep;

const SESSION_POST_FAILURE_CONTROL_PATH: &str = "/test/control/session-post-failure";

fn streamable_http_server_bin() -> Result<PathBuf, CargoBinError> {
    codex_utils_cargo_bin::cargo_bin("test_streamable_http_server")
}

fn init_params() -> InitializeRequestParams {
    InitializeRequestParams {
        meta: None,
        capabilities: ClientCapabilities {
            experimental: None,
            extensions: None,
            roots: None,
            sampling: None,
            elicitation: Some(ElicitationCapability {
                form: Some(FormElicitationCapability {
                    schema_validation: None,
                }),
                url: None,
            }),
            tasks: None,
        },
        client_info: Implementation {
            name: "codex-test".into(),
            version: "0.0.0-test".into(),
            title: Some("Codex rmcp recovery test".into()),
            description: None,
            icons: None,
            website_url: None,
        },
        protocol_version: ProtocolVersion::V_2025_06_18,
    }
}

pub(crate) fn expected_echo_result(message: &str) -> CallToolResult {
    CallToolResult {
        content: Vec::new(),
        structured_content: Some(json!({
            "echo": format!("ECHOING: {message}"),
            "env": null,
        })),
        is_error: Some(false),
        meta: None,
    }
}

pub(crate) async fn create_client(base_url: &str) -> anyhow::Result<RmcpClient> {
    let client = RmcpClient::new_streamable_http_client(
        "test-streamable-http",
        &format!("{base_url}/mcp"),
        Some("test-bearer".to_string()),
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;

    client
        .initialize(
            init_params(),
            Some(Duration::from_secs(5)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({})),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;

    Ok(client)
}

/// Creates a Streamable HTTP RMCP client that sends traffic through the remote
/// runtime HTTP API.
pub(crate) async fn create_remote_client(
    base_url: &str,
    http_client: ExecServerClient,
) -> anyhow::Result<RmcpClient> {
    let client = RmcpClient::new_streamable_http_client(
        "test-streamable-http-remote",
        &format!("{base_url}/mcp"),
        Some("test-bearer".to_string()),
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        Arc::new(http_client),
        /*auth_provider*/ None,
    )
    .await?;

    client
        .initialize(
            init_params(),
            Some(Duration::from_secs(5)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(json!({})),
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;

    Ok(client)
}

pub(crate) async fn call_echo_tool(
    client: &RmcpClient,
    message: &str,
) -> anyhow::Result<CallToolResult> {
    client
        .call_tool(
            "echo".to_string(),
            Some(json!({ "message": message })),
            /*meta*/ None,
            Some(Duration::from_secs(5)),
        )
        .await
}

pub(crate) async fn arm_session_post_failure(
    base_url: &str,
    status: u16,
    remaining: usize,
) -> anyhow::Result<()> {
    let response = reqwest::Client::new()
        .post(format!("{base_url}{SESSION_POST_FAILURE_CONTROL_PATH}"))
        .json(&json!({
            "status": status,
            "remaining": remaining,
        }))
        .send()
        .await?;

    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    Ok(())
}

pub(crate) async fn spawn_streamable_http_server() -> anyhow::Result<(Child, String)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);

    let bind_addr = format!("127.0.0.1:{port}");
    let base_url = format!("http://{bind_addr}");
    let mut child = Command::new(streamable_http_server_bin()?)
        .kill_on_drop(true)
        .env("MCP_STREAMABLE_HTTP_BIND_ADDR", &bind_addr)
        .spawn()?;

    wait_for_streamable_http_server(&mut child, &bind_addr, Duration::from_secs(5)).await?;
    Ok((child, base_url))
}

/// Owns the exec-server process used by the remote-client integration test.
pub(crate) struct ExecServerProcess {
    _codex_home: TempDir,
    child: Child,
    pub(crate) client: ExecServerClient,
}

impl Drop for ExecServerProcess {
    /// Stops the local exec-server process best-effort when the test exits.
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// Starts a local exec-server and connects an initialized `ExecServerClient`.
pub(crate) async fn spawn_exec_server() -> anyhow::Result<ExecServerProcess> {
    let codex_home = TempDir::new()?;
    let mut child = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["exec-server", "--listen", "ws://127.0.0.1:0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .env("CODEX_HOME", codex_home.path())
        .spawn()?;

    let websocket_url = read_exec_server_listen_url(&mut child).await?;
    let client = ExecServerClient::connect_websocket(RemoteExecServerConnectArgs::new(
        websocket_url,
        "rmcp-client-remote-http-test".to_string(),
    ))
    .await?;

    Ok(ExecServerProcess {
        _codex_home: codex_home,
        child,
        client,
    })
}

/// Reads the websocket URL printed by `codex exec-server --listen`.
async fn read_exec_server_listen_url(child: &mut Child) -> anyhow::Result<String> {
    let stdout = child
        .stdout
        .take()
        .context("failed to capture exec-server stdout")?;
    let mut lines = BufReader::new(stdout).lines();
    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            anyhow::bail!("timed out waiting for exec-server listen URL");
        }

        let line = tokio::time::timeout(remaining, lines.next_line())
            .await
            .context("timed out waiting for exec-server stdout")??
            .context("exec-server stdout closed before emitting listen URL")?;
        let listen_url = line.trim();
        if listen_url.starts_with("ws://") {
            return Ok(listen_url.to_string());
        }
    }
}

async fn wait_for_streamable_http_server(
    server_child: &mut Child,
    address: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(status) = server_child.try_wait()? {
            return Err(anyhow::anyhow!(
                "streamable HTTP server exited early with status {status}"
            ));
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(anyhow::anyhow!(
                "timed out waiting for streamable HTTP server at {address}: deadline reached"
            ));
        }

        match tokio::time::timeout(remaining, TcpStream::connect(address)).await {
            Ok(Ok(_)) => return Ok(()),
            Ok(Err(error)) => {
                if Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for streamable HTTP server at {address}: {error}"
                    ));
                }
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "timed out waiting for streamable HTTP server at {address}: connect call timed out"
                ));
            }
        }

        sleep(Duration::from_millis(50)).await;
    }
}
