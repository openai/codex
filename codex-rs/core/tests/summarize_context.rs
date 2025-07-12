#![expect(clippy::unwrap_used, clippy::expect_used)]

//! Tests for the `Op::SummarizeContext` operation added to verify that
//! summarization requests are properly handled and injected as user input.

use std::time::Duration;

use codex_core::Codex;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
mod test_support;
use tempfile::TempDir;
use test_support::load_default_config_for_test;
use tokio::time::timeout;

/// Helper function to set up a codex session and wait for it to be configured
async fn setup_configured_codex_session() -> Codex {
    let codex_home = TempDir::new().unwrap();
    let config = load_default_config_for_test(&codex_home);
    let (codex, _, _) = codex_core::codex_wrapper::init_codex(config).await.unwrap();
    codex
}

#[tokio::test]
async fn test_summarize_context_spawns_new_agent_task() {
    // Test the specific behavior: when there's no current task,
    // SummarizeContext should spawn a new AgentTask with the summarization prompt
    let codex = setup_configured_codex_session().await;

    // At this point, there should be no current task running
    let _sub_id = codex.submit(Op::SummarizeContext).await.unwrap();

    let event = timeout(Duration::from_secs(5), codex.next_event())
        .await
        .expect("timeout waiting for task started event")
        .expect("codex closed");

    assert!(
        matches!(event.msg, EventMsg::TaskStarted),
        "Expected TaskStarted when no current task exists - should spawn new AgentTask"
    );
}
