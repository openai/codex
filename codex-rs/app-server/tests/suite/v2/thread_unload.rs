//! Idle unloading must finish even when resource teardown exceeds the shutdown warning deadline.

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use pretty_assertions::assert_eq;
use std::fs::OpenOptions;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn idle_unload_finishes_after_slow_writer_shutdown() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config("thread_unload_delay_secs = 0\napprovals_reviewer = \"user\"")
        .write(home.path())?;
    let mut client = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_json_logging("warn")
        // Writer cleanup blocks one worker while the test queries the server.
        .with_env_overrides(&[("TOKIO_WORKER_THREADS", Some("2"))])
        .build_initialized()
        .await?;
    let started = client.start_thread(ThreadStartParams::default()).await?;
    let thread_id = started.thread.id;

    // Block the real cross-process writer-lock cleanup. Other app-server tasks,
    // including the ten-second shutdown deadline and JSON-RPC, can still run.
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
    // Unsubscribe only schedules shutdown. Keep the lock until the shutdown
    // future has actually crossed its warning deadline.
    client
        .wait_for_json_log_event(
            "codex.app_server.thread_shutdown_slow",
            Duration::from_secs(/*secs*/ 30),
        )
        .await
        .context("idle unload never reached the shutdown warning deadline")?;
    let loaded: ThreadLoadedListResponse = client
        .request(|request_id| ClientRequest::ThreadLoadedList {
            request_id,
            params: ThreadLoadedListParams::default(),
        })
        .await?;
    assert_eq!(
        loaded,
        ThreadLoadedListResponse {
            data: vec![thread_id.clone()],
            next_cursor: None
        }
    );

    // Once persistence can finish, delayed shutdown must still remove the runtime
    // and report closure without another client subscription or unload request.
    drop(coordination_lock);
    let closed: ThreadClosedNotification = timeout(
        Duration::from_secs(/*secs*/ 10),
        client.read_notification("thread/closed"),
    )
    .await
    .context("slow shutdown never finalized idle unload")??;
    assert_eq!(closed, ThreadClosedNotification { thread_id });
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
    Ok(())
}
