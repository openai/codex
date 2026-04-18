#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_core::config::Constrained;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_mode_does_not_inject_skills_message() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let codex_home = Arc::new(TempDir::new()?);
    let skill_dir = codex_home.path().join("skills/demo");
    std::fs::create_dir_all(&skill_dir)?;
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: demo\ndescription: build charts\n---\n\n# body\n",
    )?;

    let builder = test_codex().with_home(codex_home);
    let test = builder
        .with_config(|config| {
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.permissions.sandbox_policy =
                Constrained::allow_any(SandboxPolicy::new_read_only_policy());
            config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
            config.inject_skills_message = true;
        })
        .build(&server)
        .await?;

    let call_id = "guardian-shell-call";
    let command = "echo guardian";
    let response_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-parent-1"),
                ev_function_call(
                    call_id,
                    "shell_command",
                    &serde_json::to_string(&json!({
                        "command": command,
                        "timeout_ms": 1_000_u64,
                    }))?,
                ),
                ev_completed("resp-parent-1"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian"),
                ev_assistant_message(
                    "msg-guardian",
                    &json!({
                        "risk_level": "low",
                        "user_authorization": "high",
                        "outcome": "allow",
                        "rationale": "The planned command is a benign echo requested by the test.",
                    })
                    .to_string(),
                ),
                ev_completed("resp-guardian"),
            ]),
            sse(vec![
                ev_assistant_message("msg-parent-2", "done"),
                ev_completed("resp-parent-2"),
            ]),
        ],
    )
    .await;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "run a benign command".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: Some(ApprovalsReviewer::GuardianSubagent),
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: test.session_configured.model.clone(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = response_log.requests();
    let guardian_request = requests
        .iter()
        .find(|request| {
            request
                .message_input_texts("developer")
                .iter()
                .any(|text| text.contains("You are judging one planned coding-agent action."))
        })
        .expect("guardian review request should be captured");

    assert!(
        requests
            .iter()
            .filter(|request| request.body_json()["model"] == test.session_configured.model)
            .flat_map(|request| request.message_input_texts("developer"))
            .any(|text| text.contains("demo: build charts")),
        "parent request should include the test skill"
    );

    let guardian_developer_text = guardian_request
        .message_input_texts("developer")
        .join("\n\n");
    assert!(
        !guardian_developer_text.contains("## Skills"),
        "guardian request should not include skills section: {guardian_developer_text}"
    );
    assert!(
        !guardian_developer_text.contains("demo: build charts"),
        "guardian request should not include skill summary: {guardian_developer_text}"
    );

    Ok(())
}
