#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::features::Feature;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
#[cfg(target_os = "macos")]
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use std::fs;

struct CommandResult {
    exit_code: Option<i64>,
    stdout: String,
}

fn parse_result(item: &Value) -> CommandResult {
    let output_str = item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell output payload");
    match serde_json::from_str::<Value>(output_str) {
        Ok(parsed) => {
            let exit_code = parsed["metadata"]["exit_code"].as_i64();
            let stdout = parsed["output"].as_str().unwrap_or_default().to_string();
            CommandResult { exit_code, stdout }
        }
        Err(_) => {
            let structured = Regex::new(r"(?s)^Exit code:\s*(-?\d+).*?Output:\n(.*)$").unwrap();
            let regex =
                Regex::new(r"(?s)^.*?Process exited with code (\d+)\n.*?Output:\n(.*)$").unwrap();
            if let Some(captures) = structured.captures(output_str) {
                let exit_code = captures.get(1).unwrap().as_str().parse::<i64>().unwrap();
                let output = captures.get(2).unwrap().as_str();
                CommandResult {
                    exit_code: Some(exit_code),
                    stdout: output.to_string(),
                }
            } else if let Some(captures) = regex.captures(output_str) {
                let exit_code = captures.get(1).unwrap().as_str().parse::<i64>().unwrap();
                let output = captures.get(2).unwrap().as_str();
                CommandResult {
                    exit_code: Some(exit_code),
                    stdout: output.to_string(),
                }
            } else {
                CommandResult {
                    exit_code: None,
                    stdout: output_str.to_string(),
                }
            }
        }
    }
}

fn shell_event_with_request_permissions(
    call_id: &str,
    command: &str,
    additional_permissions: &PermissionProfile,
) -> Result<Value> {
    let args = json!({
        "command": command,
        "timeout_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::WithAdditionalPermissions,
        "additional_permissions": additional_permissions,
    });
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "shell_command", &args_str))
}

async fn submit_turn(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy,
            sandbox_policy,
            model: session_model,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    Ok(())
}

async fn wait_for_completion(test: &TestCodex) {
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
}

