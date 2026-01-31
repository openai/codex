#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used)]
// unified exec is not supported on Windows OS
use std::sync::Arc;

use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::features::Feature;
use codex_core::models_manager::manager::ModelsManager;
use codex_core::models_manager::manager::RefreshStrategy;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandSource;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_models_once_with_delay;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use wiremock::BodyPrintLimit;
use wiremock::MockServer;

const REMOTE_MODEL_SLUG: &str = "codex-test";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_remote_model_uses_unified_exec() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;

    let remote_model = ModelInfo {
        slug: REMOTE_MODEL_SLUG.to_string(),
        display_name: "Remote Test".to_string(),
        description: Some("A remote model that requires the test shell".to_string()),
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

    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.features.enable(Feature::RemoteModels);
            config.model = Some("gpt-5.1".to_string());
        });
    let TestCodex {
        codex,
        cwd,
        config,
        thread_manager,
        ..
    } = builder.build(&server).await?;

    let models_manager = thread_manager.get_models_manager();
    let available_model =
        wait_for_model_available(&models_manager, REMOTE_MODEL_SLUG, &config).await;

    assert_eq!(available_model.model, REMOTE_MODEL_SLUG);

    let requests = models_mock.requests();
    assert_eq!(
        requests.len(),
        1,
        "expected a single /models refresh request for the remote models feature"
    );
    assert_eq!(requests[0].url.path(), "/v1/models");

    let model_info = models_manager
        .get_model_info(REMOTE_MODEL_SLUG, &config)
        .await;
    assert_eq!(model_info.shell_type, ConfigShellToolType::UnifiedExec);

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: Some(REMOTE_MODEL_SLUG.to_string()),
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let call_id = "call";
    let args = json!({
        "cmd": "/bin/echo call",
        "yield_time_ms": 250,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    mount_sse_sequence(&server, responses).await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "run call".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: REMOTE_MODEL_SLUG.to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let begin_event = wait_for_event_match(&codex, |msg| match msg {
        EventMsg::ExecCommandBegin(event) if event.call_id == call_id => Some(event.clone()),
        _ => None,
    })
    .await;

    assert_eq!(begin_event.source, ExecCommandSource::UnifiedExecStartup);

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_truncation_policy_without_override_preserves_remote() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;

    let slug = "codex-test-truncation-policy";
    let remote_model = test_remote_model_with_policy(
        slug,
        ModelVisibility::List,
        1,
        TruncationPolicyConfig::bytes(12_000),
    );
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.features.enable(Feature::RemoteModels);
            config.model = Some("gpt-5.1".to_string());
        });
    let test = builder.build(&server).await?;

    let models_manager = test.thread_manager.get_models_manager();
    wait_for_model_available(&models_manager, slug, &test.config).await;

    let model_info = models_manager.get_model_info(slug, &test.config).await;
    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::bytes(12_000)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_truncation_policy_with_tool_output_override() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;

    let slug = "codex-test-truncation-override";
    let remote_model = test_remote_model_with_policy(
        slug,
        ModelVisibility::List,
        1,
        TruncationPolicyConfig::bytes(10_000),
    );
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.features.enable(Feature::RemoteModels);
            config.model = Some("gpt-5.1".to_string());
            config.tool_output_token_limit = Some(50);
        });
    let test = builder.build(&server).await?;

    let models_manager = test.thread_manager.get_models_manager();
    wait_for_model_available(&models_manager, slug, &test.config).await;

    let model_info = models_manager.get_model_info(slug, &test.config).await;
    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::bytes(200)
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_apply_remote_base_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;

    let model = "test-gpt-5-remote";

    let remote_base = "Use the remote base instructions only.";
    let remote_model = ModelInfo {
        slug: model.to_string(),
        display_name: "Parallel Remote".to_string(),
        description: Some("A remote model with custom instructions".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority: 1,
        upgrade: None,
        base_instructions: remote_base.to_string(),
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
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;

    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| {
            config.features.enable(Feature::RemoteModels);
            config.model = Some("gpt-5.1".to_string());
        });
    let TestCodex {
        codex,
        cwd,
        config,
        thread_manager,
        ..
    } = builder.build(&server).await?;

    let models_manager = thread_manager.get_models_manager();
    wait_for_model_available(&models_manager, model, &config).await;

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: Some(model.to_string()),
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello remote".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: model.to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    let base_model_info = models_manager.get_model_info("gpt-5.1", &config).await;
    let body = response_mock.single_request().body_json();
    let instructions = body["instructions"].as_str().unwrap();
    assert_eq!(instructions, base_model_info.base_instructions);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_preserve_builtin_presets() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::start().await;
    let remote_model = test_remote_model("remote-alpha", ModelVisibility::List, 0);
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model.clone()],
        },
    )
    .await;

    let codex_home = TempDir::new()?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let manager = ModelsManager::with_provider(
        codex_home.path().to_path_buf(),
        codex_core::auth::AuthManager::from_auth_for_testing(auth),
        provider,
    );

    let available = manager
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
        .await;
    let remote = available
        .iter()
        .find(|model| model.model == "remote-alpha")
        .expect("remote model should be listed");
    let mut expected_remote: ModelPreset = remote_model.into();
    expected_remote.is_default = remote.is_default;
    assert_eq!(*remote, expected_remote);
    let default_model = available
        .iter()
        .find(|model| model.show_in_picker)
        .expect("default model should be set");
    assert!(default_model.is_default);
    assert_eq!(
        available.iter().filter(|model| model.is_default).count(),
        1,
        "expected a single default model"
    );
    assert!(
        available
            .iter()
            .any(|model| model.model == "gpt-5.1-codex-max"),
        "builtin presets should remain available after refresh"
    );
    assert_eq!(
        models_mock.requests().len(),
        1,
        "expected a single /models request"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_merge_adds_new_high_priority_first() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::start().await;
    let remote_model = test_remote_model("remote-top", ModelVisibility::List, -10_000);
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;

    let codex_home = TempDir::new()?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let manager = ModelsManager::with_provider(
        codex_home.path().to_path_buf(),
        codex_core::auth::AuthManager::from_auth_for_testing(auth),
        provider,
    );

    let available = manager
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
        .await;
    assert_eq!(
        available.first().map(|model| model.model.as_str()),
        Some("remote-top")
    );
    assert_eq!(
        models_mock.requests().len(),
        1,
        "expected a single /models request"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_merge_replaces_overlapping_model() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::start().await;
    let slug = bundled_model_slug();
    let mut remote_model = test_remote_model(&slug, ModelVisibility::List, 0);
    remote_model.display_name = "Overridden".to_string();
    remote_model.description = Some("Overridden description".to_string());
    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model.clone()],
        },
    )
    .await;

    let codex_home = TempDir::new()?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let manager = ModelsManager::with_provider(
        codex_home.path().to_path_buf(),
        codex_core::auth::AuthManager::from_auth_for_testing(auth),
        provider,
    );

    let available = manager
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
        .await;
    let overridden = available
        .iter()
        .find(|model| model.model == slug)
        .expect("overlapping model should be listed");
    assert_eq!(overridden.display_name, remote_model.display_name);
    assert_eq!(
        overridden.description,
        remote_model
            .description
            .expect("remote model should include description")
    );
    assert_eq!(
        models_mock.requests().len(),
        1,
        "expected a single /models request"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_merge_preserves_bundled_models_on_empty_response() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::start().await;
    let models_mock = mount_models_once(&server, ModelsResponse { models: Vec::new() }).await;

    let codex_home = TempDir::new()?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let manager = ModelsManager::with_provider(
        codex_home.path().to_path_buf(),
        codex_core::auth::AuthManager::from_auth_for_testing(auth),
        provider,
    );

    let available = manager
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
        .await;
    let bundled_slug = bundled_model_slug();
    assert!(
        available.iter().any(|model| model.model == bundled_slug),
        "bundled models should remain available after empty remote response"
    );
    assert_eq!(
        models_mock.requests().len(),
        1,
        "expected a single /models request"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_request_times_out_after_5s() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::start().await;
    let remote_model = test_remote_model("remote-timeout", ModelVisibility::List, 0);
    let models_mock = mount_models_once_with_delay(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
        Duration::from_secs(6),
    )
    .await;

    let codex_home = TempDir::new()?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let manager = ModelsManager::with_provider(
        codex_home.path().to_path_buf(),
        codex_core::auth::AuthManager::from_auth_for_testing(auth),
        provider,
    );

    let start = Instant::now();
    let model = timeout(
        Duration::from_secs(7),
        manager.get_default_model(&None, &config, RefreshStrategy::OnlineIfUncached),
    )
    .await;
    let elapsed = start.elapsed();
    // get_model should return a default model even when refresh times out
    let default_model = model.expect("get_model should finish and return default model");
    assert!(
        default_model == "gpt-5.2-codex",
        "get_model should return default model when refresh times out, got: {default_model}"
    );
    let _ = server
        .received_requests()
        .await
        .expect("mock server should capture requests")
        .iter()
        .map(|req| format!("{} {}", req.method, req.url.path()))
        .collect::<Vec<String>>();
    assert!(
        elapsed >= Duration::from_millis(4_500),
        "expected models call to block near the timeout; took {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(5_800),
        "expected models call to time out before the delayed response; took {elapsed:?}"
    );
    assert_eq!(
        models_mock.requests().len(),
        1,
        "expected a single /models request"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_models_hide_picker_only_models() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_sandbox!(Ok(()));

    let server = MockServer::start().await;
    let remote_model = test_remote_model("codex-auto-balanced", ModelVisibility::Hide, 0);
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
        },
    )
    .await;

    let codex_home = TempDir::new()?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let manager = ModelsManager::with_provider(
        codex_home.path().to_path_buf(),
        codex_core::auth::AuthManager::from_auth_for_testing(auth),
        provider,
    );

    let selected = manager
        .get_default_model(&None, &config, RefreshStrategy::OnlineIfUncached)
        .await;
    assert_eq!(selected, "gpt-5.2-codex");

    let available = manager
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
        .await;
    let hidden = available
        .iter()
        .find(|model| model.model == "codex-auto-balanced")
        .expect("hidden remote model should be listed");
    assert!(!hidden.show_in_picker, "hidden models should remain hidden");

    Ok(())
}

