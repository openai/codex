//! Attached-thread fork and discovery after restart and SQLite recovery, including pre-database parents.
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_fake_paginated_rollout;
use app_test_support::create_fake_rollout;
use app_test_support::create_mock_responses_server_repeating_assistant;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadDeleteParams;
use codex_app_server_protocol::ThreadDeleteResponse;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::ThreadLoadedListResponse;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_state::SqliteConfig;
use codex_utils_absolute_path::test_support::PathExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;

#[derive(Clone, Copy)]
enum Recovery {
    Restart,
    Missing,
    Corrupt,
}

#[test_case(ThreadHistoryMode::Legacy, Recovery::Restart; "legacy restart")]
#[test_case(ThreadHistoryMode::Paginated, Recovery::Restart; "paginated restart")]
#[test_case(ThreadHistoryMode::Legacy, Recovery::Missing; "legacy missing database")]
#[test_case(ThreadHistoryMode::Paginated, Recovery::Corrupt; "paginated corrupt database")]
#[tokio::test]
async fn saved_attached_thread_upgrade_restart_and_delete(
    mode: ThreadHistoryMode,
    recovery: Recovery,
) -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(home.path())?;
    // Both parents were persisted before this app-server (and its SQLite migration) starts.
    let create_rollout = match mode {
        ThreadHistoryMode::Legacy => create_fake_rollout,
        ThreadHistoryMode::Paginated => create_fake_paginated_rollout,
    };
    let parent = create_rollout(
        home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "BLUE HERON 72",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let other = create_rollout(
        home.path(),
        "2025-01-06T12-00-00",
        "2025-01-06T12:00:00Z",
        "unrelated",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let mut children = Vec::new();
    for _ in 0..2 {
        let id = mcp
            .send_thread_fork_request(ThreadForkParams {
                thread_id: parent.clone(),
                attach_to_source: true,
                exclude_turns: true,
                ..Default::default()
            })
            .await?;
        let fork: ThreadForkResponse = mcp.read_response(id).await?;
        assert!(!fork.thread.ephemeral);
        assert_eq!(fork.thread.forked_from_id, Some(parent.clone()));
        children.push(fork.thread.id);
    }
    // Newer attached threads fill the raw scan page. Listing must refill across hidden
    // results, with and without title filters and in both scan directions.
    for (params, expected) in [
        (
            json!({"limit": 1, "searchTerm": "BLUE HERON"}),
            vec![&parent],
        ),
        (
            json!({"limit": 1, "searchTerm": "BLUE HERON", "useStateDbOnly": true}),
            vec![&parent],
        ),
        (
            json!({"limit": 2, "modelProviders": ["mock_provider"]}),
            vec![&other, &parent],
        ),
        (
            json!({"limit": 2, "modelProviders": ["mock_provider"], "sortDirection": "asc"}),
            vec![&parent, &other],
        ),
        (
            json!({"limit": 2, "useStateDbOnly": true}),
            vec![&other, &parent],
        ),
        // The existing parent filter continues to mean spawned children.
        (json!({"parentThreadId": parent}), vec![]),
    ] {
        let id = mcp.send_raw_request("thread/list", Some(params)).await?;
        let ordinary: ThreadListResponse = mcp.read_response(id).await?;
        assert_eq!(
            ordinary.data.iter().map(|t| &t.id).collect::<Vec<_>>(),
            expected
        );
    }
    let sqlite = SqliteConfig::new_for_testing(home.path().canonicalize()?.as_path().abs());
    assert!(mcp.shutdown_gracefully().await?.success());
    // No child has taken a turn yet; its attachment must still survive a restart.
    match recovery {
        Recovery::Restart => {}
        Recovery::Missing => tokio::fs::remove_file(sqlite.state_db_path()).await?,
        Recovery::Corrupt => {
            tokio::fs::write(sqlite.state_db_path(), b"damaged state database").await?
        }
    }
    let mut mcp = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let id = mcp
        .send_raw_request(
            "thread/list",
            Some(json!({"useStateDbOnly": true, "limit": 2})),
        )
        .await?;
    let ordinary: ThreadListResponse = mcp.read_response(id).await?;
    assert_eq!(
        ordinary.data.iter().map(|t| &t.id).collect::<Vec<_>>(),
        vec![&other, &parent]
    );
    let mut cursor = None;
    let mut found = Vec::new();
    loop {
        let id = mcp
            .send_raw_request(
                "thread/list",
                Some(json!({
                    "attachedToThreadId": parent, "limit": 1, "cursor": cursor
                })),
            )
            .await?;
        let page: ThreadListResponse = mcp.read_response(id).await?;
        assert_eq!(page.data.len(), 1);
        found.push(page.data[0].id.clone());
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
        assert!(
            found.len() <= children.len(),
            "attached thread cursor must advance"
        );
    }
    found.sort();
    children.sort();
    assert_eq!(found, children);
    let id = mcp
        .send_raw_request("thread/list", Some(json!({"attachedToThreadId": other})))
        .await?;
    let other_list: ThreadListResponse = mcp.read_response(id).await?;
    assert!(other_list.data.is_empty());
    let id = mcp
        .send_raw_request(
            "thread/resume",
            Some(json!({
                "threadId": children[0], "excludeTurns": true,
                "developerInstructions": "Follow BIRCH-849."
            })),
        )
        .await?;
    let resumed: ThreadResumeResponse = mcp.read_response(id).await?;
    assert_eq!(resumed.thread.id, children[0]);
    // Ordinary forks do not inherit attachment.
    let id = mcp
        .send_thread_fork_request(ThreadForkParams {
            thread_id: children[0].clone(),
            ..Default::default()
        })
        .await?;
    let independent: ThreadForkResponse = mcp.read_response(id).await?;
    let id = mcp
        .send_raw_request(
            "thread/list",
            Some(json!({"attachedToThreadId": children[0]})),
        )
        .await?;
    let attached: ThreadListResponse = mcp.read_response(id).await?;
    assert!(attached.data.is_empty());
    let id = mcp
        .send_thread_delete_request(ThreadDeleteParams {
            thread_id: independent.thread.id,
        })
        .await?;
    let _: ThreadDeleteResponse = mcp.read_response(id).await?;
    let id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: children[0].clone(),
            input: vec![UserInput::Text {
                text: "continue".into(),
                text_elements: vec![],
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = mcp.read_response(id).await?;
    mcp.read_stream_until_notification_message("turn/completed")
        .await?;
    let requests = server.received_requests().await.expect("response requests");
    let input = requests
        .iter()
        .find(|r| r.url.path().ends_with("/responses"))
        .expect("resumed attached thread sends a model request")
        .body_json::<serde_json::Value>()?["input"]
        .to_string();
    assert!(
        input.contains("BIRCH-849"),
        "resume applies client instructions"
    );
    let id = mcp
        .send_raw_request("thread/resume", Some(json!({"threadId": parent})))
        .await?;
    let _: ThreadResumeResponse = mcp.read_response(id).await?;
    let id = mcp
        .send_thread_delete_request(ThreadDeleteParams {
            thread_id: parent.clone(),
        })
        .await?;
    let error = mcp
        .read_stream_until_error_message(RequestId::Integer(id))
        .await?;
    assert!(
        error
            .error
            .message
            .contains("delete the attached thread first")
    );
    let id = mcp
        .send_thread_loaded_list_request(ThreadLoadedListParams::default())
        .await?;
    let loaded: ThreadLoadedListResponse = mcp.read_response(id).await?;
    assert!(
        loaded.data.contains(&parent),
        "rejection must leave the parent loaded"
    );
    for child in children {
        let id = mcp
            .send_thread_delete_request(ThreadDeleteParams { thread_id: child })
            .await?;
        let _: ThreadDeleteResponse = mcp.read_response(id).await?;
    }
    let id = mcp
        .send_thread_delete_request(ThreadDeleteParams { thread_id: parent })
        .await?;
    let _: ThreadDeleteResponse = mcp.read_response(id).await?;
    let id = mcp.send_raw_request("thread/list", Some(json!({}))).await?;
    let ordinary: ThreadListResponse = mcp.read_response(id).await?;
    assert_eq!(
        ordinary.data.iter().map(|t| &t.id).collect::<Vec<_>>(),
        vec![&other]
    );
    Ok(())
}

#[tokio::test]
async fn saved_attached_thread_rejects_conflicting_fork_and_discovery_filters() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri()).write(home.path())?;
    let parent = create_fake_rollout(
        home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "parent",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    for (method, params, message) in [
        (
            "thread/fork",
            json!({"threadId": parent, "attachToSource": true, "ephemeral": true}),
            "attachToSource cannot be combined",
        ),
        (
            "thread/list",
            json!({"attachedToThreadId": parent, "parentThreadId": parent}),
            "attachedToThreadId cannot be combined",
        ),
        (
            "thread/list",
            json!({"attachedToThreadId": parent, "ancestorThreadId": parent}),
            "attachedToThreadId cannot be combined",
        ),
        (
            "thread/fork",
            json!({"threadId": parent, "attachToSource": true, "deferGoalContinuation": true}),
            "attachToSource cannot be combined",
        ),
    ] {
        let id = mcp.send_raw_request(method, Some(params)).await?;
        let error = mcp
            .read_stream_until_error_message(RequestId::Integer(id))
            .await?;
        assert!(
            error.error.message.starts_with(message),
            "{:?}",
            error.error
        );
    }
    let id = mcp
        .send_raw_request("thread/list", Some(json!({"attachedToThreadId": parent})))
        .await?;
    let page: ThreadListResponse = mcp.read_response(id).await?;
    assert!(
        page.data.is_empty(),
        "failed forks must leave no discoverable attached thread"
    );
    Ok(())
}
