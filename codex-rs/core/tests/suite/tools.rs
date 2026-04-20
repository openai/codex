#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_config::Constrained;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::sandboxing::SandboxPermissions;
use codex_features::Feature;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use serial_test::serial;
use tempfile::TempDir;

fn tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn ev_namespaced_function_call(
    call_id: &str,
    namespace: &str,
    name: &str,
    arguments: &str,
) -> Value {
    json!({
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "call_id": call_id,
            "namespace": namespace,
            "name": name,
            "arguments": arguments,
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_tool_unknown_returns_custom_output_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build(&server).await?;

    let call_id = "custom-unsupported";
    let tool_name = "unsupported_tool";

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(call_id, tool_name, "\"payload\""),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn_with_policies(
        "invoke custom tool",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let item = mock.single_request().custom_tool_call_output(call_id);
    let output = item
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected = format!("unsupported custom tool call: {tool_name}");
    assert_eq!(output, expected);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial(mcp_test_value)]
async fn historical_unavailable_mcp_call_is_exposed_as_placeholder_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let historical_call_id = "historical-mcp-call";
    let retry_call_id = "retry-mcp-call";
    let server_name = "rmcp";
    let unavailable_tool_namespace = "mcp__rmcp__";
    let unavailable_tool_name = "echo";
    let unavailable_tool_display_name = "mcp__rmcp__echo";
    let server = start_mock_server().await;
    let rmcp_test_server_bin = match stdio_server_bin() {
        Ok(bin) => bin,
        Err(err) => {
            eprintln!("test_stdio_server binary not available, skipping test: {err}");
            return Ok(());
        }
    };
    let codex_home = Arc::new(TempDir::new()?);
    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_home(Arc::clone(&codex_home))
        .with_config(move |config| {
            config
                .features
                .enable(Feature::UnavailableDummyTools)
                .expect("unavailable dummy tools should be enabled for this test");
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                server_name.to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::Stdio {
                        command: rmcp_test_server_bin,
                        args: Vec::new(),
                        env: Some(HashMap::new()),
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    experimental_environment: None,
                    enabled: true,
                    required: false,
                    supports_parallel_tool_calls: false,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                    default_tools_approval_mode: None,
                    enabled_tools: None,
                    disabled_tools: None,
                    scopes: None,
                    oauth_resource: None,
                    tools: HashMap::new(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test mcp servers should accept any configuration");
        });
    let test = builder.build(&server).await?;

    let first_turn_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_namespaced_function_call(
                    historical_call_id,
                    unavailable_tool_namespace,
                    unavailable_tool_name,
                    r#"{"message":"ping"}"#,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "rmcp echo tool completed"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn("call the rmcp echo tool").await?;
    let rollout_path = test.codex.rollout_path().context("rollout path")?;
    assert_eq!(first_turn_mock.requests().len(), 2);
    drop(test);

    let retry_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-3"),
                ev_namespaced_function_call(
                    retry_call_id,
                    unavailable_tool_namespace,
                    unavailable_tool_name,
                    r#"{"message":"ping again"}"#,
                ),
                ev_completed("resp-3"),
            ]),
            sse(vec![
                ev_response_created("resp-4"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-4"),
            ]),
        ],
    )
    .await;

    let mut resume_builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .features
            .enable(Feature::UnavailableDummyTools)
            .expect("unavailable dummy tools should be enabled for this test");
    });
    let test = resume_builder
        .resume(&server, codex_home, rollout_path)
        .await?;

    test.submit_turn("retry the rmcp echo tool").await?;

    let requests = retry_mock.requests();
    assert_eq!(requests.len(), 2);
    let first_request_tools = tool_names(&requests[0].body_json());
    assert!(
        first_request_tools
            .iter()
            .any(|name| name == unavailable_tool_display_name),
        "historical unavailable MCP call should add a placeholder tool; got {first_request_tools:?}"
    );
    let output_text = requests[1]
        .function_call_output_text(retry_call_id)
        .context("placeholder tool output present")?;
    assert!(output_text.contains("not currently available"));
    assert!(
        !output_text.contains("unsupported call"),
        "placeholder handler should answer instead of falling back to unsupported call: {output_text}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_escalated_permissions_rejected_then_ok() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-shell-json");
    let test = builder.build(&server).await?;

    let command = ["/bin/echo", "shell ok"];
    let call_id_blocked = "shell-blocked";
    let call_id_success = "shell-success";

    let first_args = json!({
        "command": command,
        "timeout_ms": 1_000,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
    });
    let second_args = json!({
        "command": command,
        "timeout_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                call_id_blocked,
                "shell",
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                call_id_success,
                "shell",
                &serde_json::to_string(&second_args)?,
            ),
            ev_completed("resp-2"),
        ]),
    )
    .await;
    let third_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-3"),
        ]),
    )
    .await;

    test.submit_turn_with_policies(
        "run the shell command",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let policy = AskForApproval::Never;
    let expected_message = format!(
        "approval policy is {policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {policy:?}"
    );

    let blocked_output = second_mock
        .single_request()
        .function_call_output_content_and_success(call_id_blocked)
        .and_then(|(content, _)| content)
        .expect("blocked output string");
    assert_eq!(
        blocked_output, expected_message,
        "unexpected rejection message"
    );

    let success_output = third_mock
        .single_request()
        .function_call_output_content_and_success(call_id_success)
        .and_then(|(content, _)| content)
        .expect("success output string");
    let output_json: Value = serde_json::from_str(&success_output)?;
    assert_eq!(
        output_json["metadata"]["exit_code"].as_i64(),
        Some(0),
        "expected exit code 0 after rerunning without escalation",
    );
    let stdout = output_json["output"].as_str().unwrap_or_default();
    let stdout_pattern = r"(?s)^shell ok\n?$";
    assert_regex_match(stdout_pattern, stdout);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sandbox_denied_shell_returns_original_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4");
    let fixture = builder.build(&server).await?;

    let call_id = "sandbox-denied-shell";
    let target_path = fixture.workspace_path("sandbox-denied.txt");
    let sentinel = "sandbox-denied sentinel output";
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        format!(
            "printf {sentinel:?}; printf {content:?} > {path:?}",
            sentinel = format!("{sentinel}\n"),
            content = "sandbox denied",
            path = &target_path
        ),
    ];
    let args = json!({
        "command": command,
        "timeout_ms": 5_000,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    fixture
        .submit_turn_with_policy(
            "run a command that should be denied by the read-only sandbox",
            SandboxPolicy::new_read_only_policy(),
        )
        .await?;

    let output_text = mock
        .function_call_output_text(call_id)
        .context("shell output present")?;
    let exit_code_line = output_text
        .lines()
        .next()
        .context("exit code line present")?;
    let exit_code = exit_code_line
        .strip_prefix("Exit code: ")
        .context("exit code prefix present")?
        .trim()
        .parse::<i32>()
        .context("exit code is integer")?;
    let body = output_text;

    let body_lower = body.to_lowercase();
    // Required for multi-OS.
    let has_denial = body_lower.contains("permission denied")
        || body_lower.contains("operation not permitted")
        || body_lower.contains("read-only file system");
    assert!(
        has_denial,
        "expected sandbox denial details in tool output: {body}"
    );
    assert!(
        body.contains(sentinel),
        "expected sentinel output from command to reach the model: {body}"
    );
    let target_path_str = target_path
        .to_str()
        .context("target path string representation")?;
    assert!(
        body.contains(target_path_str),
        "expected sandbox error to mention denied path: {body}"
    );
    assert!(
        !body_lower.contains("failed in sandbox"),
        "expected original tool output, found fallback message: {body}"
    );
    assert_ne!(
        exit_code, 0,
        "sandbox denial should surface a non-zero exit code"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_enforces_glob_deny_read_policy() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let read_only_policy = SandboxPolicy::new_read_only_policy();
    let read_only_policy_for_config = read_only_policy.clone();
    let mut builder = test_codex()
        .with_model("gpt-5.4")
        .with_config(move |config| {
            config.permissions.sandbox_policy = Constrained::allow_any(read_only_policy_for_config);
            let mut file_system_sandbox_policy = FileSystemSandboxPolicy::default();
            file_system_sandbox_policy
                .entries
                .push(FileSystemSandboxEntry {
                    path: FileSystemPath::GlobPattern {
                        pattern: format!("{}/**/*.env", config.cwd.as_path().display()),
                    },
                    access: FileSystemAccessMode::None,
                });
            config.permissions.file_system_sandbox_policy = file_system_sandbox_policy;
        });
    let fixture = builder.build(&server).await?;

    let fixture_dir = fixture.workspace_path("glob-deny-read");
    fs::create_dir_all(&fixture_dir).context("create glob deny-read fixture directory")?;
    let denied_path = fixture_dir.join("secret.env");
    let allowed_path = fixture_dir.join("notes.txt");
    let secret = "shell glob deny-read secret";
    let allowed = "shell glob deny-read allowed";
    fs::write(&denied_path, format!("{secret}\n")).context("write denied fixture")?;
    fs::write(&allowed_path, format!("{allowed}\n")).context("write allowed fixture")?;

    let call_id = "shell-glob-deny-read";
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "status=0; cat \"$1\" || status=$?; cat \"$2\"; exit \"$status\"".to_string(),
        "sh".to_string(),
        denied_path.to_string_lossy().into_owned(),
        allowed_path.to_string_lossy().into_owned(),
    ];
    let args = json!({
        "command": command,
        "timeout_ms": 1_000,
    });

    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    fixture
        .submit_turn_with_policy("read the fixture files", read_only_policy)
        .await?;

    let output_text = mock
        .function_call_output_text(call_id)
        .context("shell output present")?;
    let exit_code_line = output_text
        .lines()
        .next()
        .context("exit code line present")?;
    let exit_code = exit_code_line
        .strip_prefix("Exit code: ")
        .context("exit code prefix present")?
        .trim()
        .parse::<i32>()
        .context("exit code is integer")?;

    assert_ne!(
        exit_code, 0,
        "glob deny-read should surface a non-zero exit code"
    );
    assert!(
        output_text.contains(allowed),
        "expected allowed file contents in shell output: {output_text}"
    );
    assert!(
        !output_text.contains(secret),
        "denied file contents leaked into shell output: {output_text}"
    );
    let output_lower = output_text.to_lowercase();
    let has_denial = output_lower.contains("permission denied")
        || output_lower.contains("operation not permitted")
        || output_lower.contains("read-only file system");
    assert!(
        has_denial,
        "expected sandbox denial details in shell output: {output_text}"
    );

    Ok(())
}

async fn collect_tools(use_unified_exec: bool) -> Result<Vec<String>> {
    let server = start_mock_server().await;

    let responses = vec![sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-1"),
    ])];
    let mock = mount_sse_sequence(&server, responses).await;

    let mut builder = test_codex().with_config(move |config| {
        if use_unified_exec {
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        } else {
            config
                .features
                .disable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
        }
    });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let first_body = mock.single_request().body_json();
    Ok(tool_names(&first_body))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_spec_toggle_end_to_end() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tools_disabled = collect_tools(/*use_unified_exec*/ false).await?;
    assert!(
        !tools_disabled.iter().any(|name| name == "exec_command"),
        "tools list should not include exec_command when disabled: {tools_disabled:?}"
    );
    assert!(
        !tools_disabled.iter().any(|name| name == "write_stdin"),
        "tools list should not include write_stdin when disabled: {tools_disabled:?}"
    );

    let tools_enabled = collect_tools(/*use_unified_exec*/ true).await?;
    assert!(
        tools_enabled.iter().any(|name| name == "exec_command"),
        "tools list should include exec_command when enabled: {tools_enabled:?}"
    );
    assert!(
        tools_enabled.iter().any(|name| name == "write_stdin"),
        "tools list should include write_stdin when enabled: {tools_enabled:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_timeout_includes_timeout_prefix_and_metadata() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-shell-json");
    let test = builder.build(&server).await?;

    let call_id = "shell-timeout";
    let timeout_ms = 50u64;
    let args = json!({
        "command": ["/bin/sh", "-c", "yes line | head -n 400; sleep 1"],
        "timeout_ms": timeout_ms,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn_with_policies(
        "run a long command",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let timeout_item = second_mock.single_request().function_call_output(call_id);

    let output_str = timeout_item
        .get("output")
        .and_then(Value::as_str)
        .expect("timeout output string");

    // The exec path can report a timeout in two ways depending on timing:
    // 1) Structured JSON with exit_code 124 and a timeout prefix (preferred), or
    // 2) A plain error string if the child is observed as killed by a signal first.
    if let Ok(output_json) = serde_json::from_str::<Value>(output_str) {
        assert_eq!(
            output_json["metadata"]["exit_code"].as_i64(),
            Some(124),
            "expected timeout exit code 124",
        );

        let stdout = output_json["output"].as_str().unwrap_or_default();
        assert!(
            stdout.contains("command timed out"),
            "timeout output missing `command timed out`: {stdout}"
        );
    } else {
        // Fallback: accept the signal classification path to deflake the test.
        let signal_pattern = r"(?is)^execution error:.*signal.*$";
        assert_regex_match(signal_pattern, output_str);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_timeout_handles_background_grandchild_stdout() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4").with_config(|config| {
        config
            .permissions
            .sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("set sandbox policy");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-grandchild-timeout";
    let pid_path = test.cwd.path().join("grandchild_pid.txt");
    let script_path = test.cwd.path().join("spawn_detached.py");
    let script = format!(
        r#"import subprocess
import time
from pathlib import Path

# Spawn a detached grandchild that inherits stdout/stderr so the pipe stays open.
proc = subprocess.Popen(["/bin/sh", "-c", "sleep 60"], start_new_session=True)
Path({pid_path:?}).write_text(str(proc.pid))
time.sleep(60)
"#
    );
    fs::write(&script_path, script)?;

    let args = json!({
        "command": ["python3", script_path.to_string_lossy()],
        "timeout_ms": 200,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    let start = Instant::now();
    let output_str = tokio::time::timeout(Duration::from_secs(10), async {
        test.submit_turn_with_policies(
            "run a command with a detached grandchild",
            AskForApproval::Never,
            SandboxPolicy::DangerFullAccess,
        )
        .await?;
        let timeout_item = second_mock.single_request().function_call_output(call_id);
        timeout_item
            .get("output")
            .and_then(Value::as_str)
            .map(str::to_string)
            .context("timeout output string")
    })
    .await
    .context("exec call should not hang waiting for grandchild pipes to close")??;
    let elapsed = start.elapsed();

    if let Ok(output_json) = serde_json::from_str::<Value>(&output_str) {
        assert_eq!(
            output_json["metadata"]["exit_code"].as_i64(),
            Some(124),
            "expected timeout exit code 124",
        );
    } else {
        let timeout_pattern = r"(?is)command timed out|timeout";
        assert_regex_match(timeout_pattern, &output_str);
    }

    assert!(
        elapsed < Duration::from_secs(9),
        "command should return shortly after timeout even with live grandchildren: {elapsed:?}"
    );

    if let Ok(pid_str) = fs::read_to_string(&pid_path)
        && let Ok(pid) = pid_str.trim().parse::<libc::pid_t>()
    {
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_spawn_failure_truncates_exec_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|cfg| {
        cfg.permissions
            .sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("set sandbox policy");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-spawn-failure";
    let bogus_component = "missing-bin-".repeat(700);
    let bogus_exe = test
        .cwd
        .path()
        .join(bogus_component)
        .to_string_lossy()
        .into_owned();

    let args = json!({
        "command": [bogus_exe],
        "timeout_ms": 1_000,
    });

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn_with_policies(
        "spawn a missing binary",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let failure_item = second_mock.single_request().function_call_output(call_id);

    let output = failure_item
        .get("output")
        .and_then(Value::as_str)
        .expect("spawn failure output string");

    let spawn_error_pattern = r#"(?s)^Exit code: -?\d+
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
execution error: .*$"#;
    let spawn_truncated_pattern = r#"(?s)^Exit code: -?\d+
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Total output lines: \d+
Output:

execution error: .*$"#;
    let spawn_error_regex = Regex::new(spawn_error_pattern)?;
    let spawn_truncated_regex = Regex::new(spawn_truncated_pattern)?;
    if !spawn_error_regex.is_match(output) && !spawn_truncated_regex.is_match(output) {
        let fallback_pattern = r"(?s)^execution error: .*$";
        assert_regex_match(fallback_pattern, output);
    }
    assert!(output.len() <= 10 * 1024);

    Ok(())
}
