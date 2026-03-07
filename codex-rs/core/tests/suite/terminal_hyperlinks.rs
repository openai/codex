use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;

const TERMINAL_HYPERLINKS_HEADER: &str = "## Terminal Hyperlinks";

async fn submit_user_turn(test: &TestCodex, prompt: &str, model: String) -> Result<()> {
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model,
            effort: test.config.model_reasoning_effort,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_hyperlinks_feature_augments_initial_request_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex()
        .with_model("gpt-5.2-codex")
        .with_config(|config| {
            let _ = config.features.enable(Feature::TerminalHyperlinks);
        });
    let test = builder.build(&server).await?;

    submit_user_turn(&test, "hello", test.session_configured.model.clone()).await?;

    let instructions_text = resp_mock.single_request().instructions_text();
    assert!(
        instructions_text.contains(TERMINAL_HYPERLINKS_HEADER),
        "expected terminal hyperlinks guidance in request instructions, got: {instructions_text:?}"
    );
    assert!(
        instructions_text.contains("Prefer paths relative to the current working directory"),
        "expected relative-path guidance in request instructions, got: {instructions_text:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn terminal_hyperlinks_feature_updates_model_switch_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;
    let mut builder = test_codex()
        .with_model("gpt-5.2-codex")
        .with_config(|config| {
            let _ = config.features.enable(Feature::TerminalHyperlinks);
        });
    let test = builder.build(&server).await?;
    let next_model = "gpt-5.1-codex-max".to_string();

    submit_user_turn(&test, "hello", test.session_configured.model.clone()).await?;

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: Some(next_model.clone()),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    submit_user_turn(&test, "switch models", next_model).await?;

    let requests = resp_mock.requests();
    let second_request = requests.last().expect("expected second request");
    let developer_texts = second_request.message_input_texts("developer");
    let model_switch_text = developer_texts
        .iter()
        .find(|text| text.contains("<model_switch>"))
        .expect("expected model switch message in developer input");

    assert!(
        model_switch_text.contains(TERMINAL_HYPERLINKS_HEADER),
        "expected terminal hyperlinks guidance in model switch message, got: {model_switch_text:?}"
    );

    Ok(())
}
