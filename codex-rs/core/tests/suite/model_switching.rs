use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::config::types::Personality;
use codex_core::features::Feature;
use codex_core::models_manager::manager::RefreshStrategy;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::default_input_modalities;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse_completed;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_change_appends_model_instructions_developer_message() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;

    let mut builder = test_codex().with_model("gpt-5.2-codex");
    let test = builder.build(&server).await?;
    let next_model = "gpt-5.1-codex-max";

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: test.session_configured.model.clone(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: Some(next_model.to_string()),
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "switch models".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: next_model.to_string(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let second_request = requests.last().expect("expected second request");
    let developer_texts = second_request.message_input_texts("developer");
    let model_switch_text = developer_texts
        .iter()
        .find(|text| text.contains("<model_switch>"))
        .expect("expected model switch message in developer input");
    assert!(
        model_switch_text.contains("The user was previously using a different model."),
        "expected model switch preamble, got: {model_switch_text:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_and_personality_change_only_appends_model_instructions() -> Result<()> {
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
            config.features.enable(Feature::Personality);
        });
    let test = builder.build(&server).await?;
    let next_model = "exp-codex-personality";

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: test.session_configured.model.clone(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: Some(next_model.to_string()),
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: Some(Personality::Pragmatic),
        })
        .await?;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "switch model and personality".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: next_model.to_string(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let second_request = requests.last().expect("expected second request");
    let developer_texts = second_request.message_input_texts("developer");
    assert!(
        developer_texts
            .iter()
            .any(|text| text.contains("<model_switch>")),
        "expected model switch message when model changes"
    );
    assert!(
        !developer_texts
            .iter()
            .any(|text| text.contains("<personality_spec>")),
        "did not expect personality update message when model changed in same turn"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_change_from_image_to_text_strips_prior_image_content() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::start().await;
    let image_model_slug = "test-image-model";
    let text_model_slug = "test-text-only-model";
    let image_model = ModelInfo {
        slug: image_model_slug.to_string(),
        display_name: "Test Image Model".to_string(),
        description: Some("supports image input".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        input_modalities: default_input_modalities(),
        priority: 1,
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_messages: None,
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        context_window: Some(272_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    };
    let mut text_model = image_model.clone();
    text_model.slug = text_model_slug.to_string();
    text_model.display_name = "Test Text Model".to_string();
    text_model.description = Some("text only".to_string());
    text_model.input_modalities = vec![InputModality::Text];
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![image_model, text_model],
        },
    )
    .await;

    let responses = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.features.enable(Feature::RemoteModels);
            config.model = Some(image_model_slug.to_string());
        });
    let test = builder.build(&server).await?;
    let models_manager = test.thread_manager.get_models_manager();
    let _ = models_manager
        .list_models(&test.config, RefreshStrategy::OnlineIfUncached)
        .await;

    let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR4nGNgYAAAAAMAASsJTYQAAAAASUVORK5CYII="
        .to_string();

    test.codex
        .submit(Op::UserTurn {
            items: vec![
                UserInput::Image {
                    image_url: image_url.clone(),
                },
                UserInput::Text {
                    text: "first turn".to_string(),
                    text_elements: Vec::new(),
                },
            ],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: image_model_slug.to_string(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::ReadOnly,
            model: text_model_slug.to_string(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = responses.requests();
    assert_eq!(requests.len(), 2, "expected two model requests");

    let first_request = requests.first().expect("expected first request");
    let first_has_input_image = first_request.inputs_of_type("message").iter().any(|item| {
        item.get("content")
            .and_then(Value::as_array)
            .is_some_and(|content| {
                content
                    .iter()
                    .any(|span| span.get("type").and_then(Value::as_str) == Some("input_image"))
            })
    });
    assert!(
        first_has_input_image,
        "first request should include the uploaded image"
    );

    let second_request = requests.last().expect("expected second request");
    let second_has_input_image = second_request.inputs_of_type("message").iter().any(|item| {
        item.get("content")
            .and_then(Value::as_array)
            .is_some_and(|content| {
                content
                    .iter()
                    .any(|span| span.get("type").and_then(Value::as_str) == Some("input_image"))
            })
    });
    assert!(
        !second_has_input_image,
        "second request should strip unsupported image content"
    );
    let second_user_texts = second_request.message_input_texts("user");
    assert!(
        second_user_texts
            .iter()
            .any(|text| text == "image content omitted because you do not support image input"),
        "second request should include the image-omitted placeholder text"
    );
    assert!(
        second_user_texts
            .iter()
            .any(|text| text == &codex_protocol::models::image_open_tag_text()),
        "second request should preserve the image open tag text"
    );
    assert!(
        second_user_texts
            .iter()
            .any(|text| text == &codex_protocol::models::image_close_tag_text()),
        "second request should preserve the image close tag text"
    );

    Ok(())
}
