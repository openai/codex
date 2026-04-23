#![allow(clippy::expect_used)]

use anyhow::Context as _;
use anyhow::ensure;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codex_config::types::McpServerConfig;
use codex_config::types::McpServerEnvVar;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::Config;
use codex_exec_server::CreateDirectoryOptions;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_mcp::MCP_SANDBOX_STATE_META_CAPABILITY;
use codex_models_manager::manager::RefreshStrategy;

use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpInvocation;
use codex_protocol::protocol::McpToolCallBeginEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use codex_utils_cargo_bin::cargo_bin;
use core_test_support::assert_regex_match;
use core_test_support::remote_env_env_var;
use core_test_support::responses;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
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
use wiremock::MockServer;

static OPENAI_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAD0AAAA9CAYAAAAeYmHpAAAE6klEQVR4Aeyau44UVxCGx1fZsmRLlm3Zoe0XcGQ5cUiCCIgJeS9CHgAhMkISQnIuGQgJEkBcxLW+nqnZ6uqqc+nuWRC7q/P3qetf9e+MtOwyX25O4Nep6JPyop++0qev9HrfgZ+F6r2DuB/vHOrt/UIkqdDHYvujOW6fO7h/CNEI+a5jc+pBR8uy0jVFsziYu5HtfSUk+Io34q921hLNctFSX0gwww+S8wce8K1LfCU+cYW4888aov8NxqvQILUPPReLOrm6zyLxa4i+6VZuFbJo8d1MOHZm+7VUtB/aIvhPWc/3SWg49JcwFLlHxuXKjtyloo+YNhuW3VS+WPBuUEMvCFKjEDVgFBQHXrnazpqiSxNZCkQ1kYiozsbm9Oz7l4i2Il7vGccGNWAc3XosDrZe/9P3ZnMmzHNEQw4smf8RQ87XEAMsC7Az0Au+dgXerfH4+sHvEc0SYGic8WBBUGqFH2gN7yDrazy7m2pbRTeRmU3+MjZmr1h6LJgPbGy23SI6GlYT0brQ71IY8Us4PNQCm+zepSbaD2BY9xCaAsD9IIj/IzFmKMSdHHonwdZATbTnYREf6/VZGER98N9yCWIvXQwXDoDdhZJoT8jwLnJXDB9w4Sb3e6nK5ndzlkTLnP3JBu4LKkbrYrU69gCVceV0JvpyuW1xlsUVngzhwMetn/XamtTORF9IO5YnWNiyeF9zCAfqR3fUW+vZZKLtgP+ts8BmQRBREAdRDhH3o8QuRh/YucNFz2BEjxbRN6LGzphfKmvP6v6QhqIQyZ8XNJ0W0X83MR1PEcJBNO2KC2Z1TW/v244scp9FwRViZxIOBF0Lctk7ZVSavdLvRlV1hz/ysUi9sr8CIcB3nvWBwA93ykTz18eAYxQ6N/K2DkPA1lv3iXCwmDUT7YkjIby9siXueIJj9H+pzSqJ9oIuJWTUgSSt4WO7o/9GGg0viR4VinNRUDoIj34xoCd6pxD3aK3zfdbnx5v1J3ZNNEJsE0sBG7N27ReDrJc4sFxz7dI/ZAbOmmiKvHBitQXpAdR6+F7v+/ol/tOouUV01EeMZQF2BoQDn6dP4XNr+j9GZEtEK1/L8pFw7bd3a53tsTa7WD+054jOFmPg1XBKPQgnqFfmFcy32ZRvjmiIIQTYFvyDxQ8nH8WIwwGwlyDjDznnilYyFr6njrlZwsKkBpO59A7OwgdzPEWRm+G+oeb7IfyNuzjEEVLrOVxJsxvxwF8kmCM6I2QYmJunz4u4TrADpfl7mlbRTWQ7VmrBzh3+C9f6Grc3YoGN9dg/SXFthpRsT6vobfXRs2VBlgBHXVMLHjDNbIZv1sZ9+X3hB09cXdH1JKViyG0+W9bWZDa/r2f9zAFR71sTzGpMSWz2iI4YssWjWo3REy1MDGjdwe5e0dFSiAC1JakBvu4/CUS8Eh6dqHdU0Or0ioY3W5ClSqDXAy7/6SRfgw8vt4I+tbvvNtFT2kVDhY5+IGb1rCqYaXNF08vSALsXCPmt0kQNqJT1p5eI1mkIV/BxCY1z85lOzeFbPBQHURkkPTlwTYK9gTVE25l84IbFFN+YJDHjdpn0gq6mrHht0dkcjbM4UL9283O5p77GN+SPW/QwVB4IUYg7Or+Kp7naR6qktP98LNF2UxWo9yObPIT9KYg+hK4i56no4rfnM0qeyFf6AwAAAP//trwR3wAAAAZJREFUAwBZ0sR75itw5gAAAABJRU5ErkJggg==";