async fn wait_for_model_available(
    manager: &Arc<ModelsManager>,
    slug: &str,
    config: &Config,
) -> ModelPreset {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(model) = {
            let guard = manager
                .list_models(config, RefreshStrategy::OnlineIfUncached)
                .await;
            guard.iter().find(|model| model.model == slug).cloned()
        } {
            return model;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for the remote model {slug} to appear");
        }
        sleep(Duration::from_millis(25)).await;
    }
}

fn bundled_model_slug() -> String {
    let response: ModelsResponse = serde_json::from_str(include_str!("../../models.json"))
        .expect("bundled models.json should deserialize");
    response
        .models
        .first()
        .expect("bundled models.json should include at least one model")
        .slug
        .clone()
}

fn test_remote_model(slug: &str, visibility: ModelVisibility, priority: i32) -> ModelInfo {
    test_remote_model_with_policy(
        slug,
        visibility,
        priority,
        TruncationPolicyConfig::bytes(10_000),
    )
}

fn test_remote_model_with_policy(
    slug: &str,
    visibility: ModelVisibility,
    priority: i32,
    truncation_policy: TruncationPolicyConfig,
) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: format!("{slug} display"),
        description: Some(format!("{slug} description")),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility,
        supported_in_api: true,
        priority,
        upgrade: None,
        base_instructions: "base instructions".to_string(),
        model_messages: None,
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy,
        supports_parallel_tool_calls: false,
        context_window: Some(272_000),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
    }
}
