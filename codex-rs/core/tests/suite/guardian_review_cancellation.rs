use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use test_case::test_case;

#[derive(Clone, Copy)]
enum Cancellation {
    DirectTool,
    CodeModeTurn,
    CodeModeCell,
}

#[test_case(Cancellation::DirectTool; "direct tool")]
#[test_case(Cancellation::CodeModeTurn; "code mode turn")]
#[test_case(Cancellation::CodeModeCell; "code mode cell")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelling_tool_aborts_its_guardian_review(cancellation: Cancellation) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .set_legacy_sandbox_policy(SandboxPolicy::new_workspace_write_policy())
                .expect("set sandbox policy");
            if !matches!(cancellation, Cancellation::DirectTool) {
                let _ = config.features.enable(Feature::CodeMode);
                let _ = config.features.enable(Feature::CodeModeInterrupt);
            }
        });
    if !matches!(cancellation, Cancellation::DirectTool) {
        builder = builder
            .with_code_mode_host_program(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?);
    }
    let test = builder.build_with_auto_env(&server).await?;
    let output_file = test.cwd.path().join("cancelled-guardian-command.txt");
    let output_path = shlex::try_join([output_file.to_string_lossy().as_ref()])?;
    let tool_args = json!({
        "cmd": format!("printf should-not-run > {output_path}"),
        "sandbox_permissions": "require_escalated",
        "justification": "Exercise tool-owned Guardian cancellation.",
    });
    let tool_call = match cancellation {
        Cancellation::DirectTool => {
            ev_function_call("reviewed-tool", "exec_command", &tool_args.to_string())
        }
        Cancellation::CodeModeTurn => ev_custom_tool_call(
            "reviewed-tool",
            "exec",
            &format!("await tools.exec_command({tool_args});"),
        ),
        Cancellation::CodeModeCell => ev_custom_tool_call(
            "reviewed-tool",
            "exec",
            &format!("yield_control(); await tools.exec_command({tool_args});"),
        ),
    };
    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("parent-start"),
            tool_call,
            ev_completed("parent-start"),
        ]),
    )
    .await;
    let yielded_parent = if matches!(cancellation, Cancellation::CodeModeCell) {
        Some(
            responses::mount_response_once_match(
                &server,
                |request: &wiremock::Request| {
                    let body: Value = serde_json::from_slice(&request.body)
                        .expect("Responses request body should be valid JSON");
                    body["input"].as_array().is_some_and(|items| {
                        items.iter().any(|item| {
                            item["type"] == "custom_tool_call_output"
                                && item["call_id"] == "reviewed-tool"
                        })
                    }) && body
                        .pointer("/client_metadata/x-openai-subagent")
                        .and_then(Value::as_str)
                        != Some("guardian")
                },
                responses::sse_response(sse(vec![
                    ev_assistant_message("parent-yielded", "cell is running"),
                    ev_completed("parent-yielded"),
                ])),
            )
            .await,
        )
    } else {
        None
    };
    let pending_guardian = responses::mount_response_once_match(
        &server,
        |request: &wiremock::Request| {
            serde_json::from_slice::<Value>(&request.body)
                .expect("Responses request body should be valid JSON")
                .pointer("/client_metadata/x-openai-subagent")
                .and_then(Value::as_str)
                == Some("guardian")
        },
        responses::sse_response(sse(vec![
            ev_response_created("pending-review"),
            ev_assistant_message(
                "review-result",
                &json!({
                    "risk_level": "low",
                    "user_authorization": "high",
                    "outcome": "allow",
                    "rationale": "The test must cancel this review before it completes.",
                })
                .to_string(),
            ),
            ev_completed("pending-review"),
        ]))
        .set_delay(Duration::from_secs(60)),
    )
    .await;

    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "start a Guardian-reviewed command".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                ..Default::default()
            }),
        )
        .await?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while pending_guardian.requests().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .context("Guardian review did not start")?;

    if let Some(yielded_parent) = yielded_parent {
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        let output = yielded_parent
            .single_request()
            .custom_tool_call_output("reviewed-tool");
        let output = &output["output"];
        let header = output
            .as_str()
            .or_else(|| output[0]["text"].as_str())
            .context("missing code-mode output")?;
        let cell_id = header
            .strip_prefix("Script running with cell ID ")
            .and_then(|rest| rest.lines().next())
            .context("missing cell id")?;
        responses::mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_function_call(
                        "terminate-cell",
                        "wait",
                        &json!({
                            "cell_id": cell_id,
                            "terminate": true,
                        })
                        .to_string(),
                    ),
                    ev_completed("terminate-cell"),
                ]),
                sse(vec![
                    ev_assistant_message("parent-done", "cell terminated"),
                    ev_completed("parent-done"),
                ]),
            ],
        )
        .await;
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "terminate the background cell".into(),
                text_elements: Vec::new(),
            }]))
            .await?;
    } else {
        test.codex.submit(Op::Interrupt).await?;
    }

    tokio::time::timeout(Duration::from_secs(10), async {
        let mut guardian_aborted = false;
        let mut parent_finished = false;
        while !guardian_aborted || !parent_finished {
            let event = test.codex.next_event().await?;
            match event.msg {
                EventMsg::GuardianAssessment(assessment)
                    if assessment.status == GuardianAssessmentStatus::Aborted =>
                {
                    guardian_aborted = true;
                }
                EventMsg::TurnComplete(_) if matches!(cancellation, Cancellation::CodeModeCell) => {
                    parent_finished = true;
                }
                EventMsg::TurnAborted(_) if !matches!(cancellation, Cancellation::CodeModeCell) => {
                    parent_finished = true;
                }
                _ => {}
            }
        }
        anyhow::Ok(())
    })
    .await
    .context("tool cancellation did not abort Guardian")??;
    assert!(!output_file.exists(), "cancelled command executed");
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
