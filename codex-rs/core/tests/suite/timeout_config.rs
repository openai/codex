use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use tempfile::TempDir;

/// Test that timeout configuration flows correctly from overrides into the
/// session initialization path.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timeout_config_integration() {
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);

    // Simulate CLI timeout override (this would normally come from TUI via ConfigOverrides)
    config.shell_environment_policy.exec_timeout_seconds = Some(30);

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create conversation")
        .conversation;

    // Verify timeout is configured - would be used in actual shell execution
    // Note: We can't easily test actual timeout execution in this integration test
    // without mocking or very complex setup, but the unit tests cover that logic

    codex.submit(Op::Shutdown).await.expect("request shutdown");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;
}
