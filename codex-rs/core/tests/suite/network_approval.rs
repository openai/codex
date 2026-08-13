use anyhow::Context;
use anyhow::Result;
use codex_config::types::ApprovalsReviewer;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::RemoveOptions;
use codex_features::Feature;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::approvals::NetworkPolicyAmendment;
use codex_protocol::approvals::NetworkPolicyRuleAction;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::protocol::TurnEnvironmentSelections;
use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use core_test_support::managed_network_requirements_loader;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_once_match;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_host_windows;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_no_remote_env;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

const NETWORK_TEST_HOST: &str = "codex-network-test.invalid";
const NETWORK_TEST_TARGET: &str = "http://codex-network-test.invalid:80";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn guardian_network_approval_preserves_action_and_outcome_routing() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let first_call_id = "guardian-network-approved";
    let second_call_id = "guardian-network-denied";
    let first_command = network_fetch_args(LOCAL_ENVIRONMENT_ID)["cmd"]
        .as_str()
        .context("expected network command")?
        .to_string();
    let second_command = first_command.clone();
    let denial = "The destination is outside the approved test boundary.";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-guardian-network-parent-1"),
                ev_function_call(
                    first_call_id,
                    "exec_command",
                    &serde_json::to_string(&network_fetch_args(LOCAL_ENVIRONMENT_ID))?,
                ),
                ev_completed("resp-guardian-network-parent-1"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-network-allow"),
                ev_assistant_message(
                    "msg-guardian-network-allow",
                    r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"The test request is safe."}"#,
                ),
                ev_completed("resp-guardian-network-allow"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-network-parent-2"),
                ev_assistant_message("msg-guardian-network-parent-2", "approved"),
                ev_completed("resp-guardian-network-parent-2"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-network-parent-3"),
                ev_function_call(
                    second_call_id,
                    "exec_command",
                    &serde_json::to_string(&network_fetch_args(LOCAL_ENVIRONMENT_ID))?,
                ),
                ev_completed("resp-guardian-network-parent-3"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-network-deny"),
                ev_assistant_message(
                    "msg-guardian-network-deny",
                    &json!({
                        "risk_level": "high",
                        "user_authorization": "low",
                        "outcome": "deny",
                        "rationale": denial,
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian-network-deny"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-network-parent-4"),
                ev_assistant_message("msg-guardian-network-parent-4", "denied"),
                ev_completed("resp-guardian-network-parent-4"),
            ]),
        ],
    )
    .await;

    for prompt in ["approve the network request", "deny the network request"] {
        submit_managed_network_turn(
            &test,
            prompt,
            vec![local(test.config.cwd.clone())],
            ApprovalsReviewer::AutoReview,
            AskForApproval::OnRequest,
        )
        .await?;
        wait_for_completion_without_network_prompt(&test).await;
    }

    let actions = guardian_network_actions(&responses)?;
    assert_eq!(actions.len(), 2);
    assert_eq!(
        actions[0],
        json!({
            "host": NETWORK_TEST_HOST,
            "port": 80,
            "protocol": "http",
            "target": NETWORK_TEST_TARGET,
            "tool": "network_access",
            "trigger": {
                "callId": first_call_id,
                "command": ["/bin/sh", "-c", first_command],
                "cwd": test.config.cwd,
                "sandboxPermissions": "use_default",
                "toolName": "exec_command",
                "tty": false,
            },
        })
    );
    assert_eq!(
        actions[1]
            .pointer("/trigger/callId")
            .and_then(Value::as_str),
        Some(second_call_id)
    );
    assert_eq!(
        actions[1]
            .pointer("/trigger/command/2")
            .and_then(Value::as_str),
        Some(second_command.as_str())
    );

    let requests = responses.requests();
    let approved_output = requests
        .iter()
        .find_map(|request| request.function_call_output_text(first_call_id))
        .context("expected approved network tool output")?;
    assert!(!approved_output.contains("rejected"));
    let denied_output = requests
        .iter()
        .find_map(|request| request.function_call_output_text(second_call_id))
        .context("expected denied network tool output")?;
    assert!(denied_output.contains(denial));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn strict_auto_review_routes_network_approval_to_guardian_when_user_reviewer_is_selected()
-> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test_with_features(
        &server,
        &[Feature::RequestPermissionsTool],
    )
    .await?;
    let permission_call_id = "strict-network-permissions";
    let network_call_id = "strict-network-access";
    let requested_permissions = RequestPermissionProfile {
        network: Some(NetworkPermissions {
            enabled: Some(true),
        }),
        ..Default::default()
    };
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-strict-network-permissions"),
                ev_function_call(
                    permission_call_id,
                    "request_permissions",
                    &serde_json::to_string(&json!({
                        "reason": "Require automatic review for the rest of this turn",
                        "permissions": requested_permissions,
                    }))?,
                ),
                ev_completed("resp-strict-network-permissions"),
            ]),
            sse(vec![
                ev_response_created("resp-strict-network-command"),
                ev_function_call(
                    network_call_id,
                    "exec_command",
                    &serde_json::to_string(&network_fetch_args(LOCAL_ENVIRONMENT_ID))?,
                ),
                ev_completed("resp-strict-network-command"),
            ]),
            sse(vec![
                ev_response_created("resp-strict-command-guardian"),
                ev_assistant_message(
                    "msg-strict-command-guardian",
                    r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"The strict-review command is safe."}"#,
                ),
                ev_completed("resp-strict-command-guardian"),
            ]),
            sse(vec![
                ev_response_created("resp-strict-network-guardian"),
                ev_assistant_message(
                    "msg-strict-network-guardian",
                    r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"The strict-review network request is safe."}"#,
                ),
                ev_completed("resp-strict-network-guardian"),
            ]),
            sse(vec![
                ev_response_created("resp-strict-network-complete"),
                ev_assistant_message("msg-strict-network-complete", "reviewed"),
                ev_completed("resp-strict-network-complete"),
            ]),
        ],
    )
    .await;

    submit_managed_network_turn(
        &test,
        "grant turn permissions, then automatically review network access",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let EventMsg::RequestPermissions(request) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RequestPermissions(_))
    })
    .await
    else {
        unreachable!("matched request permissions event")
    };
    assert_eq!(request.call_id, permission_call_id);
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: permission_call_id.to_string(),
            response: RequestPermissionsResponse {
                permissions: request.permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: true,
            },
        })
        .await?;
    wait_for_completion_without_network_prompt(&test).await;

    let actions = guardian_network_actions(&responses)?;
    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0]
            .pointer("/trigger/callId")
            .and_then(Value::as_str),
        Some(network_call_id)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn cancelled_guardian_network_review_fails_closed_without_rewriting_turn_state() -> Result<()>
{
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let call_id = "guardian-network-cancelled";
    let marker = "guardian cancellation must preserve this turn marker";
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            !is_guardian_request(request) && request_body_contains(request, marker)
        },
        sse(vec![
            ev_response_created("resp-guardian-cancel-parent"),
            ev_function_call(
                call_id,
                "exec_command",
                &serde_json::to_string(&network_fetch_args(LOCAL_ENVIRONMENT_ID))?,
            ),
            ev_completed("resp-guardian-cancel-parent"),
        ]),
    )
    .await;
    let pending_guardian = mount_response_once_match(
        &server,
        is_guardian_request,
        sse_response(sse(vec![
            ev_response_created("resp-guardian-cancelled-review"),
            ev_assistant_message("msg-guardian-cancelled-review", r#"{"outcome":"allow"}"#),
            ev_completed("resp-guardian-cancelled-review"),
        ]))
        .set_delay(Duration::from_secs(30)),
    )
    .await;
    submit_managed_network_turn(
        &test,
        marker,
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::AutoReview,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_response_request(&pending_guardian).await;
    test.codex.submit(Op::Interrupt).await?;
    let mut saw_turn_aborted = false;
    let mut saw_guardian_aborted = false;
    while !saw_turn_aborted || !saw_guardian_aborted {
        let event = tokio::time::timeout(Duration::from_secs(5), test.codex.next_event())
            .await
            .context("timed out waiting for parent and Guardian cancellation")?
            .context("event stream ended while waiting for cancellation")?;
        saw_turn_aborted |= matches!(&event.msg, EventMsg::TurnAborted(_));
        saw_guardian_aborted |= matches!(
            &event.msg,
            EventMsg::GuardianAssessment(assessment)
                if assessment.status == GuardianAssessmentStatus::Aborted
        );
    }

    let state_check = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            !is_guardian_request(request)
                && request_body_contains(request, "verify preserved state")
        },
        sse(vec![
            ev_response_created("resp-guardian-cancel-state-check"),
            ev_assistant_message("msg-guardian-cancel-state-check", "state preserved"),
            ev_completed("resp-guardian-cancel-state-check"),
        ]),
    )
    .await;
    submit_managed_network_turn(
        &test,
        "verify preserved state",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_turn_complete(&test).await;
    assert!(state_check.single_request().body_contains_text(marker));

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn timed_out_guardian_network_review_uses_timeout_outcome_without_user_fallback() -> Result<()>
{
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let call_id = "guardian-network-timeout";
    let poll_call_id = "guardian-network-timeout-poll";
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            !is_guardian_request(request)
                && request_body_contains(request, "time out the Guardian network review")
        },
        sse(vec![
            ev_response_created("resp-guardian-timeout-parent"),
            ev_function_call(
                call_id,
                "exec_command",
                &serde_json::to_string(&network_fetch_args(LOCAL_ENVIRONMENT_ID))?,
            ),
            ev_completed("resp-guardian-timeout-parent"),
        ]),
    )
    .await;
    let pending_guardian = mount_response_once_match(
        &server,
        is_guardian_request,
        sse_response(sse(vec![
            ev_response_created("resp-guardian-timeout-review"),
            ev_assistant_message("msg-guardian-timeout-review", r#"{"outcome":"allow"}"#),
            ev_completed("resp-guardian-timeout-review"),
        ]))
        .set_delay(Duration::from_secs(300)),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            !is_guardian_request(request) && request_body_contains(request, call_id)
        },
        sse(vec![
            ev_response_created("resp-guardian-timeout-parent-followup"),
            ev_function_call(
                poll_call_id,
                "write_stdin",
                &serde_json::to_string(&json!({
                    "session_id": 1000,
                    "chars": "",
                    "yield_time_ms": 1_000,
                }))?,
            ),
            ev_completed("resp-guardian-timeout-parent-followup"),
        ]),
    )
    .await;
    let parent_final = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            !is_guardian_request(request) && request_body_contains(request, poll_call_id)
        },
        sse(vec![
            ev_response_created("resp-guardian-timeout-parent-final"),
            ev_assistant_message("msg-guardian-timeout-parent-final", "timed out"),
            ev_completed("resp-guardian-timeout-parent-final"),
        ]),
    )
    .await;

    submit_managed_network_turn(
        &test,
        "time out the Guardian network review",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::AutoReview,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_response_request(&pending_guardian).await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(91)).await;
    tokio::time::resume();
    wait_for_completion_without_network_prompt(&test).await;

    let tool_output = parent_final
        .requests()
        .iter()
        .find_map(|request| request.function_call_output_text(poll_call_id))
        .context("expected timed-out Guardian tool output")?;
    assert!(
        tool_output.contains(concat!(
            "The automatic permission approval review did not finish before its deadline. ",
            "Do not assume the action is unsafe based on the timeout alone. ",
            "You may retry once, or ask the user for guidance or explicit approval."
        )),
        "unexpected timed-out Guardian tool output: {tool_output}"
    );
    assert!(!tool_output.contains("rejected by user"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn background_network_approval_uses_active_turn_after_original_turn_completes() -> Result<()>
{
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test_with_features(
        &server,
        &[Feature::RequestPermissionsTool],
    )
    .await?;
    let start_call_id = "cross-turn-network-start";
    let permission_call_id = "cross-turn-network-permissions";
    let stdin_call_id = "cross-turn-network-stdin";
    let requested_permissions = RequestPermissionProfile {
        network: Some(NetworkPermissions {
            enabled: Some(true),
        }),
        ..Default::default()
    };
    let command = format!(
        "read _; python3 -c \"import urllib.request; urllib.request.build_opener(urllib.request.ProxyHandler()).open('{NETWORK_TEST_TARGET}', timeout=2).read()\"; echo CROSS-TURN-NETWORK-COMPLETE; read _"
    );
    let mut start_args = network_exec_args(&command);
    start_args["environment_id"] = json!(LOCAL_ENVIRONMENT_ID);
    start_args["tty"] = json!(true);
    start_args["yield_time_ms"] = json!(250);
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-cross-turn-network-start"),
                ev_function_call(
                    start_call_id,
                    "exec_command",
                    &serde_json::to_string(&start_args)?,
                ),
                ev_completed("resp-cross-turn-network-start"),
            ]),
            sse(vec![
                ev_response_created("resp-cross-turn-network-first-complete"),
                ev_assistant_message("msg-cross-turn-network-first-complete", "terminal started"),
                ev_completed("resp-cross-turn-network-first-complete"),
            ]),
            sse(vec![
                ev_response_created("resp-cross-turn-network-permissions"),
                ev_function_call(
                    permission_call_id,
                    "request_permissions",
                    &serde_json::to_string(&json!({
                        "reason": "Automatically review the existing terminal's network access",
                        "permissions": requested_permissions,
                    }))?,
                ),
                ev_completed("resp-cross-turn-network-permissions"),
            ]),
            sse(vec![
                ev_response_created("resp-cross-turn-network-stdin"),
                ev_function_call(
                    stdin_call_id,
                    "write_stdin",
                    &serde_json::to_string(&json!({
                        "session_id": 1000,
                        "chars": "continue\n",
                        "yield_time_ms": 1_000,
                    }))?,
                ),
                ev_completed("resp-cross-turn-network-stdin"),
            ]),
            sse(vec![
                ev_response_created("resp-cross-turn-network-guardian"),
                ev_assistant_message(
                    "msg-cross-turn-network-guardian",
                    r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"The existing terminal's network request is safe."}"#,
                ),
                ev_completed("resp-cross-turn-network-guardian"),
            ]),
            sse(vec![
                ev_response_created("resp-cross-turn-network-second-complete"),
                ev_assistant_message(
                    "msg-cross-turn-network-second-complete",
                    "network request approved",
                ),
                ev_completed("resp-cross-turn-network-second-complete"),
            ]),
        ],
    )
    .await;

    submit_managed_network_turn(
        &test,
        "start a background terminal that waits before requesting network access",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let EventMsg::TurnComplete(first_turn) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await
    else {
        unreachable!("matched first turn completion")
    };
    assert_eq!(test.codex.list_background_terminals().await.len(), 1);

    submit_managed_network_turn(
        &test,
        "allow the existing background terminal to request network access",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let EventMsg::TurnStarted(active_turn) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnStarted(_))
    })
    .await
    else {
        unreachable!("matched second turn start")
    };
    let EventMsg::RequestPermissions(request) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::RequestPermissions(_))
    })
    .await
    else {
        unreachable!("matched request permissions event")
    };
    assert_eq!(request.call_id, permission_call_id);
    test.codex
        .submit(Op::RequestPermissionsResponse {
            id: permission_call_id.to_string(),
            response: RequestPermissionsResponse {
                permissions: request.permissions,
                scope: PermissionGrantScope::Turn,
                strict_auto_review: true,
            },
        })
        .await?;
    let assessment = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::GuardianAssessment(assessment)
                if assessment.status == GuardianAssessmentStatus::Approved
        ) || matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    let EventMsg::GuardianAssessment(assessment) = assessment else {
        panic!("expected Guardian to approve the background terminal's network request");
    };
    assert_eq!(assessment.turn_id, active_turn.turn_id);
    assert_ne!(assessment.turn_id, first_turn.turn_id);
    wait_for_turn_complete(&test).await;

    let actions = guardian_network_actions(&responses)?;
    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0]
            .pointer("/trigger/callId")
            .and_then(Value::as_str),
        Some(start_call_id)
    );
    let stdin_output = responses
        .requests()
        .iter()
        .find_map(|request| request.function_call_output_text(stdin_call_id))
        .context("expected background terminal network request output")?;
    assert!(!stdin_output.contains("blocked by policy"));
    assert_eq!(
        test.codex.list_background_terminals().await.len(),
        1,
        "approved network access must not terminate the background process"
    );
    test.codex.submit(Op::CleanBackgroundTerminals).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn user_network_approval_once_session_and_denial_semantics() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let environments = vec![local(test.config.cwd.clone())];

    mount_exec_network_turn(
        &server,
        "resp-user-network-once-1",
        "user-network-once-1",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "approve this network request once",
        environments.clone(),
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_network_approval(&test, LOCAL_ENVIRONMENT_ID).await?;
    assert!(
        approval
            .call_id
            .starts_with("network#local#http#codex-network-test.invalid#80#")
    );
    assert_eq!(approval.approval_id.as_deref(), None);
    let first_approval_call_id = approval.call_id.clone();
    assert!(!approval.turn_id.is_empty());
    assert_eq!(approval.cwd, test.config.cwd);
    assert_eq!(
        approval.reason.as_deref(),
        Some("codex-network-test.invalid is not in the allowed_domains")
    );
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: Some(approval.turn_id),
            decision: ReviewDecision::Approved,
        })
        .await?;
    wait_for_turn_complete(&test).await;

    mount_exec_network_turn(
        &server,
        "resp-user-network-once-2",
        "user-network-once-2",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "the once decision must prompt again",
        environments.clone(),
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_network_approval(&test, LOCAL_ENVIRONMENT_ID).await?;
    assert_eq!(approval.approval_id.as_deref(), None);
    assert_ne!(approval.call_id, first_approval_call_id);
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: Some(approval.turn_id),
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;
    wait_for_turn_complete(&test).await;

    mount_exec_network_turn(
        &server,
        "resp-user-network-session",
        "user-network-session",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "the session decision must bypass another prompt",
        environments.clone(),
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_completion_without_network_prompt(&test).await;

    let different_port_target = format!("http://{NETWORK_TEST_HOST}:81");
    let different_port_command = format!(
        "python3 -c \"import urllib.request; urllib.request.build_opener(urllib.request.ProxyHandler()).open('{different_port_target}', timeout=2).read()\""
    );
    let denied_responses = mount_exec_network_turn(
        &server,
        "resp-user-network-port",
        "user-network-port",
        network_exec_args(&different_port_command),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "a different port must prompt",
        environments,
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_network_approval_target(
        &test,
        LOCAL_ENVIRONMENT_ID,
        &different_port_target,
        NetworkApprovalProtocol::Http,
    )
    .await?;
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: Some(approval.turn_id),
            decision: ReviewDecision::denied("rejected by user"),
        })
        .await?;
    wait_for_turn_complete(&test).await;
    let denied_output = denied_responses
        .requests()
        .iter()
        .find_map(|request| request.function_call_output_text("user-network-port"))
        .context("expected user-denied network output")?;
    assert!(denied_output.contains("rejected by user"));
    assert!(!denied_output.contains("blocked by policy"));

    let socks_target = format!("socks5-tcp://{NETWORK_TEST_HOST}:443");
    let socks_command = format!(
        r#"python3 -c "import os,socket,urllib.parse; proxy=urllib.parse.urlparse(os.environ['ALL_PROXY']); host='{NETWORK_TEST_HOST}'.encode(); sock=socket.create_connection((proxy.hostname, proxy.port)); sock.sendall(b'\x05\x01\x00'); assert sock.recv(2) == b'\x05\x00'; sock.sendall(b'\x05\x01\x00\x03' + bytes([len(host)]) + host + (443).to_bytes(2, 'big')); print(sock.recv(10))""#
    );
    let abort_response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-user-network-abort"),
            ev_function_call(
                "user-network-abort",
                "exec_command",
                &serde_json::to_string(&network_exec_args(&socks_command))?,
            ),
            ev_completed("resp-user-network-abort"),
        ]),
    )
    .await;
    submit_managed_network_turn(
        &test,
        "a different protocol must prompt and the user abort must stay a user outcome",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_network_approval_target(
        &test,
        LOCAL_ENVIRONMENT_ID,
        &socks_target,
        NetworkApprovalProtocol::Socks5Tcp,
    )
    .await?;
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: Some(approval.turn_id),
            decision: ReviewDecision::Abort,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    abort_response.single_request();

    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn latest_network_rejection_wins_for_multiple_reviews_of_one_execution() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let call_id = "network-multiple-reviews";
    let second_target = format!("http://{NETWORK_TEST_HOST}:81");
    let command = format!(
        "python3 -c \"import threading,urllib.request; threads=[threading.Thread(target=urllib.request.build_opener(urllib.request.ProxyHandler()).open,args=(url,),kwargs={{'timeout': 10}}) for url in ('{NETWORK_TEST_TARGET}','{second_target}')]; [thread.start() for thread in threads]; [thread.join() for thread in threads]\""
    );
    let mut args = network_exec_args(&command);
    args["yield_time_ms"] = json!(10_000);
    let responses =
        mount_exec_network_turn(&server, "resp-network-multiple-reviews", call_id, args).await?;

    submit_managed_network_turn(
        &test,
        "review both network requests from one execution",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;

    let mut approvals = Vec::new();
    for _ in 0..2 {
        let event = wait_for_event_with_timeout(
            &test.codex,
            |event| {
                matches!(
                    event,
                    EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
                )
            },
            Duration::from_secs(30),
        )
        .await;
        let EventMsg::ExecApprovalRequest(approval) = event else {
            anyhow::bail!("execution completed before both network approvals were requested");
        };
        approvals.push(approval);
    }
    assert_eq!(approvals[0].turn_id, approvals[1].turn_id);
    let mut actual_targets = approvals
        .iter()
        .map(|approval| approval.command[1].clone())
        .collect::<Vec<_>>();
    actual_targets.sort();
    let mut expected_targets = vec![NETWORK_TEST_TARGET.to_string(), second_target];
    expected_targets.sort();
    assert_eq!(actual_targets, expected_targets);

    let first_rejection = "first network approval was rejected";
    let latest_rejection = "latest network approval was rejected";
    for (approval, rejection) in approvals
        .into_iter()
        .zip([first_rejection, latest_rejection])
    {
        test.codex
            .submit(Op::ExecApproval {
                id: approval.effective_approval_id(),
                turn_id: Some(approval.turn_id),
                decision: ReviewDecision::denied(rejection),
            })
            .await?;
    }
    wait_for_turn_complete(&test).await;

    let output = responses
        .requests()
        .iter()
        .find_map(|request| request.function_call_output_text(call_id))
        .context("expected output from the execution with multiple network reviews")?;
    assert!(output.contains(latest_rejection), "{output}");
    assert!(!output.contains(first_rejection), "{output}");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn allowing_network_policy_amendment_persists_context_and_bypasses_prompt() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let environments = vec![local(test.config.cwd.clone())];
    let first_responses = mount_exec_network_turn(
        &server,
        "resp-network-amendment-1",
        "network-amendment-1",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "persist an allow rule for this host",
        environments.clone(),
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_network_approval(&test, LOCAL_ENVIRONMENT_ID).await?;
    let amendments = approval
        .proposed_network_policy_amendments
        .clone()
        .context("expected network policy amendments")?;
    assert_eq!(
        amendments,
        vec![
            NetworkPolicyAmendment {
                host: NETWORK_TEST_HOST.to_string(),
                action: NetworkPolicyRuleAction::Allow,
            },
            NetworkPolicyAmendment {
                host: NETWORK_TEST_HOST.to_string(),
                action: NetworkPolicyRuleAction::Deny,
            },
        ]
    );
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: Some(approval.turn_id),
            decision: ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment: amendments[0].clone(),
            },
        })
        .await?;
    wait_for_turn_complete(&test).await;

    let policy = fs::read_to_string(test.home.path().join("rules/default.rules"))?;
    assert!(policy.contains(
        r#"network_rule(host="codex-network-test.invalid", protocol="http", decision="allow""#
    ));
    assert!(first_responses.requests().iter().any(|request| {
        request.body_contains_text(
            "Allowed network rule saved in execpolicy (allowlist): codex-network-test.invalid",
        )
    }));
    mount_exec_network_turn(
        &server,
        "resp-network-amendment-2",
        "network-amendment-2",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "reuse the persisted allow rule",
        environments,
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_completion_without_network_prompt(&test).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn denying_network_policy_amendment_persists_and_blocks_request() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let responses = mount_exec_network_turn(
        &server,
        "resp-network-deny-amendment",
        "network-deny-amendment",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "persist a deny rule for this host",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_network_approval(&test, LOCAL_ENVIRONMENT_ID).await?;
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: Some(approval.turn_id),
            decision: ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment: NetworkPolicyAmendment {
                    host: NETWORK_TEST_HOST.to_string(),
                    action: NetworkPolicyRuleAction::Deny,
                },
            },
        })
        .await?;
    wait_for_turn_complete(&test).await;

    let policy = fs::read_to_string(test.home.path().join("rules/default.rules"))?;
    assert!(policy.contains(
        r#"network_rule(host="codex-network-test.invalid", protocol="http", decision="deny""#
    ));
    let output = responses
        .requests()
        .iter()
        .find_map(|request| request.function_call_output_text("network-deny-amendment"))
        .context("expected denied network tool output")?;
    assert!(output.contains("rejected by user"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn failed_network_policy_amendment_denies_request_and_does_not_approve_host() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let environments = vec![local(test.config.cwd.clone())];
    let first_responses = mount_exec_network_turn(
        &server,
        "resp-network-failed-amendment-1",
        "network-failed-amendment-1",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "reject an invalid network policy amendment",
        environments.clone(),
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_network_approval(&test, LOCAL_ENVIRONMENT_ID).await?;
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: Some(approval.turn_id),
            decision: ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment: NetworkPolicyAmendment {
                    host: "not-the-approved-host.invalid".to_string(),
                    action: NetworkPolicyRuleAction::Allow,
                },
            },
        })
        .await?;
    wait_for_turn_complete(&test).await;

    let denied_output = first_responses
        .requests()
        .iter()
        .find_map(|request| request.function_call_output_text("network-failed-amendment-1"))
        .context("expected the failed policy amendment to reject the network request")?;
    assert!(denied_output.contains("blocked by policy"));
    mount_exec_network_turn(
        &server,
        "resp-network-failed-amendment-2",
        "network-failed-amendment-2",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "a failed policy amendment must not approve the host for the session",
        environments,
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    let approval = expect_network_approval(&test, LOCAL_ENVIRONMENT_ID).await?;
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: Some(approval.turn_id),
            decision: ReviewDecision::denied("reject the retried network request"),
        })
        .await?;
    wait_for_turn_complete(&test).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn unattributed_network_request_uses_active_turn_environment_fallback() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses a raw TCP proxy fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let pending_model = mount_response_once_match(
        &server,
        |request: &wiremock::Request| request_body_contains(request, "hold the active turn"),
        sse_response(sse(vec![
            ev_response_created("resp-unattributed-network"),
            ev_assistant_message("msg-unattributed-network", "done"),
            ev_completed("resp-unattributed-network"),
        ]))
        .set_delay(Duration::from_secs(30)),
    )
    .await;
    submit_managed_network_turn(
        &test,
        "hold the active turn",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_response_request(&pending_model).await;

    let proxy_addr = test
        .session_configured
        .network_proxy
        .as_ref()
        .context("expected managed network proxy")?
        .http_addr
        .clone();
    let proxy_request = tokio::spawn(raw_http_proxy_request(proxy_addr, NETWORK_TEST_HOST));
    let approval = expect_network_approval(&test, LOCAL_ENVIRONMENT_ID).await?;
    assert_eq!(approval.command, ["network-access", NETWORK_TEST_TARGET]);
    assert_eq!(approval.cwd, test.config.cwd);
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: Some(approval.turn_id),
            decision: ReviewDecision::Approved,
        })
        .await?;
    let response = tokio::time::timeout(Duration::from_secs(10), proxy_request)
        .await
        .context("unattributed proxy request did not finish")???;
    assert!(response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.1 502"));

    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn ambiguous_unattributed_network_request_is_not_assigned_to_active_calls() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses POSIX shell and raw TCP fixtures");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let first_marker = test.cwd.path().join("ambiguous-network-first");
    let second_marker = test.cwd.path().join("ambiguous-network-second");
    let wait_command = |marker: &std::path::Path| {
        format!(
            "touch '{}' && while true; do sleep 1; done",
            marker.display()
        )
    };
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| request_body_contains(request, "start two active commands"),
        sse(vec![
            ev_response_created("resp-ambiguous-network"),
            ev_function_call(
                "ambiguous-network-first",
                "exec_command",
                &serde_json::to_string(&network_exec_args(&wait_command(&first_marker)))?,
            ),
            ev_function_call(
                "ambiguous-network-second",
                "exec_command",
                &serde_json::to_string(&network_exec_args(&wait_command(&second_marker)))?,
            ),
            ev_completed("resp-ambiguous-network"),
        ]),
    )
    .await;
    submit_managed_network_turn(
        &test,
        "start two active commands",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_paths(&[&first_marker, &second_marker]).await?;

    let proxy_addr = test
        .session_configured
        .network_proxy
        .as_ref()
        .context("expected managed network proxy")?
        .http_addr
        .clone();
    let response = tokio::time::timeout(
        Duration::from_secs(10),
        raw_http_proxy_request(proxy_addr, NETWORK_TEST_HOST),
    )
    .await
    .context("ambiguous proxy request did not finish")??;
    assert!(response.starts_with("HTTP/1.1 403"));
    assert!(
        tokio::time::timeout(
            Duration::from_secs(1),
            wait_for_event(&test.codex, |event| matches!(
                event,
                EventMsg::ExecApprovalRequest(_)
            ))
        )
        .await
        .is_err(),
        "ambiguous request was incorrectly assigned to an active call"
    );

    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    test.codex.submit(Op::CleanBackgroundTerminals).await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if test.codex.list_background_terminals().await.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("timed out waiting for background terminal cleanup")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn guardian_receives_exact_triggers_for_concurrent_network_requests() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let barrier_dir = TempDir::new_in(test.cwd.path())?;
    let first_marker = barrier_dir.path().join("first");
    let second_marker = barrier_dir.path().join("second");
    let network_command = |marker: &PathBuf, peer_marker: &PathBuf, host: &str| {
        format!(
            "touch '{}' && while [ ! -e '{}' ]; do sleep 0.01; done && python3 -c \"import urllib.request; urllib.request.build_opener(urllib.request.ProxyHandler()).open('http://{host}', timeout=10).read()\"",
            marker.display(),
            peer_marker.display(),
        )
    };
    let first_command = network_command(&first_marker, &second_marker, "1.1.1.1");
    let second_command = network_command(&second_marker, &first_marker, "8.8.8.8");
    let mut first_args = network_exec_args(&first_command);
    first_args["yield_time_ms"] = json!(10_000);
    let mut second_args = network_exec_args(&second_command);
    second_args["yield_time_ms"] = json!(10_000);
    let first_denial = "first concurrent network request denied";
    let second_denial = "second concurrent network request denied";
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            !is_guardian_request(request)
                && request_body_contains(request, "run both network requests")
                && !request_body_contains(request, "exec-network-first")
        },
        sse(vec![
            ev_response_created("resp-network-concurrent"),
            ev_function_call(
                "exec-network-first",
                "exec_command",
                &serde_json::to_string(&first_args)?,
            ),
            ev_function_call(
                "exec-network-second",
                "exec_command",
                &serde_json::to_string(&second_args)?,
            ),
            ev_completed("resp-network-concurrent"),
        ]),
    )
    .await;
    let first_guardian = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| guardian_request_is_for(request, "exec-network-first"),
        sse(vec![
            ev_response_created("resp-network-guardian-1"),
            ev_assistant_message(
                "msg-network-guardian-1",
                &json!({
                    "risk_level": "high",
                    "user_authorization": "low",
                    "outcome": "deny",
                    "rationale": first_denial,
                })
                .to_string(),
            ),
            ev_completed("resp-network-guardian-1"),
        ]),
    )
    .await;
    let second_guardian = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| guardian_request_is_for(request, "exec-network-second"),
        sse(vec![
            ev_response_created("resp-network-guardian-2"),
            ev_assistant_message(
                "msg-network-guardian-2",
                &json!({
                    "risk_level": "high",
                    "user_authorization": "low",
                    "outcome": "deny",
                    "rationale": second_denial,
                })
                .to_string(),
            ),
            ev_completed("resp-network-guardian-2"),
        ]),
    )
    .await;
    let final_response = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            !is_guardian_request(request)
                && request_body_contains(request, "exec-network-first")
                && request_body_contains(request, "exec-network-second")
        },
        sse(vec![
            ev_response_created("resp-network-done"),
            ev_assistant_message("msg-network-done", "done"),
            ev_completed("resp-network-done"),
        ]),
    )
    .await;

    submit_managed_network_turn(
        &test,
        "run both network requests",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::AutoReview,
        AskForApproval::OnRequest,
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let actual_triggers = loop {
        let mut actual_triggers = guardian_network_triggers(&[&first_guardian, &second_guardian])?;
        actual_triggers.sort_unstable();
        actual_triggers.dedup();
        if actual_triggers.len() == 2 {
            break actual_triggers;
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for both Guardian network reviews");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    wait_for_turn_complete(&test).await;

    assert_eq!(
        actual_triggers,
        vec![
            ("exec-network-first".to_string(), first_command),
            ("exec-network-second".to_string(), second_command),
        ]
    );
    let requests = final_response.requests();
    let first_output = requests
        .iter()
        .find_map(|request| request.function_call_output_text("exec-network-first"))
        .context("expected first concurrent tool output")?;
    let second_output = requests
        .iter()
        .find_map(|request| request.function_call_output_text("exec-network-second"))
        .context("expected second concurrent tool output")?;
    assert!(first_output.contains(first_denial));
    assert!(!first_output.contains(second_denial));
    assert!(second_output.contains(second_denial));
    assert!(!second_output.contains(first_denial));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "requires the trusted Linux proxy bridge"
)]
async fn guardian_receives_exact_trigger_for_single_network_request() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let command = "python3 -c \"import urllib.request; opener = urllib.request.build_opener(urllib.request.ProxyHandler()); print('OK:' + opener.open('http://1.1.1.1', timeout=10).read().decode(errors='replace'))\"".to_string();
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-network-single"),
                ev_function_call(
                    "exec-network-single",
                    "exec_command",
                    &serde_json::to_string(&network_exec_args(&command))?,
                ),
                ev_completed("resp-network-single"),
            ]),
            sse(vec![
                ev_response_created("resp-network-guardian"),
                ev_assistant_message("msg-network-guardian", r#"{"outcome":"deny"}"#),
                ev_completed("resp-network-guardian"),
            ]),
            sse(vec![
                ev_response_created("resp-network-done"),
                ev_assistant_message("msg-network-done", "done"),
                ev_completed("resp-network-done"),
            ]),
        ],
    )
    .await;

    submit_managed_network_turn(
        &test,
        "run one network request",
        vec![local(test.config.cwd.clone())],
        ApprovalsReviewer::AutoReview,
        AskForApproval::OnRequest,
    )
    .await?;
    wait_for_turn_complete(&test).await;

    assert_eq!(
        guardian_network_triggers(&[&responses])?,
        vec![("exec-network-single".to_string(), command)]
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_guardian_network_decisions_are_scoped_to_each_request_and_environment() -> Result<()>
{
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let remote = test.executor_environment().selection().clone();
    assert_eq!(remote.environment_id, REMOTE_ENVIRONMENT_ID);
    let session_call_id = "remote-guardian-session-approval";
    let session_command = remote_network_proxy_request_command("REMOTE_GUARDIAN_SESSION");
    let session_probe_call_id = "remote-guardian-session-probe";
    let session_probe_command =
        remote_network_proxy_request_command("REMOTE_GUARDIAN_SESSION_PROBE");

    let cases = [
        (
            "remote-guardian-deny",
            remote_network_proxy_request_command("REMOTE_GUARDIAN_DENY"),
            remote.clone(),
            "The first remote request must be denied.",
            false,
        ),
        (
            "remote-guardian-allow",
            remote_network_proxy_request_command("REMOTE_GUARDIAN_ALLOW"),
            remote.clone(),
            "This remote request is safe to allow once.",
            true,
        ),
        (
            "remote-guardian-deny-again",
            remote_network_proxy_request_command("REMOTE_GUARDIAN_DENY_AGAIN"),
            remote.clone(),
            "A previous remote approval must not approve a later request.",
            false,
        ),
        (
            "local-guardian-deny",
            remote_network_proxy_request_command("LOCAL_GUARDIAN_DENY"),
            local(test.cwd.path().abs()),
            "A remote approval must not approve the local environment.",
            false,
        ),
    ];

    let mut scripted_responses = Vec::with_capacity(cases.len() * 3 + 4);
    for (call_id, command, _, rationale, approved) in &cases {
        if *call_id == "local-guardian-deny" {
            scripted_responses.push(sse(vec![
                ev_response_created("resp-remote-guardian-session-approval-parent"),
                ev_function_call(
                    session_call_id,
                    "exec_command",
                    &serde_json::to_string(&network_exec_args(&session_command))?,
                ),
                ev_completed("resp-remote-guardian-session-approval-parent"),
            ]));
            scripted_responses.push(sse(vec![
                ev_response_created("resp-remote-guardian-session-approval-done"),
                ev_assistant_message("msg-remote-guardian-session-approval-done", "done"),
                ev_completed("resp-remote-guardian-session-approval-done"),
            ]));
        }
        scripted_responses.push(sse(vec![
            ev_response_created(&format!("resp-{call_id}-parent")),
            ev_function_call(
                call_id,
                "exec_command",
                &serde_json::to_string(&network_exec_args(command))?,
            ),
            ev_completed(&format!("resp-{call_id}-parent")),
        ]));
        let assessment = json!({
            "risk_level": if *approved { "low" } else { "high" },
            "user_authorization": if *approved { "high" } else { "low" },
            "outcome": if *approved { "allow" } else { "deny" },
            "rationale": rationale,
        });
        scripted_responses.push(sse(vec![
            ev_response_created(&format!("resp-{call_id}-guardian")),
            ev_assistant_message(&format!("msg-{call_id}-guardian"), &assessment.to_string()),
            ev_completed(&format!("resp-{call_id}-guardian")),
        ]));
        scripted_responses.push(sse(vec![
            ev_response_created(&format!("resp-{call_id}-done")),
            ev_assistant_message(&format!("msg-{call_id}-done"), "done"),
            ev_completed(&format!("resp-{call_id}-done")),
        ]));
    }
    scripted_responses.push(sse(vec![
        ev_response_created("resp-remote-guardian-session-probe-parent"),
        ev_function_call(
            session_probe_call_id,
            "exec_command",
            &serde_json::to_string(&network_exec_args(&session_probe_command))?,
        ),
        ev_completed("resp-remote-guardian-session-probe-parent"),
    ]));
    scripted_responses.push(sse(vec![
        ev_response_created("resp-remote-guardian-session-probe-done"),
        ev_assistant_message("msg-remote-guardian-session-probe-done", "done"),
        ev_completed("resp-remote-guardian-session-probe-done"),
    ]));
    let responses = mount_sse_sequence(&server, scripted_responses).await;

    for (call_id, _, environment, _, _) in &cases {
        if *call_id == "local-guardian-deny" {
            submit_managed_network_turn(
                &test,
                "approve the remote destination for this session",
                vec![remote.clone()],
                ApprovalsReviewer::User,
                AskForApproval::OnRequest,
            )
            .await?;
            let approval = expect_network_approval(&test, REMOTE_ENVIRONMENT_ID).await?;
            test.codex
                .submit(Op::ExecApproval {
                    id: approval.effective_approval_id(),
                    turn_id: Some(approval.turn_id),
                    decision: ReviewDecision::ApprovedForSession,
                })
                .await?;
            tokio::time::timeout(Duration::from_secs(15), wait_for_turn_complete(&test))
                .await
                .context("remote session approval should complete")?;
        }
        let prompt = format!("review network request {call_id}");
        submit_managed_network_turn(
            &test,
            &prompt,
            vec![environment.clone()],
            ApprovalsReviewer::AutoReview,
            AskForApproval::OnRequest,
        )
        .await?;
        tokio::time::timeout(
            Duration::from_secs(15),
            wait_for_completion_without_network_prompt(&test),
        )
        .await
        .with_context(|| format!("Guardian review for {call_id} should complete"))?;
    }
    submit_managed_network_turn(
        &test,
        "verify the remote session approval remains active",
        vec![remote],
        ApprovalsReviewer::User,
        AskForApproval::OnRequest,
    )
    .await?;
    tokio::time::timeout(
        Duration::from_secs(15),
        wait_for_completion_without_network_prompt(&test),
    )
    .await
    .context("remote session approval should bypass another network prompt")?;

    let mut expected_actions = Vec::with_capacity(cases.len());
    for (call_id, command, environment, _, _) in &cases {
        let cwd = environment
            .cwd
            .to_abs_path()
            .with_context(|| format!("resolve the environment cwd for {call_id}"))?;
        expected_actions.push(json!({
            "host": NETWORK_TEST_HOST,
            "port": 80,
            "protocol": "http",
            "target": NETWORK_TEST_TARGET,
            "tool": "network_access",
            "trigger": {
                "callId": call_id,
                "command": ["/bin/sh", "-c", command],
                "cwd": cwd,
                "sandboxPermissions": "use_default",
                "toolName": "exec_command",
                "tty": false,
            },
        }));
    }
    assert_eq!(guardian_network_actions(&responses)?, expected_actions);

    let requests = responses.requests();
    for (call_id, marker) in [
        (session_call_id, "REMOTE_GUARDIAN_SESSION:HTTP/1.1 502"),
        (
            session_probe_call_id,
            "REMOTE_GUARDIAN_SESSION_PROBE:HTTP/1.1 502",
        ),
    ] {
        let output = requests
            .iter()
            .find_map(|request| request.function_call_output_text(call_id))
            .with_context(|| format!("expected remote session network output for {call_id}"))?;
        assert!(output.contains(marker));
        assert!(!output.contains("rejected"));
    }
    for (call_id, _, _, rationale, approved) in &cases {
        let output = requests
            .iter()
            .find_map(|request| request.function_call_output_text(call_id))
            .with_context(|| format!("expected network tool output for {call_id}"))?;
        if *approved {
            assert!(output.contains("REMOTE_GUARDIAN_ALLOW:HTTP/1.1 502"));
            assert!(!output.contains("rejected"));
        } else {
            assert!(output.contains(rationale));
            assert!(!output.contains("HTTP/1.1 502"));
        }
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approved_network_host_for_one_environment_still_prompts_in_another() -> Result<()> {
    skip_if_target_windows!(Ok(()), "uses the POSIX/Python network fixture");
    skip_if_host_windows!(Ok(()));
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_no_remote_env!(Ok(()));

    let server = start_mock_server().await;
    let test = managed_network_unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-network-approval-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_cwd_uri = PathUri::from_host_native_path(&remote_cwd)?;
    test.fs()
        .create_directory(
            &remote_cwd_uri,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    let environments = vec![
        local(local_cwd.path().abs()),
        TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: PathUri::from_abs_path(&remote_cwd),
            workspace_roots: vec![PathUri::from_abs_path(&remote_cwd)],
        },
    ];

    mount_exec_network_turn(
        &server,
        "resp-network-local",
        "exec-network-local",
        network_fetch_args(LOCAL_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "fetch from the local environment",
        environments.clone(),
        ApprovalsReviewer::User,
        AskForApproval::UnlessTrusted,
    )
    .await?;
    let approval = expect_network_approval(&test, LOCAL_ENVIRONMENT_ID).await?;
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;
    wait_for_turn_complete(&test).await;

    let remote_responses = mount_exec_network_turn(
        &server,
        "resp-network-remote",
        "exec-network-remote",
        network_fetch_args(REMOTE_ENVIRONMENT_ID),
    )
    .await?;
    submit_managed_network_turn(
        &test,
        "fetch from the remote environment",
        environments.clone(),
        ApprovalsReviewer::User,
        AskForApproval::UnlessTrusted,
    )
    .await?;
    let approval = expect_network_approval(&test, REMOTE_ENVIRONMENT_ID).await?;
    let rejection = "approval request failed because the client disconnected";
    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::denied(rejection),
        })
        .await?;
    wait_for_turn_complete(&test).await;
    assert_eq!(
        remote_responses.function_call_output_text("exec-network-remote"),
        Some(rejection.to_string())
    );

    test.fs()
        .remove(
            &remote_cwd_uri,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

async fn managed_network_unified_exec_test(server: &wiremock::MockServer) -> Result<TestCodex> {
    managed_network_unified_exec_test_with_features(server, &[]).await
}

async fn managed_network_unified_exec_test_with_features(
    server: &wiremock::MockServer,
    features: &[Feature],
) -> Result<TestCodex> {
    let home = Arc::new(TempDir::new()?);
    fs::write(
        home.path().join("config.toml"),
        r#"default_permissions = "workspace"

[permissions.workspace.filesystem]
":minimal" = "read"

[permissions.workspace.network]
enabled = true
mode = "limited"
allow_local_binding = true
"#,
    )?;
    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Enabled,
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );
    let permission_profile_for_config = permission_profile.clone();
    let features = features.to_vec();
    let mut builder = test_codex()
        .with_home(home)
        .with_cloud_config_bundle(managed_network_requirements_loader())
        .with_config(move |config| {
            config.use_experimental_unified_exec_tool = true;
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("test config should allow feature update");
            for feature in &features {
                config
                    .features
                    .enable(*feature)
                    .expect("test config should allow feature update");
            }
            config.permissions.approval_policy = Constrained::allow_any(approval_policy);
            config
                .permissions
                .set_permission_profile(permission_profile_for_config)
                .expect("set permission profile");
        });
    let test = builder.build_with_remote_and_local_env(server).await?;
    assert!(
        test.config.managed_network_requirements_enabled(),
        "expected managed network requirements to be enabled"
    );
    assert!(
        test.config.permissions.network.is_some(),
        "expected managed network proxy config to be present"
    );
    test.session_configured
        .network_proxy
        .as_ref()
        .expect("expected runtime managed network proxy addresses");

    Ok(test)
}

async fn mount_exec_network_turn(
    server: &wiremock::MockServer,
    response_prefix: &str,
    call_id: &str,
    args: Value,
) -> Result<ResponseMock> {
    let responses = vec![
        sse(vec![
            ev_response_created(&format!("{response_prefix}-1")),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed(&format!("{response_prefix}-1")),
        ]),
        sse(vec![
            ev_response_created(&format!("{response_prefix}-2")),
            ev_assistant_message(&format!("{response_prefix}-msg"), "done"),
            ev_completed(&format!("{response_prefix}-2")),
        ]),
    ];
    Ok(mount_sse_sequence(server, responses).await)
}

fn network_fetch_args(environment_id: &str) -> Value {
    let command = format!(
        "python3 -c \"import urllib.request; opener = urllib.request.build_opener(urllib.request.ProxyHandler()); print('OK:' + opener.open('http://{NETWORK_TEST_HOST}', timeout=2).read().decode(errors='replace'))\""
    );
    let mut args = network_exec_args(&command);
    args["environment_id"] = json!(environment_id);
    args
}

fn network_exec_args(command: &str) -> Value {
    json!({
        "shell": "/bin/sh",
        "cmd": command,
        "login": false,
        "yield_time_ms": 1_000,
    })
}

fn remote_network_proxy_request_command(marker: &str) -> String {
    let host = NETWORK_TEST_HOST;
    format!(
        "python3 -c \"import os,socket,urllib.parse; proxy = urllib.parse.urlparse(os.environ['HTTP_PROXY']); sock = socket.create_connection((proxy.hostname, proxy.port), timeout=10); sock.sendall(b'GET http://{host} HTTP/1.1\\r\\nHost: {host}\\r\\nConnection: close\\r\\n\\r\\n'); print('{marker}:' + sock.makefile('rb').readline().decode(errors='replace'))\""
    )
}

async fn submit_managed_network_turn(
    test: &TestCodex,
    prompt: &str,
    environments: Vec<TurnEnvironmentSelection>,
    approvals_reviewer: ApprovalsReviewer,
    approval_policy: AskForApproval,
) -> Result<()> {
    let permission_profile = PermissionProfile::workspace_write_with(
        &[],
        NetworkSandboxPolicy::Enabled,
        /*exclude_tmpdir_env_var*/ false,
        /*exclude_slash_tmp*/ false,
    );
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(permission_profile, test.config.cwd.as_path());
    let turn_environment_selections =
        TurnEnvironmentSelections::new(test.config.cwd.clone(), environments);

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(turn_environment_selections),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(approvals_reviewer),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    Ok(())
}

fn decoded_request_body(request: &wiremock::Request) -> Option<Vec<u8>> {
    let is_zstd = request
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|entry| entry.trim().eq_ignore_ascii_case("zstd"))
        });
    if is_zstd {
        zstd::stream::decode_all(std::io::Cursor::new(&request.body)).ok()
    } else {
        Some(request.body.clone())
    }
}

