//! Core integration coverage for MCP Streamable HTTP servers.
//!
//! These tests live in the core RMCP suite and verify that Codex can discover,
//! connect to, and call Streamable HTTP MCP servers in both local and
//! remote-aware placements.

use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::Path;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use anyhow::ensure;
use codex_config::types::McpServerTransportConfig;
use codex_exec_server::Environment;
use codex_exec_server::HttpRequestParams;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use codex_utils_cargo_bin::cargo_bin;
use core_test_support::responses;
use core_test_support::responses::mount_sse_once;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use reqwest::Client;
use reqwest::StatusCode;
use serde_json::Value;
use serde_json::json;
use serial_test::serial;
use tempfile::tempdir;
use tokio::process::Child;
use tokio::process::Command;
use tokio::time::Instant;
use tokio::time::sleep;

use super::EnvVarGuard;
use super::TestMcpServerOptions;
use super::copy_binary_to_remote_env;
use super::insert_mcp_server;
use super::remote_aware_experimental_environment;
use super::remote_env_container_name;
use super::wait_for_mcp_tool;
use super::write_fallback_oauth_tokens;

/// Remote runtime websocket URL used by remote-aware MCP integration tests.
const REMOTE_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_TEST_REMOTE_EXEC_SERVER_URL";
/// OAuth metadata path served by the Streamable HTTP MCP test server.
const STREAMABLE_HTTP_METADATA_PATH: &str = "/.well-known/oauth-authorization-server/mcp";

/// Streamable HTTP test server plus the process handle needed for cleanup.
struct StreamableHttpTestServer {
    server_url: String,
    process: StreamableHttpTestServerProcess,
}

/// Tracks whether the Streamable HTTP test server runs on the host or remotely.
enum StreamableHttpTestServerProcess {
    Local(Child),
    Remote(RemoteStreamableHttpServer),
}

/// Remote Streamable HTTP server process and copied files to remove on drop.
struct RemoteStreamableHttpServer {
    container_name: String,
    pid: String,
    paths_to_remove: Vec<String>,
}

impl Drop for RemoteStreamableHttpServer {
    /// Stops the remote process and removes copied test artifacts best-effort.
    fn drop(&mut self) {
        self.kill();
        if self.paths_to_remove.is_empty() {
            return;
        }
        let script = format!("rm -f {}", self.paths_to_remove.join(" "));
        let _ = StdCommand::new("docker")
            .args(["exec", &self.container_name, "sh", "-lc", &script])
            .output();
    }
}

impl RemoteStreamableHttpServer {
    /// Stops the remote Streamable HTTP test server process.
    fn kill(&self) {
        let _ = StdCommand::new("docker")
            .args(["exec", &self.container_name, "kill", &self.pid])
            .output();
    }
}

impl StreamableHttpTestServer {
    /// Returns the MCP endpoint URL that Codex should connect to.
    fn url(&self) -> &str {
        &self.server_url
    }

    /// Stops the local or remote test server and waits for local process exit.
    async fn shutdown(mut self) {
        match &mut self.process {
            StreamableHttpTestServerProcess::Local(child) => match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    let _ = child.kill().await;
                }
                Err(error) => {
                    eprintln!("failed to check streamable http server status: {error}");
                    let _ = child.kill().await;
                }
            },
            StreamableHttpTestServerProcess::Remote(server) => {
                server.kill();
            }
        }
        if let StreamableHttpTestServerProcess::Local(child) = &mut self.process
            && let Err(error) = child.wait().await
        {
            eprintln!("failed to await streamable http server shutdown: {error}");
        }
    }
}

