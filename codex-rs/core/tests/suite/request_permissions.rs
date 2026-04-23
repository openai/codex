#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::sandboxing::SandboxPermissions;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::AdditionalPermissionProfile as PermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::Path;

fn absolute_path(path: &Path) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(path).expect("absolute path")
}

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

fn shell_event_with_request_permissions<S: serde::Serialize>(
    call_id: &str,
    command: &str,
    additional_permissions: &S,
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

fn request_permissions_tool_event(
    call_id: &str,
    reason: &str,
    permissions: &RequestPermissionProfile,
) -> Result<Value> {
    let args = json!({
        "reason": reason,
        "permissions": permissions,
    });
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "request_permissions", &args_str))
}

fn shell_command_event(call_id: &str, command: &str) -> Result<Value> {
    let args = json!({
        "command": command,
        "timeout_ms": 1_000_u64,
    });
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "shell_command", &args_str))
}

fn exec_command_event(call_id: &str, command: &str) -> Result<Value> {
    let args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
    });
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "exec_command", &args_str))
}

fn exec_command_event_with_request_permissions<S: serde::Serialize>(
    call_id: &str,
    command: &str,
    additional_permissions: &S,
) -> Result<Value> {
    let args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::WithAdditionalPermissions,
        "additional_permissions": additional_permissions,
    });
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "exec_command", &args_str))
}

fn exec_command_event_with_missing_additional_permissions(
    call_id: &str,
    command: &str,
) -> Result<Value> {
    let args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::WithAdditionalPermissions,
    });
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "exec_command", &args_str))
}

fn shell_event_with_raw_request_permissions(
    call_id: &str,
    command: &str,
    workdir: Option<&str>,
    additional_permissions: Value,
) -> Result<Value> {
    let args = json!({
        "command": command,
        "workdir": workdir,
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
            environments: None,
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy,
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox_policy,
            permission_profile: None,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
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

async fn wait_for_exec_approval_or_completion(
    test: &TestCodex,
) -> Option<ExecApprovalRequestEvent> {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::ExecApprovalRequest(approval) => Some(approval),
        EventMsg::TurnComplete(_) => None,
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn expect_request_permissions_event(
    test: &TestCodex,
    expected_call_id: &str,
) -> RequestPermissionProfile {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::RequestPermissions(request) => {
            assert_eq!(request.call_id, expected_call_id);
            request.permissions
        }
        EventMsg::TurnComplete(_) => panic!("expected request_permissions before completion"),
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

fn requested_directory_write_permissions(path: &Path) -> RequestPermissionProfile {
    RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![absolute_path(path)]),
        )),
        ..RequestPermissionProfile::default()
    }
}

fn normalized_directory_write_permissions(path: &Path) -> Result<RequestPermissionProfile> {
    Ok(RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![AbsolutePathBuf::try_from(path.canonicalize()?)?]),
        )),
        ..RequestPermissionProfile::default()
    })
}