fn request_body_contains(request: &wiremock::Request, text: &str) -> bool {
    decoded_request_body(request)
        .and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
}

fn is_guardian_request(request: &wiremock::Request) -> bool {
    decoded_request_body(request)
        .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
        .is_some_and(|body| {
            body.pointer("/client_metadata/x-openai-subagent")
                .and_then(Value::as_str)
                == Some("guardian")
        })
}

fn guardian_request_is_for(request: &wiremock::Request, call_id: &str) -> bool {
    decoded_request_body(request)
        .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
        .filter(|body| {
            body.pointer("/client_metadata/x-openai-subagent")
                .and_then(Value::as_str)
                == Some("guardian")
        })
        .and_then(|body| {
            body.get("input")
                .and_then(Value::as_array)
                .and_then(|input| {
                    input
                        .iter()
                        .rev()
                        .find(|item| item.get("role").and_then(Value::as_str) == Some("user"))
                })
                .cloned()
        })
        .is_some_and(|latest_user_message| latest_user_message.to_string().contains(call_id))
}

fn guardian_network_triggers(responses: &[&ResponseMock]) -> Result<Vec<(String, String)>> {
    responses
        .iter()
        .flat_map(|responses| responses.requests())
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .map(|request| {
            let user_texts = request.message_input_texts("user");
            let action: Value = serde_json::from_str(
                user_texts
                    .iter()
                    .rev()
                    .find(|text| text.contains("\"tool\": \"network_access\""))
                    .context("expected network access JSON in Guardian request")?
                    .trim(),
            )?;
            Ok((
                action
                    .pointer("/trigger/callId")
                    .and_then(Value::as_str)
                    .context("expected exact trigger call id")?
                    .to_string(),
                action
                    .pointer("/trigger/command/2")
                    .and_then(Value::as_str)
                    .context("expected exact trigger command")?
                    .to_string(),
            ))
        })
        .collect()
}

