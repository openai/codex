#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;

use anyhow::Context;
use anyhow::Result;
use codex_core::features::Feature;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::sandboxing::SandboxPermissions;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use which::which;

fn skip_if_no_python3() -> bool {
    which("python3").is_err()
}

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

fn parse_structured_tool_output(output: &str) -> Result<Value> {
    serde_json::from_str(output).context("structured tool output should be valid json")
}

async fn collect_tools(enable_python_tool: bool) -> Result<Vec<String>> {
    let server = start_mock_server().await;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(move |config| {
        if enable_python_tool {
            config.features.enable(Feature::PythonTool);
        } else {
            config.features.disable(Feature::PythonTool);
        }
    });
    let test = builder.build(&server).await?;
    test.submit_turn("list tools").await?;

    let body = mock.single_request().body_json();
    Ok(tool_names(&body))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_spec_toggle_end_to_end() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let tools_disabled = collect_tools(false).await?;
    assert!(
        !tools_disabled.iter().any(|name| name == "python"),
        "tools list should not include python when disabled: {tools_disabled:?}"
    );

    let tools_enabled = collect_tools(true).await?;
    assert!(
        tools_enabled.iter().any(|name| name == "python"),
        "tools list should include python when enabled: {tools_enabled:?}"
    );
    for shell_tool in [
        "shell",
        "shell_command",
        "local_shell",
        "exec_command",
        "write_stdin",
    ] {
        assert!(
            !tools_enabled.iter().any(|name| name == shell_tool),
            "tools list should not include {shell_tool} when python_tool is enabled: {tools_enabled:?}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_appends_developer_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("hello").await?;

    let request = mock.single_request();
    let developer_messages = request.message_input_texts("developer");
    assert!(
        developer_messages
            .iter()
            .any(|msg| msg.contains("Python tool mode is enabled.")),
        "expected python tool developer instructions in prompt, got: {developer_messages:?}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_executes_code_and_returns_structured_output() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if skip_if_no_python3() {
        return Ok(());
    }

    let server = start_mock_server().await;
    let call_id = "python-basic";
    let args = json!({"code": "print('hi from python')"});
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "python", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_model("gpt-5").with_config(|config| {
        config.features.enable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("run python").await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("python output present")?;
    let output_json = parse_structured_tool_output(&output)?;
    assert_eq!(output_json["metadata"]["exit_code"].as_i64(), Some(0));
    let stdout = output_json["output"].as_str().unwrap_or_default();
    assert_regex_match(r"(?s)^hi from python\n?$", stdout);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_call_fails_when_feature_is_disabled() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "python-disabled";
    let args = json!({"code": "print('should not run')"});
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "python", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.disable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("run python while disabled").await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("python output present")?;
    assert_eq!(output, "unsupported call: python");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_passes_args_to_script() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if skip_if_no_python3() {
        return Ok(());
    }

    let server = start_mock_server().await;
    let call_id = "python-args";
    let args = json!({
        "code": "import sys; print('|'.join(sys.argv[1:]))",
        "args": ["alpha", "beta", "gamma"],
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "python", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("run python args").await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("python output present")?;
    let output_json = parse_structured_tool_output(&output)?;
    assert_eq!(output_json["metadata"]["exit_code"].as_i64(), Some(0));
    let stdout = output_json["output"].as_str().unwrap_or_default();
    assert_regex_match(r"(?s)^alpha\|beta\|gamma\n?$", stdout);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_respects_workdir() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if skip_if_no_python3() {
        return Ok(());
    }

    let server = start_mock_server().await;
    let call_id = "python-workdir";
    let args = json!({
        "code": "import os; print(os.getcwd())",
        "workdir": "nested/dir",
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "python", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    let nested_dir = test.workspace_path("nested/dir");
    fs::create_dir_all(&nested_dir)?;
    test.submit_turn("run python in workdir").await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("python output present")?;
    let output_json = parse_structured_tool_output(&output)?;
    assert_eq!(output_json["metadata"]["exit_code"].as_i64(), Some(0));
    let stdout = output_json["output"].as_str().unwrap_or_default();
    assert!(stdout.contains(nested_dir.to_string_lossy().as_ref()));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_reports_nonzero_exit_and_stderr() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if skip_if_no_python3() {
        return Ok(());
    }

    let server = start_mock_server().await;
    let call_id = "python-nonzero";
    let args = json!({
        "code": "import sys; sys.stderr.write('boom\\n'); raise SystemExit(7)",
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "python", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("run failing python").await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("python output present")?;
    let output_json = parse_structured_tool_output(&output)?;
    assert_eq!(output_json["metadata"]["exit_code"].as_i64(), Some(7));
    let stdout = output_json["output"].as_str().unwrap_or_default();
    assert!(stdout.contains("boom"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_escalation_rejected_with_never_approval() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if skip_if_no_python3() {
        return Ok(());
    }

    let server = start_mock_server().await;
    let call_id = "python-escalation";
    let args = json!({
        "code": "print('blocked')",
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "python", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("run escalated python").await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("python output present")?;
    let policy = AskForApproval::Never;
    let expected_message = format!(
        "approval policy is {policy:?}; reject command — you should not ask for escalated permissions if the approval policy is {policy:?}"
    );
    assert_eq!(output, expected_message);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_timeout_includes_timeout_message() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if skip_if_no_python3() {
        return Ok(());
    }

    let server = start_mock_server().await;
    let call_id = "python-timeout";
    let args = json!({
        "code": "import time; time.sleep(2); print('done')",
        "timeout_ms": 50,
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "python", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("run slow python").await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("python output present")?;
    if let Ok(output_json) = parse_structured_tool_output(&output) {
        assert_eq!(output_json["metadata"]["exit_code"].as_i64(), Some(124));
        let stdout = output_json["output"].as_str().unwrap_or_default();
        assert!(stdout.contains("command timed out"));
    } else {
        assert_regex_match(r"(?is)^execution error:.*signal.*$", &output);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_timeout_alias_matches_timeout_ms_behavior() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if skip_if_no_python3() {
        return Ok(());
    }

    let server = start_mock_server().await;
    let call_id = "python-timeout-alias";
    let args = json!({
        "code": "import time; time.sleep(2)",
        "timeout": 50,
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "python", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    test.submit_turn("run slow python with timeout alias")
        .await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("python output present")?;
    if let Ok(output_json) = parse_structured_tool_output(&output) {
        assert_eq!(output_json["metadata"]["exit_code"].as_i64(), Some(124));
    } else {
        assert_regex_match(r"(?is)^execution error:.*signal.*$", &output);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn python_tool_surfaces_sandbox_denial_output() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if skip_if_no_python3() {
        return Ok(());
    }

    let server = start_mock_server().await;
    let call_id = "python-sandbox-denied";
    let args = json!({
        "code": "from pathlib import Path\nPath('sandbox-denied-python.txt').write_text('blocked')",
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "python", &serde_json::to_string(&args)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::PythonTool);
    });
    let test = builder.build(&server).await?;
    test.submit_turn_with_policy("run denied python", SandboxPolicy::ReadOnly)
        .await?;

    let output = mock
        .function_call_output_text(call_id)
        .context("python output present")?;
    let output_json = parse_structured_tool_output(&output)?;
    let exit_code = output_json["metadata"]["exit_code"]
        .as_i64()
        .context("exit code should exist")?;
    assert_ne!(exit_code, 0);

    let body = output_json["output"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        body.contains("permission denied")
            || body.contains("operation not permitted")
            || body.contains("read-only file system"),
        "expected sandbox denial details in output: {output_json}"
    );

    Ok(())
}
