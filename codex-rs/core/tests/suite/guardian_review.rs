#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentDecisionSource;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::fs_wait;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_response_sequence;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::sse_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;
use tempfile::TempDir;
use wiremock::ResponseTemplate;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_review_session_does_not_inherit_legacy_notify() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };

    let notify_dir = TempDir::new()?;
    let notify_script = notify_dir.path().join("notify.sh");
    fs::write(
        &notify_script,
        r#"#!/bin/bash
set -e
payload_path="$(dirname "${0}")/notify.jsonl"
printf '%s\n' "${@: -1}" >> "${payload_path}""#,
    )?;
    fs::set_permissions(&notify_script, fs::Permissions::from_mode(0o755))?;
    let notify_file = notify_dir.path().join("notify.jsonl");
    let notify_script_str = notify_script.to_str().unwrap().to_string();
    let sandbox_policy_for_config = sandbox_policy.clone();

    let mut builder = test_codex().with_config(move |config| {
        config.notify = Some(vec![notify_script_str]);
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        assert!(
            config
                .set_legacy_sandbox_policy(sandbox_policy_for_config)
                .is_ok(),
            "set sandbox policy"
        );
    });
    let test = builder.build(&server).await?;

    let output_file = test.cwd.path().join("guardian-review-notify.txt");
    let command = format!("printf guardian-approved > {}", output_file.display());
    let tool_args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise Guardian approval routing.",
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-tool"),
                ev_function_call(
                    "exec-call",
                    "exec_command",
                    &serde_json::to_string(&tool_args)?,
                ),
                ev_completed("resp-parent-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-review"),
                ev_assistant_message(
                    "msg-guardian-review",
                    &json!({
                        "risk_level": "low",
                        "user_authorization": "high",
                        "outcome": "allow",
                        "rationale": "The command writes a marker file in the workspace.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian-review"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-done"),
                ev_assistant_message("msg-parent-done", "done"),
                ev_completed("resp-parent-done"),
            ]),
        ],
    )
    .await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "run a command that requires Guardian review".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                sandbox_policy: Some(sandbox_policy),
                ..Default::default()
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let guardian_request = responses
        .requests()
        .into_iter()
        .find(|request| request.body_contains_text("Exercise Guardian approval routing."))
        .expect("expected Guardian review request");
    assert!(guardian_request.body_contains_text(&command));

    fs_wait::wait_for_path_exists(&notify_file, Duration::from_secs(5)).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let notify_payload_raw = tokio::fs::read_to_string(&notify_file).await?;
    let payloads: Vec<Value> = notify_payload_raw
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<_, _>>()?;

    assert_eq!(
        payloads.len(),
        1,
        "unexpected notify payloads: {payloads:?}"
    );
    assert_eq!(
        payloads[0]["input-messages"],
        json!(["run a command that requires Guardian review"])
    );
    assert_eq!(payloads[0]["last-assistant-message"], json!("done"));
    assert!(
        !notify_payload_raw.contains(
            "The following is the Codex agent history whose request action you are assessing."
        ),
        "Guardian review transcript leaked into legacy notify payload: {notify_payload_raw}"
    );
    assert_eq!(fs::read_to_string(&output_file)?, "guardian-approved");

    Ok(())
}

#[derive(Clone, Copy)]
enum GuardianFailurePolicy {
    FailClosed,
    FailOpen,
}

async fn run_guardian_api_failure_command(policy: GuardianFailurePolicy) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = start_mock_server().await;
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![],
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    let sandbox_policy_for_config = sandbox_policy.clone();
    let mut builder = test_codex().with_config(move |config| {
        config.allow_guardian_fail_open = matches!(policy, GuardianFailurePolicy::FailOpen);
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config
            .set_legacy_sandbox_policy(sandbox_policy_for_config)
            .unwrap_or_else(|err| panic!("set sandbox policy: {err}"));
    });
    let test = builder.build(&server).await?;

    let (output_name, output_contents) = match policy {
        GuardianFailurePolicy::FailClosed => {
            ("guardian-failed-closed.txt", "guardian-failed-closed")
        }
        GuardianFailurePolicy::FailOpen => ("guardian-failed-open.txt", "guardian-failed-open"),
    };
    let output_file = test.cwd.path().join(output_name);
    let command = format!("printf {output_contents} > {}", output_file.display());
    let tool_args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise Guardian fail-open routing.",
    });
    let error_message = "Guardian service unavailable";
    let responses = mount_response_sequence(
        &server,
        vec![
            sse_response(sse(vec![
                ev_response_created("resp-parent-tool"),
                ev_function_call(
                    "exec-call",
                    "exec_command",
                    &serde_json::to_string(&tool_args)?,
                ),
                ev_completed("resp-parent-tool"),
            ])),
            ResponseTemplate::new(400).set_body_json(json!({
                "error": {
                    "message": error_message,
                    "type": "invalid_request_error",
                }
            })),
            sse_response(sse(vec![
                ev_response_created("resp-parent-done"),
                ev_assistant_message("msg-parent-done", "done"),
                ev_completed("resp-parent-done"),
            ])),
        ],
    )
    .await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "run a command that requires Guardian review".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
                sandbox_policy: Some(sandbox_policy),
                ..Default::default()
            },
        })
        .await?;

    let mut terminal_assessment = None;
    loop {
        let event = wait_for_event(&test.codex, |_| true).await;
        match event {
            EventMsg::GuardianAssessment(assessment)
                if assessment.status != GuardianAssessmentStatus::InProgress =>
            {
                terminal_assessment = Some(assessment);
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    let Some(assessment) = terminal_assessment else {
        anyhow::bail!("expected terminal Guardian assessment");
    };
    assert!(
        assessment
            .rationale
            .as_deref()
            .is_some_and(|rationale| rationale.contains(error_message))
    );
    match policy {
        GuardianFailurePolicy::FailClosed => {
            assert_eq!(assessment.status, GuardianAssessmentStatus::Denied);
            assert_eq!(
                assessment.decision_source,
                Some(GuardianAssessmentDecisionSource::Agent)
            );
            assert!(!output_file.exists());
        }
        GuardianFailurePolicy::FailOpen => {
            assert_eq!(assessment.status, GuardianAssessmentStatus::Approved);
            assert_eq!(
                assessment.decision_source,
                Some(GuardianAssessmentDecisionSource::OrganizationPolicy)
            );
            assert!(
                assessment
                    .rationale
                    .as_deref()
                    .is_some_and(|rationale| rationale.contains("Organization policy"))
            );
            assert_eq!(fs::read_to_string(&output_file)?, output_contents);
        }
    }

    let Some(guardian_request) = responses
        .requests()
        .into_iter()
        .find(|request| request.body_contains_text("Exercise Guardian fail-open routing."))
    else {
        anyhow::bail!("expected Guardian review request");
    };
    assert!(guardian_request.body_contains_text(&command));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_api_failure_blocks_command_by_default() -> Result<()> {
    run_guardian_api_failure_command(GuardianFailurePolicy::FailClosed).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_api_failure_executes_command_when_organization_allows_fail_open() -> Result<()> {
    run_guardian_api_failure_command(GuardianFailurePolicy::FailOpen).await
}
