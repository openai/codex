use anyhow::Context;
use anyhow::Result;
use app_test_support::DEFAULT_CLIENT_NAME;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadReadStateChangedNotification;
use codex_app_server_protocol::ThreadReadStateUpdateResponse;
use codex_app_server_protocol::ThreadRevertParams;
use codex_app_server_protocol::ThreadRevertResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::test]
async fn read_state_methods_require_experimental_capability() -> Result<()> {
    let mut server = TestAppServer::builder().without_auto_env().build().await?;
    server
        .initialize_with_capabilities(
            ClientInfo {
                name: DEFAULT_CLIENT_NAME.to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: false,
                ..Default::default()
            }),
        )
        .await?;
    let id = server
        .send_raw_request(
            "thread/readState/update",
            Some(json!({
                "threadId": "unknown",
                "expectedRevision": "none", "operation": {"type": "unread"}
            })),
        )
        .await?;
    let error = timeout(
        TIMEOUT,
        server.read_stream_until_error_message(RequestId::Integer(id)),
    )
    .await??;
    assert!(error.error.message.contains("experimental"), "{error:?}");
    Ok(())
}

#[tokio::test]
async fn read_state_tracks_terminal_activity_and_reverted_history() -> Result<()> {
    let responses = create_mock_responses_server_repeating_assistant("Ready").await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses.uri())
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let started = server
        .start_thread(ThreadStartParams {
            model: Some("mock-model".into()),
            history_mode: Some(ThreadHistoryMode::Paginated),
            ..Default::default()
        })
        .await?;
    let thread_id = started.thread.id;
    let completed = timeout(
        TIMEOUT,
        server.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "Finish this task.".into(),
                text_elements: vec![],
            }],
            ..Default::default()
        }),
    )
    .await??;
    let loaded: ThreadReadResponse = server
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: thread_id.clone(),
                include_turns: false,
            },
        })
        .await?;
    let first = loaded
        .read_state
        .context("durable completed user thread should have read state")?;
    assert_eq!(
        first.first_unread,
        Some(codex_app_server_protocol::ThreadUnreadPosition::Turn {
            turn_id: completed.turn.id.clone()
        })
    );

    let reloaded: ThreadReadResponse = server
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: thread_id.clone(),
                include_turns: false,
            },
        })
        .await?;
    assert_eq!(
        reloaded.read_state,
        Some(first.clone()),
        "loading a snapshot must never acknowledge the activity"
    );

    let id = server
        .send_raw_request("thread/list", Some(json!({"limit": 1})))
        .await?;
    let listed: ThreadListResponse = timeout(TIMEOUT, server.read_response(id)).await??;
    let receipts = listed.read_states.context("durable list receipts")?;
    assert_eq!(listed.data.len(), 1);
    assert_eq!(listed.data[0].id, thread_id);
    assert_eq!(receipts, [(thread_id.clone(), first.clone())].into());

    let changed: ThreadReadStateChangedNotification = timeout(
        TIMEOUT,
        server.read_notification("thread/readState/changed"),
    )
    .await??;
    assert_eq!(changed.read_state, first);

    let id = server
        .send_raw_request(
            "thread/readState/update",
            Some(json!({
                "threadId": thread_id,
                "expectedRevision": first.revision, "operation": {"type": "unread"}
            })),
        )
        .await?;
    let marked: ThreadReadStateUpdateResponse =
        timeout(TIMEOUT, server.read_response(id)).await??;
    assert_eq!(
        marked.read_state.first_unread,
        Some(codex_app_server_protocol::ThreadUnreadPosition::ThreadStart)
    );
    let changed: ThreadReadStateChangedNotification = timeout(
        TIMEOUT,
        server.read_notification("thread/readState/changed"),
    )
    .await??;
    assert_eq!(changed.read_state, marked.read_state);

    // The initial snapshot predates this explicit unread mark. Both stale actions
    // must conflict, and neither may erase the manual marker or change the revision.
    for operation in ["read", "unread"] {
        let id = server
            .send_raw_request(
                "thread/readState/update",
                Some(json!({
                    "threadId": thread_id,
                    "expectedRevision": first.revision,
                    "operation": {"type": operation}
                })),
            )
            .await?;
        let error = timeout(
            TIMEOUT,
            server.read_stream_until_error_message(RequestId::Integer(id)),
        )
        .await??;
        assert_eq!(
            error.error.data,
            Some(json!({"reason": "readStateConflict", "readState": marked.read_state}))
        );
    }
    let id = server
        .send_raw_request(
            "thread/readState/update",
            Some(json!({
                "threadId": thread_id,
                "expectedRevision": marked.read_state.revision,
                "operation": {"type": "read"}
            })),
        )
        .await?;
    let fresh: ThreadReadStateUpdateResponse = timeout(TIMEOUT, server.read_response(id)).await??;
    assert_eq!(fresh.read_state.first_unread, None);

    let _: ThreadRevertResponse = server
        .request(|request_id| ClientRequest::ThreadRevert {
            request_id,
            params: ThreadRevertParams {
                thread_id: thread_id.clone(),
                before_turn_id: completed.turn.id,
            },
        })
        .await?;
    let reverted: ThreadReadResponse = server
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id,
                include_turns: false,
            },
        })
        .await?;
    let after = reverted.read_state.context("reverted snapshot")?;
    assert_eq!(after.first_unread, None);
    assert_ne!(after.revision, first.revision);
    Ok(())
}
