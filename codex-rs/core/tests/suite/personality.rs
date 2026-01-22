use codex_core::config::types::Personality;
use codex_core::models_manager::manager::ModelsManager;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn sse_completed(id: &str) -> String {
    sse(vec![ev_response_created(id), ev_completed(id)])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_personality_does_not_mutate_base_instructions_without_template() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_personality = Some(Personality::Friendly);

    let model_info = ModelsManager::construct_model_info_offline("gpt-5.1", &config);
    assert_eq!(
        model_info.get_model_instructions(config.model_personality),
        model_info.base_instructions
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn base_instructions_override_disables_personality_template() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model_personality = Some(Personality::Friendly);
    config.base_instructions = Some("override instructions".to_string());

    let model_info = ModelsManager::construct_model_info_offline("gpt-5.2-codex", &config);

    assert_eq!(model_info.base_instructions, "override instructions");
    assert_eq!(
        model_info.get_model_instructions(config.model_personality),
        "override instructions"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_personality_none_does_not_add_update_message() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex().with_model("gpt-5.2-codex");
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: test.config.approval_policy.value(),
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: test.session_configured.model.clone(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = resp_mock.single_request();
    let developer_texts = request.message_input_texts("developer");
    assert!(
        !developer_texts
            .iter()
            .any(|text| text.contains("<personality_spec>")),
        "did not expect a personality update message when personality is None"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_personality_some_adds_update_message() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex().with_model("gpt-5.2-codex");
    let test = builder.build(&server).await?;

    let model_info = ModelsManager::construct_model_info_offline("gpt-5.2-codex", &test.config);
    let personality_message = model_info
        .model_instructions_template
        .as_ref()
        .and_then(|template| template.personality_messages.as_ref())
        .and_then(|messages| messages.0.get(&Personality::Friendly))
        .expect("friendly personality message should exist")
        .to_string();

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: test.config.approval_policy.value(),
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: test.session_configured.model.clone(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: Some(Personality::Friendly),
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = resp_mock.single_request();
    let developer_texts = request.message_input_texts("developer");
    let personality_text = developer_texts
        .iter()
        .find(|text| text.contains("<personality_spec>"))
        .expect("expected personality update message in developer input");

    assert!(
        personality_text.contains("The user has requested a new communication style."),
        "expected personality update preamble, got {personality_text:?}"
    );
    assert!(
        personality_text.contains(&personality_message),
        "expected personality update to include the friendly template"
    );

    Ok(())
}
