#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(target_os = "macos")]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::features::Feature;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
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

fn request_permissions_tool_event(
    call_id: &str,
    reason: &str,
    permissions: &PermissionProfile,
) -> Result<Value> {
    let args = json!({
        "reason": reason,
        "permissions": permissions,
    });
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "request_permissions", &args_str))
}

fn exec_command_event(call_id: &str, command: &str) -> Result<Value> {
    let args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
    });
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "exec_command", &args_str))
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

fn requested_directory_write_permissions(path: &Path) -> PermissionProfile {
    PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![absolute_path(path)]),
        }),
        ..Default::default()
    }
}

fn normalized_directory_write_permissions(path: &Path) -> Result<PermissionProfile> {
    Ok(PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: Some(vec![]),
            write: Some(vec![AbsolutePathBuf::try_from(path.canonicalize()?)?]),
        }),
        ..Default::default()
    })
}

fn parse_result(item: &Value) -> (Option<i64>, String) {
    let output_str = item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell output payload");
    match serde_json::from_str::<Value>(output_str) {
        Ok(parsed) => {
            let exit_code = parsed["metadata"]["exit_code"].as_i64();
            let stdout = parsed["output"].as_str().unwrap_or_default().to_string();
            (exit_code, stdout)
        }
        Err(_) => {
            let structured = Regex::new(r"(?s)^Exit code:\s*(-?\d+).*?Output:\n(.*)$").unwrap();
            let regex =
                Regex::new(r"(?s)^.*?Process exited with code (\d+)\n.*?Output:\n(.*)$").unwrap();
            if let Some(captures) = structured.captures(output_str) {
                let exit_code = captures.get(1).unwrap().as_str().parse::<i64>().unwrap();
                let output = captures.get(2).unwrap().as_str();
                (Some(exit_code), output.to_string())
            } else if let Some(captures) = regex.captures(output_str) {
                let exit_code = captures.get(1).unwrap().as_str().parse::<i64>().unwrap();
                let output = captures.get(2).unwrap().as_str();
                (Some(exit_code), output.to_string())
            } else {
                (None, output_str.to_string())
            }
        }
    }
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
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    Ok(())
}

async fn expect_request_permissions_event(
    test: &TestCodex,
    expected_call_id: &str,
) -> PermissionProfile {
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

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn approved_folder_write_request_permissions_unblocks_later_exec_without_sandbox_args()
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
            .enable(Feature::RequestPermissions)
            .expect("test config should allow feature update");
        config
            .features
            .enable(Feature::RequestPermissionsTool)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;

    let requested_dir = tempfile::tempdir()?;
    let requested_file = requested_dir.path().join("allowed-write.txt");
    let command = format!(
        "printf {:?} > {:?} && cat {:?}",
        "folder-grant-ok", requested_file, requested_file
    );
    let requested_permissions = requested_directory_write_permissions(requested_dir.path());
    let normalized_requested_permissions =
        normalized_directory_write_permissions(requested_dir.path())?;

    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-request-permissions-1"),
                request_permissions_tool_event(
                    "permissions-call",
                    "Allow writing outside the workspace",
                    &requested_permissions,
                )?,
                ev_completed("resp-request-permissions-1"),
            ]),
            sse(vec![
                ev_response_created("resp-request-permissions-2"),
                exec_command_event("exec-call", &command)?,
                ev_completed("resp-request-permissions-2"),
            ]),
            sse(vec![
                ev_response_created("resp-request-permissions-3"),
                ev_assistant_message("msg-request-permissions-1", "done"),
                ev_completed("resp-request-permissions-3"),
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
            },
        })
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
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }

    let exec_output = responses
        .function_call_output_text("exec-call")
        .map(|output| json!({ "output": output }))
        .unwrap_or_else(|| panic!("expected exec-call output"));
    let (exit_code, stdout) = parse_result(&exec_output);
    assert!(exit_code.is_none() || exit_code == Some(0));
    assert!(stdout.contains("folder-grant-ok"));
    assert!(
        requested_file.exists(),
        "touch command should create the file"
    );
    assert_eq!(fs::read_to_string(&requested_file)?, "folder-grant-ok");

    Ok(())
}
