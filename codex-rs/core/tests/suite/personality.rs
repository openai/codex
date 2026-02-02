use codex_core::config::types::Personality;
use codex_core::features::Feature;
use codex_core::models_manager::manager::ModelsManager;
use codex_core::models_manager::manager::RefreshStrategy;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelInstructionsVariables;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use wiremock::BodyPrintLimit;
use wiremock::MockServer;

const LOCAL_FRIENDLY_TEMPLATE: &str =
    "You optimize for team morale and being a supportive teammate as much as code quality.";
const LOCAL_PRAGMATIC_TEMPLATE: &str = "You are a deeply pragmatic, effective software engineer.";

fn sse_completed(id: &str) -> String {
    sse(vec![ev_response_created(id), ev_completed(id)])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn personality_does_not_mutate_base_instructions_without_template() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::Personality);
    config.personality = Some(Personality::Friendly);

    let model_info = ModelsManager::construct_model_info_offline("gpt-5.1", &config);
    assert_eq!(
        model_info.get_model_instructions(config.personality),
        model_info.base_instructions
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn base_instructions_override_disables_personality_template() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::Personality);
    config.personality = Some(Personality::Friendly);
    config.base_instructions = Some("override instructions".to_string());

    let model_info = ModelsManager::construct_model_info_offline("gpt-5.2-codex", &config);

    assert_eq!(model_info.base_instructions, "override instructions");
    assert_eq!(
        model_info.get_model_instructions(config.personality),
        "override instructions"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_personality_none_does_not_add_update_message() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex()
        .with_model("gpt-5.2-codex")
        .with_config(|config| {
            config.features.disable(Feature::RemoteModels);
            config.features.enable(Feature::Personality);
        });
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
async fn config_personality_some_sets_instructions_template() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;
    let mut builder = test_codex()
        .with_model("gpt-5.2-codex")
        .with_config(|config| {
            config.features.disable(Feature::RemoteModels);
            config.features.enable(Feature::Personality);
            config.personality = Some(Personality::Friendly);
        });
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
    let instructions_text = request.instructions_text();

    assert!(
        instructions_text.contains(LOCAL_FRIENDLY_TEMPLATE),
        "expected personality update to include the local friendly template, got: {instructions_text:?}"
    );

    let developer_texts = request.message_input_texts("developer");
    for text in developer_texts {
        assert!(
            !text.contains("<personality_spec>"),
            "expected no personality update message in developer input"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_personality_some_adds_update_message() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;
    let mut builder = test_codex()
        .with_model("exp-codex-personality")
        .with_config(|config| {
            config.features.disable(Feature::RemoteModels);
            config.features.enable(Feature::Personality);
        });
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

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: Some(Personality::Pragmatic),
        })
        .await?;

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

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two requests");
    let request = requests
        .last()
        .expect("expected personality update request");

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
        personality_text.contains(LOCAL_PRAGMATIC_TEMPLATE),
        "expected personality update to include the local pragmatic template, got: {personality_text:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_personality_same_value_does_not_add_update_message() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;
    let mut builder = test_codex()
        .with_model("exp-codex-personality")
        .with_config(|config| {
            config.features.disable(Feature::RemoteModels);
            config.features.enable(Feature::Personality);
            config.personality = Some(Personality::Pragmatic);
        });
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

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: Some(Personality::Pragmatic),
        })
        .await?;

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

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two requests");
    let request = requests
        .last()
        .expect("expected second request after personality override");

    let developer_texts = request.message_input_texts("developer");
    let personality_text = developer_texts
        .iter()
        .find(|text| text.contains("<personality_spec>"));
    assert!(
        personality_text.is_none(),
        "expected no personality preamble for unchanged personality, got {personality_text:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn instructions_uses_base_if_feature_disabled() -> anyhow::Result<()> {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.disable(Feature::Personality);
    config.personality = Some(Personality::Friendly);

    let model_info = ModelsManager::construct_model_info_offline("gpt-5.2-codex", &config);
    assert_eq!(
        model_info.get_model_instructions(config.personality),
        model_info.base_instructions
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_personality_skips_if_feature_disabled() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let resp_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;
    let mut builder = test_codex()
        .with_model("exp-codex-personality")
        .with_config(|config| {
            config.features.disable(Feature::RemoteModels);
            config.features.disable(Feature::Personality);
        });
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

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: Some(Personality::Pragmatic),
        })
        .await?;

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

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two requests");
    let request = requests
        .last()
        .expect("expected personality update request");

    let developer_texts = request.message_input_texts("developer");
    let personality_text = developer_texts
        .iter()
        .find(|text| text.contains("<personality_spec>"));
    assert!(
        personality_text.is_none(),
        "expected no personality preamble, got {personality_text:?}"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ignores_remote_personality_if_remote_models_disabled() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;

    let remote_slug = "gpt-5.2-codex";
    let remote_personality_message = "Friendly from remote template";
    let remote_model = ModelInfo {
        slug: remote_slug.to_string(),
        display_name: "Remote personality test".to_string(),
        description: Some("Remote model with personality template".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::UnifiedExec,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 1,
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_messages: Some(ModelMessages {
            instructions_template: Some("Base instructions\n{{ personality }}\n".to_string()),
            instructions_variables: Some(ModelInstructionsVariables {
                personality_default: None,
                personality_friendly: Some(remote_personality_message.to_string()),
                personality_pragmatic: None,
            }),
        }),
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        context_window: Some(128_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    };

    let _models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = test_codex()
        .with_auth(codex_core::CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.features.disable(Feature::RemoteModels);
            config.features.enable(Feature::Personality);
            config.model = Some(remote_slug.to_string());
            config.personality = Some(Personality::Friendly);
        });
    let test = builder.build(&server).await?;

    wait_for_model_available(
        &test.thread_manager.get_models_manager(),
        remote_slug,
        &test.config,
    )
    .await;

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
            model: remote_slug.to_string(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = resp_mock.single_request();
    let instructions_text = request.instructions_text();

    assert!(
        instructions_text.contains("You are Codex, a coding agent based on GPT-5. You and the user share the same workspace and collaborate to achieve the user's goals."),
        "expected instructions to use the template instructions, got: {instructions_text:?}"
    );
    assert!(
        instructions_text.contains(
            "You optimize for team morale and being a supportive teammate as much as code quality."
        ),
        "expected instructions to include the local friendly personality template, got: {instructions_text:?}"
    );
    assert!(
        !instructions_text.contains("{{ personality }}"),
        "expected legacy personality placeholder to be replaced, got: {instructions_text:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_model_friendly_personality_instructions_with_feature() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;

    let remote_slug = "codex-remote-default-personality";
    let default_personality_message = "Default from remote template";
    let friendly_personality_message = "Friendly variant";
    let remote_model = ModelInfo {
        slug: remote_slug.to_string(),
        display_name: "Remote default personality test".to_string(),
        description: Some("Remote model with default personality template".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::UnifiedExec,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 1,
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_messages: Some(ModelMessages {
            instructions_template: Some("Base instructions\n{{ personality }}\n".to_string()),
            instructions_variables: Some(ModelInstructionsVariables {
                personality_default: Some(default_personality_message.to_string()),
                personality_friendly: Some(friendly_personality_message.to_string()),
                personality_pragmatic: Some("Pragmatic variant".to_string()),
            }),
        }),
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        context_window: Some(128_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    };

    let _models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;

    let resp_mock = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = test_codex()
        .with_auth(codex_core::CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.features.enable(Feature::RemoteModels);
            config.features.enable(Feature::Personality);
            config.model = Some(remote_slug.to_string());
            config.personality = Some(Personality::Friendly);
        });
    let test = builder.build(&server).await?;

    wait_for_model_available(
        &test.thread_manager.get_models_manager(),
        remote_slug,
        &test.config,
    )
    .await;

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
            model: remote_slug.to_string(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: Some(Personality::Friendly),
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = resp_mock.single_request();
    let instructions_text = request.instructions_text();

    assert!(
        instructions_text.contains(friendly_personality_message),
        "expected instructions to include the remote friendly personality template, got: {instructions_text:?}"
    );
    assert!(
        !instructions_text.contains(default_personality_message),
        "expected instructions to skip the remote default personality template, got: {instructions_text:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_personality_remote_model_template_includes_update_message() -> anyhow::Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;

    let remote_slug = "codex-remote-personality";
    let remote_friendly_message = "Friendly from remote template";
    let remote_pragmatic_message = "Pragmatic from remote template";
    let remote_model = ModelInfo {
        slug: remote_slug.to_string(),
        display_name: "Remote personality test".to_string(),
        description: Some("Remote model with personality template".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::UnifiedExec,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 1,
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_messages: Some(ModelMessages {
            instructions_template: Some("Base instructions\n{{ personality }}\n".to_string()),
            instructions_variables: Some(ModelInstructionsVariables {
                personality_default: None,
                personality_friendly: Some(remote_friendly_message.to_string()),
                personality_pragmatic: Some(remote_pragmatic_message.to_string()),
            }),
        }),
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        context_window: Some(128_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    };

    let _models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;

    let resp_mock = mount_sse_sequence(
        &server,
        vec![sse_completed("resp-1"), sse_completed("resp-2")],
    )
    .await;

    let mut builder = test_codex()
        .with_auth(codex_core::CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.features.enable(Feature::RemoteModels);
            config.features.enable(Feature::Personality);
            config.model = Some("gpt-5.2-codex".to_string());
        });
    let test = builder.build(&server).await?;

    wait_for_model_available(
        &test.thread_manager.get_models_manager(),
        remote_slug,
        &test.config,
    )
    .await;

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
            model: Some(remote_slug.to_string()),
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: Some(Personality::Pragmatic),
        })
        .await?;

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
            model: remote_slug.to_string(),
            effort: test.config.model_reasoning_effort,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two requests");
    let request = requests
        .last()
        .expect("expected personality update request");
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
        personality_text.contains(remote_pragmatic_message),
        "expected personality update to include remote template, got: {personality_text:?}"
    );

    Ok(())
}

async fn wait_for_model_available(
    manager: &Arc<ModelsManager>,
    slug: &str,
    config: &codex_core::config::Config,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let models = manager
            .list_models(config, RefreshStrategy::OnlineIfUncached)
            .await;
        if models.iter().any(|model| model.model == slug) {
            return;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for the remote model {slug} to appear");
        }
        sleep(Duration::from_millis(25)).await;
    }
}
