#![allow(clippy::expect_used)]

use std::fs;
use std::sync::Arc;

use anyhow::Result;
use codex_core::config::types::Personality;
use codex_core::features::Feature;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::ContextSnapshotRenderMode;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;

const PRETURN_CONTEXT_DIFF_CWD: &str = "PRETURN_CONTEXT_DIFF_CWD";

fn context_snapshot_options() -> ContextSnapshotOptions {
    ContextSnapshotOptions::default()
        .render_mode(ContextSnapshotRenderMode::KindWithTextPrefix { max_chars: 96 })
}

fn format_labeled_requests_snapshot(
    scenario: &str,
    sections: &[(&str, &ResponsesRequest)],
) -> String {
    context_snapshot::format_labeled_requests_snapshot(
        scenario,
        sections,
        &context_snapshot_options(),
    )
}

fn agents_message_count(request: &ResponsesRequest) -> usize {
    request
        .message_input_texts("user")
        .iter()
        .filter(|text| text.starts_with("# AGENTS.md instructions for "))
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_model_visible_layout_turn_overrides() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "turn one complete"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "turn two complete"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_model("gpt-5.2-codex")
        .with_config(|config| {
            config.features.enable(Feature::Personality);
            config.personality = Some(Personality::Pragmatic);
        });
    let test = builder.build(&server).await?;
    let preturn_context_diff_cwd = test.cwd_path().join(PRETURN_CONTEXT_DIFF_CWD);
    fs::create_dir_all(&preturn_context_diff_cwd)?;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "first turn".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: test.session_configured.model.clone(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "second turn with context updates".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: preturn_context_diff_cwd,
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: test.session_configured.model.clone(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: Some(Personality::Friendly),
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2, "expected two requests");
    insta::assert_snapshot!(
        "model_visible_layout_turn_overrides",
        format_labeled_requests_snapshot(
            "Second turn changes cwd, approval policy, and personality while keeping model constant.",
            &[
                ("First Request (Baseline)", &requests[0]),
                ("Second Request (Turn Overrides)", &requests[1]),
            ]
        )
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
// TODO(ccunningham): Diff `user_instructions` and emit updates when AGENTS.md content changes
// (for example after cwd changes), then update this test to assert refreshed AGENTS content.
async fn snapshot_model_visible_layout_cwd_change_does_not_refresh_agents() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "turn one complete"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "turn two complete"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex().with_model("gpt-5.2-codex");
    let test = builder.build(&server).await?;
    let cwd_one = test.cwd_path().join("agents_one");
    let cwd_two = test.cwd_path().join("agents_two");
    fs::create_dir_all(&cwd_one)?;
    fs::create_dir_all(&cwd_two)?;
    fs::write(
        cwd_one.join("AGENTS.md"),
        "# AGENTS one\n\n<INSTRUCTIONS>\nTurn one agents instructions.\n</INSTRUCTIONS>\n",
    )?;
    fs::write(
        cwd_two.join("AGENTS.md"),
        "# AGENTS two\n\n<INSTRUCTIONS>\nTurn two agents instructions.\n</INSTRUCTIONS>\n",
    )?;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "first turn in agents_one".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd_one.clone(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: test.session_configured.model.clone(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "second turn in agents_two".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd_two,
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: test.session_configured.model.clone(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2, "expected two requests");
    assert_eq!(
        agents_message_count(&requests[0]),
        1,
        "expected exactly one AGENTS message in first request"
    );
    assert_eq!(
        agents_message_count(&requests[1]),
        1,
        "expected AGENTS to refresh after cwd change, but current behavior only keeps history AGENTS"
    );
    insta::assert_snapshot!(
        "model_visible_layout_cwd_change_does_not_refresh_agents",
        format_labeled_requests_snapshot(
            "Second turn changes cwd to a directory with different AGENTS.md; current behavior does not emit refreshed AGENTS instructions.",
            &[
                ("First Request (agents_one)", &requests[0]),
                ("Second Request (agents_two cwd)", &requests[1]),
            ]
        )
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_model_visible_layout_resume_with_personality_change() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut initial_builder = test_codex().with_config(|config| {
        config.model = Some("gpt-5.2".to_string());
    });
    let initial = initial_builder.build(&server).await?;
    let codex = Arc::clone(&initial.codex);
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    let initial_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-initial"),
            ev_assistant_message("msg-1", "recorded before resume"),
            ev_completed("resp-initial"),
        ]),
    )
    .await;
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "seed resume history".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let initial_request = initial_mock.single_request();

    let resumed_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-resume"),
            ev_assistant_message("msg-2", "first resumed turn"),
            ev_completed("resp-resume"),
        ]),
    )
    .await;

    let mut resume_builder = test_codex().with_config(|config| {
        config.model = Some("gpt-5.2-codex".to_string());
        config.features.enable(Feature::Personality);
        config.personality = Some(Personality::Pragmatic);
    });
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;
    resumed
        .codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "resume and change personality".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: resumed.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: resumed.session_configured.model.clone(),
            effort: resumed.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: Some(Personality::Friendly),
        })
        .await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let resumed_request = resumed_mock.single_request();
    insta::assert_snapshot!(
        "model_visible_layout_resume_with_personality_change",
        format_labeled_requests_snapshot(
            "First post-resume turn where resumed config model differs from rollout and personality changes.",
            &[
                ("Last Request Before Resume", &initial_request),
                ("First Request After Resume", &resumed_request),
            ]
        )
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_model_visible_layout_resume_override_matches_rollout_model() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut initial_builder = test_codex().with_config(|config| {
        config.model = Some("gpt-5.2".to_string());
    });
    let initial = initial_builder.build(&server).await?;
    let codex = Arc::clone(&initial.codex);
    let home = initial.home.clone();
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    let initial_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-initial"),
            ev_assistant_message("msg-1", "recorded before resume"),
            ev_completed("resp-initial"),
        ]),
    )
    .await;
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "seed resume history".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let initial_request = initial_mock.single_request();

    let resumed_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-resume"),
            ev_assistant_message("msg-2", "first resumed turn"),
            ev_completed("resp-resume"),
        ]),
    )
    .await;

    let mut resume_builder = test_codex().with_config(|config| {
        config.model = Some("gpt-5.2-codex".to_string());
    });
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;
    resumed
        .codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: Some("gpt-5.2".to_string()),
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    resumed
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first resumed turn after model override".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let resumed_request = resumed_mock.single_request();
    insta::assert_snapshot!(
        "model_visible_layout_resume_override_matches_rollout_model",
        format_labeled_requests_snapshot(
            "First post-resume turn where pre-turn override sets model to rollout model; no model-switch update should appear.",
            &[
                ("Last Request Before Resume", &initial_request),
                ("First Request After Resume + Override", &resumed_request),
            ]
        )
    );

    Ok(())
}
