//! Idle unloading must finish when another process holds the writer coordination lock.

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use pretty_assertions::assert_eq;
use std::fs::OpenOptions;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn idle_unload_finishes_while_writer_coordination_is_locked() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config("thread_unload_delay_secs = 0\napprovals_reviewer = \"user\"")
        .write(home.path())?;
    let mut client = TestAppServer::builder()
        .with_codex_home(home.path())
        // Writer cleanup blocks one worker while the test queries the server.
        .with_env_overrides(&[("TOKIO_WORKER_THREADS", Some("2"))])
        .build_initialized()
        .await?;
    let started = client.start_thread(ThreadStartParams::default()).await?;
    let thread_id = started.thread.id;

    // Persist a turn so a cold resume can verify durable history after unload.
    client
        .start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;

    // Hold coordination through unload: cleanup must time out without keeping
    // the thread loaded or retaining its writer ownership.
    let coordination_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(home.path().join("thread-writer-locks/.coordination.lock"))?;
    coordination_lock.lock()?;
    let unsubscribed: ThreadUnsubscribeResponse = client
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams {
                thread_id: thread_id.clone(),
            },
        })
        .await?;
    assert_eq!(
        unsubscribed,
        ThreadUnsubscribeResponse {
            status: ThreadUnsubscribeStatus::Unsubscribed
        }
    );
    let closed: ThreadClosedNotification = timeout(
        Duration::from_secs(/*secs*/ 60),
        client.read_notification("thread/closed"),
    )
    .await
    .context("writer coordination contention prevented idle unload")??;
    assert_eq!(
        closed,
        ThreadClosedNotification {
            thread_id: thread_id.clone()
        }
    );
    let loaded: ThreadLoadedListResponse = client
        .request(|request_id| ClientRequest::ThreadLoadedList {
            request_id,
            params: ThreadLoadedListParams::default(),
        })
        .await?;
    assert_eq!(
        loaded,
        ThreadLoadedListResponse {
            data: Vec::new(),
            next_cursor: None
        }
    );
    // Cleanup may leave a lock file behind, but must release ownership so the
    // same thread can resume after the other process releases coordination.
    drop(coordination_lock);
    let resumed: ThreadResumeResponse = client
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: thread_id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(
        (resumed.thread.id, resumed.thread.turns.len()),
        (thread_id, 1)
    );
    Ok(())
}
