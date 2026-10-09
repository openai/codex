//! Verifies that reused Guardian reviewers use Blue for Daybreak without changing parent programs.

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::TurnStartOptions;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::approvals::GuardianAssessmentStatus;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::turn_input::CyberAccessProgram;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[test_case(CodexAuth::from_api_key("test-api-key"); "api_key")]
#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(); "chatgpt")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_reused_reviewer_uses_blue_without_changing_parent_cyber_access_program(
    auth: CodexAuth,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_auth(auth)
        .with_model_info_override("gpt-6.1-sol", |_| {})
        .with_model_info_override("gpt-5.5", |model| {
            model.guardian = None;
            model.auto_review_model_override = Some("gpt-6.1-sol".to_owned());
        })
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .set_legacy_sandbox_policy(SandboxPolicy::new_workspace_write_policy())
                .expect("set sandbox policy");
            config
                .features
                .enable(Feature::ApiKeyCyberAccessPrograms)
                .expect("enable API-key Cyber access programs");
        })
        .build_with_auto_env(&server)
        .await?;

    let cases = [
        (
            Some(CyberAccessProgram::DaybreakBlue),
            Some("daybreak_blue"),
        ),
        (Some(CyberAccessProgram::DaybreakRed), Some("daybreak_red")),
        (Some(CyberAccessProgram::Standard), Some("standard")),
        (None, None),
    ];
    let mut events = Vec::new();
    for (index, _) in cases.iter().enumerate() {
        events.push(responses::sse(vec![
            responses::ev_function_call(
                &format!("command-{index}"),
                "exec_command",
                &json!({
                    "cmd": format!("echo guardian-program-{index}"),
                    "sandbox_permissions": "require_escalated",
                    "justification": "Run the requested command.",
                })
                .to_string(),
            ),
            responses::ev_completed(&format!("parent-call-{index}")),
        ]));
        events.push(responses::sse(vec![
            responses::ev_assistant_message(
                &format!("approval-{index}"),
                r#"{"risk_level":"low","user_authorization":"high","outcome":"allow","rationale":"requested command"}"#,
            ),
            responses::ev_completed(&format!("review-{index}")),
        ]));
        events.push(responses::sse(vec![responses::ev_completed(&format!(
            "parent-done-{index}"
        ))]));
    }
    let requests = responses::mount_sse_sequence(&server, events).await;

    for (index, (program, _)) in cases.iter().enumerate() {
        test.codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: format!("Run echo guardian-program-{index}."),
                    text_elements: Vec::new(),
                }])
                .on_start(TurnStartOptions {
                    cyber_access_program: *program,
                    ..Default::default()
                }),
            )
            .await?;
        let mut assessments = Vec::new();
        loop {
            match test.codex.next_event().await?.msg {
                EventMsg::GuardianAssessment(assessment) => assessments.push(assessment.status),
                EventMsg::TurnComplete(_) => break,
                _ => {}
            }
        }
        assert_eq!(
            assessments,
            vec![
                GuardianAssessmentStatus::InProgress,
                GuardianAssessmentStatus::Approved,
            ]
        );
    }

    let requests = requests.requests();
    let (guardian_requests, parent_requests): (Vec<_>, Vec<_>) = requests
        .iter()
        .map(responses::ResponsesRequest::body_json)
        .partition(|body| body["client_metadata"]["x-openai-subagent"] == "guardian");
    let expected_parent_programs = cases
        .iter()
        .map(|(_, name)| name.map(|name| json!({ "cyber": name })))
        .collect::<Vec<_>>();
    let expected_guardian_programs = [
        Some(json!({ "cyber": "daybreak_blue" })),
        Some(json!({ "cyber": "daybreak_blue" })),
        Some(json!({ "cyber": "standard" })),
        None,
    ];
    assert_eq!(
        guardian_requests
            .iter()
            .map(|body| (
                body["model"].clone(),
                body["reasoning"]["effort"].clone(),
                body.get("access_programs").cloned(),
            ))
            .collect::<Vec<_>>(),
        expected_guardian_programs
            .iter()
            .map(|program| (json!("gpt-6.1-sol"), json!("low"), program.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        parent_requests
            .iter()
            .map(|body| (body["model"].clone(), body.get("access_programs").cloned()))
            .collect::<Vec<_>>(),
        expected_parent_programs
            .iter()
            .flat_map(|program| std::iter::repeat_n((json!("gpt-5.5"), program.clone()), 2))
            .collect::<Vec<_>>()
    );
    let reviewer_thread_ids = guardian_requests
        .iter()
        .map(|body| {
            body["client_metadata"]["thread_id"]
                .as_str()
                .expect("Guardian request should have a thread id")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reviewer_thread_ids,
        vec![reviewer_thread_ids[0]; cases.len()],
        "program changes should reuse the reviewer session"
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