fn assert_wall_time_line(line: &str) {
    assert_regex_match(r"^Wall time: [0-9]+(?:\.[0-9]+)? seconds$", line);
}

fn split_wall_time_wrapped_output(output: &str) -> &str {
    let Some((wall_time, rest)) = output.split_once('\n') else {
        panic!("wall-time output should contain an Output section: {output}");
    };
    assert_wall_time_line(wall_time);
    let Some(output) = rest.strip_prefix("Output:\n") else {
        panic!("wall-time output should contain Output marker: {output}");
    };
    output
}

fn assert_wall_time_header(output: &str) {
    let Some((wall_time, marker)) = output.split_once('\n') else {
        panic!("wall-time header should contain an Output marker: {output}");
    };
    assert_wall_time_line(wall_time);
    assert_eq!(marker, "Output:");
}

#[derive(Debug, PartialEq, Eq)]
enum McpCallEvent {
    Begin(String),
    End(String),
}

const REMOTE_MCP_ENVIRONMENT: &str = "remote";

fn remote_aware_experimental_environment() -> Option<String> {
    // These tests run locally in normal CI and against the Docker-backed
    // executor in full-ci. Match that shared test environment instead of
    // parameterizing each stdio MCP test with its own local/remote cases.
    std::env::var_os(remote_env_env_var()).map(|_| REMOTE_MCP_ENVIRONMENT.to_string())
}

/// Returns the stdio MCP test server command path for the active test placement.
///
/// Local test runs can execute the host-built test binary directly. Remote-aware
/// runs start MCP stdio through the executor inside Docker, so the host path
/// would be meaningless to the process that actually launches the server. When
/// the remote test environment is active, copy the binary into the executor
/// container and return that in-container path instead.
fn remote_aware_stdio_server_bin() -> anyhow::Result<String> {
    let bin = stdio_server_bin()?;
    let Some(container_name) = remote_env_container_name()? else {
        return Ok(bin);
    };

    // Keep the Docker path rewrite scoped to tests that use `build_remote_aware`.
    // Other MCP tests still start their stdio server from the orchestrator test
    // process, even when the full-ci remote env is present.
    //
    // Remote-aware MCP tests run the executor inside Docker. The stdio test
    // server is built on the host, so hand the executor a copied in-container
    // path instead of the host build artifact path.
    // Several remote-aware MCP tests can run in parallel; give each copied
    // binary its own path so one test cannot replace another test's executable.
    copy_binary_to_remote_env(&container_name, Path::new(&bin), "test_stdio_server")
}

/// Returns the Docker container used by remote-aware MCP tests, when active.
fn remote_env_container_name() -> anyhow::Result<Option<String>> {
    let Some(container_name) = std::env::var_os(remote_env_env_var()) else {
        return Ok(None);
    };
    Ok(Some(container_name.into_string().map_err(|value| {
        anyhow::anyhow!("remote env container name must be utf-8: {value:?}")
    })?))
}

/// Builds a collision-resistant in-container path for copied test binaries.
fn unique_remote_path(binary_name: &str) -> anyhow::Result<String> {
    let unique_suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(format!(
        "/tmp/codex-remote-env/{binary_name}-{}-{unique_suffix}",
        std::process::id()
    ))
}

/// Copies a host-built helper binary into the remote test container.
fn copy_binary_to_remote_env(
    container_name: &str,
    host_path: &Path,
    binary_name: &str,
) -> anyhow::Result<String> {
    let remote_path = unique_remote_path(binary_name)?;
    let mkdir_output = StdCommand::new("docker")
        .args([
            "exec",
            container_name,
            "mkdir",
            "-p",
            "/tmp/codex-remote-env",
        ])
        .output()
        .context("create remote MCP test binary directory")?;
    ensure!(
        mkdir_output.status.success(),
        "docker mkdir remote MCP test binary directory failed: stdout={} stderr={}",
        String::from_utf8_lossy(&mkdir_output.stdout).trim(),
        String::from_utf8_lossy(&mkdir_output.stderr).trim()
    );

    let container_target = format!("{container_name}:{remote_path}");
    let copy_output = StdCommand::new("docker")
        .arg("cp")
        .arg(host_path)
        .arg(&container_target)
        .output()
        .with_context(|| {
            format!(
                "copy {} to remote MCP test env",
                host_path.to_string_lossy()
            )
        })?;
    ensure!(
        copy_output.status.success(),
        "docker cp {binary_name} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&copy_output.stdout).trim(),
        String::from_utf8_lossy(&copy_output.stderr).trim()
    );

    let chmod_output = StdCommand::new("docker")
        .args(["exec", container_name, "chmod", "+x", remote_path.as_str()])
        .output()
        .with_context(|| format!("mark remote {binary_name} executable"))?;
    ensure!(
        chmod_output.status.success(),
        "docker chmod {binary_name} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&chmod_output.stdout).trim(),
        String::from_utf8_lossy(&chmod_output.stderr).trim()
    );

    Ok(remote_path)
}

