#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(target_os = "macos")]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
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
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
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
    permissions: &RequestPermissionProfile,
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

fn build_add_file_patch(patch_path: &Path, content: &str) -> String {
    format!(
        "*** Begin Patch\n*** Add File: {}\n+{}\n*** End Patch\n",
        patch_path.display(),
        content
    )
}

fn guardian_allow_message(message_id: &str, rationale: &str) -> Value {
    ev_assistant_message(
        message_id,
        &serde_json::json!({
            "risk_level": "low",
            "user_authorization": "high",
            "outcome": "allow",
            "rationale": rationale,
        })
        .to_string(),
    )
}

fn workspace_write_excluding_tmp() -> PermissionProfile {
    PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Restricted,
        /*exclude_tmpdir_env_var*/ true,
        /*exclude_slash_tmp*/ true,
    )
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
    permission_profile: PermissionProfile,
    approvals_reviewer: Option<ApprovalsReviewer>,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                approvals_reviewer,
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(codex_protocol::config_types::CollaborationMode {
                    mode: codex_protocol::config_types::ModeKind::Default,
                    settings: codex_protocol::config_types::Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
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

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn approved_folder_write_request_permissions_unblocks_later_exec_without_sandbox_args()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = workspace_write_excluding_tmp();
    let permission_profile_for_config = permission_profile.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config
            .permissions
            .set_permission_profile(permission_profile_for_config)
            .expect("set permission profile");
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
        permission_profile,
        Some(ApprovalsReviewer::User),
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
                approvals_reviewer: None,
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

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn approved_folder_write_request_permissions_unblocks_later_apply_patch() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    for review_after_grant in [
        ReviewAfterGrant::None,
        ReviewAfterGrant::LegacyStrictAutoReview,
        ReviewAfterGrant::ExplicitAutoReview,
    ] {
        apply_patch_after_request_permissions(review_after_grant).await?;
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum ReviewAfterGrant {
    None,
    LegacyStrictAutoReview,
    ExplicitAutoReview,
}

impl ReviewAfterGrant {
    fn routes_to_guardian(self) -> bool {
        match self {
            Self::None => false,
            Self::LegacyStrictAutoReview | Self::ExplicitAutoReview => true,
        }
    }

    fn requested_file_name(self) -> &'static str {
        match self {
            Self::None => "allowed-patch.txt",
            Self::LegacyStrictAutoReview => "strict-allowed-patch.txt",
            Self::ExplicitAutoReview => "reviewer-override-allowed-patch.txt",
        }
    }

    fn patch_content(self) -> &'static str {
        match self {
            Self::None => "patched-via-request-permissions",
            Self::LegacyStrictAutoReview => "patched-after-strict-review",
            Self::ExplicitAutoReview => "patched-after-reviewer-override",
        }
    }

    fn response_prefix(self) -> &'static str {
        match self {
            Self::None => "resp-request-permissions-patch",
            Self::LegacyStrictAutoReview => "resp-strict-request-permissions-patch",
            Self::ExplicitAutoReview => "resp-reviewer-override-request-permissions-patch",
        }
    }

    fn response(self, permissions: RequestPermissionProfile) -> RequestPermissionsResponse {
        RequestPermissionsResponse {
            permissions,
            scope: PermissionGrantScope::Turn,
            strict_auto_review: matches!(self, Self::LegacyStrictAutoReview),
            approvals_reviewer: matches!(self, Self::ExplicitAutoReview)
                .then_some(ApprovalsReviewer::AutoReview),
        }
    }
}

