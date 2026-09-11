//! Ensures Guardian authorization survives compaction and internal context, but not user changes.

use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_core::context::ContextualUserFragment;
use codex_core::context::InternalContextSource;
use codex_core::context::InternalModelContextFragment;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::skip_if_wine_exec;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use test_case::test_case;
use tokio::sync::oneshot;

#[derive(Clone, Copy)]
enum PendingReviewChange {
    UserInstruction,
    VerifiedAnswer,
}

#[test_case(PendingReviewChange::UserInstruction; "new user instruction")]
#[test_case(PendingReviewChange::VerifiedAnswer; "verified answer")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_revalidates_owning_session_before_allow(
    change: PendingReviewChange,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let (parent_completion_tx, parent_completion_rx) = oneshot::channel();
    let (review_completion_tx, review_completion_rx) = oneshot::channel();
    let parent_followup = if matches!(change, PendingReviewChange::VerifiedAnswer) {
        responses::sse(vec![
            responses::ev_function_call(
                "confirm-command",
                "request_user_input",
                &json!({"questions": [{
                    "id": "execute", "header": "Execute", "question": "May I run the command?",
                    "options": [
                        {"label": "Yes", "description": "Run the command."},
                        {"label": "No", "description": "Do not run the command."}
                    ]
                }]})
                .to_string(),
            ),
            responses::ev_completed("parent-followup"),
        ])
    } else {
        responses::sse(vec![responses::ev_completed("parent-followup")])
    };
    let (streaming_server, _) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![
                    responses::ev_response_created("parent-start"),
                    responses::ev_custom_tool_call(
                        "reviewed-tool",
                        "exec",
                        "yield_control(); await tools.exec_command({cmd: 'printf authorized', login: false, sandbox_permissions: 'require_escalated', justification: 'Exercise Guardian authorization freshness.'});",
                    ),
                ]),
            },
            StreamingSseChunk {
                gate: Some(parent_completion_rx),
                body: responses::sse(vec![responses::ev_completed("parent-start")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: Some(review_completion_rx),
            body: responses::sse(vec![
                responses::ev_response_created("review"),
                responses::ev_assistant_message(
                    "review-result",
                    &json!({
                        "risk_level": "low", "user_authorization": "high", "outcome": "allow",
                        "rationale": "The user authorized this command in the reviewed snapshot.",
                    }).to_string(),
                ),
                responses::ev_completed("review"),
            ]),
        }],
        vec![StreamingSseChunk { gate: None, body: parent_followup }],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![responses::ev_completed("user-change")]),
        }],
    ]).await;
    let base_url = format!("{}/v1", streaming_server.uri());
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_model("test-gpt-5.1-codex")
        .with_config(move |config| {
            config.model_provider.base_url = Some(base_url);
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .set_legacy_sandbox_policy(SandboxPolicy::new_workspace_write_policy())
                .expect("set sandbox policy");
            for feature in [
                Feature::GuardianThreadContext,
                Feature::CodeMode,
                Feature::CodeModeInterrupt,
                Feature::DefaultModeRequestUserInput,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable test feature");
            }
        })
        .with_code_mode_host_program(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?)
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Run the command in a background cell.".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                ..Default::default()
            }),
        )
        .await?;

    // The parent stream cannot finish until Guardian has captured its prompt, so the
    // second request is deterministically the pending review, not a parent follow-up.
    tokio::time::timeout(
        Duration::from_secs(/*secs*/ 10),
        streaming_server.wait_for_request_count(/*count*/ 2),
    )
    .await
    .context("Guardian review did not start")?;
    let review: Value = serde_json::from_slice(&streaming_server.requests().await[1])?;
    assert_eq!(
        review.pointer("/client_metadata/x-openai-subagent"),
        Some(&json!("guardian"))
    );
    let before = test.codex.guardian_authorization_version().await;
    parent_completion_tx
        .send(())
        .expect("release parent completion");
    if matches!(change, PendingReviewChange::VerifiedAnswer) {
        let request = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::RequestUserInput(request) => Some(request.clone()),
            _ => None,
        })
        .await;
        test.codex
            .submit(Op::UserInputAnswer {
                id: request.turn_id,
                response: RequestUserInputResponse {
                    answers: HashMap::from([(
                        "execute".to_owned(),
                        RequestUserInputAnswer {
                            answers: vec!["No. Do not run the command.".to_owned()],
                        },
                    )]),
                },
            })
            .await?;
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    if matches!(change, PendingReviewChange::UserInstruction) {
        test.submit_text_turn("Do not run the command.").await?;
    }
    assert_ne!(before, test.codex.guardian_authorization_version().await);
    review_completion_tx
        .send(())
        .expect("release Guardian response");
    let status = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::GuardianAssessment(assessment)
            if assessment.status != GuardianAssessmentStatus::InProgress =>
        {
            Some(assessment.status)
        }
        _ => None,
    })
    .await;
    assert_eq!(status, GuardianAssessmentStatus::Aborted);
    test.codex.shutdown_and_wait().await?;
    streaming_server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_authorization_revision_survives_compaction_not_user_input() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("initial")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("enable context windows");
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("Inspect the deployment.").await?;
    let mut expected = test.codex.guardian_authorization_version().await;

    let internal_context = InternalModelContextFragment::new(
        InternalContextSource::from_static("goal"),
        "Inspecting the deployment.",
    );
    let notification_text = internal_context.render();
    test.codex
        .inject_response_items(vec![ContextualUserFragment::into(internal_context)])
        .await?;
    assert_eq!(test.codex.guardian_authorization_version().await, expected);

    // The same text submitted by the user must invalidate, even if it looks internal.
    responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("user-followup")]),
    )
    .await;
    test.submit_text_turn(&notification_text).await?;
    expected.user_message_revision += 1;
    assert_eq!(test.codex.guardian_authorization_version().await, expected);

    // Failed image preparation must not turn a real user message into internal context.
    responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("user-image")]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Image {
            image_url: "data:image/png;base64,not-an-image".to_owned(),
            detail: None,
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    expected.user_message_revision += 1;
    assert_eq!(test.codex.guardian_authorization_version().await, expected);

    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(test.codex.guardian_authorization_version().await, expected);

    Ok(())
}
