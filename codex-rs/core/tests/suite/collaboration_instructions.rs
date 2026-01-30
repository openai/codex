use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use codex_core::CodexAuth;
use codex_core::features::Feature;
use codex_core::protocol::COLLABORATION_MODE_CLOSE_TAG;
use codex_core::protocol::COLLABORATION_MODE_OPEN_TAG;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::openai_models::CollaborationModesMessages;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelInstructionsTemplate;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde_json::Value;

fn sse_completed(id: &str) -> String {
    sse(vec![ev_response_created(id), ev_completed(id)])
}

const TEST_COLLAB_MODEL: &str = "test-collab-template";
const MODEL_FALLBACK_TEXT: &str = "model fallback";
const CACHE_FILE: &str = "models_cache.json";

fn cached_collab_builder() -> TestCodexBuilder {
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model(TEST_COLLAB_MODEL)
        .with_config(|config| {
            config.features.enable(Feature::RemoteModels);
            config.model_provider.request_max_retries = Some(0);
            config.model_provider.stream_max_retries = Some(0);
        });
    builder = builder.with_pre_build_hook(|home| {
        write_models_cache(home).expect("models cache should be written");
    });
    builder
}

fn write_models_cache(home: &Path) -> Result<()> {
    let cache = ModelsCache {
        fetched_at: Utc::now(),
        etag: None,
        models: vec![test_collab_model(TEST_COLLAB_MODEL)],
    };
    let contents = serde_json::to_vec_pretty(&cache)?;
    std::fs::write(home.join(CACHE_FILE), contents)?;
    Ok(())
}

fn test_collab_model(slug: &str) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: "Test collab model".to_string(),
        description: Some("test collab model".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: "medium".to_string(),
        }],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 1,
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_instructions_template: Some(ModelInstructionsTemplate {
            template: "template".to_string(),
            personality_messages: None,
            collaboration_modes_messages: Some(CollaborationModesMessages(BTreeMap::from([(
                ModeKind::Custom,
                MODEL_FALLBACK_TEXT.to_string(),
            )]))),
        }),
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
    }
}

#[derive(Debug, Clone, Serialize)]
struct ModelsCache {
    fetched_at: DateTime<Utc>,
    #[serde(default)]
    etag: Option<String>,
    models: Vec<ModelInfo>,
}

fn collab_mode_with_instructions(instructions: Option<&str>) -> CollaborationMode {
    collab_mode_with_model("gpt-5.1", instructions)
}

fn collab_mode_with_model(model: &str, instructions: Option<&str>) -> CollaborationMode {
    CollaborationMode {
        mode: ModeKind::Custom,
        settings: Settings {
            model: model.to_string(),
            reasoning_effort: None,
            developer_instructions: instructions.map(str::to_string),
        },
    }
}

fn developer_texts(input: &[Value]) -> Vec<String> {
    input
        .iter()
        .filter_map(|item| {
            let role = item.get("role")?.as_str()?;
            if role != "developer" {
                return None;
            }
            let text = item
                .get("content")?
                .as_array()?
                .first()?
                .get("text")?
                .as_str()?;
            Some(text.to_string())
        })
        .collect()
}

fn collab_xml(text: &str) -> String {
    format!("{COLLABORATION_MODE_OPEN_TAG}{text}{COLLABORATION_MODE_CLOSE_TAG}")
}

