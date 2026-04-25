#![cfg(not(target_os = "windows"))]

use std::sync::Arc;

use anyhow::Result;
use codex_core::CodexThread;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::UndoCompletedEvent;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

async fn undo_harness() -> Result<TestCodexHarness> {
    TestCodexHarness::with_builder(test_codex().with_model("gpt-5.4")).await
}

async fn invoke_undo(codex: &Arc<CodexThread>) -> Result<UndoCompletedEvent> {
    codex.submit(Op::Undo).await?;
    let event = wait_for_event_match(codex, |msg| match msg {
        EventMsg::UndoCompleted(done) => Some(done.clone()),
        _ => None,
    })
    .await;
    Ok(event)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_reports_feature_removal() -> Result<()> {
    let harness = undo_harness().await?;
    let codex = Arc::clone(&harness.test().codex);

    let event = invoke_undo(&codex).await?;

    assert!(!event.success, "expected undo to fail");
    assert_eq!(
        event.message.as_deref(),
        Some("Undo is no longer available.")
    );

    Ok(())
}
