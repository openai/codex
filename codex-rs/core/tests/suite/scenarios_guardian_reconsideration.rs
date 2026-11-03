//! Denial reconsideration preserves review context and publishes only the final decision.

use anyhow::Context;
use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use test_case::test_case;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SecondPass {
    Assessment(&'static str),
    Failure,
    Timeout,
}

#[test_case(false, "deny", None, None, GuardianAssessmentStatus::Denied; "disabled")]
#[test_case(true, "allow", None, None, GuardianAssessmentStatus::Approved; "allow_unchanged")]
#[test_case(true, "deny", Some(SecondPass::Assessment(r#"{"outcome":"allow"}"#)), None, GuardianAssessmentStatus::Approved; "deny_then_allow")]
#[test_case(true, "deny", Some(SecondPass::Assessment(r#"{"outcome":"deny"}"#)), None, GuardianAssessmentStatus::Denied; "deny_then_deny")]
#[test_case(true, "deny", Some(SecondPass::Assessment("invalid assessment")), None, GuardianAssessmentStatus::Denied; "invalid_confirmation_keeps_denial")]
#[test_case(true, "deny", Some(SecondPass::Failure), None, GuardianAssessmentStatus::Denied; "failed_confirmation_keeps_denial")]
#[test_case(true, "deny", Some(SecondPass::Timeout), None, GuardianAssessmentStatus::Denied; "timed_out_confirmation_keeps_denial")]
#[test_case(true, "deny", Some(SecondPass::Assessment(r#"{"outcome":"deny"}"#)), Some("For this review only, examine the authorization evidence again. Return the same JSON schema."), GuardianAssessmentStatus::Denied; "configured_prompt")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_denial_reconsideration(
    enabled: bool,
    first: &str,
    second: Option<SecondPass>,
    prompt: Option<&'static str>,
    expected_status: GuardianAssessmentStatus,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));
    let builder = test_codex()
        .with_model("gpt-5.5")
        .with_config(move |config| {
            super::configure_scenario_catalog(config);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permission profile");
            config.guardian_denial_reconsideration_prompt = prompt.map(str::to_owned);
            if second == Some(SecondPass::Timeout) {
                config.guardian_review_timeout = Some(Duration::from_secs(10));
            }
            // Apply to the selected reviewer too, including per-model reviewer overrides.
            for model in &mut config
                .model_catalog
                .as_mut()
                .expect("scenario model catalog")
                .models
            {
                model.auto_compact_token_limit = Some(100_000);
            }
            if enabled {
                config
                    .features
                    .enable(Feature::GuardianDenialReconsideration)
                    .expect("enable Guardian denial reconsideration");
            }
        });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let test = harness.test();
    let first_assessment = if second == Some(SecondPass::Timeout) {
        json!({
            "outcome": first,
            "risk_level": "high",
            "user_authorization": "low",
            "rationale": "The user did not authorize writing this marker.",
        })
    } else {
        json!({"outcome": first})
    };
    let mut replies = vec![
        sse(vec![
            ev_function_call(
                "reviewed-action",
                "exec_command",
                r#"{"cmd":"echo reconsidered > reconsideration.txt","sandbox_permissions":"require_escalated","justification":"Write the requested marker."}"#,
            ),
            ev_completed("parent-action"),
        ]),
        sse(vec![
            ev_assistant_message("first-assessment", &first_assessment.to_string()),
            // Cross the pre-turn compaction threshold. Reconsideration must preserve
            // the original evidence instead of issuing a compaction request.
            ev_completed_with_tokens("first-review", /*total_tokens*/ 100_001),
        ]),
    ]
    .into_iter()
    .map(responses::sse_response)
    .collect::<Vec<_>>();
    if let Some(second) = second {
        replies.push(match second {
            SecondPass::Assessment(assessment) => responses::sse_response(sse(vec![
                ev_assistant_message("second-assessment", assessment),
                ev_completed("second-review"),
            ])),
            SecondPass::Failure => responses::sse_response(responses::sse_failed(
                "second-review",
                "invalid_prompt",
                "Reconsideration request failed.",
            )),
            // Keep the second response pending beyond the shared deadline. Its
            // eventual allow must not replace the already completed denial.
            SecondPass::Timeout => responses::sse_response(sse(vec![
                ev_assistant_message("late-allow", r#"{"outcome":"allow"}"#),
                ev_completed("second-review"),
            ]))
            .set_delay(Duration::from_secs(60)),
        });
    }
    replies.push(responses::sse_response(sse(vec![
        ev_assistant_message("parent-result", "Finished reviewing the request."),
        ev_completed("parent-done"),
    ])));
    let mock = responses::mount_response_sequence(harness.server(), replies).await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Write the marker file if the security policy permits it.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let mut statuses = Vec::new();
    let mut final_assessment = None;
    loop {
        match test.codex.next_event().await?.msg {
            EventMsg::GuardianAssessment(event) => {
                statuses.push(event.status);
                final_assessment = Some(event);
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }
    assert_eq!(
        statuses,
        vec![GuardianAssessmentStatus::InProgress, expected_status]
    );
    if second == Some(SecondPass::Timeout) {
        let assessment = final_assessment.context("final Guardian assessment")?;
        assert_eq!(
            (
                assessment.risk_level,
                assessment.user_authorization,
                assessment.rationale,
            ),
            (
                Some(GuardianRiskLevel::High),
                Some(GuardianUserAuthorization::Low),
                Some("The user did not authorize writing this marker.".to_owned()),
            )
        );
    }
    assert_eq!(
        harness.path_exists("reconsideration.txt").await?,
        expected_status == GuardianAssessmentStatus::Approved,
    );
    let requests = mock.requests();
    assert_eq!(requests.len(), if second.is_some() { 4 } else { 3 });
    if second == Some(SecondPass::Timeout) {
        let output = requests[3]
            .function_call_output_text("reviewed-action")
            .context("denied tool output")?;
        assert!(output.contains("The user did not authorize writing this marker."));
    }
    if second.is_some() {
        let first_input = requests[1].input();
        let second_input = requests[2].input();
        assert_eq!(&second_input[..first_input.len()], first_input.as_slice());
        let followup = requests[2].message_input_texts("developer");
        let followup = followup
            .last()
            .context("reconsideration developer message")?;
        if let Some(prompt) = prompt {
            assert_eq!(followup, prompt);
        } else {
            assert!(followup.starts_with("For this review only, reconsider your previous denial"));
        }
        assert_eq!(
            requests[1].body_json()["model"],
            requests[2].body_json()["model"]
        );
        assert_eq!(
            requests[1].body_json()["reasoning"],
            requests[2].body_json()["reasoning"]
        );
    }
    if second == Some(SecondPass::Assessment(r#"{"outcome":"allow"}"#)) && prompt.is_none() {
        let mut snapshot = context_snapshot::format_request_history_snapshot(
            "Guardian denies an action, reconsiders it with a review-scoped developer message, and allows it before the command executes. The parent receives only the final result.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        );
        let environment_id = &test.executor_environment().selection().environment_id;
        snapshot = snapshot.replace(&format!("{environment_id:?}"), "\"<ENVIRONMENT>\"");
        for (pattern, replacement) in [
            (r"Chunk ID: [a-z0-9]+", "Chunk ID: <CHUNK_ID>"),
            (
                r"Wall time: [0-9]+(?:\.[0-9]+)? seconds",
                "Wall time: <DURATION> seconds",
            ),
            (r#"(?m)^(\s*"cwd": )"[^"]*""#, "$1\"<CWD>\""),
            (
                r#""command": \[\s*(?:"[^"]*",\s*)*"echo reconsidered > reconsideration.txt"\s*\]"#,
                "\"command\": [\"<SHELL>\", \"echo reconsidered > reconsideration.txt\"]",
            ),
        ] {
            snapshot = regex_lite::Regex::new(pattern)?
                .replace_all(&snapshot, replacement)
                .into_owned();
        }
        insta::assert_snapshot!("guardian_denial_reconsideration", snapshot);
    }
    Ok(())
}
