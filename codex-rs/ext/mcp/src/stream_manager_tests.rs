//! Exercises stream activation, task ownership, and cancellation.
#![allow(clippy::unwrap_used)]

use anyhow::Result;
use codex_extension_api::ExtensionData;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadReadyInput;
use codex_extension_api::ThreadStartInput;
use codex_login::CodexAuth;
use codex_mcp::McpEventStreamOpener;
use codex_mcp::McpResourceClient;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use tokio::time::timeout;

use super::event_stream_tests::Fixture;
use super::event_stream_tests::StaticAuth;
use super::event_stream_tests::WAIT;
use crate::McpEventStreamManager;
use crate::McpEventStreamUpdate;

async fn start_subscription(
    manager: &McpEventStreamManager,
    opener: &McpEventStreamOpener,
    thread_id: ThreadId,
) -> Result<u64> {
    manager
        .start(
            thread_id,
            "sub".into(),
            opener.clone(),
            "test.event".into(),
            json!({}),
            /*request_meta*/ None,
        )
        .await
}

async fn connect_loaded_task(
    manager: &McpEventStreamManager,
    thread_id: ThreadId,
    client: Arc<McpResourceClient>,
) {
    let session_store = ExtensionData::new("test");
    let thread_store = ExtensionData::new(thread_id.to_string());
    manager
        .on_thread_start(ThreadStartInput {
            config: &(),
            session_source: &SessionSource::Exec,
            persistent_thread_state_available: false,
            environments: &[],
            mcp_resource_client: Some(client),
            extension_metrics: None,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;
    manager
        .on_thread_ready(ThreadReadyInput {
            config: &(),
            session_source: &SessionSource::Exec,
            session_store: &session_store,
            thread_store: &thread_store,
        })
        .await;
}

#[tokio::test]
async fn reloaded_tasks_can_cancel_surviving_streams() -> Result<()> {
    for removed_before_ready in [false, true] {
        let mut fixture = Fixture::new().await?;
        let opener = McpResourceClient::new(Arc::clone(&fixture.runtime)).event_stream_opener()?;
        let (manager, mut updates) = McpEventStreamManager::new();
        let thread_id = ThreadId::new();
        let (result, stream) =
            tokio::join!(start_subscription(&manager, &opener, thread_id), async {
                let stream = timeout(WAIT, fixture.opened.recv()).await.unwrap().unwrap();
                stream.notify("notifications/events/active");
                stream
            });
        result?;
        timeout(WAIT, updates.recv()).await?.unwrap();
        fixture.runtime.shutdown().await;
        drop(fixture.runtime);

        let mut replacement = Fixture::new().await?;
        let client = Arc::new(McpResourceClient::new(Arc::clone(&replacement.runtime)));
        replacement.runtime_input.mcp_servers.clear();
        if removed_before_ready {
            replacement.runtime.replace(replacement.runtime_input).await;
            connect_loaded_task(&manager, thread_id, client).await;
        } else {
            connect_loaded_task(&manager, thread_id, client).await;
            let expected = stream.notify("notifications/events/event");
            let McpEventStreamUpdate::Notification { notification, .. } =
                timeout(WAIT, updates.recv()).await?.unwrap()
            else {
                panic!("event expected after task reload");
            };
            assert_eq!(notification, expected);
            replacement.runtime.replace(replacement.runtime_input).await;
        }

        timeout(WAIT, stream.notifications.closed()).await?;
        let McpEventStreamUpdate::Ended { result, .. } =
            timeout(WAIT, updates.recv()).await?.unwrap()
        else {
            panic!("stream should end after the event server is removed");
        };
        assert!(result.is_err());
        manager.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn streams_outlive_tasks_and_activate_with_full_output() -> Result<()> {
    let mut fixture = Fixture::new().await?;
    let opener = McpResourceClient::new(Arc::clone(&fixture.runtime)).event_stream_opener()?;
    let (manager, mut updates) = McpEventStreamManager::new();
    let first_thread = ThreadId::new();
    let second_thread = ThreadId::new();
    let mut streams = Vec::new();
    for thread_id in [first_thread, second_thread] {
        let start = manager.start(
            thread_id,
            "sub".into(),
            opener.clone(),
            "test.event".into(),
            json!({"resource_id": "resource-123"}),
            Some(serde_json::Map::from_iter([(
                "saved_id".into(),
                json!("sub"),
            )])),
        );
        tokio::pin!(start);
        let stream = tokio::select! {
            result = &mut start => panic!("start returned before activation: {result:?}"),
            stream = timeout(WAIT, fixture.opened.recv()) => stream?.unwrap(),
        };
        let mut params = stream.request["params"].clone();
        params["_meta"]
            .as_object_mut()
            .unwrap()
            .remove("progressToken");
        assert_eq!(
            params,
            json!({"name": "test.event", "arguments": {"resource_id": "resource-123"}, "_meta": {"saved_id": "sub"}})
        );
        stream.notify("notifications/events/active");
        timeout(WAIT, start).await??;
        timeout(WAIT, updates.recv()).await?.unwrap();
        streams.push(stream);
    }
    start_subscription(&manager, &opener, first_thread).await?;
    assert!(fixture.opened.try_recv().is_err());

    fixture.runtime.shutdown().await;
    let expected = streams[1].notify("notifications/events/event");
    let McpEventStreamUpdate::Notification {
        thread_id,
        subscription_id,
        notification,
        ..
    } = timeout(WAIT, updates.recv()).await?.unwrap()
    else {
        panic!("event expected");
    };
    assert_eq!(
        (thread_id, subscription_id, notification),
        (second_thread, "sub".into(), expected)
    );
    for _ in 0..=updates.max_capacity() {
        streams[0].notify("notifications/events/event");
    }
    timeout(WAIT, async {
        while updates.capacity() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await?;

    let third_thread = ThreadId::new();
    let (started, stream) = tokio::join!(
        timeout(WAIT, start_subscription(&manager, &opener, third_thread)),
        async {
            let stream = timeout(WAIT, fixture.opened.recv()).await.unwrap().unwrap();
            stream.notify("notifications/events/active");
            stream
        }
    );
    started??;
    assert_eq!(updates.capacity(), 0);
    manager.cancel(third_thread, "sub").await;
    timeout(WAIT, stream.notifications.closed()).await?;

    manager.cancel(first_thread, "sub").await;
    timeout(WAIT, streams[0].notifications.closed()).await?;
    manager.shutdown().await;
    timeout(WAIT, streams[1].notifications.closed()).await?;

    Ok(())
}

#[tokio::test]
async fn queued_updates_keep_their_stream_attempt_after_replacement() -> Result<()> {
    let mut fixture = Fixture::new().await?;
    let opener = McpResourceClient::new(Arc::clone(&fixture.runtime)).event_stream_opener()?;
    let (manager, mut updates) = McpEventStreamManager::new();
    let thread_id = ThreadId::new();
    let (first_attempt, stream) =
        tokio::join!(start_subscription(&manager, &opener, thread_id), async {
            let stream = timeout(WAIT, fixture.opened.recv()).await.unwrap().unwrap();
            stream.notify("notifications/events/active");
            stream
        });
    let first_attempt = first_attempt?;
    assert_eq!(
        start_subscription(&manager, &opener, thread_id).await?,
        first_attempt
    );

    stream.notify("notifications/events/terminated");
    // Leave activation, termination, and the ending queued while opening a replacement.
    timeout(WAIT, async {
        while updates.len() < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    manager.cancel(thread_id, "sub").await;

    let (second_attempt, replacement) =
        tokio::join!(start_subscription(&manager, &opener, thread_id), async {
            let stream = timeout(WAIT, fixture.opened.recv()).await.unwrap().unwrap();
            stream.notify("notifications/events/active");
            stream
        });
    let second_attempt = second_attempt?;
    assert_ne!(first_attempt, second_attempt);

    let mut queued_attempts = Vec::new();
    for _ in 0..4 {
        let stream_attempt_id = match timeout(WAIT, updates.recv()).await?.unwrap() {
            McpEventStreamUpdate::Notification {
                stream_attempt_id, ..
            } => stream_attempt_id,
            McpEventStreamUpdate::Ended {
                stream_attempt_id,
                result,
                ..
            } => {
                result?;
                assert_eq!(stream_attempt_id, first_attempt);
                stream_attempt_id
            }
        };
        queued_attempts.push(stream_attempt_id);
    }
    assert_eq!(
        queued_attempts,
        vec![first_attempt, first_attempt, first_attempt, second_attempt]
    );
    manager.shutdown().await;
    timeout(WAIT, replacement.notifications.closed()).await?;
    Ok(())
}

#[tokio::test]
async fn cancelling_before_activation_releases_the_subscription_id() -> Result<()> {
    let mut fixture = Fixture::new().await?;
    let opener = McpResourceClient::new(Arc::clone(&fixture.runtime)).event_stream_opener()?;
    let (manager, _updates) = McpEventStreamManager::new();
    let thread_id = ThreadId::new();
    let start = start_subscription(&manager, &opener, thread_id);
    tokio::pin!(start);
    let stream = tokio::select! {
        result = &mut start => panic!("start returned before activation: {result:?}"),
        stream = timeout(WAIT, fixture.opened.recv()) => stream?.unwrap(),
    };
    manager.cancel(thread_id, "sub").await;
    assert!(timeout(WAIT, start).await?.is_err());
    timeout(WAIT, stream.notifications.closed()).await?;
    let (result, replacement) =
        tokio::join!(start_subscription(&manager, &opener, thread_id), async {
            let stream = timeout(WAIT, fixture.opened.recv()).await.unwrap().unwrap();
            stream.notify("notifications/events/active");
            stream
        });
    result?;
    manager.shutdown().await;
    timeout(WAIT, replacement.notifications.closed()).await?;
    Ok(())
}

#[tokio::test]
async fn access_changes_cancel_streams_with_full_output() -> Result<()> {
    for remove_server in [false, true] {
        let mut fixture = Fixture::new().await?;
        let opener = McpResourceClient::new(Arc::clone(&fixture.runtime)).event_stream_opener()?;
        let (manager, mut updates) = McpEventStreamManager::new();
        let thread_id = ThreadId::new();
        let (result, stream) =
            tokio::join!(start_subscription(&manager, &opener, thread_id), async {
                let stream = timeout(WAIT, fixture.opened.recv()).await.unwrap().unwrap();
                stream.notify("notifications/events/active");
                stream
            });
        result?;
        timeout(WAIT, updates.recv()).await?.unwrap();
        for _ in 0..=updates.max_capacity() {
            stream.notify("notifications/events/event");
        }
        timeout(WAIT, async {
            while updates.capacity() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        if remove_server {
            fixture.runtime_input.mcp_servers.clear();
            fixture.runtime.replace(fixture.runtime_input).await;
        } else {
            fixture
                .auth
                .set_external_auth(Arc::new(StaticAuth(
                    CodexAuth::from_external_chatgpt_tokens(
                        "header.e30.other",
                        "other-account",
                        /*chatgpt_plan_type*/ None,
                    )?,
                )))
                .await?;
        }
        // Access changes close the stream even while its output is blocked.
        timeout(WAIT, stream.notifications.closed()).await?;
        let result = timeout(WAIT, async {
            loop {
                if let McpEventStreamUpdate::Ended { result, .. } = updates.recv().await.unwrap() {
                    break result;
                }
            }
        })
        .await?;
        assert!(result.is_err());
        assert!(
            opener
                .open("test.event", &json!({}), /*request_meta*/ None)
                .await
                .is_err()
        );
    }
    Ok(())
}