/// What this tests: Codex can discover and call a Streamable HTTP MCP tool in
/// both local and remote-aware placements, and the tool observes the expected
/// environment value from the server process that actually handled the request.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_tool_call_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: script the model responses so Codex will call the MCP echo tool
    // and then complete the turn after the tool result is returned.
    let server = responses::start_mock_server().await;

    let call_id = "call-456";
    let server_name = "rmcp_http";
    let namespace = format!("mcp__{server_name}__");

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                "echo",
                "{\"message\":\"ping\"}",
            ),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message(
                "msg-1",
                "rmcp streamable http echo tool completed successfully.",
            ),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    // Phase 2: start the Streamable HTTP MCP test server in the active
    // placement. In full CI this may be the remote executor container; locally
    // it is a host process.
    let expected_env_value = "propagated-env-http";
    let Some(http_server) =
        start_streamable_http_test_server(expected_env_value, /*expected_token*/ None).await?
    else {
        return Ok(());
    };
    let server_url = http_server.url().to_string();

    // Phase 3: configure Codex with the Streamable HTTP MCP server and build a
    // fixture that selects remote MCP placement only when the remote test
    // environment is active.
    let fixture = test_codex()
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                McpServerTransportConfig::StreamableHttp {
                    url: server_url,
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    // Phase 4: submit the user turn that should trigger the MCP tool call.
    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp streamable http echo tool".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
            environments: None,
        })
        .await?;

    // Phase 5: assert Codex begins the expected tool invocation.
    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;

    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.invocation.server, server_name);
    assert_eq!(begin.invocation.tool, "echo");

    // Phase 6: assert the tool result proves the server handled the request and
    // propagated the expected environment value.
    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };

    let result = end
        .result
        .as_ref()
        .expect("rmcp echo tool should return success");
    assert_eq!(result.is_error, Some(false));
    assert!(
        result.content.is_empty(),
        "content should default to an empty array"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content");
    let Value::Object(map) = structured else {
        panic!("structured content should be an object: {structured:?}");
    };
    let echo_value = map
        .get("echo")
        .and_then(Value::as_str)
        .expect("echo payload present");
    assert_eq!(echo_value, "ECHOING: ping");
    let env_value = map
        .get("env")
        .and_then(Value::as_str)
        .expect("env snapshot inserted");
    assert_eq!(env_value, expected_env_value);

    // Phase 7: verify the scripted model calls were consumed and clean up the
    // placement-aware MCP server.
    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    server.verify().await;

    http_server.shutdown().await;

    Ok(())
}

/// This test writes to a fallback credentials file in CODEX_HOME.
/// Ideally, we wouldn't need to serialize the test but it's much more cumbersome to wire CODEX_HOME through the code.
#[test]
#[serial(codex_home)]
fn streamable_http_with_oauth_round_trip() -> anyhow::Result<()> {
    const TEST_STACK_SIZE_BYTES: usize = 8 * 1024 * 1024;

    let handle = std::thread::Builder::new()
        .name("streamable_http_with_oauth_round_trip".to_string())
        .stack_size(TEST_STACK_SIZE_BYTES)
        .spawn(|| -> anyhow::Result<()> {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()?;
            runtime.block_on(streamable_http_with_oauth_round_trip_impl())
        })?;

    match handle.join() {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!(
            "streamable_http_with_oauth_round_trip thread panicked"
        )),
    }
}