#[tokio::test(flavor = "current_thread")]
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
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let requested_dir = test.workspace_path("requested-dir");
    fs::create_dir_all(&requested_dir)?;
    let requested_dir_canonical = requested_dir.canonicalize()?;
    let requested_write = requested_dir.join("requested-but-unused.txt");
    let _ = fs::remove_file(&requested_write);
    let call_id = "request_permissions_skip_approval";
    let command = "touch requested-dir/requested-but-unused.txt";
    let requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![absolute_path(&requested_dir_canonical)]),
        )),
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
async fn request_permissions_tool_is_auto_denied_when_granular_request_permissions_is_disabled()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::Granular(GranularApprovalConfig {
        sandbox_approval: true,
        rules: true,
        skill_approval: true,
        request_permissions: false,
        mcp_elicitations: true,
    });
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let requested_dir = test.workspace_path("request-permissions-reject");
    fs::create_dir_all(&requested_dir)?;
    let requested_permissions = requested_directory_write_permissions(&requested_dir);
    let call_id = "request_permissions_reject_auto_denied";
    let event = request_permissions_tool_event(
        call_id,
        "Request access through the standalone tool",
        &requested_permissions,
    )?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-request-permissions-reject-1"),
            event,
            ev_completed("resp-request-permissions-reject-1"),
        ]),
    )
    .await;
    let results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-request-permissions-reject-1", "done"),
            ev_completed("resp-request-permissions-reject-2"),
        ]),
    )
    .await;

    submit_turn(
        &test,
        "request permissions under granular.request_permissions = false",
        approval_policy,
        sandbox_policy,
    )
    .await?;

    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::RequestPermissions(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(
        matches!(event, EventMsg::TurnComplete(_)),
        "request_permissions should not emit a prompt when granular.request_permissions is false: {event:?}"
    );

    let call_output = results.single_request().function_call_output(call_id);
    let result: RequestPermissionsResponse =
        serde_json::from_str(call_output["output"].as_str().unwrap_or_default())?;
    assert_eq!(
        result,
        RequestPermissionsResponse {
            permissions: RequestPermissionProfile::default(),
            scope: PermissionGrantScope::Turn,
            strict_auto_review: false,
        }
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn relative_additional_permissions_resolve_against_tool_workdir() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let nested_dir = test.workspace_path("nested");
    fs::create_dir_all(&nested_dir)?;
    let nested_dir_canonical = nested_dir.canonicalize()?;
    let requested_write = nested_dir.join("relative-write.txt");
    let _ = fs::remove_file(&requested_write);

    let call_id = "request_permissions_relative_workdir";
    let command = "touch relative-write.txt";
    let expected_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            /*read*/ None,
            Some(vec![absolute_path(&nested_dir_canonical)]),
        )),
        ..Default::default()
    };
    let event = shell_event_with_raw_request_permissions(
        call_id,
        command,
        Some("nested"),
        json!({
            "file_system": {
                "write": ["."],
            },
        }),
    )?;

    let _ = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-relative-1"),
            event,
            ev_completed("resp-relative-1"),
        ]),
    )
    .await;
    let results = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-relative-1", "done"),
            ev_completed("resp-relative-2"),
        ]),
    )
    .await;

    submit_turn(&test, call_id, approval_policy, sandbox_policy.clone()).await?;

    let approval = expect_exec_approval(&test, command).await;
    assert_eq!(
        approval.additional_permissions,
        Some(expected_permissions.clone())
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
async fn read_only_with_additional_permissions_does_not_widen_to_unrequested_cwd_write()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
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
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![absolute_path(&requested_write)]),
        )),
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
        result.exit_code != Some(0),
        "unrequested cwd write should stay denied: {:?} {}",
        result.exit_code,
        result.stdout
    );
    assert!(
        !requested_write.exists(),
        "requested path should remain untouched when the command targets an unrequested file"
    );
    assert!(
        !unrequested_write.exists(),
        "unrequested cwd write should remain blocked"
    );

    let _ = fs::remove_file(unrequested_write);
    let _ = fs::remove_file(requested_write);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn read_only_with_additional_permissions_does_not_widen_to_unrequested_tmp_write()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
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
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![absolute_path(&requested_write)]),
        )),
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
        result.exit_code != Some(0),
        "unrequested tmp write should stay denied: {:?} {}",
        result.exit_code,
        result.stdout
    );
    assert!(
        !requested_write.exists(),
        "requested path should remain untouched when the command targets an unrequested file"
    );
    assert!(
        !tmp_write.exists(),
        "unrequested tmp write should remain blocked"
    );

    let _ = fs::remove_file(tmp_write);
    let _ = fs::remove_file(requested_write);
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
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
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
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
    let requested_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![absolute_path(outside_dir.path())]),
        )),
        ..RequestPermissionProfile::default()
    };
    let normalized_requested_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![AbsolutePathBuf::try_from(
                outside_dir.path().canonicalize()?,
            )?]),
        )),
        ..RequestPermissionProfile::default()
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
        Some(normalized_requested_permissions.into())
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
async fn with_additional_permissions_denied_approval_blocks_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
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
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![absolute_path(outside_dir.path())]),
        )),
        ..Default::default()
    };
    let normalized_requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![AbsolutePathBuf::try_from(
                outside_dir.path().canonicalize()?,
            )?]),
        )),
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

