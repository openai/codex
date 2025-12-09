#![cfg(not(target_os = "windows"))]
// unified exec is not supported on Windows OS
use std::sync::Arc;

use anyhow::Result;
use codex_core::features::Feature;
use codex_core::openai_models::models_manager::ModelsManager;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandSource;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ClientVersion;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_models_once;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_sandbox;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use serde_json::json;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
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
        default_reasoning_level: ReasoningEffort::Medium,
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::UnifiedExec,
        visibility: ModelVisibility::List,
        minimal_client_version: ClientVersion(0, 1, 0),
        supported_in_api: true,
        priority: 1,
        upgrade: None,
        base_instructions: None,
    };

    let models_mock = mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
            etag: String::new(),
        },
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::RemoteModels);
        config.model = "gpt-5.1".to_string();
    });

    let TestCodex {
        codex,
        cwd,
        config,
        conversation_manager,
        ..
    } = builder.build(&server).await?;

    let models_manager = conversation_manager.get_models_manager();
    let available_model = wait_for_model_available(&models_manager, REMOTE_MODEL_SLUG).await;

    assert_eq!(available_model.model, REMOTE_MODEL_SLUG);

    let requests = models_mock.requests();
    assert_eq!(
        requests.len(),
        1,
        "expected a single /models refresh request for the remote models feature"
    );
    assert_eq!(requests[0].url.path(), "/v1/models");

    let family = models_manager
        .construct_model_family(REMOTE_MODEL_SLUG, &config)
        .await;
    assert_eq!(family.shell_type, ConfigShellToolType::UnifiedExec);

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: Some(REMOTE_MODEL_SLUG.to_string()),
            effort: None,
            summary: None,
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
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: REMOTE_MODEL_SLUG.to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let begin_event = wait_for_event_match(&codex, |msg| match msg {
        EventMsg::ExecCommandBegin(event) if event.call_id == call_id => Some(event.clone()),
        _ => None,
    })
    .await;

    assert_eq!(begin_event.source, ExecCommandSource::UnifiedExecStartup);

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

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
        default_reasoning_level: ReasoningEffort::Medium,
        supported_reasoning_levels: vec![ReasoningEffortPreset {
            effort: ReasoningEffort::Medium,
            description: ReasoningEffort::Medium.to_string(),
        }],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        minimal_client_version: ClientVersion(0, 1, 0),
        supported_in_api: true,
        priority: 1,
        upgrade: None,
        base_instructions: Some(remote_base.to_string()),
    };
    mount_models_once(
        &server,
        ModelsResponse {
            models: vec![remote_model],
            etag: String::new(),
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

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::RemoteModels);
        config.model = "gpt-5.1".to_string();
    });

    let TestCodex {
        codex,
        cwd,
        conversation_manager,
        ..
    } = builder.build(&server).await?;

    let models_manager = conversation_manager.get_models_manager();
    wait_for_model_available(&models_manager, model).await;

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: Some(model.to_string()),
            effort: None,
            summary: None,
        })
        .await?;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello remote".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: model.to_string(),
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_event(&codex, |event| matches!(event, EventMsg::TaskComplete(_))).await;

    let body = response_mock.single_request().body_json();
    let instructions = body["instructions"].as_str().unwrap();
    assert_eq!(instructions, remote_base);

    Ok(())
}

async fn wait_for_model_available(manager: &Arc<ModelsManager>, slug: &str) -> ModelPreset {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(model) = {
            let guard = manager.list_models().await;
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
