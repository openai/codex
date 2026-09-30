//! Naming persists an empty paginated thread for resume before and after a restart.

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSetNameResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn named_empty_paginated_thread_resumes_after_restart() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(codex_home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let start_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            history_mode: Some(ThreadHistoryMode::Paginated),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(READ_TIMEOUT, app_server.read_response(start_id)).await??;
    let thread_id = thread.id;
    let name = "Scheduled task";
    let set_name_id = app_server
        .send_thread_set_name_request(ThreadSetNameParams {
            thread_id: thread_id.clone(),
            name: name.to_string(),
        })
        .await?;
    let _: ThreadSetNameResponse =
        timeout(READ_TIMEOUT, app_server.read_response(set_name_id)).await??;

    // Run-now resumes immediately after naming, before any user turn exists.
    let resume_id = app_server
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(READ_TIMEOUT, app_server.read_response(resume_id)).await??;
    assert_eq!(thread.id, thread_id);
    assert_eq!(thread.name.as_deref(), Some(name));
    assert!(
        timeout(READ_TIMEOUT, app_server.shutdown_gracefully())
            .await??
            .success()
    );
    drop(app_server);

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let resume_id = app_server
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let ThreadResumeResponse { thread, .. } =
        timeout(READ_TIMEOUT, app_server.read_response(resume_id)).await??;
    assert_eq!(thread.id, thread_id);
    assert_eq!(thread.name.as_deref(), Some(name));

    let input = vec![UserInput::Text {
        text: "Scheduled run".to_string(),
        text_elements: Vec::new(),
    }];
    let completed = timeout(
        READ_TIMEOUT,
        app_server.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.clone(),
            input,
            ..Default::default()
        }),
    )
    .await??;
    assert_eq!(completed.thread_id, thread_id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert_eq!(completed.turn.error, None);
    Ok(())
}