#[tokio::test(flavor = "current_thread")]
async fn request_permissions_grants_apply_to_later_exec_command_calls() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let outside_dir = tempfile::tempdir()?;
    let outside_write = outside_dir.path().join("sticky-write.txt");
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "sticky-grant-ok", outside_write, outside_write
    );
    let requested_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![absolute_path(outside_dir.path())]),
        )),
        ..Default::default()
    };
    let normalized_requested_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![AbsolutePathBuf::try_from(
                outside_dir.path().canonicalize()?,
            )?]),
        )),
        ..Default::default()
    };
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-sticky-1"),
                request_permissions_tool_event(
                    "permissions-call",
                    "Allow writing outside the workspace",
                    &requested_permissions,
                )?,
                ev_completed("resp-sticky-1"),
            ]),
            sse(vec![
                ev_response_created("resp-sticky-2"),
                exec_command_event("exec-call", &command)?,
                ev_completed("resp-sticky-2"),
            ]),
            sse(vec![
                ev_response_created("resp-sticky-3"),
                ev_assistant_message("msg-sticky-1", "done"),
                ev_completed("resp-sticky-3"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "write outside the workspace",
        approval_policy,
        sandbox_policy,
    )
    .await?;

    let granted_permissions = expect_request_permissions_event(&test, "permissions-call").await;
    assert_eq!(
        granted_permissions,
        normalized_requested_permissions.clone()
    );
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: "permissions-call".to_string(),
            response: RequestPermissionsResponse {
                permissions: normalized_requested_permissions.clone(),
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;

    if let Some(approval) = wait_for_exec_approval_or_completion(&test).await {
        assert_eq!(
            approval.additional_permissions,
            Some(normalized_requested_permissions.clone().into())
        );
        test.codex
            .submit(Op::ExecApproval {
                id: approval.effective_approval_id(),
                turn_id: None,
                decision: ReviewDecision::Approved,
            })
            .await?;
        wait_for_completion(&test).await;
    }

    let exec_output = responses
        .function_call_output_text("exec-call")
        .map(|output| json!({ "output": output }))
        .unwrap_or_else(|| panic!("expected exec-call output"));
    let result = parse_result(&exec_output);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout.trim(), "sticky-grant-ok");
    assert_eq!(fs::read_to_string(&outside_write)?, "sticky-grant-ok");

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn request_permissions_preapprove_explicit_exec_permissions_outside_on_request() -> Result<()>
{
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let outside_dir = tempfile::tempdir()?;
    let outside_write = outside_dir.path().join("sticky-explicit-write.txt");
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "sticky-explicit-grant-ok", outside_write, outside_write
    );
    let requested_permissions = requested_directory_write_permissions(outside_dir.path());
    let normalized_requested_permissions =
        normalized_directory_write_permissions(outside_dir.path())?;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-sticky-explicit-1"),
                request_permissions_tool_event(
                    "permissions-call",
                    "Allow writing outside the workspace",
                    &requested_permissions,
                )?,
                ev_completed("resp-sticky-explicit-1"),
            ]),
            sse(vec![
                ev_response_created("resp-sticky-explicit-2"),
                exec_command_event_with_request_permissions(
                    "exec-call",
                    &command,
                    &requested_permissions,
                )?,
                ev_completed("resp-sticky-explicit-2"),
            ]),
            sse(vec![
                ev_response_created("resp-sticky-explicit-3"),
                ev_assistant_message("msg-sticky-explicit-1", "done"),
                ev_completed("resp-sticky-explicit-3"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "write outside the workspace",
        approval_policy,
        sandbox_policy,
    )
    .await?;

    let granted_permissions = expect_request_permissions_event(&test, "permissions-call").await;
    assert_eq!(
        granted_permissions,
        normalized_requested_permissions.clone()
    );
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: "permissions-call".to_string(),
            response: RequestPermissionsResponse {
                permissions: normalized_requested_permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;

    if let Some(approval) = wait_for_exec_approval_or_completion(&test).await {
        test.codex
            .submit(Op::ExecApproval {
                id: approval.effective_approval_id(),
                turn_id: None,
                decision: ReviewDecision::Approved,
            })
            .await?;
        wait_for_completion(&test).await;
    }

    let exec_output = responses
        .function_call_output_text("exec-call")
        .map(|output| json!({ "output": output }))
        .unwrap_or_else(|| panic!("expected exec-call output"));
    let result = parse_result(&exec_output);
    assert!(
        result.exit_code.is_none_or(|exit_code| exit_code == 0),
        "expected success output, got exit_code={:?}, stdout={:?}",
        result.exit_code,
        result.stdout
    );
    assert_eq!(result.stdout.trim(), "sticky-explicit-grant-ok");
    assert_eq!(
        fs::read_to_string(&outside_write)?,
        "sticky-explicit-grant-ok"
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn request_permissions_grants_apply_to_later_shell_command_calls() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let outside_dir = tempfile::tempdir()?;
    let outside_write = outside_dir.path().join("sticky-shell-write.txt");
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "sticky-shell-grant-ok", outside_write, outside_write
    );
    let requested_permissions = requested_directory_write_permissions(outside_dir.path());
    let normalized_requested_permissions =
        normalized_directory_write_permissions(outside_dir.path())?;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-sticky-shell-1"),
                request_permissions_tool_event(
                    "permissions-call",
                    "Allow writing outside the workspace",
                    &requested_permissions,
                )?,
                ev_completed("resp-sticky-shell-1"),
            ]),
            sse(vec![
                ev_response_created("resp-sticky-shell-2"),
                shell_command_event("shell-call", &command)?,
                ev_completed("resp-sticky-shell-2"),
            ]),
            sse(vec![
                ev_response_created("resp-sticky-shell-3"),
                ev_assistant_message("msg-sticky-shell-1", "done"),
                ev_completed("resp-sticky-shell-3"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "write outside the workspace",
        approval_policy,
        sandbox_policy,
    )
    .await?;

    let granted_permissions = expect_request_permissions_event(&test, "permissions-call").await;
    assert_eq!(
        granted_permissions,
        normalized_requested_permissions.clone()
    );
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: "permissions-call".to_string(),
            response: RequestPermissionsResponse {
                permissions: normalized_requested_permissions.clone(),
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;

    if let Some(approval) = wait_for_exec_approval_or_completion(&test).await {
        test.codex
            .submit(Op::ExecApproval {
                id: approval.effective_approval_id(),
                turn_id: None,
                decision: ReviewDecision::Approved,
            })
            .await?;
        wait_for_completion(&test).await;
    }

    let shell_output = responses
        .function_call_output_text("shell-call")
        .map(|output| json!({ "output": output }))
        .unwrap_or_else(|| panic!("expected shell-call output"));
    let result = parse_result(&shell_output);
    assert!(
        result.exit_code.is_none_or(|exit_code| exit_code == 0),
        "expected success output, got exit_code={:?}, stdout={:?}",
        result.exit_code,
        result.stdout
    );
    assert_eq!(result.stdout.trim(), "sticky-shell-grant-ok");
    assert_eq!(fs::read_to_string(&outside_write)?, "sticky-shell-grant-ok");

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn request_permissions_grants_apply_to_later_shell_command_calls_without_inline_permission_feature()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let outside_dir = tempfile::tempdir()?;
    let outside_write = outside_dir
        .path()
        .join("sticky-shell-feature-independent.txt");
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "sticky-shell-feature-independent-ok", outside_write, outside_write
    );
    let requested_permissions = requested_directory_write_permissions(outside_dir.path());
    let normalized_requested_permissions =
        normalized_directory_write_permissions(outside_dir.path())?;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-sticky-shell-independent-1"),
                request_permissions_tool_event(
                    "permissions-call",
                    "Allow writing outside the workspace",
                    &requested_permissions,
                )?,
                ev_completed("resp-sticky-shell-independent-1"),
            ]),
            sse(vec![
                ev_response_created("resp-sticky-shell-independent-2"),
                shell_command_event("shell-call", &command)?,
                ev_completed("resp-sticky-shell-independent-2"),
            ]),
            sse(vec![
                ev_response_created("resp-sticky-shell-independent-3"),
                ev_assistant_message("msg-sticky-shell-independent-1", "done"),
                ev_completed("resp-sticky-shell-independent-3"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "write outside the workspace without inline permission feature",
        approval_policy,
        sandbox_policy,
    )
    .await?;

    let granted_permissions = expect_request_permissions_event(&test, "permissions-call").await;
    assert_eq!(
        granted_permissions,
        normalized_requested_permissions.clone()
    );
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: "permissions-call".to_string(),
            response: RequestPermissionsResponse {
                permissions: normalized_requested_permissions.clone(),
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;

    if let Some(approval) = wait_for_exec_approval_or_completion(&test).await {
        test.codex
            .submit(Op::ExecApproval {
                id: approval.effective_approval_id(),
                turn_id: None,
                decision: ReviewDecision::Approved,
            })
            .await?;
        wait_for_completion(&test).await;
    }

    let shell_output = responses
        .function_call_output_text("shell-call")
        .map(|output| json!({ "output": output }))
        .unwrap_or_else(|| panic!("expected shell-call output"));
    let result = parse_result(&shell_output);
    assert!(
        result.exit_code.is_none_or(|exit_code| exit_code == 0),
        "expected success output, got exit_code={:?}, stdout={:?}",
        result.exit_code,
        result.stdout
    );
    assert_eq!(result.stdout.trim(), "sticky-shell-feature-independent-ok");
    assert_eq!(
        fs::read_to_string(&outside_write)?,
        "sticky-shell-feature-independent-ok"
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn partial_request_permissions_grants_do_not_preapprove_new_permissions() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let first_dir = tempfile::tempdir()?;
    let second_dir = tempfile::tempdir()?;
    let second_write = second_dir.path().join("partial-grant-write.txt");
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "partial-grant-ok", second_write, second_write
    );

    let requested_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![
                absolute_path(first_dir.path()),
                absolute_path(second_dir.path()),
            ]),
        )),
        ..RequestPermissionProfile::default()
    };
    let normalized_requested_permissions = RequestPermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![
                AbsolutePathBuf::try_from(first_dir.path().canonicalize()?)?,
                AbsolutePathBuf::try_from(second_dir.path().canonicalize()?)?,
            ]),
        )),
        ..RequestPermissionProfile::default()
    };
    let granted_permissions = normalized_directory_write_permissions(first_dir.path())?;
    let second_dir_permissions = requested_directory_write_permissions(second_dir.path());
    let merged_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![]),
            Some(vec![
                AbsolutePathBuf::try_from(first_dir.path().canonicalize()?)?,
                AbsolutePathBuf::try_from(second_dir.path().canonicalize()?)?,
            ]),
        )),
        ..Default::default()
    };

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-partial-1"),
                request_permissions_tool_event(
                    "permissions-call",
                    "Allow writing outside the workspace",
                    &requested_permissions,
                )?,
                ev_completed("resp-partial-1"),
            ]),
            sse(vec![
                ev_response_created("resp-partial-2"),
                exec_command_event_with_request_permissions(
                    "exec-call",
                    &command,
                    &second_dir_permissions,
                )?,
                ev_completed("resp-partial-2"),
            ]),
            sse(vec![
                ev_response_created("resp-partial-3"),
                ev_assistant_message("msg-partial-1", "done"),
                ev_completed("resp-partial-3"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "write outside the workspace",
        approval_policy,
        sandbox_policy,
    )
    .await?;

    let initial_request = expect_request_permissions_event(&test, "permissions-call").await;
    assert_eq!(initial_request, normalized_requested_permissions);
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: "permissions-call".to_string(),
            response: RequestPermissionsResponse {
                permissions: granted_permissions.clone(),
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;

    let approval = expect_exec_approval(&test, &command).await;
    let approval_permissions = approval
        .additional_permissions
        .clone()
        .unwrap_or_else(|| panic!("expected merged additional permissions"));
    assert_eq!(approval_permissions.network, None);

    let approval_file_system = approval_permissions
        .file_system
        .unwrap_or_else(|| panic!("expected filesystem permissions"));
    let (approval_reads, approval_writes) = approval_file_system
        .legacy_read_write_roots()
        .unwrap_or_else(|| panic!("expected legacy-compatible permissions"));
    assert!(approval_reads.as_ref().is_none_or(Vec::is_empty));

    let mut approval_writes = approval_writes.unwrap_or_default();
    approval_writes.sort_by_key(|path| path.display().to_string());

    let (_, expected_writes) = merged_permissions
        .file_system
        .unwrap_or_else(|| panic!("expected merged filesystem permissions"))
        .legacy_read_write_roots()
        .unwrap_or_else(|| panic!("expected legacy-compatible permissions"));
    let mut expected_writes = expected_writes.unwrap_or_default();
    expected_writes.sort_by_key(|path| path.display().to_string());

    assert_eq!(approval_writes, expected_writes);
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;
    wait_for_completion(&test).await;

    let exec_output = responses
        .function_call_output_text("exec-call")
        .map(|output| json!({ "output": output }))
        .unwrap_or_else(|| panic!("expected exec-call output"));
    let result = parse_result(&exec_output);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout.trim(), "partial-grant-ok");
    assert_eq!(fs::read_to_string(&second_write)?, "partial-grant-ok");

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn request_permissions_grants_do_not_carry_across_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let outside_dir = tempfile::tempdir()?;
    let requested_permissions = requested_directory_write_permissions(outside_dir.path());
    let normalized_requested_permissions =
        normalized_directory_write_permissions(outside_dir.path())?;

    let _first_turn = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-turn-1"),
                request_permissions_tool_event(
                    "permissions-call",
                    "Allow writing outside the workspace",
                    &requested_permissions,
                )?,
                ev_completed("resp-turn-1"),
            ]),
            sse(vec![
                ev_response_created("resp-turn-2"),
                ev_assistant_message("msg-turn-1", "done"),
                ev_completed("resp-turn-2"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "request permissions for later use",
        approval_policy,
        sandbox_policy.clone(),
    )
    .await?;

    let granted_permissions = expect_request_permissions_event(&test, "permissions-call").await;
    assert_eq!(
        granted_permissions,
        normalized_requested_permissions.clone()
    );
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: "permissions-call".to_string(),
            response: RequestPermissionsResponse {
                permissions: normalized_requested_permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: false,
            },
        })
        .await?;
    wait_for_completion(&test).await;

    let second_turn = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-turn-3"),
                exec_command_event_with_missing_additional_permissions(
                    "exec-call",
                    "printf 'should not run'",
                )?,
                ev_completed("resp-turn-3"),
            ]),
            sse(vec![
                ev_response_created("resp-turn-4"),
                ev_assistant_message("msg-turn-2", "done"),
                ev_completed("resp-turn-4"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "try to reuse permissions in a later turn",
        approval_policy,
        sandbox_policy,
    )
    .await?;
    wait_for_completion(&test).await;

    let output = second_turn
        .function_call_output_text("exec-call")
        .unwrap_or_else(|| panic!("expected exec-call output"));
    assert!(output.contains("missing `additional_permissions`"));

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn request_permissions_session_grants_carry_across_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = workspace_write_excluding_tmp();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy_for_config);
        config
            .features
            .enable(Feature::ExecPermissionApprovals)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let outside_dir = tempfile::tempdir()?;
    let outside_write = outside_dir.path().join("session-sticky-write.txt");
    let requested_permissions = requested_directory_write_permissions(outside_dir.path());
    let normalized_requested_permissions =
        normalized_directory_write_permissions(outside_dir.path())?;
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "session-sticky-ok", outside_write, outside_write
    );

    let _first_turn = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-session-turn-1"),
                request_permissions_tool_event(
                    "permissions-call",
                    "Allow writing outside the workspace",
                    &requested_permissions,
                )?,
                ev_completed("resp-session-turn-1"),
            ]),
            sse(vec![
                ev_response_created("resp-session-turn-2"),
                ev_assistant_message("msg-session-turn-1", "done"),
                ev_completed("resp-session-turn-2"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "request session permissions for later use",
        approval_policy,
        sandbox_policy.clone(),
    )
    .await?;

    let granted_permissions = expect_request_permissions_event(&test, "permissions-call").await;
    assert_eq!(
        granted_permissions,
        normalized_requested_permissions.clone()
    );
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: "permissions-call".to_string(),
            response: RequestPermissionsResponse {
                permissions: normalized_requested_permissions,
                scope: PermissionGrantScope::Session,
                strict_auto_review: false,
            },
        })
        .await?;
    wait_for_completion(&test).await;

    let second_turn = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-session-turn-3"),
                exec_command_event("exec-call", &command)?,
                ev_completed("resp-session-turn-3"),
            ]),
            sse(vec![
                ev_response_created("resp-session-turn-4"),
                ev_assistant_message("msg-session-turn-2", "done"),
                ev_completed("resp-session-turn-4"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "reuse session permissions in a later turn",
        approval_policy,
        sandbox_policy,
    )
    .await?;

    let completion_event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    if let EventMsg::ExecApprovalRequest(approval) = completion_event {
        test.codex
            .submit(Op::ExecApproval {
                id: approval.effective_approval_id(),
                turn_id: None,
                decision: ReviewDecision::Approved,
            })
            .await?;
        wait_for_completion(&test).await;
    }

    let exec_output = second_turn
        .function_call_output_text("exec-call")
        .map(|output| json!({ "output": output }))
        .unwrap_or_else(|| panic!("expected exec-call output"));
    let result = parse_result(&exec_output);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout.trim(), "session-sticky-ok");
    assert_eq!(fs::read_to_string(&outside_write)?, "session-sticky-ok");

    Ok(())
}