fn guardian_network_actions(responses: &ResponseMock) -> Result<Vec<Value>> {
    responses
        .requests()
        .into_iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
                && request
                    .message_input_texts("user")
                    .iter()
                    .any(|text| text.contains("\"tool\": \"network_access\""))
        })
        .map(|request| {
            let user_texts = request.message_input_texts("user");
            serde_json::from_str(
                user_texts
                    .iter()
                    .rev()
                    .find(|text| text.contains("\"tool\": \"network_access\""))
                    .context("expected network access JSON in Guardian request")?
                    .trim(),
            )
            .context("parse Guardian network action")
        })
        .collect()
}

async fn expect_network_approval(
    test: &TestCodex,
    expected_environment_id: &str,
) -> Result<ExecApprovalRequestEvent> {
    expect_network_approval_target(
        test,
        expected_environment_id,
        NETWORK_TEST_TARGET,
        NetworkApprovalProtocol::Http,
    )
    .await
}

async fn expect_network_approval_target(
    test: &TestCodex,
    expected_environment_id: &str,
    expected_target: &str,
    expected_protocol: NetworkApprovalProtocol,
) -> Result<ExecApprovalRequestEvent> {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let remaining = deadline
        .checked_duration_since(std::time::Instant::now())
        .context("timed out waiting for network approval request")?;
    let event = wait_for_event_with_timeout(
        &test.codex,
        |event| {
            matches!(
                event,
                EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
            )
        },
        remaining,
    )
    .await;
    match event {
        EventMsg::ExecApprovalRequest(approval) => {
            assert_eq!(
                approval.command,
                vec!["network-access".to_string(), expected_target.to_string()]
            );
            assert_eq!(
                approval.network_approval_context,
                Some(NetworkApprovalContext {
                    host: NETWORK_TEST_HOST.to_string(),
                    protocol: expected_protocol,
                })
            );
            assert_eq!(
                approval.environment_id.as_deref(),
                Some(expected_environment_id)
            );
            Ok(approval)
        }
        EventMsg::TurnComplete(_) => {
            panic!("expected network approval request before completion");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion_without_network_prompt(test: &TestCodex) {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    match event {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ExecApprovalRequest(approval) => {
            panic!(
                "unexpected network approval request: {:?}",
                approval.command
            )
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_response_request(responses: &ResponseMock) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if !responses.requests().is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for Responses API request");
}

async fn raw_http_proxy_request(proxy_addr: String, host: &str) -> std::io::Result<String> {
    let mut stream = tokio::net::TcpStream::connect(proxy_addr).await?;
    stream
        .write_all(
            format!("GET http://{host}/ HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;
    Ok(String::from_utf8_lossy(&response).into_owned())
}

async fn wait_for_paths(paths: &[&std::path::Path]) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if paths.iter().all(|path| path.exists()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("timed out waiting for commands to start")?;
    Ok(())
}

async fn wait_for_turn_complete(test: &TestCodex) {
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
}
