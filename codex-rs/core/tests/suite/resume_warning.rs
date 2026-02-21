#![allow(clippy::unwrap_used, clippy::expect_used)]

use codex_core::CodexAuth;
use codex_core::NewThread;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ModeKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::ResumedHistory;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::TurnStartedEvent;
use codex_protocol::protocol::UserMessageEvent;
use codex_protocol::protocol::WarningEvent;
use core::time::Duration;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use tempfile::TempDir;

fn resume_history(
    config: &codex_core::config::Config,
    previous_model: &str,
    rollout_path: &std::path::Path,
) -> InitialHistory {
    let turn_id = "resume-warning-seed-turn".to_string();
    let turn_ctx = TurnContextItem {
        turn_id: Some(turn_id.clone()),
        cwd: config.cwd.clone(),
        approval_policy: config.permissions.approval_policy.value(),
        sandbox_policy: config.permissions.sandbox_policy.get().clone(),
        network: None,
        model: previous_model.to_string(),
        personality: None,
        collaboration_mode: None,
        effort: config.model_reasoning_effort,
        summary: config.model_reasoning_summary,
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: None,
    };

    InitialHistory::Resumed(ResumedHistory {
        conversation_id: ThreadId::default(),
        history: vec![
            RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: turn_id.clone(),
                model_context_window: None,
                collaboration_mode_kind: ModeKind::Default,
            })),
            RolloutItem::EventMsg(EventMsg::UserMessage(UserMessageEvent {
                message: "seed".to_string(),
                images: None,
                local_images: vec![],
                text_elements: vec![],
            })),
            RolloutItem::TurnContext(turn_ctx),
            RolloutItem::EventMsg(EventMsg::TurnComplete(TurnCompleteEvent {
                turn_id,
                last_agent_message: None,
            })),
        ],
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

    let thread_manager = codex_core::test_support::thread_manager_with_models_provider(
        CodexAuth::from_api_key("test"),
        config.model_provider.clone(),
    );
    let auth_manager =
        codex_core::test_support::auth_manager_from_auth(CodexAuth::from_api_key("test"));

    // Act: resume the conversation.
    let NewThread {
        thread: conversation,
        ..
    } = thread_manager
        .resume_thread_with_history(config, initial_history, auth_manager, false)
        .await
        .expect("resume conversation");

    // Assert: a Warning event is emitted describing the model mismatch.
    let warning = wait_for_event(&conversation, |ev| {
        matches!(
            ev,
            EventMsg::Warning(WarningEvent { message })
                if message.contains("previous-model") && message.contains("current-model")
        )
    })
    .await;
    let EventMsg::Warning(WarningEvent { message }) = warning else {
        panic!("expected warning event");
    };
    assert!(message.contains("previous-model"));
    assert!(message.contains("current-model"));

    // Drain the TurnComplete/Shutdown window to avoid leaking tasks between tests.
    // The warning is emitted during initialization, so a short sleep is sufficient.
    tokio::time::sleep(Duration::from_millis(50)).await;
}
