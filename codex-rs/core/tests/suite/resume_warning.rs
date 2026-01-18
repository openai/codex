#![allow(clippy::unwrap_used, clippy::expect_used)]

use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::NewThread;
use codex_core::ThreadManager;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InitialHistory;
use codex_core::protocol::ResumedHistory;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::SessionMetaLine;
use codex_core::protocol::SessionSource;
use codex_core::protocol::TurnContextItem;
use codex_core::protocol::WarningEvent;
use codex_protocol::ThreadId;
use codex_protocol::models::BaseInstructions;
use core::time::Duration;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use tempfile::TempDir;
use wiremock::MockServer;

use codex_core::built_in_model_providers;
use codex_core::protocol::SessionMeta;

fn resume_history(
    config: &codex_core::config::Config,
    previous_model: &str,
    rollout_path: &std::path::Path,
) -> InitialHistory {
    let turn_ctx = TurnContextItem {
        cwd: config.cwd.clone(),
        approval_policy: config.approval_policy.value(),
        sandbox_policy: config.sandbox_policy.get().clone(),
        model: previous_model.to_string(),
        effort: config.model_reasoning_effort,
        summary: config.model_reasoning_summary,
        base_instructions: None,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: None,
    };

    InitialHistory::Resumed(ResumedHistory {
        conversation_id: ThreadId::default(),
        history: vec![RolloutItem::TurnContext(turn_ctx)],
        rollout_path: rollout_path.to_path_buf(),
    })
}

fn resume_history_with_base_instructions(
    config: &codex_core::config::Config,
    previous_model: &str,
    rollout_path: &std::path::Path,
    base_instructions: &str,
) -> InitialHistory {
    let session_meta = SessionMeta {
        id: ThreadId::default(),
        forked_from_id: None,
        timestamp: "2025-01-01T00-00-00".to_string(),
        cwd: config.cwd.clone(),
        originator: "resume_test".to_string(),
        cli_version: "0.0.0".to_string(),
        source: SessionSource::Cli,
        model_provider: None,
        base_instructions: Some(BaseInstructions {
            text: base_instructions.to_string(),
        }),
    };
    let session_meta_line = SessionMetaLine {
        meta: session_meta,
        git: None,
    };
    let turn_ctx = TurnContextItem {
        cwd: config.cwd.clone(),
        approval_policy: config.approval_policy.value(),
        sandbox_policy: config.sandbox_policy.get().clone(),
        model: previous_model.to_string(),
        effort: config.model_reasoning_effort,
        summary: config.model_reasoning_summary,
        base_instructions: None,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: None,
    };

    InitialHistory::Resumed(ResumedHistory {
        conversation_id: ThreadId::default(),
        history: vec![
            RolloutItem::SessionMeta(session_meta_line),
            RolloutItem::TurnContext(turn_ctx),
        ],
        rollout_path: rollout_path.to_path_buf(),
    })
}

fn non_openai_model_provider(server: &MockServer) -> codex_core::ModelProviderInfo {
    let mut provider = built_in_model_providers()["openai"].clone();
    provider.name = "OpenAI (test)".into();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_warning_when_resumed_model_differs() {
    // Arrange a config with a current model and a prior rollout recorded under a different model.
    let home = TempDir::new().expect("tempdir");
    let mut config = load_default_config_for_test(&home).await;
    config.model = Some("current-model".to_string());
    // Ensure cwd is absolute (the helper sets it to the temp dir already).
    assert!(config.cwd.is_absolute());

    let rollout_path = home.path().join("rollout.jsonl");
    std::fs::write(&rollout_path, "").expect("create rollout placeholder");

    let initial_history = resume_history(&config, "previous-model", &rollout_path);

    let thread_manager = ThreadManager::with_models_provider(
        CodexAuth::from_api_key("test"),
        config.model_provider.clone(),
    );
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test"));

    // Act: resume the conversation.
    let NewThread {
        thread: conversation,
        ..
    } = thread_manager
        .resume_thread_with_history(config, initial_history, auth_manager)
        .await
        .expect("resume conversation");

    // Assert: a Warning event is emitted describing the model mismatch.
    let warning = wait_for_event(&conversation, |ev| matches!(ev, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning else {
        panic!("expected warning event");
    };
    assert!(message.contains("previous-model"));
    assert!(message.contains("current-model"));

    // Drain the TurnComplete/Shutdown window to avoid leaking tasks between tests.
    // The warning is emitted during initialization, so a short sleep is sufficient.
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_preserves_base_instructions_from_session_meta() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let response_mock = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let home = TempDir::new().expect("tempdir");
    let mut config = load_default_config_for_test(&home).await;
    config.model = Some("gpt-5.1".to_string());
    config.model_provider = non_openai_model_provider(&server);

    let rollout_path = home.path().join("rollout.jsonl");
    std::fs::write(&rollout_path, "").expect("create rollout placeholder");

    let expected_instructions = "original base instructions";
    let initial_history = resume_history_with_base_instructions(
        &config,
        "gpt-5.1",
        &rollout_path,
        expected_instructions,
    );

    let thread_manager = ThreadManager::with_models_provider(
        CodexAuth::from_api_key("test"),
        config.model_provider.clone(),
    );
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test"));

    let NewThread {
        thread: conversation,
        ..
    } = thread_manager
        .resume_thread_with_history(config, initial_history, auth_manager)
        .await
        .expect("resume conversation");

    conversation
        .submit(codex_core::protocol::Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .expect("submit user input");
    wait_for_event(&conversation, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let body = response_mock.single_request().body_json();
    let instructions = body["instructions"].as_str().unwrap_or_default();
    assert_eq!(instructions, expected_instructions);
}
