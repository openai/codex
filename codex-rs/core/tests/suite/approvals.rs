#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::time::Duration;

async fn submit_turn(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();

    test.codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: prompt.into(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy,
            sandbox_policy,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    Ok(())
}

fn env_output_from_item(item: &Value) -> String {
    let output_str = item
        .get("output")
        .and_then(Value::as_str)
        .expect("output payload should be a string");
    match serde_json::from_str::<Value>(output_str) {
        Ok(parsed) => parsed
            .get("output")
            .and_then(Value::as_str)
            .expect("structured exec output text")
            .to_string(),
        Err(_) => output_str.to_string(),
    }
}

fn strict_workspace_write_policy(network_access: bool) -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

async fn wait_for_completion_or_approval(test: &TestCodex) -> bool {
    loop {
        let event =
            wait_for_event_with_timeout(&test.codex, |_| true, Duration::from_secs(5)).await;
        match event {
            EventMsg::ExecApprovalRequest(_) => return true,
            EventMsg::TaskComplete(_) => return false,
            _ => {}
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn danger_full_access_on_request_runs_without_network_restrictions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = SandboxPolicy::DangerFullAccess;
        config.model = "gpt-5".to_string();
        config.model_family =
            find_family_for_model("gpt-5").expect("gpt-5 should map to a known family");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-env-danger";
    let command = vec!["/bin/sh", "-c", "env | sort"];
    let args = json!({
        "command": command,
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
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "inspect env",
        AskForApproval::OnRequest,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TaskComplete(_))
    })
    .await;

    let output = results_mock.single_request().function_call_output(call_id);
    let env_output = env_output_from_item(&output);

    if env_output.contains("LandlockRestrict") {
        eprintln!("skipping test because Linux sandbox is unavailable: {env_output:?}");
        return Ok(());
    }

    assert!(
        !env_output.contains("CODEX_SANDBOX_NETWORK_DISABLED"),
        "DangerFullAccess should not inject sandbox network flag: {env_output:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn readonly_policy_auto_approves_under_sandbox() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = SandboxPolicy::ReadOnly;
        config.model = "gpt-5".to_string();
        config.model_family =
            find_family_for_model("gpt-5").expect("gpt-5 should map to a known family");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-env-sandbox";
    let command = vec!["/bin/sh", "-c", "env | sort"];
    let args = json!({
        "command": command,
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
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "inspect env",
        AskForApproval::OnRequest,
        SandboxPolicy::ReadOnly,
    )
    .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TaskComplete(_))
    })
    .await;

    let output = results_mock.single_request().function_call_output(call_id);
    let env_output = env_output_from_item(&output);

    if env_output.contains("LandlockRestrict") {
        eprintln!("skipping test because Linux sandbox is unavailable: {env_output:?}");
        return Ok(());
    }

    assert!(
        env_output.contains("CODEX_SANDBOX_NETWORK_DISABLED=1"),
        "ReadOnly sandbox should disable network: {env_output:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn readonly_policy_escalation_requires_approval_and_runs_unsandboxed() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = SandboxPolicy::ReadOnly;
        config.model = "gpt-5".to_string();
        config.model_family =
            find_family_for_model("gpt-5").expect("gpt-5 should map to a known family");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-env-escalate";
    let command = vec!["/bin/sh", "-c", "env | sort"];
    let args = json!({
        "command": command,
        "timeout_ms": 1_000,
        "with_escalated_permissions": true,
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
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "inspect env",
        AskForApproval::OnRequest,
        SandboxPolicy::ReadOnly,
    )
    .await?;

    let approval_event = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ExecApprovalRequest(_))
    })
    .await;
    let approval_request = match approval_event {
        EventMsg::ExecApprovalRequest(event) => event,
        other => panic!("unexpected event: {other:?}"),
    };
    let expected_command: Vec<String> = command.iter().map(|value| value.to_string()).collect();
    assert_eq!(approval_request.command, expected_command);

    test.codex
        .submit(Op::ExecApproval {
            id: "0".into(),
            decision: codex_protocol::protocol::ReviewDecision::Approved,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TaskComplete(_))
    })
    .await;

    let output = results_mock.single_request().function_call_output(call_id);
    let env_output = env_output_from_item(&output);

    assert!(
        env_output.contains("CODEX_SANDBOX_NETWORK_DISABLED=1"),
        "Approved escalation should keep network disabled: {env_output:?}"
    );
    assert!(
        !env_output.contains("CODEX_SANDBOX="),
        "Approved escalation should bypass sandbox env var: {env_output:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn readonly_policy_write_attempt_returns_sandbox_error() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = SandboxPolicy::ReadOnly;
        config.model = "gpt-5".to_string();
        config.model_family =
            find_family_for_model("gpt-5").expect("gpt-5 should map to a known family");
    });
    let test = builder.build(&server).await?;

    let call_id = "readonly-write";
    let target_path = test.cwd.path().join("blocked.txt");
    let script = format!(
        "echo blocked > {} && cat {}",
        target_path.display(),
        target_path.display()
    );
    let command = vec!["/bin/sh".to_string(), "-c".to_string(), script];
    let args = json!({
        "command": command,
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
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "try to write a file",
        AskForApproval::OnRequest,
        SandboxPolicy::ReadOnly,
    )
    .await?;

    if wait_for_completion_or_approval(&test).await {
        eprintln!("skipping readonly write test because sandbox approval was required");
        return Ok(());
    }

    let output_item = results_mock.single_request().function_call_output(call_id);
    let output_str = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("readonly output string");

    let (exit_code, summary) = if let Ok(output_json) = serde_json::from_str::<Value>(output_str) {
        let exit_code = output_json["metadata"]["exit_code"]
            .as_i64()
            .unwrap_or_default();
        let summary = output_json
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        (exit_code, summary)
    } else {
        (-1, output_str.to_string())
    };

    assert!(
        exit_code != 0,
        "readonly write should fail inside sandbox: {output_str}"
    );
    assert!(
        summary.contains("failed in sandbox"),
        "readonly sandbox failures should be surfaced: {summary}"
    );
    assert!(
        !target_path.exists(),
        "readonly sandbox should prevent creating {}",
        target_path.display()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_policy_allows_workspace_writes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = strict_workspace_write_policy(false);
        config.model = "gpt-5".to_string();
        config.model_family =
            find_family_for_model("gpt-5").expect("gpt-5 should map to a known family");
    });
    let test = builder.build(&server).await?;

    let call_id = "workspace-write";
    let target_path = test.cwd.path().join("workspace.txt");
    let script = format!(
        "echo workspace ok > {} && cat {}",
        target_path.display(),
        target_path.display()
    );
    let command = vec!["/bin/sh".to_string(), "-c".to_string(), script];
    let args = json!({
        "command": command,
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
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "write to workspace",
        AskForApproval::OnRequest,
        strict_workspace_write_policy(false),
    )
    .await?;

    if wait_for_completion_or_approval(&test).await {
        eprintln!("skipping workspace write test because sandbox approval was required");
        return Ok(());
    }

    let output_item = results_mock.single_request().function_call_output(call_id);
    let output_str = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("workspace write output string");
    let stdout = if let Ok(output_json) = serde_json::from_str::<Value>(output_str) {
        assert_eq!(
            output_json["metadata"]["exit_code"].as_i64(),
            Some(0),
            "workspace write should succeed"
        );
        output_json["output"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    } else {
        output_str.to_string()
    };
    if stdout.contains("LandlockRestrict") {
        eprintln!("skipping workspace write test because Linux sandbox is unavailable: {stdout:?}");
        return Ok(());
    }
    assert!(
        stdout.contains("workspace ok"),
        "workspace write output missing expected text: {stdout}"
    );
    assert!(
        target_path.exists(),
        "workspace policy should allow writing {}",
        target_path.display()
    );
    let file_contents = fs::read_to_string(&target_path)?;
    assert!(
        file_contents.contains("workspace ok"),
        "workspace file contents not written correctly: {file_contents}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_policy_blocks_outside_workspace_writes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = strict_workspace_write_policy(false);
        config.model = "gpt-5".to_string();
        config.model_family =
            find_family_for_model("gpt-5").expect("gpt-5 should map to a known family");
    });
    let test = builder.build(&server).await?;

    let call_id = "workspace-outside-write";
    let outside_path = test
        .cwd
        .path()
        .parent()
        .expect("workspace should have parent")
        .join("outside.txt");
    let script = format!("echo outside > {}", outside_path.display());
    let command = vec!["/bin/sh".to_string(), "-c".to_string(), script];
    let args = json!({
        "command": command,
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
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "write outside workspace",
        AskForApproval::OnRequest,
        strict_workspace_write_policy(false),
    )
    .await?;

    if wait_for_completion_or_approval(&test).await {
        eprintln!("skipping outside write test because sandbox approval was required");
        return Ok(());
    }

    let output_item = results_mock.single_request().function_call_output(call_id);
    let output_str = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("outside write output string");

    let (exit_code, summary) = if let Ok(output_json) = serde_json::from_str::<Value>(output_str) {
        let exit_code = output_json["metadata"]["exit_code"]
            .as_i64()
            .unwrap_or_default();
        let summary = output_json
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        (exit_code, summary)
    } else {
        (-1, output_str.to_string())
    };

    assert!(
        exit_code != 0,
        "outside workspace writes should fail: {output_str}"
    );
    assert!(
        summary.contains("failed in sandbox"),
        "outside write failure should mention sandbox: {summary}"
    );
    assert!(
        !outside_path.exists(),
        "workspace sandbox should prevent creating {}",
        outside_path.display()
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_policy_without_network_access_disables_network() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = strict_workspace_write_policy(false);
        config.model = "gpt-5".to_string();
        config.model_family =
            find_family_for_model("gpt-5").expect("gpt-5 should map to a known family");
    });
    let test = builder.build(&server).await?;

    let call_id = "workspace-network-off";
    let command = vec!["/bin/sh", "-c", "env | sort"];
    let args = json!({
        "command": command,
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
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "inspect env",
        AskForApproval::OnRequest,
        strict_workspace_write_policy(false),
    )
    .await?;

    if wait_for_completion_or_approval(&test).await {
        eprintln!("skipping workspace network test because sandbox approval was required");
        return Ok(());
    }

    let output_item = results_mock.single_request().function_call_output(call_id);
    let env_output = env_output_from_item(&output_item);

    if env_output.contains("LandlockRestrict") {
        eprintln!(
            "skipping workspace network test because Linux sandbox is unavailable: {env_output:?}"
        );
        return Ok(());
    }

    assert!(
        env_output.contains("CODEX_SANDBOX_NETWORK_DISABLED=1"),
        "workspace write without network should disable networking: {env_output:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_write_policy_with_network_access_enables_network() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = strict_workspace_write_policy(true);
        config.model = "gpt-5".to_string();
        config.model_family =
            find_family_for_model("gpt-5").expect("gpt-5 should map to a known family");
    });
    let test = builder.build(&server).await?;

    let call_id = "workspace-network-on";
    let command = vec!["/bin/sh", "-c", "env | sort"];
    let args = json!({
        "command": command,
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
    let results_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "inspect env",
        AskForApproval::OnRequest,
        strict_workspace_write_policy(true),
    )
    .await?;

    if wait_for_completion_or_approval(&test).await {
        eprintln!("skipping workspace network enabled test because sandbox approval was required");
        return Ok(());
    }

    let output_item = results_mock.single_request().function_call_output(call_id);
    let env_output = env_output_from_item(&output_item);

    if env_output.contains("LandlockRestrict") {
        eprintln!(
            "skipping workspace network enabled test because Linux sandbox is unavailable: {env_output:?}"
        );
        return Ok(());
    }

    assert!(
        !env_output.contains("CODEX_SANDBOX_NETWORK_DISABLED"),
        "workspace write with network access should not disable networking: {env_output:?}"
    );

    Ok(())
}
