#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::openai_models::AutoReviewMessages;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::fs_wait;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_wine_exec;
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
async fn guardian_session_prewarms_and_is_reused_for_first_review() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let catalog_template = "Catalog-provided Guardian template:\n{{ tenant_policy_config }}";
    let mut review_model = codex_models_manager::bundled_models_response()?
        .models
        .into_iter()
        .find(|model| model.slug == "codex-auto-review")
        .expect("bundled auto-review model");
    let model_messages = review_model
        .model_messages
        .as_mut()
        .expect("auto-review model messages");
    model_messages.auto_review = Some(AutoReviewMessages {
        policy: None,
        policy_template: Some(catalog_template.to_string()),
    });

    let tool_args = json!({
        "cmd": "true",
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise Guardian approval routing.",
    })
    .to_string();
    let server = start_websocket_server(vec![
        vec![vec![ev_response_created("warm-1"), ev_completed("warm-1")]],
        vec![vec![ev_response_created("warm-2"), ev_completed("warm-2")]],
        vec![vec![
            ev_response_created("approval-request"),
            ev_function_call("approval-call", "exec_command", &tool_args),
            ev_completed("approval-request"),
        ]],
        vec![vec![
            ev_response_created("guardian-review"),
            ev_completed("guardian-review"),
        ]],
    ])
    .await;
    let mut builder = test_codex().with_config(move |config| {
        config.model_catalog = Some(ModelsResponse {
            models: vec![review_model],
        });
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    });

    let test = builder.build_with_websocket_server(&server).await?;
    let (first, second) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
            server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0)
        )
    })
    .await?;
    let prewarm_requests = [first.body_json(), second.body_json()];
    let guardian_prewarm = prewarm_requests
        .iter()
        .find(|request| {
            request["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .expect("guardian startup prewarm request");
    assert_eq!(guardian_prewarm["generate"].as_bool(), Some(false));
    let guardian_instructions = guardian_prewarm["instructions"]
        .as_str()
        .expect("guardian instructions");
    assert!(guardian_instructions.contains("Catalog-provided Guardian template:"));
    assert!(guardian_instructions.contains("- Organization: default generic tenant."));
    assert!(!guardian_instructions.contains("{{ tenant_policy_config }}"));
    assert!(guardian_instructions.contains("final message must be strict JSON"));
    let guardian_thread_id = guardian_prewarm["client_metadata"]["thread_id"]
        .as_str()
        .expect("guardian thread id");

    test.codex
        .submit(
            vec![UserInput::Text {
                text: "run a command that requires Guardian review".into(),
                text_elements: Vec::new(),
            }]
            .into(),
        )
        .await?;
    let guardian_review = tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request(/*connection_index*/ 3, /*request_index*/ 0),
    )
    .await?
    .body_json();
    assert_eq!(
        guardian_review["client_metadata"]["x-openai-subagent"].as_str(),
        Some("guardian")
    );
    assert_eq!(
        guardian_review["client_metadata"]["thread_id"].as_str(),
        Some(guardian_thread_id)
    );
    assert_eq!(guardian_review.get("generate"), None);

    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_denial_rejects_tool_call_with_rationale() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

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
    });
    let test = builder.build_with_auto_env(&server).await?;

    let output_file = test.cwd.path().join("guardian-denied.txt");
    let command = format!("printf should-not-run > {}", output_file.display());
    let tool_args = json!({
        "cmd": command,
        "yield_time_ms": 1_000_u64,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "Exercise Guardian denial routing.",
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-tool-denied"),
                ev_function_call(
                    "exec-call-denied",
                    "exec_command",
                    &serde_json::to_string(&tool_args)?,
                ),
                ev_completed("resp-parent-tool-denied"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-denied"),
                ev_assistant_message(
                    "msg-guardian-denied",
                    &json!({
                        "risk_level": "high",
                        "user_authorization": "low",
                        "outcome": "deny",
                        "rationale": "The requested write has unacceptable test risk.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian-denied"),
            ]),
            sse(vec![
                ev_response_created("resp-parent-after-denial"),
                ev_assistant_message("msg-parent-after-denial", "denied"),
                ev_completed("resp-parent-after-denial"),
            ]),
        ],
    )
    .await;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "run a command that Guardian should deny".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: codex_protocol::protocol::ThreadSettingsOverrides {
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
    let guardian_request = requests
        .iter()
        .find(|request| request.body_contains_text("Exercise Guardian denial routing."))
        .expect("expected Guardian review request");
    assert!(guardian_request.body_contains_text(&command));

    let tool_output = requests
        .iter()
        .find_map(|request| request.function_call_output_text("exec-call-denied"))
        .expect("expected rejected tool output to be returned to the parent model");
    assert!(
        tool_output.contains("The requested write has unacceptable test risk."),
        "Guardian rationale missing from rejected tool output: {tool_output}"
    );
    assert!(
        !output_file.exists(),
        "Guardian-denied command unexpectedly executed"
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