#[allow(clippy::expect_used)]
async fn streamable_http_with_oauth_round_trip_impl() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    // Phase 1: script the model responses so Codex will call the OAuth-backed
    // MCP echo tool and then finish the turn after receiving the result.
    let server = responses::start_mock_server().await;

    let call_id = "call-789";
    let server_name = "rmcp_http_oauth";
    let tool_name = format!("mcp__{server_name}__echo");
    let namespace = format!("mcp__{server_name}__");

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                "echo",
                "{\"message\":\"ping\"}",
            ),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message(
                "msg-1",
                "rmcp streamable http oauth echo tool completed successfully.",
            ),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    // Phase 2: start the Streamable HTTP MCP test server with bearer-token
    // enforcement enabled so the client must use stored OAuth credentials.
    let expected_env_value = "propagated-env-http-oauth";
    let expected_token = "initial-access-token";
    let client_id = "test-client-id";
    let refresh_token = "initial-refresh-token";
    let Some(http_server) =
        start_streamable_http_test_server(expected_env_value, Some(expected_token)).await?
    else {
        return Ok(());
    };
    let server_url = http_server.url().to_string();

    // Phase 3: seed an isolated CODEX_HOME with fallback OAuth tokens for this
    // server so the test does not share credentials with other suite cases.
    let temp_home = Arc::new(tempdir()?);
    let _codex_home_guard = EnvVarGuard::set("CODEX_HOME", temp_home.path().as_os_str());
    write_fallback_oauth_tokens(
        temp_home.path(),
        server_name,
        &server_url,
        client_id,
        expected_token,
        refresh_token,
    )?;

    // Phase 4: configure Codex with the OAuth-backed Streamable HTTP MCP
    // server and build the fixture in the active local or remote-aware mode.
    let fixture = test_codex()
        .with_home(temp_home.clone())
        .with_config(move |config| {
            // Keep OAuth credentials isolated to this test home because Bazel
            // runs the full core suite in one process.
            config.mcp_oauth_credentials_store_mode = serde_json::from_value(json!("file"))
                .expect("`file` should deserialize as OAuthCredentialsStoreMode");
            insert_mcp_server(
                config,
                server_name,
                McpServerTransportConfig::StreamableHttp {
                    url: server_url,
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    // Phase 5: wait for MCP discovery to publish the expected tool before the
    // turn is submitted, which keeps failures tied to server startup/discovery.
    wait_for_mcp_tool(&fixture, &tool_name).await?;

    // Phase 6: submit the user turn that should invoke the OAuth-backed tool.
    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp streamable http oauth echo tool".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
            environments: None,
        })
        .await?;

    // Phase 7: assert Codex begins the expected tool invocation.
    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;

    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.invocation.server, server_name);
    assert_eq!(begin.invocation.tool, "echo");

    // Phase 8: assert the tool result proves the authenticated request reached
    // the server and preserved the expected environment value.
    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };

    let result = end
        .result
        .as_ref()
        .expect("rmcp echo tool should return success");
    assert_eq!(result.is_error, Some(false));
    assert!(
        result.content.is_empty(),
        "content should default to an empty array"
    );

    let structured = result
        .structured_content
        .as_ref()
        .expect("structured content");
    let Value::Object(map) = structured else {
        panic!("structured content should be an object: {structured:?}");
    };
    let echo_value = map
        .get("echo")
        .and_then(Value::as_str)
        .expect("echo payload present");
    assert_eq!(echo_value, "ECHOING: ping");
    let env_value = map
        .get("env")
        .and_then(Value::as_str)
        .expect("env snapshot inserted");
    assert_eq!(env_value, expected_env_value);

    // Phase 9: verify the scripted model calls were consumed and clean up the
    // placement-aware MCP server.
    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    server.verify().await;

    http_server.shutdown().await;

    Ok(())
}

/// Starts the Streamable HTTP MCP test server in the active test placement.
async fn start_streamable_http_test_server(
    expected_env_value: &str,
    expected_token: Option<&str>,
) -> anyhow::Result<Option<StreamableHttpTestServer>> {
    let rmcp_http_server_bin = match cargo_bin("test_streamable_http_server") {
        Ok(path) => path,
        Err(err) => {
            eprintln!("test_streamable_http_server binary not available, skipping test: {err}");
            return Ok(None);
        }
    };

    if let Some(container_name) = remote_env_container_name()? {
        return Ok(Some(
            start_remote_streamable_http_test_server(
                &container_name,
                &rmcp_http_server_bin,
                expected_env_value,
                expected_token,
            )
            .await?,
        ));
    }

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let bind_addr = format!("127.0.0.1:{port}");
    let server_url = format!("http://{bind_addr}/mcp");

    let mut command = Command::new(&rmcp_http_server_bin);
    command
        .kill_on_drop(true)
        .env("MCP_STREAMABLE_HTTP_BIND_ADDR", &bind_addr)
        .env("MCP_TEST_VALUE", expected_env_value);
    if let Some(expected_token) = expected_token {
        command.env("MCP_EXPECT_BEARER", expected_token);
    }
    let mut child = command.spawn()?;

    wait_for_local_streamable_http_server(&mut child, &server_url, Duration::from_secs(5)).await?;
    Ok(Some(StreamableHttpTestServer {
        server_url,
        process: StreamableHttpTestServerProcess::Local(child),
    }))
}

