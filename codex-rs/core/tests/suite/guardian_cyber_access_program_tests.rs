//! Verifies reviewer program selection and parent-only enrollment across reviewer reuse.

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::TurnStartOptions;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::approvals::GuardianAssessmentStatus;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
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

#[test_case(CodexAuth::from_api_key("test-api-key"), false; "api_key")]
#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(), false; "chatgpt")]
#[test_case(CodexAuth::from_api_key("test-api-key"), true; "api_key_parent_only")]
#[test_case(CodexAuth::create_dummy_chatgpt_auth_for_testing(), true; "chatgpt_parent_only")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_reused_reviewer_respects_parent_program_scope(
    auth: CodexAuth,
    parent_only: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(
        Ok(()),
        "Guardian approval actions require host-native paths"
    );

    let server = responses::start_mock_server().await;
    let mut test = test_codex()
        .with_auth(auth)
        .with_model_info_override("gpt-6.1-sol", |_| {})
        .with_model_info_override("gpt-5.5", |model| {
            model.guardian = None;
            model.auto_review_model_override = Some("gpt-6.1-sol".to_owned());
        })
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.workspace_roots_explicit = false;
            let permission_profile = PermissionProfile::from_legacy_sandbox_policy_for_cwd(
                &SandboxPolicy::new_workspace_write_policy(),
                config.cwd.as_path(),
            );
            config
                .permissions
                .can_set_permission_profile(&permission_profile)
                .expect("set permission profile");
            config
                .permissions
                .set_workspace_roots(vec![config.cwd.clone()]);
            config
                .permissions
                .set_permission_profile(permission_profile)
                .expect("set permission profile");
            config.workspace_roots = config.permissions.workspace_roots().to_vec();
            config
                .features
                .enable(Feature::ApiKeyCyberAccessPrograms)
                .expect("enable API-key Cyber access programs");
        })
        .build_with_auto_env(&server)
        .await?;
    if parent_only {
        let mut options = test.start_thread_options().await;
        options
            .thread_extension_init
            .insert(codex_core::RequiredCyberAccessPrograms {
                allowed: vec![
                    CyberAccessProgram::DaybreakBlue,
                    CyberAccessProgram::DaybreakRed,
                    CyberAccessProgram::Standard,
                ],
            });
        test.codex.shutdown_and_wait().await?;
        test.codex = test.thread_manager.start_thread(options).await?.thread;
        test.codex
            .thread_extension_data()
            .remove::<codex_core::RequiredCyberAccessPrograms>();
    }

    let mut cases = vec![
        (
            Some(CyberAccessProgram::DaybreakBlue),
            Some("daybreak_blue"),
        ),
        (Some(CyberAccessProgram::DaybreakRed), Some("daybreak_red")),
        (Some(CyberAccessProgram::Standard), Some("standard")),
        (None, None),
    ];
    if parent_only {
        cases.pop();
    }
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
    let expected_guardian_programs = if parent_only {
        vec![None; cases.len()]
    } else {
        vec![
            Some(json!({ "cyber": "daybreak_blue" })),
            Some(json!({ "cyber": "daybreak_blue" })),
            Some(json!({ "cyber": "standard" })),
            None,
        ]
    };
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