fn count_exact(texts: &[String], target: &str) -> usize {
    texts.iter().filter(|text| text.as_str() == target).count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_collaboration_instructions_by_default() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let test = test_codex().build(&server).await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    assert_eq!(dev_texts.len(), 1);
    assert!(dev_texts[0].contains("`approval_policy`"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_input_includes_collaboration_instructions_after_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let test = test_codex().build(&server).await?;

    let collab_text = "collab instructions";
    let collaboration_mode = collab_mode_with_instructions(Some(collab_text));
    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: Some(collaboration_mode),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_exact(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_instructions_added_on_user_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let test = test_codex().build(&server).await?;
    let collab_text = "turn instructions";
    let collaboration_mode = collab_mode_with_instructions(Some(collab_text));

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            cwd: test.config.cwd.clone(),
            approval_policy: test.config.approval_policy.value(),
            sandbox_policy: test.config.sandbox_policy.get().clone(),
            model: test.session_configured.model.clone(),
            effort: None,
            summary: test.config.model_reasoning_summary,
            collaboration_mode: Some(collaboration_mode),
            final_output_json_schema: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_exact(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_then_user_turn_uses_updated_collaboration_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let test = test_codex().build(&server).await?;
    let collab_text = "override instructions";
    let collaboration_mode = collab_mode_with_instructions(Some(collab_text));

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: Some(collaboration_mode),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            cwd: test.config.cwd.clone(),
            approval_policy: test.config.approval_policy.value(),
            sandbox_policy: test.config.sandbox_policy.get().clone(),
            model: test.session_configured.model.clone(),
            effort: None,
            summary: test.config.model_reasoning_summary,
            collaboration_mode: None,
            final_output_json_schema: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_exact(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_turn_overrides_collaboration_instructions_after_override() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let test = test_codex().build(&server).await?;
    let base_text = "base instructions";
    let base_mode = collab_mode_with_instructions(Some(base_text));
    let turn_text = "turn override";
    let turn_mode = collab_mode_with_instructions(Some(turn_text));

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: Some(base_mode),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            cwd: test.config.cwd.clone(),
            approval_policy: test.config.approval_policy.value(),
            sandbox_policy: test.config.sandbox_policy.get().clone(),
            model: test.session_configured.model.clone(),
            effort: None,
            summary: test.config.model_reasoning_summary,
            collaboration_mode: Some(turn_mode),
            final_output_json_schema: None,
            personality: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let base_text = collab_xml(base_text);
    let turn_text = collab_xml(turn_text);
    assert_eq!(count_exact(&dev_texts, &base_text), 1);
    assert_eq!(count_exact(&dev_texts, &turn_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_mode_update_emits_new_instruction_message() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let test = test_codex().build(&server).await?;
    let first_text = "first instructions";
    let second_text = "second instructions";

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: Some(collab_mode_with_instructions(Some(first_text))),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
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
            collaboration_mode: Some(collab_mode_with_instructions(Some(second_text))),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req2.single_request().input();
    let dev_texts = developer_texts(&input);
    let first_text = collab_xml(first_text);
    let second_text = collab_xml(second_text);
    assert_eq!(count_exact(&dev_texts, &first_text), 1);
    assert_eq!(count_exact(&dev_texts, &second_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_mode_update_noop_does_not_append() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let test = test_codex().build(&server).await?;
    let collab_text = "same instructions";

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: Some(collab_mode_with_instructions(Some(collab_text))),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 1".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
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
            collaboration_mode: Some(collab_mode_with_instructions(Some(collab_text))),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello 2".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req2.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_exact(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_replays_collaboration_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let _req1 = mount_sse_once(&server, sse_completed("resp-1")).await;
    let req2 = mount_sse_once(&server, sse_completed("resp-2")).await;

    let mut builder = test_codex();
    let initial = builder.build(&server).await?;
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    let home = initial.home.clone();

    let collab_text = "resume instructions";
    initial
        .codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: Some(collab_mode_with_instructions(Some(collab_text))),
            personality: None,
        })
        .await?;

    initial
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&initial.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let resumed = builder.resume(&server, home, rollout_path).await?;
    resumed
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "after resume".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&resumed.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req2.single_request().input();
    let dev_texts = developer_texts(&input);
    let collab_text = collab_xml(collab_text);
    assert_eq!(count_exact(&dev_texts, &collab_text), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_collaboration_instructions_are_ignored() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let test = test_codex().build(&server).await?;

    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: Some(collab_mode_with_instructions(Some(""))),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    assert_eq!(dev_texts.len(), 1);
    let collab_text = collab_xml("");
    assert_eq!(count_exact(&dev_texts, &collab_text), 0);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_instructions_precedence_mode_overrides_model_template() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = cached_collab_builder();
    let test = builder.build(&server).await?;

    let mode_text = "mode instructions";
    let collaboration_mode = collab_mode_with_model(TEST_COLLAB_MODEL, Some(mode_text));
    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: Some(collaboration_mode),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let input = req.single_request().input();
    let dev_texts = developer_texts(&input);
    let mode_text = collab_xml(mode_text);
    assert_eq!(count_exact(&dev_texts, &mode_text), 1);
    let last_collab = dev_texts
        .iter()
        .rev()
        .find(|text| text.starts_with(COLLABORATION_MODE_OPEN_TAG))
        .cloned();
    assert_eq!(last_collab, Some(mode_text));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_instructions_fall_back_to_model_template_when_mode_empty() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let req = mount_sse_once(&server, sse_completed("resp-1")).await;

    let mut builder = cached_collab_builder();
    let test = builder.build(&server).await?;

    let collaboration_mode = collab_mode_with_model(TEST_COLLAB_MODEL, None);
    test.codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            collaboration_mode: Some(collaboration_mode),
            personality: None,
        })
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = req.single_request();
    let model = request
        .body_json()
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    assert_eq!(model, TEST_COLLAB_MODEL.to_string());
    let input = request.input();
    let dev_texts = developer_texts(&input);
    let model_text = collab_xml(MODEL_FALLBACK_TEXT);
    assert_eq!(count_exact(&dev_texts, &model_text), 1);

    Ok(())
}
