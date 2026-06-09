#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::sandboxing::SandboxPermissions;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::fs_wait;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_compacts_between_reviews_before_the_next_request() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    const FIRST_REVIEW_TOTAL_TOKENS: i64 = 500_000;
    const COMPACTION_TOTAL_TOKENS: i64 = 50;
    const AUTO_COMPACT_TOKEN_LIMIT: i64 = 200_000;
    const REQUEST_RETRIES: u64 = 0;
    const STREAM_RETRIES: u64 = 0;

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
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config
            .set_legacy_sandbox_policy(sandbox_policy_for_config)
            .expect("set sandbox policy");
        config.model_auto_compact_token_limit = Some(AUTO_COMPACT_TOKEN_LIMIT);
        config.model_provider.request_max_retries = Some(REQUEST_RETRIES);
        config.model_provider.stream_max_retries = Some(STREAM_RETRIES);
        config.model_provider.supports_websockets = false;
        config
            .features
            .enable(Feature::RemoteCompactionV2)
            .expect("enable remote compaction v2");
    });
    let test = builder.build(&server).await?;

    let first_justification = "Approve the first eager-compaction command.";
    let second_justification = "Approve the second eager-compaction command.";
    let first_args = json!({
        "cmd": "sleep 0.2; printf first",
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": first_justification,
    });
    let second_args = json!({
        "cmd": "printf second",
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": second_justification,
    });
    let guardian_assessment = json!({
        "risk_level": "low",
        "user_authorization": "high",
        "outcome": "allow",
        "rationale": "The command writes a bounded marker file requested by the user.",
    })
    .to_string();
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-first-tool"),
                ev_function_call(
                    "exec-first",
                    "exec_command",
                    &serde_json::to_string(&first_args)?,
                ),
                ev_completed("resp-parent-first-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-first"),
                ev_assistant_message("msg-guardian-first", &guardian_assessment),
                ev_completed_with_tokens("resp-guardian-first", FIRST_REVIEW_TOTAL_TOKENS),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-compact"),
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "compaction",
                        "encrypted_content": "EAGER_GUARDIAN_COMPACTED_CONTEXT",
                    }
                }),
                ev_completed_with_tokens("resp-guardian-compact", COMPACTION_TOTAL_TOKENS),
            ]),
            sse(vec![
                ev_response_created("resp-parent-second-tool"),
                ev_function_call(
                    "exec-second",
                    "exec_command",
                    &serde_json::to_string(&second_args)?,
                ),
                ev_completed("resp-parent-second-tool"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-second"),
                ev_assistant_message("msg-guardian-second", &guardian_assessment),
                ev_completed("resp-guardian-second"),
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
                text: "run both commands that require Guardian review".into(),
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

    let requests = responses.requests();
    let first_guardian_index = requests
        .iter()
        .position(|request| request.body_contains_text(first_justification))
        .expect("first Guardian request");
    let second_guardian_index = requests
        .iter()
        .position(|request| request.body_contains_text(second_justification))
        .expect("second Guardian request");
    let compaction_indexes = requests
        .iter()
        .enumerate()
        .filter_map(|(index, request)| {
            request.body_json()["input"]
                .as_array()
                .is_some_and(|input| {
                    input.iter().any(|item| {
                        item.get("type").and_then(Value::as_str) == Some("compaction_trigger")
                    })
                })
                .then_some(index)
        })
        .collect::<Vec<_>>();

    assert_eq!(compaction_indexes.len(), 1);
    assert!(
        first_guardian_index < compaction_indexes[0]
            && compaction_indexes[0] < second_guardian_index,
        "expected eager compaction between Guardian reviews, requests: {requests:#?}"
    );

    Ok(())
}

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
        config
            .set_legacy_sandbox_policy(sandbox_policy_for_config)
            .expect("set sandbox policy");
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