async fn wait_for_mcp_tool(fixture: &TestCodex, tool_name: &str) -> anyhow::Result<()> {
    let tools_ready_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        fixture.codex.submit(Op::ListMcpTools).await?;
        let list_event = wait_for_event_with_timeout(
            &fixture.codex,
            |ev| matches!(ev, EventMsg::McpListToolsResponse(_)),
            Duration::from_secs(10),
        )
        .await;
        let EventMsg::McpListToolsResponse(tool_list) = list_event else {
            unreachable!("event guard guarantees McpListToolsResponse");
        };
        if tool_list.tools.contains_key(tool_name) {
            return Ok(());
        }

        let available_tools: Vec<&str> = tool_list.tools.keys().map(String::as_str).collect();
        if Instant::now() >= tools_ready_deadline {
            panic!(
                "timed out waiting for MCP tool {tool_name} to become available; discovered tools: {available_tools:?}"
            );
        }
        sleep(Duration::from_millis(200)).await;
    }
}

#[derive(Default)]
struct TestMcpServerOptions {
    experimental_environment: Option<String>,
    supports_parallel_tool_calls: bool,
    tool_timeout_sec: Option<Duration>,
}

fn stdio_transport(
    command: String,
    env: Option<HashMap<String, String>>,
    env_vars: Vec<McpServerEnvVar>,
) -> McpServerTransportConfig {
    stdio_transport_with_cwd(command, env, env_vars, /*cwd*/ None)
}

fn stdio_transport_with_cwd(
    command: String,
    env: Option<HashMap<String, String>>,
    env_vars: Vec<McpServerEnvVar>,
    cwd: Option<PathBuf>,
) -> McpServerTransportConfig {
    McpServerTransportConfig::Stdio {
        command,
        args: Vec::new(),
        env,
        env_vars,
        cwd,
    }
}