/// Starts the Streamable HTTP MCP test server inside the remote test container.
async fn start_remote_streamable_http_test_server(
    container_name: &str,
    rmcp_http_server_bin: &Path,
    expected_env_value: &str,
    expected_token: Option<&str>,
) -> anyhow::Result<StreamableHttpTestServer> {
    let remote_path = copy_binary_to_remote_env(
        container_name,
        rmcp_http_server_bin,
        "test_streamable_http_server",
    )?;
    let bound_addr_file = format!("{remote_path}.addr");
    let log_file = format!("{remote_path}.log");
    let mut env_assignments = vec![
        format!(
            "MCP_STREAMABLE_HTTP_BIND_ADDR={}",
            sh_single_quote("0.0.0.0:0")
        ),
        format!(
            "MCP_STREAMABLE_HTTP_BOUND_ADDR_FILE={}",
            sh_single_quote(&bound_addr_file)
        ),
        format!("MCP_TEST_VALUE={}", sh_single_quote(expected_env_value)),
    ];
    if let Some(expected_token) = expected_token {
        env_assignments.push(format!(
            "MCP_EXPECT_BEARER={}",
            sh_single_quote(expected_token)
        ));
    }

    let script = format!(
        "{} nohup {} > {} 2>&1 < /dev/null & echo $!",
        env_assignments.join(" "),
        sh_single_quote(&remote_path),
        sh_single_quote(&log_file)
    );
    let start_output = StdCommand::new("docker")
        .args(["exec", container_name, "sh", "-lc", &script])
        .output()
        .context("start remote streamable HTTP MCP test server")?;
    ensure!(
        start_output.status.success(),
        "docker start streamable HTTP MCP test server failed: stdout={} stderr={}",
        String::from_utf8_lossy(&start_output.stdout).trim(),
        String::from_utf8_lossy(&start_output.stderr).trim()
    );
    let pid = String::from_utf8(start_output.stdout)
        .context("remote streamable HTTP server pid must be utf-8")?
        .trim()
        .to_string();
    ensure!(
        !pid.is_empty(),
        "remote streamable HTTP server pid is empty"
    );

    let remote_bind_addr =
        wait_for_remote_bound_addr(container_name, &bound_addr_file, Duration::from_secs(5))
            .await?;
    let container_ip = remote_container_ip(container_name)?;
    let server_url = format!("http://{}:{}/mcp", container_ip, remote_bind_addr.port());
    // The orchestrator can see the Docker container IP, but the behavior under
    // test is whether the remote-side MCP client can reach it. Probe through
    // remote HTTP before handing the URL to the Codex fixture.
    wait_for_remote_streamable_http_server(&server_url, Duration::from_secs(5)).await?;
    if expected_token.is_some() {
        wait_for_streamable_http_metadata(&server_url, Duration::from_secs(5)).await?;
    }

    Ok(StreamableHttpTestServer {
        server_url,
        process: StreamableHttpTestServerProcess::Remote(RemoteStreamableHttpServer {
            container_name: container_name.to_string(),
            pid,
            paths_to_remove: vec![remote_path, bound_addr_file, log_file],
        }),
    })
}

/// Single-quotes a value for the small shell snippets sent through Docker.
fn sh_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Waits until the remote test server writes the socket address it bound to.
async fn wait_for_remote_bound_addr(
    container_name: &str,
    bound_addr_file: &str,
    timeout: Duration,
) -> anyhow::Result<SocketAddr> {
    let deadline = Instant::now() + timeout;
    loop {
        let output = StdCommand::new("docker")
            .args(["exec", container_name, "cat", bound_addr_file])
            .output()
            .context("read remote streamable HTTP server bound address")?;
        if output.status.success() {
            let bound_addr = String::from_utf8(output.stdout)
                .context("remote streamable HTTP bound address must be utf-8")?;
            return bound_addr
                .trim()
                .parse()
                .context("parse remote streamable HTTP bound address");
        }
        if Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "timed out waiting for remote streamable HTTP bound address: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        sleep(Duration::from_millis(50)).await;
    }
}

