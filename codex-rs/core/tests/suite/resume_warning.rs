#![allow(clippy::unwrap_used, clippy::expect_used)]

use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::NewThread;
use codex_core::ThreadManager;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InitialHistory;
use codex_core::protocol::ResumedHistory;
use codex_core::protocol::RolloutItem;
use codex_core::protocol::TurnContextItem;
use codex_core::protocol::WarningEvent;
use codex_protocol::ThreadId;
use core::time::Duration;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use tempfile::TempDir;

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