async fn apply_patch_after_request_permissions(review_after_grant: ReviewAfterGrant) -> Result<()> {
    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = workspace_write_excluding_tmp();
    let permission_profile_for_config = permission_profile.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config
            .permissions
            .set_permission_profile(permission_profile_for_config)
            .expect("set permission profile");
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

    let requested_dir = tempfile::tempdir()?;
    let requested_file_name = review_after_grant.requested_file_name();
    let patch_content = review_after_grant.patch_content();
    let requested_file = requested_dir
        .path()
        .canonicalize()?
        .join(requested_file_name);
    let requested_permissions = requested_directory_write_permissions(requested_dir.path());
    let normalized_requested_permissions =
        normalized_directory_write_permissions(requested_dir.path())?;
    let patch = build_add_file_patch(&requested_file, patch_content);

    let response_prefix = review_after_grant.response_prefix();
    let mut sse_sequence = vec![
        sse(vec![
            ev_response_created(&format!("{response_prefix}-1")),
            request_permissions_tool_event(
                "permissions-call",
                "Allow patching outside the workspace",
                &requested_permissions,
            )?,
            ev_completed(&format!("{response_prefix}-1")),
        ]),
        sse(vec![
            ev_response_created(&format!("{response_prefix}-2")),
            ev_apply_patch_custom_tool_call("apply-patch-call", &patch),
            ev_completed(&format!("{response_prefix}-2")),
        ]),
    ];
    if review_after_grant.routes_to_guardian() {
        sse_sequence.push(sse(vec![
            ev_response_created(&format!("{response_prefix}-guardian")),
            guardian_allow_message(
                "msg-strict-request-permissions-patch-guardian",
                "The patch stays within the turn grant.",
            ),
            ev_completed(&format!("{response_prefix}-guardian")),
        ]));
    }
    sse_sequence.push(sse(vec![
        ev_response_created(&format!("{response_prefix}-3")),
        ev_assistant_message("msg-request-permissions-patch-1", "done"),
        ev_completed(&format!("{response_prefix}-3")),
    ]));
    let responses = mount_sse_sequence(&server, sse_sequence).await;

    submit_turn(
        &test,
        "patch outside the workspace",
        approval_policy,
        permission_profile,
        Some(ApprovalsReviewer::User),
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
            response: review_after_grant.response(normalized_requested_permissions),
        })
        .await?;

    if review_after_grant.routes_to_guardian() {
        wait_for_completion(&test).await;
        let guardian_request = responses
            .requests()
            .into_iter()
            .find(|request| request.body_contains_text(requested_file_name))
            .expect("expected guardian request for strict apply_patch");
        assert!(guardian_request.body_contains_text(requested_file_name));
        assert!(guardian_request.body_contains_text(patch_content));
    } else {
        let event = wait_for_event(&test.codex, |event| {
            matches!(
                event,
                EventMsg::ApplyPatchApprovalRequest(_) | EventMsg::TurnComplete(_)
            )
        })
        .await;
        match event {
            EventMsg::TurnComplete(_) => {}
            EventMsg::ApplyPatchApprovalRequest(approval) => {
                panic!(
                    "unexpected apply_patch approval request after granted permissions: {approval:?}",
                )
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    let patch_output = responses
        .requests()
        .into_iter()
        .find_map(|request| {
            request
                .input()
                .into_iter()
                .find(|item| {
                    item.get("type").and_then(Value::as_str) == Some("custom_tool_call_output")
                        && item.get("call_id").and_then(Value::as_str) == Some("apply-patch-call")
                })
                .and_then(|item| {
                    item.get("output")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
        })
        .map(|output| json!({ "output": output }))
        .unwrap_or_else(|| panic!("expected apply-patch-call output"));
    let (exit_code, stdout) = parse_result(&patch_output);
    assert!(exit_code.is_none() || exit_code == Some(0));
    assert!(
        stdout.contains("Success."),
        "unexpected patch output: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(&requested_file)?,
        format!("{patch_content}\n")
    );

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg(target_os = "macos")]
async fn session_request_permissions_reviewer_override_is_turn_scoped() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = workspace_write_excluding_tmp();
    let permission_profile_for_config = permission_profile.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config
            .permissions
            .set_permission_profile(permission_profile_for_config)
            .expect("set permission profile");
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

    let requested_dir = tempfile::tempdir()?;
    let first_file = requested_dir
        .path()
        .canonicalize()?
        .join("session-reviewer-override-first.txt");
    let second_file = requested_dir
        .path()
        .canonicalize()?
        .join("session-reviewer-override-second.txt");
    let requested_permissions = requested_directory_write_permissions(requested_dir.path());
    let normalized_requested_permissions =
        normalized_directory_write_permissions(requested_dir.path())?;
    let first_patch = build_add_file_patch(&first_file, "first-turn-reviewed");
    let second_patch = build_add_file_patch(&second_file, "second-turn-not-reviewed");

    let first_turn = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-session-reviewer-override-1"),
                request_permissions_tool_event(
                    "permissions-call",
                    "Allow patching outside the workspace",
                    &requested_permissions,
                )?,
                ev_completed("resp-session-reviewer-override-1"),
            ]),
            sse(vec![
                ev_response_created("resp-session-reviewer-override-2"),
                ev_apply_patch_custom_tool_call("apply-patch-call-1", &first_patch),
                ev_completed("resp-session-reviewer-override-2"),
            ]),
            sse(vec![
                ev_response_created("resp-session-reviewer-override-guardian"),
                guardian_allow_message(
                    "msg-session-reviewer-override-guardian",
                    "The patch stays within the session permission grant.",
                ),
                ev_completed("resp-session-reviewer-override-guardian"),
            ]),
            sse(vec![
                ev_response_created("resp-session-reviewer-override-3"),
                ev_assistant_message("msg-session-reviewer-override-1", "done"),
                ev_completed("resp-session-reviewer-override-3"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "grant session permissions and auto review this turn",
        approval_policy,
        permission_profile.clone(),
        Some(ApprovalsReviewer::User),
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
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            },
        })
        .await?;
    wait_for_completion(&test).await;

    let first_guardian_requests = first_turn
        .requests()
        .into_iter()
        .filter(|request| {
            request
                .instructions_text()
                .starts_with("You are judging one planned coding-agent action.")
        })
        .collect::<Vec<_>>();
    assert_eq!(first_guardian_requests.len(), 1);
    assert!(first_guardian_requests[0].body_contains_text("session-reviewer-override-first.txt"));
    assert_eq!(fs::read_to_string(&first_file)?, "first-turn-reviewed\n");

    let second_turn = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-session-reviewer-override-4"),
                ev_apply_patch_custom_tool_call("apply-patch-call-2", &second_patch),
                ev_completed("resp-session-reviewer-override-4"),
            ]),
            sse(vec![
                ev_response_created("resp-session-reviewer-override-guardian-unexpected"),
                guardian_allow_message(
                    "msg-session-reviewer-override-unexpected-guardian",
                    "This response is only consumed if the reviewer override leaks.",
                ),
                ev_completed("resp-session-reviewer-override-guardian-unexpected"),
            ]),
            sse(vec![
                ev_response_created("resp-session-reviewer-override-5"),
                ev_assistant_message("msg-session-reviewer-override-2", "done"),
                ev_completed("resp-session-reviewer-override-5"),
            ]),
        ],
    )
    .await;

    submit_turn(
        &test,
        "reuse the session permission in a later turn",
        approval_policy,
        permission_profile,
        Some(ApprovalsReviewer::User),
    )
    .await?;
    wait_for_completion(&test).await;

    assert_eq!(
        fs::read_to_string(&second_file)?,
        "second-turn-not-reviewed\n"
    );
    let second_guardian_requests = second_turn
        .requests()
        .into_iter()
        .filter(|request| {
            request
                .instructions_text()
                .starts_with("You are judging one planned coding-agent action.")
        })
        .collect::<Vec<_>>();
    assert!(
        second_guardian_requests.is_empty(),
        "session-scoped permission grants must not carry the auto-review override into later turns"
    );

    Ok(())
}