/// Reads the container IP that the host-side test process can use.
fn remote_container_ip(container_name: &str) -> anyhow::Result<String> {
    let output = StdCommand::new("docker")
        .args([
            "inspect",
            "-f",
            "{{range .NetworkSettings.Networks}}{{println .IPAddress}}{{end}}",
            container_name,
        ])
        .output()
        .context("inspect remote MCP test container IP")?;
    ensure!(
        output.status.success(),
        "docker inspect remote MCP test container IP failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let inspect_output =
        String::from_utf8(output.stdout).context("remote MCP test container IP must be utf-8")?;
    let ip = inspect_output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_string();
    if ip.is_empty() {
        Ok("127.0.0.1".to_string())
    } else {
        Ok(ip)
    }
}

/// Waits for the local Streamable HTTP test server to publish OAuth metadata.
async fn wait_for_local_streamable_http_server(
    server_child: &mut Child,
    server_url: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + timeout;
    let metadata_url = streamable_http_metadata_url(server_url);
    let client = Client::builder().no_proxy().build()?;
    loop {
        if let Some(status) = server_child.try_wait()? {
            return Err(anyhow::anyhow!(
                "streamable HTTP server exited early with status {status}"
            ));
        }

        let remaining = deadline.saturating_duration_since(Instant::now());

        if remaining.is_zero() {
            return Err(anyhow::anyhow!(
                "timed out waiting for streamable HTTP server metadata at {metadata_url}: deadline reached"
            ));
        }

        match tokio::time::timeout(remaining, client.get(&metadata_url).send()).await {
            Ok(Ok(response)) if response.status() == StatusCode::OK => return Ok(()),
            Ok(Ok(response)) => {
                if Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for streamable HTTP server metadata at {metadata_url}: HTTP {}",
                        response.status()
                    ));
                }
            }
            Ok(Err(error)) => {
                if Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for streamable HTTP server metadata at {metadata_url}: {error}"
                    ));
                }
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "timed out waiting for streamable HTTP server metadata at {metadata_url}: request timed out"
                ));
            }
        }

        sleep(Duration::from_millis(50)).await;
    }
}

/// Waits for the remote Streamable HTTP test server via remote HTTP.
async fn wait_for_remote_streamable_http_server(
    server_url: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    let websocket_url = std::env::var(REMOTE_EXEC_SERVER_URL_ENV_VAR).with_context(|| {
        format!("{REMOTE_EXEC_SERVER_URL_ENV_VAR} must be set for remote streamable HTTP MCP tests")
    })?;
    let environment = Environment::create_for_tests(Some(websocket_url))?;
    let http_client = environment.get_http_client();
    let metadata_url = streamable_http_metadata_url(server_url);
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(anyhow::anyhow!(
                "timed out waiting for remote streamable HTTP server metadata at {metadata_url}: deadline reached"
            ));
        }

        let request = HttpRequestParams {
            method: "GET".to_string(),
            url: metadata_url.clone(),
            headers: Vec::new(),
            body: None,
            timeout_ms: Some(remaining.as_millis().clamp(1, 1_000) as u64),
            request_id: "buffered-request".to_string(),
            stream_response: false,
        };
        match http_client.http_request(request).await {
            Ok(response) if response.status == StatusCode::OK.as_u16() => return Ok(()),
            Ok(response) => {
                if Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for remote streamable HTTP server metadata at {metadata_url}: HTTP {}",
                        response.status
                    ));
                }
            }
            Err(error) => {
                if Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for remote streamable HTTP server metadata at {metadata_url}: {error}"
                    ));
                }
            }
        }

        sleep(Duration::from_millis(50)).await;
    }
}

/// Waits for OAuth metadata from the host-side test process.
async fn wait_for_streamable_http_metadata(
    server_url: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + timeout;
    let metadata_url = streamable_http_metadata_url(server_url);
    let client = Client::builder().no_proxy().build()?;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(anyhow::anyhow!(
                "timed out waiting for streamable HTTP server metadata at {metadata_url}: deadline reached"
            ));
        }

        match tokio::time::timeout(remaining, client.get(&metadata_url).send()).await {
            Ok(Ok(response)) if response.status() == StatusCode::OK => return Ok(()),
            Ok(Ok(response)) => {
                if Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for streamable HTTP server metadata at {metadata_url}: HTTP {}",
                        response.status()
                    ));
                }
            }
            Ok(Err(error)) => {
                if Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "timed out waiting for streamable HTTP server metadata at {metadata_url}: {error}"
                    ));
                }
            }
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "timed out waiting for streamable HTTP server metadata at {metadata_url}: request timed out"
                ));
            }
        }

        sleep(Duration::from_millis(50)).await;
    }
}

/// Builds the OAuth metadata URL for the test Streamable HTTP MCP endpoint.
fn streamable_http_metadata_url(server_url: &str) -> String {
    let base_url = server_url.strip_suffix("/mcp").unwrap_or(server_url);
    format!("{base_url}{STREAMABLE_HTTP_METADATA_PATH}")
}
