//! Review requests preserve required evidence while fitting the aggregate budget.
//! Parent token-budget mode must not replace Guardian's summary compaction.

use anyhow::Result;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_core::config::CurrentTimeReminderConfig;
use codex_core::config::RolloutBudgetConfig;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::AutoReviewMessages;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(160_000, &[0, 0]; "complete_instructions_fit")]
#[test_case(28_000, &[0, 1]; "oversized_history_compacts_then_truncates")]
#[test_case(16_000, &[1]; "first_review_can_shorten_oversized_instructions")]
async fn review_preserves_user_instructions_until_request_budgeting(
    window: i64,
    compactions_per_turn: &[usize],
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_model_info_override("gpt-5.6-luna", move |model| {
            model.context_window = Some(window);
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.auto_review_model_override = Some("gpt-5.6-luna".to_owned());
        })
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;
    let command = json!({
        "cmd": "echo complete-instructions",
        "sandbox_permissions": "require_escalated",
        "justification": "Run the requested command."
    })
    .to_string();
    let events = compactions_per_turn
        .iter()
        .enumerate()
        .flat_map(|(turn, compactions)| {
            let mut events = vec![
                sse(vec![
                    ev_function_call(&format!("exec-{turn}"), "exec_command", &command),
                    ev_completed(&format!("parent-{turn}")),
                ]),
                sse(vec![
                    ev_assistant_message(
                        "decision",
                        r#"{"risk_level":"low","user_authorization":"high","outcome":"allow"}"#,
                    ),
                    ev_completed(&format!("review-{turn}")),
                ]),
                sse(vec![
                    ev_assistant_message("done", "done"),
                    ev_completed(&format!("done-{turn}")),
                ]),
            ];
            for _ in 0..*compactions {
                events.insert(
                    /*index*/ 1,
                    sse(vec![
                        json!({
                            "type": "response.output_item.done",
                            "item": {"type": "compaction", "encrypted_content": "Earlier reviewer context."},
                        }),
                        ev_completed("review-compaction"),
                    ]),
                );
            }
            events
        })
        .collect();
    let responses = responses::mount_sse_sequence(&server, events).await;
    let padding = "é🙂 ".repeat(/*n*/ 3_500);
    let initial = format!(
        "{padding}Run the requested echo command. You may edit scratch files only.{padding}"
    );
    test.submit_text_turn(&initial).await?;
    // This whole message exceeds the old transcript allowance. A following
    // restriction must still reach the reviewer, with the original source order.
    let followup = format!("{padding}{padding}Keep all files private.{padding}{padding}");
    let approval = format!(
        "{}\nApproved action: {command}",
        codex_guardian_context::MANUAL_APPROVAL_DEVELOPER_PREFIX
    );
    let restriction = "Revoke permission to edit files. Only run the echo command.";
    // V2 retains the first review's user input. The smallest window exercises
    // first-review truncation only; the larger windows cover follow-up delivery.
    if compactions_per_turn.len() == 2 {
        test.codex
            .inject_response_items(vec![
                responses::user_message_item(&followup),
                ResponseItem::Message {
                    id: None,
                    role: "developer".to_owned(),
                    content: vec![ContentItem::InputText {
                        text: approval.clone(),
                    }],
                    phase: None,
                    internal_chat_message_metadata_passthrough: None,
                },
            ])
            .await?;
        test.submit_text_turn(restriction).await?;
    }
    let (compact_requests, requests): (Vec<_>, Vec<_>) = responses
        .requests()
        .into_iter()
        .partition(|request| !request.inputs_of_type("compaction_trigger").is_empty());
    let reviews = requests
        .iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    assert_eq!(reviews.len(), compactions_per_turn.len());
    let initial_inputs = reviews[0].message_input_text_groups("user");
    let initial_context = initial_inputs.last().expect("first review input").concat();
    if compactions_per_turn[0] == 0 {
        assert!(initial_context.contains(&initial));
    } else {
        assert!(initial_context.contains("<truncated omitted_approx_tokens="));
    }
    assert_eq!(
        compact_requests.len(),
        compactions_per_turn.iter().sum::<usize>()
    );
    if let Some(review) = reviews.get(1) {
        let delta_inputs = review.message_input_text_groups("user");
        let delta = delta_inputs.last().expect("delta review input");
        let delta_context = delta.concat();
        if window == 160_000 {
            assert!(delta_context.contains(&followup));
        } else {
            assert!(delta_context.contains("<truncated omitted_approx_tokens="));
            assert!(
                delta_context.contains(
                    "User instructions and prior approvals may be incomplete where marked."
                )
            );
        }
        let approval_start = delta_context
            .find(&approval)
            .expect("complete prior approval");
        let restriction_start = delta_context.find(restriction).expect("later restriction");
        assert!(approval_start + approval.len() < restriction_start);
        assert!(
            !delta_context.contains(&initial),
            "old entries are not resent"
        );
        assert_eq!(
            reviews[0].body_json()["client_metadata"]["thread_id"],
            review.body_json()["client_metadata"]["thread_id"]
        );
    }
    assert!(
        requests
            .last()
            .expect("parent resumes after review")
            .function_call_output(&format!("exec-{}", reviews.len() - 1))
            .to_string()
            .contains("complete-instructions")
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[derive(Clone, Copy)]
enum ReviewerResponse {
    Decision,
    ToolContinuation,
    UncompactableContinuation,
    CompactionError,
    NextReview,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(1, ReviewerResponse::Decision; "required_context_fails_closed")]
#[test_case(4_500, ReviewerResponse::ToolContinuation; "oversized_tool_continuation_compacts")]
#[test_case(4_500, ReviewerResponse::UncompactableContinuation; "ineffective_compaction_fails_closed")]
#[test_case(4_500, ReviewerResponse::CompactionError; "compaction_service_error_does_not_request_user_approval")]
#[test_case(6_000, ReviewerResponse::NextReview; "incoming_review_compacts_existing_history")]
async fn review_respects_complete_context_budget(
    window: i64,
    reviewer_response: ReviewerResponse,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                if matches!(reviewer_response, ReviewerResponse::CompactionError) {
                    ""
                } else {
                    "[auto_review]\nrequired_on_models = [\"gpt-5.5\"]\n"
                },
            ),
        )
        .with_model_info_override("gpt-5.6-luna", move |model| {
            model.context_window = Some(window);
            model
                .model_messages
                .as_mut()
                .expect("bundled reviewer model messages")
                .auto_review = Some(AutoReviewMessages {
                policy: Some("Review actions against user authorization.".to_owned()),
                policy_template: Some("{{ tenant_policy_config }}".to_owned()),
                node_repl_policy: None,
                rejection_instructions: None,
                timeout_instructions: None,
            });
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.auto_review_model_override = Some("gpt-5.6-luna".to_owned());
        })
        .with_config(move |config| {
            config
                .features
                .enable(Feature::TokenBudget)
                .expect("parent uses token-budget mode while Guardian uses summary compaction");
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .features
                .enable(Feature::CurrentTimeReminder)
                .expect("test permits time reminders");
            config.current_time_reminder = Some(CurrentTimeReminderConfig::default());
            config.rollout_budget = Some(RolloutBudgetConfig {
                limit_tokens: 1_000_000,
                reminder_at_remaining_tokens: Vec::new(),
                sampling_token_weight: 1.0,
                prefill_token_weight: 1.0,
            });
        });
    if matches!(
        reviewer_response,
        ReviewerResponse::ToolContinuation
            | ReviewerResponse::UncompactableContinuation
            | ReviewerResponse::CompactionError
    ) {
        builder = builder
            .with_code_mode_host_program(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?);
    }
    let test = builder.build_with_auto_env(&server).await?;
    let command = json!({"cmd": "echo guardian-budget-test", "sandbox_permissions": "require_escalated", "justification": "Run the requested command."}).to_string();
    let mut commentary =
        ev_assistant_message("commentary", &"optional old commentary ".repeat(/*n*/ 900));
    commentary["item"]["phase"] = json!("commentary");
    let mut events = vec![
        sse(vec![
            ev_response_created("parent-action"),
            commentary,
            ev_function_call("exec-over-budget", "exec_command", &command),
            ev_completed("parent-action"),
        ]),
        sse(vec![
            ev_response_created("parent-done"),
            ev_assistant_message("done", "done"),
            ev_completed("parent-done"),
        ]),
    ];
    if window > 1 {
        events.insert(
            /*index*/ 1,
            sse(vec![
                ev_response_created("review"),
                match reviewer_response {
                    ReviewerResponse::Decision => ev_assistant_message(
                        "decision",
                        r#"{"risk_level":"low","user_authorization":"high","outcome":"allow"}"#,
                    ),
                    // V2 retains user review inputs. Give it assistant history to
                    // discard so compaction makes room for the next review.
                    ReviewerResponse::NextReview => ev_assistant_message(
                        "decision",
                        &json!({
                            "risk_level": "low",
                            "user_authorization": "high",
                            "outcome": "allow",
                            "rationale": "Previous review reasoning. ".repeat(/*n*/ 256),
                        })
                        .to_string(),
                    ),
                    ReviewerResponse::ToolContinuation
                    | ReviewerResponse::UncompactableContinuation
                    | ReviewerResponse::CompactionError => ev_custom_tool_call(
                        "reviewer-inspect",
                        "exec",
                        "text('inspection-output'.repeat(600));",
                    ),
                },
                ev_completed("review"),
            ]),
        );
    }
    if matches!(reviewer_response, ReviewerResponse::ToolContinuation) {
        events.insert(
            /*index*/ 2,
            sse(vec![
                ev_assistant_message(
                    "after-compaction",
                    r#"{"risk_level":"low","user_authorization":"high","outcome":"allow"}"#,
                ),
                ev_completed("after-compaction"),
            ]),
        );
    }
    if matches!(
        reviewer_response,
        ReviewerResponse::UncompactableContinuation
            | ReviewerResponse::NextReview
            | ReviewerResponse::CompactionError
    ) {
        events.extend([
            sse(vec![
                ev_function_call("retry-command", "exec_command", &command),
                ev_completed("retry-action"),
            ]),
            sse(vec![
                ev_assistant_message(
                    "retry-decision",
                    r#"{"risk_level":"low","user_authorization":"high","outcome":"allow"}"#,
                ),
                ev_completed("retry-review"),
            ]),
            sse(vec![
                ev_assistant_message("retry-done", "done"),
                ev_completed("retry-done"),
            ]),
        ]);
    }
    if !matches!(
        reviewer_response,
        ReviewerResponse::Decision | ReviewerResponse::CompactionError
    ) {
        let summary = if matches!(
            reviewer_response,
            ReviewerResponse::UncompactableContinuation
        ) {
            "still oversized ".repeat(/*n*/ 4_000)
        } else {
            "Previous review evidence and inspection results.".to_owned()
        };
        let index = if matches!(reviewer_response, ReviewerResponse::NextReview) {
            4
        } else {
            2
        };
        events.insert(
            index,
            sse(vec![
                json!({
                    "type": "response.output_item.done",
                    "item": {"type": "compaction", "encrypted_content": summary},
                }),
                ev_completed("review-compaction"),
            ]),
        );
    }
    let mut events = events
        .into_iter()
        .map(responses::sse_response)
        .collect::<Vec<_>>();
    if matches!(reviewer_response, ReviewerResponse::CompactionError) {
        events.insert(
            /*index*/ 2,
            wiremock::ResponseTemplate::new(400).set_body_json(json!({
                "error": {"code": "context_length_exceeded", "message": "compaction service context window exceeded"}
            })),
        );
    }
    let responses = responses::mount_response_sequence(&server, events).await;
    test.submit_text_turn("Run the command if the approval reviewer allows it.")
        .await?;
    let (compact_requests, requests): (Vec<_>, Vec<_>) = responses
        .requests()
        .into_iter()
        .partition(|request| !request.inputs_of_type("compaction_trigger").is_empty());
    let guardian_requests = requests
        .iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .collect::<Vec<_>>();
    if window == 1 {
        assert_eq!(requests.len(), 2);
        assert!(guardian_requests.is_empty());
        let output = requests[1].function_call_output("exec-over-budget");
        assert!(
            output.to_string().contains("reviewer input budget"),
            "expected required-evidence budget rejection: {output}"
        );
    } else {
        let recovered = matches!(reviewer_response, ReviewerResponse::ToolContinuation);
        assert_eq!(requests.len(), if recovered { 4 } else { 3 });
        assert_eq!(guardian_requests.len(), if recovered { 2 } else { 1 });
        if recovered {
            assert_eq!(compact_requests.len(), 1);
            let compact = &compact_requests[0];
            assert!(
                compact
                    .body_json()
                    .to_string()
                    .contains("inspection-output")
            );
            let recovered = guardian_requests[1].body_json();
            assert!(
                recovered["input"]
                    .to_string()
                    .contains("Previous review evidence")
            );
            assert_eq!(
                guardian_requests[0].body_json()["client_metadata"]["thread_id"],
                recovered["client_metadata"]["thread_id"]
            );
            assert!(
                requests
                    .last()
                    .expect("parent resumes after review")
                    .function_call_output("exec-over-budget")
                    .to_string()
                    .contains("guardian-budget-test")
            );
        }
        if matches!(
            reviewer_response,
            ReviewerResponse::UncompactableContinuation
        ) {
            let output = requests
                .last()
                .expect("parent continues after the rejected review")
                .function_call_output("exec-over-budget");
            assert!(
                output.to_string().contains("reviewer input budget"),
                "expected context-budget rejection: {output}"
            );
        }
        if matches!(reviewer_response, ReviewerResponse::CompactionError) {
            assert!(
                requests
                    .last()
                    .expect("parent resumes after review")
                    .function_call_output("exec-over-budget")
                    .to_string()
                    .contains("context window")
            );
        }
        let request = guardian_requests[0];
        let context = request.message_input_texts("user").join("\n");
        assert!(context.contains("<guardian_context_omission>"));
        assert!(!context.contains("optional old commentary"));
        assert!(context.contains("Run the command if the approval reviewer allows it."));
        assert!(context.contains("echo guardian-budget-test"));
        let developer_context = request.message_input_texts("developer").join("\n");
        assert!(developer_context.contains("<current_time_reminder>"));
        assert!(developer_context.contains("<rollout_budget>"));
        if matches!(
            reviewer_response,
            ReviewerResponse::UncompactableContinuation
                | ReviewerResponse::NextReview
                | ReviewerResponse::CompactionError
        ) {
            let instruction = if matches!(reviewer_response, ReviewerResponse::NextReview) {
                "New instructions: retry the requested command and keep all files private. "
                    .repeat(/*n*/ 12)
            } else {
                "Retry the requested command.".to_owned()
            };
            test.submit_text_turn(&instruction).await?;
            let (compact_requests, requests): (Vec<_>, Vec<_>) = responses
                .requests()
                .into_iter()
                .partition(|request| !request.inputs_of_type("compaction_trigger").is_empty());
            assert_eq!(
                requests.len(),
                6,
                "parent resumed with: {}",
                requests
                    .last()
                    .expect("parent resumes after the retry")
                    .function_call_output("retry-command")
            );
            assert_eq!(compact_requests.len(), 1);
            if matches!(reviewer_response, ReviewerResponse::NextReview) {
                let compact = &compact_requests[0];
                assert!(
                    !compact
                        .body_json()
                        .to_string()
                        .contains("New instructions:"),
                    "incoming evidence must remain pending during compaction"
                );
                assert!(
                    requests[4].body_json()["input"]
                        .to_string()
                        .contains("New instructions:")
                );
                assert!(
                    requests[4].body_json()["input"]
                        .to_string()
                        .contains("Previous review evidence")
                );
                assert_eq!(
                    request.body_json()["client_metadata"]["thread_id"],
                    requests[4].body_json()["client_metadata"]["thread_id"]
                );
            } else {
                assert_ne!(
                    request.body_json()["client_metadata"]["thread_id"],
                    requests[4].body_json()["client_metadata"]["thread_id"],
                    "failed compaction must retire the reviewer"
                );
            }
            assert!(
                requests[5]
                    .function_call_output("retry-command")
                    .to_string()
                    .contains("guardian-budget-test")
            );
        }
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[derive(Clone, Copy)]
enum OversizedActionReview {
    Fits,
    UserFallback,
    RequiredGuardian,
}

#[test_case(OversizedActionReview::Fits; "large_action_receives_automatic_review")]
#[test_case(OversizedActionReview::UserFallback; "optional_review_requests_user_approval")]
#[test_case(OversizedActionReview::RequiredGuardian; "required_review_rejects_incomplete_action")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_action_preserves_review_policy_and_next_review(
    review: OversizedActionReview,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );
    let server = responses::start_mock_server().await;
    let mut builder = test_codex()
        .with_model_info_override("gpt-5.5", |model| {
            model.auto_review_model_override = Some("gpt-5.6-luna".to_owned());
        })
        .with_model_info_override("gpt-5.6-luna", move |model| {
            model.context_window = Some(if matches!(review, OversizedActionReview::Fits) {
                160_000
            } else {
                16_000
            });
        })
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        });
    if matches!(review, OversizedActionReview::RequiredGuardian) {
        builder = builder.with_model("gpt-5.5").with_cloud_config_bundle(
            CloudConfigBundleFixture::loader_with_enterprise_requirement(
                "[auto_review]\nrequired_on_models = [\"gpt-5.5\"]\n",
            ),
        );
    }
    let test = builder.build_with_auto_env(&server).await?;
    let oversized_command =
        "echo ".to_owned() + &"large-action".repeat(/*n*/ 20_000) + "required suffix";
    let oversized = json!({
        "cmd": oversized_command,
        "sandbox_permissions": "require_escalated",
        "justification": "Run the requested command.",
    })
    .to_string();
    let next = json!({
        "cmd": "echo complete-action-review",
        "sandbox_permissions": "require_escalated",
        "justification": "Run the requested command.",
    })
    .to_string();
    let mut events = vec![
        sse(vec![
            ev_function_call("oversized", "exec_command", &oversized),
            ev_completed("oversized"),
        ]),
        sse(vec![
            ev_function_call("next", "exec_command", &next),
            ev_completed("next"),
        ]),
        sse(vec![
            ev_assistant_message("decision", r#"{"outcome":"allow"}"#),
            ev_completed("review"),
        ]),
        sse(vec![
            ev_assistant_message("done", "done"),
            ev_completed("done"),
        ]),
    ];
    if matches!(review, OversizedActionReview::Fits) {
        // A real policy denial must not fall through to a user prompt. Avoid
        // executing the huge command while asserting its complete review input.
        events.insert(
            /*index*/ 1,
            sse(vec![
                ev_assistant_message(
                    "decision",
                    r#"{"outcome":"deny","rationale":"denied by classifier"}"#,
                ),
                ev_completed("large-review"),
            ]),
        );
    }
    let response = responses::mount_sse_sequence(&server, events).await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Run the commands if the approval reviewer allows them.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let mut terminal_assessment = None;
    let event = wait_for_event(&test.codex, |event| {
        if let EventMsg::GuardianAssessment(assessment) = event
            && assessment.status != codex_protocol::protocol::GuardianAssessmentStatus::InProgress
        {
            terminal_assessment = Some((assessment.status, assessment.risk_level));
        }
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    if let EventMsg::ExecApprovalRequest(approval) = event {
        assert!(
            matches!(review, OversizedActionReview::UserFallback),
            "only exhausted optional review may ask the user"
        );
        assert_eq!(
            terminal_assessment,
            Some((
                codex_protocol::protocol::GuardianAssessmentStatus::Aborted,
                None
            ))
        );
        assert_eq!(approval.command.last(), Some(&oversized_command));
        test.codex
            .submit(Op::ExecApproval {
                id: approval.effective_approval_id(),
                turn_id: None,
                decision: ReviewDecision::denied("rejected by user"),
            })
            .await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    } else {
        assert!(
            !matches!(review, OversizedActionReview::UserFallback),
            "exhausted optional review must request user approval"
        );
    }
    let requests = response.requests();
    let fits = matches!(review, OversizedActionReview::Fits);
    assert_eq!(requests.len(), if fits { 5 } else { 4 });
    assert!(
        requests[if fits { 2 } else { 1 }]
            .function_call_output("oversized")
            .to_string()
            .contains(match review {
                OversizedActionReview::Fits => "denied by classifier",
                OversizedActionReview::UserFallback => "rejected by user",
                OversizedActionReview::RequiredGuardian => "reviewer input budget",
            })
    );
    let reviews = requests
        .iter()
        .filter(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .collect::<Vec<_>>();
    assert_eq!(reviews.len(), if fits { 2 } else { 1 });
    if fits {
        let parts = reviews[0].message_input_texts("user");
        assert!(parts.concat().contains(&oversized_command));
        assert!(parts.iter().all(|part| part.len() <= 36_000));
    }
    let context = reviews
        .last()
        .expect("review of the next action")
        .message_input_texts("user")
        .concat();
    assert!(context.contains("echo complete-action-review"));
    assert!(
        requests
            .last()
            .expect("parent resumes after review")
            .function_call_output("next")
            .to_string()
            .contains("complete-action-review")
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