fn insert_mcp_server(
    config: &mut Config,
    server_name: &str,
    transport: McpServerTransportConfig,
    options: TestMcpServerOptions,
) {
    let mut servers = config.mcp_servers.get().clone();
    servers.insert(
        server_name.to_string(),
        McpServerConfig {
            transport,
            experimental_environment: options.experimental_environment,
            enabled: true,
            required: false,
            supports_parallel_tool_calls: options.supports_parallel_tool_calls,
            disabled_reason: None,
            startup_timeout_sec: Some(Duration::from_secs(10)),
            tool_timeout_sec: options.tool_timeout_sec,
            default_tools_approval_mode: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );
    if let Err(err) = config.mcp_servers.set(servers) {
        panic!("test mcp servers should accept any configuration: {err}");
    }
}

async fn call_cwd_tool(
    server: &MockServer,
    fixture: &TestCodex,
    server_name: &str,
    call_id: &str,
) -> anyhow::Result<Value> {
    let namespace = format!("mcp__{server_name}__");
    mount_sse_once(
        server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(call_id, &namespace, "cwd", r#"{}"#),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp cwd tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let session_model = fixture.session_configured.model.clone();
    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp cwd tool".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: fixture.config.cwd.to_path_buf(),
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

    wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;
    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };
    let structured_content = end
        .result
        .as_ref()
        .expect("rmcp cwd tool should return success")
        .structured_content
        .as_ref()
        .expect("structured content")
        .clone();

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    Ok(structured_content)
}

fn assert_cwd_tool_output(structured: &Value, expected_cwd: &Path) {
    let actual_cwd = structured
        .get("cwd")
        .and_then(Value::as_str)
        .expect("cwd tool should return a string cwd");

    if std::env::var_os(remote_env_env_var()).is_some() {
        assert_eq!(
            structured,
            &json!({
                "cwd": expected_cwd.to_string_lossy(),
            })
        );
        return;
    }

    // Local Windows can report the same absolute directory through an 8.3 path.
    // Canonical paths keep the assertion focused on cwd precedence.
    assert_eq!(
        Path::new(actual_cwd)
            .canonicalize()
            .expect("cwd tool path should exist"),
        expected_cwd
            .canonicalize()
            .expect("expected cwd should exist"),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn stdio_server_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "call-123";
    let server_name = "rmcp";
    let namespace = format!("mcp__{server_name}__");

    let call_mock = mount_sse_once(
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
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let expected_env_value = "propagated-env";
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(
                    rmcp_test_server_bin,
                    Some(HashMap::from([(
                        "MCP_TEST_VALUE".to_string(),
                        expected_env_value.to_string(),
                    )])),
                    Vec::new(),
                ),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp echo tool".into(),
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

    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;

    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.invocation.server, server_name);
    assert_eq!(begin.invocation.tool, "echo");

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

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let output_item = final_mock.single_request().function_call_output(call_id);
    let request = call_mock.single_request();
    assert!(
        request.tool_by_name(&namespace, "echo").is_some(),
        "direct MCP tool should be sent as a namespace child tool: {:?}",
        request.body_json()
    );

    let output_text = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("function_call_output output should be a string");
    let wrapped_payload = split_wall_time_wrapped_output(output_text);
    let output_json: Value = serde_json::from_str(wrapped_payload)
        .expect("wrapped MCP output should preserve structured JSON");
    assert_eq!(output_json["echo"], "ECHOING: ping");
    assert_eq!(output_json["env"], expected_env_value);

    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_cwd)]
async fn stdio_server_uses_configured_cwd_before_runtime_fallback() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let server_name = "rmcp_configured_cwd";
    let expected_cwd = Arc::new(Mutex::new(None::<PathBuf>));
    let expected_cwd_for_config = Arc::clone(&expected_cwd);
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_workspace_setup(|cwd, fs| async move {
            fs.create_directory(
                &cwd.join("mcp-configured-cwd"),
                CreateDirectoryOptions { recursive: true },
                /*sandbox*/ None,
            )
            .await?;
            Ok::<(), anyhow::Error>(())
        })
        .with_config(move |config| {
            let configured_cwd = config.cwd.join("mcp-configured-cwd").into_path_buf();
            *expected_cwd_for_config
                .lock()
                .expect("expected cwd lock should not be poisoned") = Some(configured_cwd.clone());
            insert_mcp_server(
                config,
                server_name,
                stdio_transport_with_cwd(
                    rmcp_test_server_bin,
                    Some(HashMap::from([(
                        "MCP_TEST_VALUE".to_string(),
                        "configured-cwd".to_string(),
                    )])),
                    Vec::new(),
                    Some(configured_cwd),
                ),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;

    let expected_cwd = expected_cwd
        .lock()
        .expect("expected cwd lock should not be poisoned")
        .clone()
        .expect("test config should record configured MCP cwd");
    let structured = call_cwd_tool(&server, &fixture, server_name, "call-configured-cwd").await?;

    assert_cwd_tool_output(&structured, &expected_cwd);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_cwd)]
async fn remote_stdio_server_uses_runtime_fallback_cwd_when_config_omits_cwd() -> anyhow::Result<()>
{
    skip_if_no_network!(Ok(()));
    if std::env::var_os(remote_env_env_var()).is_none() {
        return Ok(());
    }

    let server = responses::start_mock_server().await;
    let server_name = "rmcp_fallback_cwd";
    let expected_cwd = Arc::new(Mutex::new(None::<PathBuf>));
    let expected_cwd_for_config = Arc::clone(&expected_cwd);
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            *expected_cwd_for_config
                .lock()
                .expect("expected cwd lock should not be poisoned") =
                Some(config.cwd.to_path_buf());
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(
                    rmcp_test_server_bin,
                    Some(HashMap::from([(
                        "MCP_TEST_VALUE".to_string(),
                        "fallback-cwd".to_string(),
                    )])),
                    Vec::new(),
                ),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;

    let expected_cwd = expected_cwd
        .lock()
        .expect("expected cwd lock should not be poisoned")
        .clone()
        .expect("test config should record runtime fallback cwd");
    let structured = call_cwd_tool(&server, &fixture, server_name, "call-fallback-cwd").await?;

    assert_cwd_tool_output(&structured, &expected_cwd);
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn stdio_mcp_tool_call_includes_sandbox_state_meta() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "sandbox-meta-call";
    let server_name = "rmcp";
    let namespace = format!("mcp__{server_name}__");
    let tool_name = format!("{namespace}sandbox_meta");

    let call_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(call_id, &namespace, "sandbox_meta", "{}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp sandbox meta completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;
    let fixture = test_codex()
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(rmcp_test_server_bin, /*env*/ None, Vec::new()),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;

    let tools_ready_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        fixture.codex.submit(Op::ListMcpTools).await?;
        let list_event = wait_for_event_with_timeout(
            &fixture.codex,
            |ev| matches!(ev, EventMsg::McpListToolsResponse(_)),
            Duration::from_secs(10),
        )
        .await;
        let EventMsg::McpListToolsResponse(tool_list) = list_event else {
            unreachable!("event guard guarantees McpListToolsResponse");
        };
        if tool_list.tools.contains_key(&tool_name) {
            break;
        }

        let available_tools: Vec<&str> = tool_list.tools.keys().map(String::as_str).collect();
        if Instant::now() >= tools_ready_deadline {
            panic!(
                "timed out waiting for MCP tool {tool_name} to become available; discovered tools: {available_tools:?}"
            );
        }
        sleep(Duration::from_millis(200)).await;
    }

    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    fixture
        .submit_turn_with_policy("call the rmcp sandbox_meta tool", sandbox_policy.clone())
        .await?;

    let request = call_mock.single_request();
    assert!(
        request.tool_by_name(&namespace, "sandbox_meta").is_some(),
        "direct MCP tool should be sent as a namespace child tool: {:?}",
        request.body_json()
    );

    let output_item = final_mock.single_request().function_call_output(call_id);
    let output_text = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("function_call_output output should be a string");
    let wrapped_payload = split_wall_time_wrapped_output(output_text);
    let output_json: Value = serde_json::from_str(wrapped_payload)
        .expect("wrapped MCP output should preserve sandbox metadata JSON");
    let Value::Object(meta) = output_json else {
        panic!("sandbox_meta should return metadata object: {output_json:?}");
    };

    let sandbox_meta = meta
        .get(MCP_SANDBOX_STATE_META_CAPABILITY)
        .expect("sandbox state metadata should be present");
    let expected_sandbox_policy = serde_json::to_value(&sandbox_policy)?;
    assert_eq!(
        sandbox_meta.get("sandboxPolicy"),
        Some(&expected_sandbox_policy)
    );
    assert_eq!(
        sandbox_meta.get("sandboxCwd").and_then(Value::as_str),
        fixture.config.cwd.as_path().to_str()
    );
    assert_eq!(sandbox_meta.get("useLegacyLandlock"), Some(&json!(false)));

    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdio_mcp_parallel_tool_calls_default_false_runs_serially() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let first_call_id = "sync-serial-1";
    let second_call_id = "sync-serial-2";
    let server_name = "rmcp";
    let namespace = format!("mcp__{server_name}__");
    let args = json!({ "sleep_after_ms": 100 }).to_string();

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(first_call_id, &namespace, "sync", &args),
            responses::ev_function_call_with_namespace(second_call_id, &namespace, "sync", &args),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp sync tools completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(rmcp_test_server_bin, /*env*/ None, Vec::new()),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    tool_timeout_sec: Some(Duration::from_secs(2)),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp sync tool twice".into(),
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

    let mut call_events = Vec::new();
    while call_events.len() < 4 {
        let event = wait_for_event(&fixture.codex, |ev| {
            matches!(
                ev,
                EventMsg::McpToolCallBegin(_) | EventMsg::McpToolCallEnd(_)
            )
        })
        .await;
        match event {
            EventMsg::McpToolCallBegin(begin) => {
                call_events.push(McpCallEvent::Begin(begin.call_id));
            }
            EventMsg::McpToolCallEnd(end) => {
                call_events.push(McpCallEvent::End(end.call_id));
            }
            _ => unreachable!("event guard guarantees MCP call events"),
        }
    }

    let event_index = |needle: McpCallEvent| {
        call_events
            .iter()
            .position(|event| event == &needle)
            .expect("expected MCP call event")
    };
    let first_begin = event_index(McpCallEvent::Begin(first_call_id.to_string()));
    let first_end = event_index(McpCallEvent::End(first_call_id.to_string()));
    let second_begin = event_index(McpCallEvent::Begin(second_call_id.to_string()));
    let second_end = event_index(McpCallEvent::End(second_call_id.to_string()));
    assert!(
        first_end < second_begin || second_end < first_begin,
        "default MCP tool calls should run serially; saw events: {call_events:?}"
    );

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = final_mock.single_request();
    for call_id in [first_call_id, second_call_id] {
        let output_text = request
            .function_call_output_text(call_id)
            .expect("function_call_output present for rmcp sync call");
        let wrapped_payload = split_wall_time_wrapped_output(&output_text);
        let output_json: Value = serde_json::from_str(wrapped_payload)
            .expect("wrapped MCP output should preserve structured JSON");
        assert_eq!(output_json, json!({ "result": "ok" }));
    }

    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stdio_mcp_parallel_tool_calls_opt_in_runs_concurrently() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let first_call_id = "sync-1";
    let second_call_id = "sync-2";
    let server_name = "rmcp";
    let namespace = format!("mcp__{server_name}__");
    let args = json!({
        "sleep_after_ms": 100,
        "barrier": {
            "id": "stdio-mcp-parallel-tool-calls",
            "participants": 2,
            "timeout_ms": 1_000
        }
    })
    .to_string();

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(first_call_id, &namespace, "sync", &args),
            responses::ev_function_call_with_namespace(second_call_id, &namespace, "sync", &args),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp sync tools completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(rmcp_test_server_bin, /*env*/ None, Vec::new()),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    supports_parallel_tool_calls: true,
                    tool_timeout_sec: Some(Duration::from_secs(2)),
                },
            );
        })
        .build_remote_aware(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp sync tool twice".into(),
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

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = final_mock.single_request();
    for call_id in [first_call_id, second_call_id] {
        let output_text = request
            .function_call_output_text(call_id)
            .expect("function_call_output present for rmcp sync call");
        let wrapped_payload = split_wall_time_wrapped_output(&output_text);
        let output_json: Value = serde_json::from_str(wrapped_payload)
            .expect("wrapped MCP output should preserve structured JSON");
        assert_eq!(output_json, json!({ "result": "ok" }));
    }

    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn stdio_image_responses_round_trip() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "img-1";
    let server_name = "rmcp";
    let tool_name = format!("mcp__{server_name}__image");
    let namespace = format!("mcp__{server_name}__");

    // First stream: model decides to call the image tool.
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(call_id, &namespace, "image", "{}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    // Second stream: after tool execution, assistant emits a message and completes.
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp image tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    // Build the stdio rmcp server and pass the image as data URL so it can construct ImageContent.
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(
                    rmcp_test_server_bin,
                    Some(HashMap::from([(
                        "MCP_TEST_IMAGE_DATA_URL".to_string(),
                        OPENAI_PNG.to_string(),
                    )])),
                    Vec::new(),
                ),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    wait_for_mcp_tool(&fixture, &tool_name).await?;

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp image tool".into(),
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

    // Wait for tool begin/end and final completion.
    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;
    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("begin");
    };
    assert_eq!(
        begin,
        McpToolCallBeginEvent {
            call_id: call_id.to_string(),
            invocation: McpInvocation {
                server: server_name.to_string(),
                tool: "image".to_string(),
                arguments: Some(json!({})),
            },
            mcp_app_resource_uri: None,
        },
    );

    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("end");
    };
    assert_eq!(end.call_id, call_id);
    assert_eq!(
        end.invocation,
        McpInvocation {
            server: server_name.to_string(),
            tool: "image".to_string(),
            arguments: Some(json!({})),
        }
    );
    let result = end.result.expect("rmcp image tool should return success");
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.content.len(), 1);
    let base64_only = OPENAI_PNG
        .strip_prefix("data:image/png;base64,")
        .expect("data url prefix");
    let entry = result.content[0].as_object().expect("content object");
    assert_eq!(entry.get("type"), Some(&json!("image")));
    assert_eq!(entry.get("mimeType"), Some(&json!("image/png")));
    assert_eq!(entry.get("data"), Some(&json!(base64_only)));

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let output_item = final_mock.single_request().function_call_output(call_id);
    assert_eq!(output_item["type"], "function_call_output");
    assert_eq!(output_item["call_id"], call_id);
    let output = output_item["output"]
        .as_array()
        .expect("image MCP output should be content items");
    assert_eq!(output.len(), 2);
    assert_wall_time_header(
        output[0]["text"]
            .as_str()
            .expect("first MCP image output item should be wall-time text"),
    );
    assert_eq!(
        output[1],
        json!({
            "type": "input_image",
            "image_url": OPENAI_PNG,
            "detail": "high"
        })
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn stdio_image_responses_preserve_original_detail_metadata() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "img-original-detail-1";
    let server_name = "rmcp";
    let tool_name = format!("mcp__{server_name}__image_scenario");
    let namespace = format!("mcp__{server_name}__");

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                "image_scenario",
                r#"{"scenario":"image_only_original_detail"}"#,
            ),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp original-detail image completed."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_model("gpt-5.3-codex")
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(rmcp_test_server_bin, /*env*/ None, Vec::new()),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    wait_for_mcp_tool(&fixture, &tool_name).await?;

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp image_scenario tool".into(),
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

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let output_item = final_mock.single_request().function_call_output(call_id);
    let output = output_item["output"]
        .as_array()
        .expect("image MCP output should be content items");
    assert_eq!(output.len(), 2);
    assert_wall_time_header(
        output[0]["text"]
            .as_str()
            .expect("first MCP image output item should be wall-time text"),
    );
    assert_eq!(
        output[1],
        json!({
            "type": "input_image",
            "image_url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
            "detail": "original",
        })
    );

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn js_repl_emit_image_preserves_original_detail_for_mcp_images() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let call_id = "js-repl-rmcp-image";
    let rmcp_test_server_bin = stdio_server_bin()?;

    let fixture = test_codex()
        .with_model("gpt-5.3-codex")
        .with_config(move |config| {
            config
                .features
                .enable(Feature::JsRepl)
                .expect("test config should allow feature update");
            insert_mcp_server(
                config,
                "rmcp",
                stdio_transport(rmcp_test_server_bin, /*env*/ None, Vec::new()),
                TestMcpServerOptions::default(),
            );
        })
        .build(&server)
        .await?;

    wait_for_mcp_tool(&fixture, "mcp__rmcp__image_scenario").await?;

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            ev_custom_tool_call(
                call_id,
                "js_repl",
                r#"
const out = await codex.tool("mcp__rmcp__image_scenario", {
  scenario: "image_only_original_detail",
});
const imageItem = out.output.find((item) => item.type === "input_image");
await codex.emitImage(imageItem);
"#,
            ),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "done"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn("use js_repl to emit the rmcp image scenario output")
        .await?;

    let output = final_mock.single_request().custom_tool_call_output(call_id);
    let output_items = output["output"]
        .as_array()
        .expect("js_repl output should be content items");
    let image_item = output_items
        .iter()
        .find(|item| item.get("type").and_then(Value::as_str) == Some("input_image"))
        .expect("js_repl should emit an input_image item");
    assert_eq!(
        image_item.get("detail").and_then(Value::as_str),
        Some("original")
    );
    assert!(
        image_item
            .get("image_url")
            .and_then(Value::as_str)
            .is_some_and(|image_url| image_url.starts_with("data:image/png;base64,")),
        "js_repl should emit a png data URL"
    );

    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn stdio_image_responses_are_sanitized_for_text_only_model() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "img-text-only-1";
    let server_name = "rmcp";
    let namespace = format!("mcp__{server_name}__");
    let text_only_model_slug = "rmcp-text-only-model";

    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![ModelInfo {
                slug: text_only_model_slug.to_string(),
                display_name: "RMCP Text Only".to_string(),
                description: Some("Test model without image input support".to_string()),
                default_reasoning_level: None,
                supported_reasoning_levels: vec![ReasoningEffortPreset {
                    effort: codex_protocol::openai_models::ReasoningEffort::Medium,
                    description: "Medium".to_string(),
                }],
                shell_type: ConfigShellToolType::Default,
                visibility: ModelVisibility::List,
                supported_in_api: true,
                priority: 1,
                additional_speed_tiers: Vec::new(),
                upgrade: None,
                base_instructions: "base instructions".to_string(),
                model_messages: None,
                supports_reasoning_summaries: false,
                default_reasoning_summary: ReasoningSummary::Auto,
                support_verbosity: false,
                default_verbosity: None,
                availability_nux: None,
                apply_patch_tool_type: None,
                web_search_tool_type: Default::default(),
                truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
                supports_parallel_tool_calls: false,
                supports_image_detail_original: false,
                context_window: Some(272_000),
                max_context_window: None,
                auto_compact_token_limit: None,
                effective_context_window_percent: 95,
                experimental_supported_tools: Vec::new(),
                input_modalities: vec![InputModality::Text],
                used_fallback_model_metadata: false,
                supports_search_tool: false,
            }],
        },
    )
    .await;

    // First stream: model decides to call the image tool.
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(call_id, &namespace, "image", "{}"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    // Second stream: after tool execution, assistant emits a message and completes.
    let final_mock = mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp image tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(
                    rmcp_test_server_bin,
                    Some(HashMap::from([(
                        "MCP_TEST_IMAGE_DATA_URL".to_string(),
                        OPENAI_PNG.to_string(),
                    )])),
                    Vec::new(),
                ),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;

    fixture
        .thread_manager
        .get_models_manager()
        .list_models(RefreshStrategy::Online)
        .await;
    assert_eq!(models_mock.requests().len(), 1);

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp image tool".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: fixture.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: text_only_model_slug.to_string(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
            environments: None,
        })
        .await?;

    wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;
    wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let output_item = final_mock.single_request().function_call_output(call_id);
    let output_text = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("function_call_output output should be a JSON string");
    let wrapped_payload = split_wall_time_wrapped_output(output_text);
    let output_json: Value = serde_json::from_str(wrapped_payload)
        .expect("function_call_output output should be valid JSON");
    assert_eq!(
        output_json,
        json!([{
            "type": "text",
            "text": "<image content omitted because you do not support image input>"
        }])
    );
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_test_value)]
async fn stdio_server_propagates_whitelisted_env_vars() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;

    let call_id = "call-1234";
    let server_name = "rmcp_whitelist";
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
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let expected_env_value = "propagated-env-from-whitelist";
    let _guard = EnvVarGuard::set("MCP_TEST_VALUE", OsStr::new(expected_env_value));
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(
                    rmcp_test_server_bin,
                    /*env*/ None,
                    vec!["MCP_TEST_VALUE".into()],
                ),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;
    let session_model = fixture.session_configured.model.clone();

    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp echo tool".into(),
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

    let begin_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;

    let EventMsg::McpToolCallBegin(begin) = begin_event else {
        unreachable!("event guard guarantees McpToolCallBegin");
    };
    assert_eq!(begin.invocation.server, server_name);
    assert_eq!(begin.invocation.tool, "echo");

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

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    server.verify().await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_env_source)]
async fn stdio_server_propagates_explicit_local_env_var_source() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let call_id = "call-local-source";
    let server_name = "rmcp_local_source";
    let namespace = format!("mcp__{server_name}__");
    let env_name = "MCP_TEST_LOCAL_SOURCE";
    let expected_env_value = "propagated-explicit-local-source";

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                "echo",
                &format!(r#"{{"message":"ping","env_var":"{env_name}"}}"#),
            ),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let _guard = EnvVarGuard::set(env_name, OsStr::new(expected_env_value));
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(
                    rmcp_test_server_bin,
                    /*env*/ None,
                    vec![McpServerEnvVar::Config {
                        name: env_name.to_string(),
                        source: Some("local".to_string()),
                    }],
                ),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;

    let session_model = fixture.session_configured.model.clone();
    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp echo tool".into(),
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

    wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;
    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };
    let structured = end
        .result
        .as_ref()
        .expect("rmcp echo tool should return success")
        .structured_content
        .as_ref()
        .expect("structured content");
    assert_eq!(structured["env"], expected_env_value);

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    server.verify().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[serial(mcp_env_source)]
async fn remote_stdio_env_var_source_does_not_copy_local_env() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    if std::env::var_os(remote_env_env_var()).is_none() {
        return Ok(());
    }

    let server = responses::start_mock_server().await;
    let call_id = "call-remote-source";
    let server_name = "rmcp_remote_source";
    let namespace = format!("mcp__{server_name}__");
    let env_name = "MCP_TEST_REMOTE_SOURCE_ONLY";

    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_function_call_with_namespace(
                call_id,
                &namespace,
                "echo",
                &format!(r#"{{"message":"ping","env_var":"{env_name}"}}"#),
            ),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "rmcp echo tool completed successfully."),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;

    let _guard = EnvVarGuard::set(env_name, OsStr::new("local-value-should-not-cross"));
    let rmcp_test_server_bin = remote_aware_stdio_server_bin()?;

    let fixture = test_codex()
        .with_config(move |config| {
            insert_mcp_server(
                config,
                server_name,
                stdio_transport(
                    rmcp_test_server_bin,
                    /*env*/ None,
                    vec![McpServerEnvVar::Config {
                        name: env_name.to_string(),
                        source: Some("remote".to_string()),
                    }],
                ),
                TestMcpServerOptions {
                    experimental_environment: remote_aware_experimental_environment(),
                    ..Default::default()
                },
            );
        })
        .build_remote_aware(&server)
        .await?;

    let session_model = fixture.session_configured.model.clone();
    fixture
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "call the rmcp echo tool".into(),
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

    wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallBegin(_))
    })
    .await;
    let end_event = wait_for_event(&fixture.codex, |ev| {
        matches!(ev, EventMsg::McpToolCallEnd(_))
    })
    .await;
    let EventMsg::McpToolCallEnd(end) = end_event else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };
    let structured = end
        .result
        .as_ref()
        .expect("rmcp echo tool should return success")
        .structured_content
        .as_ref()
        .expect("structured content");
    assert_eq!(structured["env"], Value::Null);

    wait_for_event(&fixture.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    server.verify().await;
    Ok(())
}

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

fn write_fallback_oauth_tokens(
    home: &Path,
    server_name: &str,
    server_url: &str,
    client_id: &str,
    access_token: &str,
    refresh_token: &str,
) -> anyhow::Result<()> {
    let expires_at = SystemTime::now()
        .checked_add(Duration::from_secs(3600))
        .ok_or_else(|| anyhow::anyhow!("failed to compute expiry time"))?
        .duration_since(UNIX_EPOCH)?
        .as_millis() as u64;

    let store = serde_json::json!({
        "stub": {
            "server_name": server_name,
            "server_url": server_url,
            "client_id": client_id,
            "access_token": access_token,
            "expires_at": expires_at,
            "refresh_token": refresh_token,
            "scopes": ["profile"],
        }
    });

    let file_path = home.join(".credentials.json");
    fs::write(&file_path, serde_json::to_vec(&store)?)?;
    Ok(())
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let original = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