async fn expect_exec_approval(
    test: &TestCodex,
    expected_command: &str,
) -> ExecApprovalRequestEvent {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::ExecApprovalRequest(approval) => {
            let last_arg = approval
                .command
                .last()
                .map(String::as_str)
                .unwrap_or_default();
            assert_eq!(last_arg, expected_command);
            approval
        }
        EventMsg::TurnComplete(_) => panic!("expected approval request before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

fn workspace_write_excluding_tmp() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn with_additional_permissions_requires_approval_under_on_request() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config.features.enable(Feature::RequestPermissions);
    });
    let test = builder.build(&server).await?;

    let requested_write = test.workspace_path("requested-but-unused.txt");
    let _ = fs::remove_file(&requested_write);
    let call_id = "request_permissions_skip_approval";
    let command = "touch requested-but-unused.txt";
    let requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![requested_write.clone()]),
        }),
        ..Default::default()
    };
    let event = shell_event_with_request_permissions(call_id, command, &requested_permissions)?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            event,
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;
    let approval = expect_exec_approval(&test, command).await;
    assert_eq!(
        approval.additional_permissions,
        Some(requested_permissions.clone())
    );
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;
    wait_for_completion(&test).await;

    let result = parse_result(&results.single_request().function_call_output(call_id));
    assert!(
        result.exit_code.is_none() || result.exit_code == Some(0),
        "unexpected exit code/output: {:?} {}",
        result.exit_code,
        result.stdout
    );
    assert!(
        requested_write.exists(),
        "touch command should create requested path"
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn read_only_with_additional_permissions_widens_to_unrequested_cwd_write() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config.features.enable(Feature::RequestPermissions);
    });
    let test = builder.build(&server).await?;

    let requested_write = test.workspace_path("requested-only-cwd.txt");
    let unrequested_write = test.workspace_path("unrequested-cwd-write.txt");
    let _ = fs::remove_file(&requested_write);
    let _ = fs::remove_file(&unrequested_write);

    let call_id = "request_permissions_cwd_widening";
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "cwd-widened", unrequested_write, unrequested_write
    );
    let requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![requested_write.clone()]),
        }),
        ..Default::default()
    };
    let event = shell_event_with_request_permissions(call_id, &command, &requested_permissions)?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-cwd-1"),
            event,
            ev_completed("resp-cwd-1"),
        ]),
    )
    .await;
    let results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-cwd-1", "done"),
            ev_completed("resp-cwd-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let approval = expect_exec_approval(&test, &command).await;
    assert_eq!(
        approval.additional_permissions,
        Some(requested_permissions.clone())
    );
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;
    wait_for_completion(&test).await;

    let result = parse_result(&results.single_request().function_call_output(call_id));
    assert!(
        result.exit_code.is_none() || result.exit_code == Some(0),
        "unexpected exit code/output: {:?} {}",
        result.exit_code,
        result.stdout
    );
    assert!(result.stdout.contains("cwd-widened"));
    assert_eq!(fs::read_to_string(&unrequested_write)?, "cwd-widened");
    assert!(
        !requested_write.exists(),
        "only the unrequested cwd path should have been written"
    );

    let _ = fs::remove_file(unrequested_write);
    let _ = fs::remove_file(requested_write);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn read_only_with_additional_permissions_widens_to_unrequested_tmp_write() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config.features.enable(Feature::RequestPermissions);
    });
    let test = builder.build(&server).await?;

    let requested_write = test.workspace_path("requested-only-tmp.txt");
    let tmp_dir = tempfile::tempdir()?;
    let tmp_write = tmp_dir.path().join("tmp-widening.txt");
    let _ = fs::remove_file(&requested_write);
    let _ = fs::remove_file(&tmp_write);

    let call_id = "request_permissions_tmp_widening";
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "tmp-widened", tmp_write, tmp_write
    );
    let requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![requested_write.clone()]),
        }),
        ..Default::default()
    };
    let event = shell_event_with_request_permissions(call_id, &command, &requested_permissions)?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-tmp-1"),
            event,
            ev_completed("resp-tmp-1"),
        ]),
    )
    .await;
    let results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-tmp-1", "done"),
            ev_completed("resp-tmp-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let approval = expect_exec_approval(&test, &command).await;
    assert_eq!(
        approval.additional_permissions,
        Some(requested_permissions.clone())
    );
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;
    wait_for_completion(&test).await;

    let result = parse_result(&results.single_request().function_call_output(call_id));
    assert!(
        result.exit_code.is_none() || result.exit_code == Some(0),
        "unexpected exit code/output: {:?} {}",
        result.exit_code,
        result.stdout
    );
    assert!(result.stdout.contains("tmp-widened"));
    assert_eq!(fs::read_to_string(&tmp_write)?, "tmp-widened");
    assert!(
        !requested_write.exists(),
        "only the unrequested tmp path should have been written"
    );

    let _ = fs::remove_file(tmp_write);
    let _ = fs::remove_file(requested_write);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn workspace_write_with_additional_permissions_can_write_outside_cwd() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config.features.enable(Feature::RequestPermissions);
    });
    let test = builder.build(&server).await?;

    let outside_dir = tempfile::tempdir()?;
    let outside_write = outside_dir.path().join("workspace-write-outside.txt");
    let placeholder = test.workspace_path("workspace-write-placeholder.txt");
    let _ = fs::remove_file(&outside_write);
    let _ = fs::remove_file(&placeholder);

    let call_id = "request_permissions_workspace_write_outside";
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "outside-cwd-ok", outside_write, outside_write
    );
    let requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![outside_dir.path().to_path_buf()]),
        }),
        ..Default::default()
    };
    let normalized_requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![outside_dir.path().canonicalize()?]),
        }),
        ..Default::default()
    };
    let event = shell_event_with_request_permissions(call_id, &command, &requested_permissions)?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-ww-1"),
            event,
            ev_completed("resp-ww-1"),
        ]),
    )
    .await;
    let results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-ww-1", "done"),
            ev_completed("resp-ww-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let approval = expect_exec_approval(&test, &command).await;
    assert_eq!(
        approval.additional_permissions,
        Some(normalized_requested_permissions)
    );
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;
    wait_for_completion(&test).await;

    let result = parse_result(&results.single_request().function_call_output(call_id));
    assert!(
        result.exit_code.is_none() || result.exit_code == Some(0),
        "unexpected exit code/output: {:?} {}",
        result.exit_code,
        result.stdout
    );
    assert!(result.stdout.contains("outside-cwd-ok"));
    assert_eq!(fs::read_to_string(&outside_write)?, "outside-cwd-ok");
    assert!(
        !placeholder.exists(),
        "placeholder path should remain untouched"
    );

    let _ = fs::remove_file(outside_write);
    let _ = fs::remove_file(placeholder);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg(unix)]
async fn with_additional_permissions_denied_approval_blocks_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config.features.enable(Feature::RequestPermissions);
    });
    let test = builder.build(&server).await?;

    let outside_dir = tempfile::tempdir()?;
    let outside_write = outside_dir.path().join("workspace-write-denied.txt");
    let _ = fs::remove_file(&outside_write);

    let call_id = "request_permissions_denied";
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "should-not-write", outside_write, outside_write
    );
    let requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![outside_dir.path().to_path_buf()]),
        }),
        ..Default::default()
    };
    let normalized_requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![outside_dir.path().canonicalize()?]),
        }),
        ..Default::default()
    };
    let event = shell_event_with_request_permissions(call_id, &command, &requested_permissions)?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-denied-1"),
            event,
            ev_completed("resp-denied-1"),
        ]),
    )
    .await;
    let results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-denied-1", "done"),
            ev_completed("resp-denied-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let approval = expect_exec_approval(&test, &command).await;
    assert_eq!(
        approval.additional_permissions,
        Some(normalized_requested_permissions)
    );
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Denied,
        })
        .await?;
    wait_for_completion(&test).await;

    let result = parse_result(&results.single_request().function_call_output(call_id));
    assert_ne!(
        result.exit_code,
        Some(0),
        "denied command should not succeed"
    );
    assert!(
        result.stdout.contains("rejected by user"),
        "unexpected denial output: {}",
        result.stdout
    );
    assert!(
        !outside_write.exists(),
        "denied command should not create file"
    );

    let _ = fs::remove_file(outside_write);
    Ok(())
}
