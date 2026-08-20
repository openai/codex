use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_escalated_command_execution_sse_response;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::QueuedSubmission;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadQueueAddParams;
use codex_app_server_protocol::ThreadQueueAddResponse;
use codex_app_server_protocol::ThreadQueueChangedNotification;
use codex_app_server_protocol::ThreadQueueDeleteParams;
use codex_app_server_protocol::ThreadQueueDeleteResponse;
use codex_app_server_protocol::ThreadQueueListParams;
use codex_app_server_protocol::ThreadQueueListResponse;
use codex_app_server_protocol::ThreadQueueReorderParams;
use codex_app_server_protocol::ThreadQueueReorderResponse;
use codex_app_server_protocol::ThreadQueueStartParams;
use codex_app_server_protocol::ThreadQueueStartResponse;
use codex_app_server_protocol::ThreadQueueUpdateParams;
use codex_app_server_protocol::ThreadQueueUpdateResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use core_test_support::skip_if_remote;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::MockServer;

const READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);

#[tokio::test]
async fn queue_requires_experimental_handshake() -> Result<()> {
    let (mut app, codex_home, _server) = queue_app(Vec::new()).await?;
    let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
    let queue = list_queue(&mut app, &thread.id).await?;
    assert!(queue.data.is_empty());
    drop(app);

    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build()
        .await?;
    app.initialize_with_capabilities(
        ClientInfo {
            name: "queue-experimental-gate".to_string(),
            title: None,
            version: "0.1.0".to_string(),
        },
        Some(InitializeCapabilities::default()),
    )
    .await?;
    let request_id = app
        .send_raw_request("thread/start", Some(json!({})))
        .await?;
    let thread = timeout(
        READ_TIMEOUT,
        app.read_response::<ThreadStartResponse>(request_id),
    )
    .await??
    .thread;
    let request_id = app
        .send_raw_request(
            "thread/queue/list",
            Some(serde_json::to_value(ThreadQueueListParams {
                thread_id: thread.id,
                cursor: None,
                limit: None,
            })?),
        )
        .await?;
    let error: JSONRPCError = timeout(
        READ_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert!(error.error.message.contains("experimental"));
    Ok(())
}

#[tokio::test]
async fn queue_crud_preserves_identity_order_and_notifications() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "uses a host-local command and cwd fixture unavailable to remote executors"
    );

    let responses = vec![
        blocked_turn_response()?,
        create_final_assistant_message_sse_response("active done")?,
        create_final_assistant_message_sse_response("queued done")?,
    ];
    let (mut app, _codex_home, _server) = queue_app(responses).await?;
    let thread_id = app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let (_, approval_id) = start_blocked_turn(&mut app, &thread_id).await?;
    let first = queue_item(
        &mut app,
        ThreadQueueAddParams {
            client_user_message_id: "first-client-message".to_string(),
            ..submission(&thread_id, "first")
        },
    )
    .await?;
    let second = queue_item(&mut app, submission(&thread_id, "second")).await?;
    let first_change: ThreadQueueChangedNotification =
        timeout(READ_TIMEOUT, app.read_notification("thread/queue/changed")).await??;
    let second_change: ThreadQueueChangedNotification =
        timeout(READ_TIMEOUT, app.read_notification("thread/queue/changed")).await??;
    assert_eq!(
        first_change,
        ThreadQueueChangedNotification {
            thread_id: thread_id.clone(),
        }
    );
    assert_eq!(
        second_change,
        ThreadQueueChangedNotification {
            thread_id: thread_id.clone(),
        }
    );

    let updated: ThreadQueueUpdateResponse = app
        .request(|request_id| ClientRequest::ThreadQueueUpdate {
            request_id,
            params: ThreadQueueUpdateParams {
                thread_id: thread_id.clone(),
                queued_submission_id: first.id.clone(),
                input: vec![text("first edited")],
            },
        })
        .await?;
    assert_eq!(updated.queued_submission.id, first.id);
    assert_eq!(
        updated.queued_submission.client_user_message_id,
        first.client_user_message_id
    );
    assert_eq!(updated.queued_submission.input, vec![text("first edited")]);
    let update_change: ThreadQueueChangedNotification =
        timeout(READ_TIMEOUT, app.read_notification("thread/queue/changed")).await??;
    assert_eq!(
        update_change,
        ThreadQueueChangedNotification {
            thread_id: thread_id.clone(),
        }
    );

    let invalid_reorder = app
        .send_raw_request(
            "thread/queue/reorder",
            Some(serde_json::to_value(ThreadQueueReorderParams {
                thread_id: thread_id.clone(),
                queued_submission_ids: vec![first.id.clone()],
            })?),
        )
        .await?;
    let error: JSONRPCError = timeout(
        READ_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(invalid_reorder)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "queue reorder must include every queued submission exactly once"
    );

    let _: ThreadQueueReorderResponse = app
        .request(|request_id| ClientRequest::ThreadQueueReorder {
            request_id,
            params: ThreadQueueReorderParams {
                thread_id: thread_id.clone(),
                queued_submission_ids: vec![second.id.clone(), first.id.clone()],
            },
        })
        .await?;
    let reorder_change: ThreadQueueChangedNotification =
        timeout(READ_TIMEOUT, app.read_notification("thread/queue/changed")).await??;
    assert_eq!(
        reorder_change,
        ThreadQueueChangedNotification {
            thread_id: thread_id.clone(),
        }
    );
    let deleted: ThreadQueueDeleteResponse = app
        .request(|request_id| ClientRequest::ThreadQueueDelete {
            request_id,
            params: ThreadQueueDeleteParams {
                thread_id: thread_id.clone(),
                queued_submission_id: second.id,
            },
        })
        .await?;
    assert!(deleted.deleted);
    let delete_change: ThreadQueueChangedNotification =
        timeout(READ_TIMEOUT, app.read_notification("thread/queue/changed")).await??;
    assert_eq!(
        delete_change,
        ThreadQueueChangedNotification {
            thread_id: thread_id.clone(),
        }
    );

    decline_approval(&mut app, approval_id).await?;
    for _ in 0..2 {
        let _: TurnCompletedNotification =
            timeout(READ_TIMEOUT, app.read_notification("turn/completed")).await??;
    }
    let drain_change: ThreadQueueChangedNotification =
        timeout(READ_TIMEOUT, app.read_notification("thread/queue/changed")).await??;
    assert_eq!(drain_change, ThreadQueueChangedNotification { thread_id });
    Ok(())
}

#[tokio::test]
async fn queue_list_returns_ordered_pages_and_lightweight_notifications() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "uses a host-local command and cwd fixture unavailable to remote executors"
    );

    let (mut app, _codex_home, _server) = queue_app(vec![blocked_turn_response()?]).await?;
    let thread_id = app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let _blocked = start_blocked_turn(&mut app, &thread_id).await?;
    let first = queue_item(
        &mut app,
        submission(&thread_id, "fitting queued submission"),
    )
    .await?;
    let queued = queue_item(
        &mut app,
        ThreadQueueAddParams {
            thread_id: thread_id.clone(),
            input: vec![text(&"x".repeat(64 * 1024))],
            client_user_message_id: "oversized-snapshot".to_string(),
        },
    )
    .await?;
    let initial_change: ThreadQueueChangedNotification =
        timeout(READ_TIMEOUT, app.read_notification("thread/queue/changed")).await??;
    assert_eq!(
        initial_change,
        ThreadQueueChangedNotification {
            thread_id: thread_id.clone(),
        }
    );
    let changed: ThreadQueueChangedNotification =
        timeout(READ_TIMEOUT, app.read_notification("thread/queue/changed")).await??;
    assert_eq!(
        changed,
        ThreadQueueChangedNotification {
            thread_id: thread_id.clone(),
        }
    );
    let first_page: ThreadQueueListResponse = app
        .request(|request_id| ClientRequest::ThreadQueueList {
            request_id,
            params: ThreadQueueListParams {
                thread_id: thread_id.clone(),
                cursor: None,
                limit: Some(1),
            },
        })
        .await?;
    assert_eq!(first_page.data, vec![first.clone()]);
    assert_eq!(first_page.next_cursor, Some("1".to_string()));
    let second_page: ThreadQueueListResponse = app
        .request(|request_id| ClientRequest::ThreadQueueList {
            request_id,
            params: ThreadQueueListParams {
                thread_id: thread_id.clone(),
                cursor: first_page.next_cursor,
                limit: Some(1),
            },
        })
        .await?;
    assert_eq!(second_page.data, vec![queued.clone()]);
    assert_eq!(second_page.next_cursor, None);
    assert_eq!(
        list_queue(&mut app, &thread_id).await?.data,
        vec![first, queued]
    );
    Ok(())
}

#[tokio::test]
async fn queue_rejects_messages_after_reaching_its_capacity() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "uses a host-local command and cwd fixture unavailable to remote executors"
    );

    let (mut app, _codex_home, _server) = queue_app(vec![blocked_turn_response()?]).await?;
    let thread_id = app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let _blocked = start_blocked_turn(&mut app, &thread_id).await?;

    for index in 0..100 {
        queue_item(&mut app, submission(&thread_id, &format!("queued {index}"))).await?;
    }
    assert_eq!(list_queue(&mut app, &thread_id).await?.data.len(), 100);

    let request_id = app
        .send_raw_request(
            "thread/queue/add",
            Some(serde_json::to_value(submission(
                &thread_id,
                "one too many",
            ))?),
        )
        .await?;
    let error: JSONRPCError = timeout(
        READ_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(
        error.error.message,
        "queue cannot contain more than 100 submissions"
    );
    Ok(())
}

#[tokio::test]
async fn idle_queue_dispatch_preserves_client_id() -> Result<()> {
    let responses = vec![create_final_assistant_message_sse_response("queued done")?];
    let (mut app, _codex_home, server) = queue_app(responses).await?;
    let thread_id = app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let queued_submission = ThreadQueueAddParams {
        thread_id: thread_id.clone(),
        input: vec![text("durable queued message")],
        client_user_message_id: "stable-queued-client-id".to_string(),
    };
    let queued = queue_item(&mut app, queued_submission.clone()).await?;
    assert_eq!(
        queued.client_user_message_id,
        queued_submission.client_user_message_id
    );
    let started: ItemStartedNotification =
        timeout(READ_TIMEOUT, app.read_notification("item/started")).await??;
    let ThreadItem::UserMessage {
        client_id, content, ..
    } = started.item
    else {
        anyhow::bail!("queued turn did not begin with its user message");
    };
    assert_eq!(client_id.as_deref(), Some("stable-queued-client-id"));
    assert_eq!(content, queued_submission.input);
    let completed: TurnCompletedNotification =
        timeout(READ_TIMEOUT, app.read_notification("turn/completed")).await??;
    assert_eq!(completed.thread_id, thread_id);
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert!(list_queue(&mut app, &thread_id).await?.data.is_empty());

    let requests = server
        .received_requests()
        .await
        .context("mock request capture unavailable")?;
    let request = requests
        .iter()
        .find(|request| request.url.path().ends_with("/responses"))
        .context("queued turn did not reach the model")?;
    let body = request.body_json::<Value>()?;
    assert!(body["input"].to_string().contains("durable queued message"));
    let metadata_header = request
        .headers
        .get("x-codex-turn-metadata")
        .context("queued model request is missing its x-codex-turn-metadata header")?
        .to_str()
        .context("queued turn metadata header is not valid ASCII")?;
    let metadata: Value = serde_json::from_str(metadata_header)?;
    assert_eq!(metadata["thread_id"].as_str(), Some(thread_id.as_str()));
    assert_eq!(
        metadata["turn_id"].as_str(),
        Some(completed.turn.id.as_str())
    );
    Ok(())
}

#[tokio::test]
async fn cold_thread_resume_dispatches_a_persisted_queued_submission() -> Result<()> {
    let responses = vec![
        create_final_assistant_message_sse_response("materialized thread")?,
        create_final_assistant_message_sse_response("cold-resumed queued message")?,
    ];
    let (mut first, codex_home, _server) = queue_app(responses).await?;
    let thread_id = first
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let _: TurnStartResponse = first
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread_id.clone(),
                input: vec![text("materialize the thread before restarting")],
                ..Default::default()
            },
        })
        .await?;
    let _: TurnCompletedNotification =
        timeout(READ_TIMEOUT, first.read_notification("turn/completed")).await??;
    drop(first);

    let mut resumed = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let queued = queue_item(
        &mut resumed,
        ThreadQueueAddParams {
            client_user_message_id: "cold-resumed-queue-item".to_string(),
            ..submission(&thread_id, "dispatch this after a cold thread resume")
        },
    )
    .await?;
    assert_eq!(
        list_queue(&mut resumed, &thread_id).await?.data,
        vec![queued]
    );
    let request_id = resumed
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let response: ThreadResumeResponse =
        timeout(READ_TIMEOUT, resumed.read_response(request_id)).await??;
    assert_eq!(thread_id, response.thread.id);

    let started: ItemStartedNotification =
        timeout(READ_TIMEOUT, resumed.read_notification("item/started")).await??;
    let ThreadItem::UserMessage {
        client_id, content, ..
    } = started.item
    else {
        anyhow::bail!("cold resume did not start the persisted queued user message");
    };
    assert_eq!(client_id.as_deref(), Some("cold-resumed-queue-item"));
    assert_eq!(
        content,
        vec![text("dispatch this after a cold thread resume")]
    );
    let completed: TurnCompletedNotification =
        timeout(READ_TIMEOUT, resumed.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert!(list_queue(&mut resumed, &thread_id).await?.data.is_empty());
    Ok(())
}

#[tokio::test]
async fn interrupt_preserves_queue_and_queue_start_can_resume_a_non_head_item() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "uses a host-local command and cwd fixture unavailable to remote executors"
    );

    let responses = vec![
        blocked_turn_response()?,
        create_final_assistant_message_sse_response("first queued message done")?,
        create_final_assistant_message_sse_response("second queued message done")?,
        create_final_assistant_message_sse_response("message added after interruption done")?,
        create_final_assistant_message_sse_response("message added after cold resume done")?,
    ];
    let (mut app, codex_home, _server) = queue_app(responses).await?;
    let thread_id = app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let (active_turn_id, _approval_id) = start_blocked_turn(&mut app, &thread_id).await?;

    let first = queue_item(&mut app, submission(&thread_id, "first queued message")).await?;
    let second = queue_item(
        &mut app,
        ThreadQueueAddParams {
            client_user_message_id: "second-queued-client-id".to_string(),
            ..submission(&thread_id, "second queued message")
        },
    )
    .await?;

    let _: TurnInterruptResponse = app
        .request(|request_id| ClientRequest::TurnInterrupt {
            request_id,
            params: TurnInterruptParams {
                thread_id: thread_id.clone(),
                turn_id: active_turn_id,
            },
        })
        .await?;
    let interrupted: TurnCompletedNotification =
        timeout(READ_TIMEOUT, app.read_notification("turn/completed")).await??;
    assert_eq!(interrupted.turn.status, TurnStatus::Interrupted);
    assert_eq!(
        vec![first.clone(), second.clone()],
        list_queue(&mut app, &thread_id).await?.data
    );

    let added_after_interrupt = queue_item(
        &mut app,
        submission(&thread_id, "message added after interruption"),
    )
    .await?;
    assert_eq!(
        vec![first.clone(), second.clone(), added_after_interrupt.clone(),],
        list_queue(&mut app, &thread_id).await?.data
    );

    let metadata_resume_request_id = app
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            exclude_turns: true,
            ..Default::default()
        })
        .await?;
    let metadata_resumed: ThreadResumeResponse =
        timeout(READ_TIMEOUT, app.read_response(metadata_resume_request_id)).await??;
    assert_eq!(metadata_resumed.thread.id, thread_id);
    assert_eq!(
        vec![first.clone(), second.clone(), added_after_interrupt.clone(),],
        list_queue(&mut app, &thread_id).await?.data
    );

    let resume_request_id = app
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let resumed: ThreadResumeResponse =
        timeout(READ_TIMEOUT, app.read_response(resume_request_id)).await??;
    assert_eq!(resumed.thread.id, thread_id);
    assert_eq!(
        vec![first.clone(), second.clone(), added_after_interrupt.clone(),],
        list_queue(&mut app, &thread_id).await?.data
    );

    drop(app);
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    let cold_resume_request_id = app
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: thread_id.clone(),
            ..Default::default()
        })
        .await?;
    let cold_resumed: ThreadResumeResponse =
        timeout(READ_TIMEOUT, app.read_response(cold_resume_request_id)).await??;
    assert_eq!(cold_resumed.thread.id, thread_id);

    let added_after_cold_resume = queue_item(
        &mut app,
        submission(&thread_id, "message added after cold resume"),
    )
    .await?;
    assert_eq!(
        vec![
            first,
            second.clone(),
            added_after_interrupt,
            added_after_cold_resume
        ],
        list_queue(&mut app, &thread_id).await?.data
    );

    let started: ThreadQueueStartResponse = app
        .request(|request_id| ClientRequest::ThreadQueueStart {
            request_id,
            params: ThreadQueueStartParams {
                thread_id: thread_id.clone(),
                queued_submission_id: Some(second.id),
            },
        })
        .await?;
    for index in 0..4 {
        let completed: TurnCompletedNotification =
            timeout(READ_TIMEOUT, app.read_notification("turn/completed")).await??;
        if index == 0 {
            assert_eq!(completed.turn.id, started.turn.id);
        }
        assert_eq!(completed.turn.status, TurnStatus::Completed);
    }
    assert!(list_queue(&mut app, &thread_id).await?.data.is_empty());

    Ok(())
}

#[tokio::test]
async fn queue_start_while_active_returns_busy_and_preserves_the_queue() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "uses a host-local command and cwd fixture unavailable to remote executors"
    );

    let server = create_mock_responses_server_sequence_unchecked(vec![
        blocked_turn_response()?,
        create_final_assistant_message_sse_response("active turn done")?,
        create_final_assistant_message_sse_response("queued message done")?,
    ])
    .await;
    let (mut app, _codex_home, _server) = queue_app_with_server(server).await?;
    let thread_id = app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let (_, approval_id) = start_blocked_turn(&mut app, &thread_id).await?;
    let queued = queue_item(
        &mut app,
        ThreadQueueAddParams {
            client_user_message_id: "active-queued-client-id".to_string(),
            ..submission(&thread_id, "send this queued message now")
        },
    )
    .await?;
    assert_eq!(queued.client_user_message_id, "active-queued-client-id");

    let start_request_id = app
        .send_raw_request("thread/queue/start", Some(json!({ "threadId": thread_id })))
        .await?;
    let error: JSONRPCError = timeout(
        READ_TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32600);
    assert_eq!(
        error.error.message,
        "thread already has an active or pending turn"
    );
    assert_eq!(list_queue(&mut app, &thread_id).await?.data, vec![queued]);
    decline_approval(&mut app, approval_id).await?;

    let started_item = loop {
        let started_item: ItemStartedNotification =
            timeout(READ_TIMEOUT, app.read_notification("item/started")).await??;
        if matches!(
            &started_item.item,
            ThreadItem::UserMessage { client_id, .. }
                if client_id.as_deref() == Some("active-queued-client-id")
        ) {
            break started_item;
        }
    };
    let ThreadItem::UserMessage { client_id, .. } = started_item.item else {
        anyhow::bail!("queued message did not start after the active turn completed");
    };
    assert_eq!(client_id.as_deref(), Some("active-queued-client-id"));

    let completed = loop {
        let completed: TurnCompletedNotification =
            timeout(READ_TIMEOUT, app.read_notification("turn/completed")).await??;
        if completed.turn.id == started_item.turn_id {
            break completed;
        }
    };
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert!(list_queue(&mut app, &thread_id).await?.data.is_empty());
    Ok(())
}

#[tokio::test]
async fn queue_start_without_id_starts_the_head_when_idle() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "uses a host-local command and cwd fixture unavailable to remote executors"
    );

    let responses = vec![
        blocked_turn_response()?,
        create_final_assistant_message_sse_response("first queued message done")?,
        create_final_assistant_message_sse_response("second queued message done")?,
    ];
    let (mut app, _codex_home, _server) = queue_app(responses).await?;
    let thread_id = app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let (active_turn_id, _approval_id) = start_blocked_turn(&mut app, &thread_id).await?;
    let first = queue_item(&mut app, submission(&thread_id, "first queued message")).await?;
    let second = queue_item(&mut app, submission(&thread_id, "second queued message")).await?;

    let _: TurnInterruptResponse = app
        .request(|request_id| ClientRequest::TurnInterrupt {
            request_id,
            params: TurnInterruptParams {
                thread_id: thread_id.clone(),
                turn_id: active_turn_id,
            },
        })
        .await?;
    let interrupted: TurnCompletedNotification =
        timeout(READ_TIMEOUT, app.read_notification("turn/completed")).await??;
    assert_eq!(interrupted.turn.status, TurnStatus::Interrupted);
    assert_eq!(
        list_queue(&mut app, &thread_id).await?.data,
        vec![first.clone(), second]
    );

    let start_request_id = app
        .send_raw_request("thread/queue/start", Some(json!({ "threadId": thread_id })))
        .await?;
    let started: ThreadQueueStartResponse =
        timeout(READ_TIMEOUT, app.read_response(start_request_id)).await??;
    let started_item = loop {
        let started_item: ItemStartedNotification =
            timeout(READ_TIMEOUT, app.read_notification("item/started")).await??;
        if matches!(
            &started_item.item,
            ThreadItem::UserMessage { client_id, .. }
                if client_id.as_deref() == Some(first.client_user_message_id.as_str())
        ) {
            break started_item;
        }
    };
    assert_eq!(started_item.turn_id, started.turn.id);
    for _ in 0..2 {
        let completed: TurnCompletedNotification =
            timeout(READ_TIMEOUT, app.read_notification("turn/completed")).await??;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
    }
    assert!(list_queue(&mut app, &thread_id).await?.data.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_new_turn_preserves_queued_messages_until_it_completes() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "uses a host-local command and cwd fixture unavailable to remote executors"
    );

    let responses = vec![
        blocked_turn_response()?,
        blocked_turn_response()?,
        create_final_assistant_message_sse_response("new turn done")?,
        create_final_assistant_message_sse_response("first queued message done")?,
        create_final_assistant_message_sse_response("second queued message done")?,
    ];
    let (mut app, _codex_home, _server) = queue_app(responses).await?;
    let thread_id = app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let (active_turn_id, _approval_id) = start_blocked_turn(&mut app, &thread_id).await?;

    let first = queue_item(
        &mut app,
        ThreadQueueAddParams {
            client_user_message_id: "first-queued-client-id".to_string(),
            ..submission(&thread_id, "first queued message")
        },
    )
    .await?;
    let second = queue_item(
        &mut app,
        ThreadQueueAddParams {
            client_user_message_id: "second-queued-client-id".to_string(),
            ..submission(&thread_id, "second queued message")
        },
    )
    .await?;

    let _: TurnInterruptResponse = app
        .request(|request_id| ClientRequest::TurnInterrupt {
            request_id,
            params: TurnInterruptParams {
                thread_id: thread_id.clone(),
                turn_id: active_turn_id,
            },
        })
        .await?;
    let interrupted: TurnCompletedNotification =
        timeout(READ_TIMEOUT, app.read_notification("turn/completed")).await??;
    assert_eq!(interrupted.turn.status, TurnStatus::Interrupted);

    let _: TurnStartResponse = app
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread_id.clone(),
                input: first.input.clone(),
                client_user_message_id: Some(first.client_user_message_id.clone()),
                ..Default::default()
            },
        })
        .await?;
    let approval = timeout(READ_TIMEOUT, app.read_stream_until_request_message()).await??;
    let ServerRequest::CommandExecutionRequestApproval {
        request_id: new_approval_id,
        ..
    } = approval
    else {
        anyhow::bail!("matching ordinary turn did not request command approval");
    };
    assert_eq!(
        vec![first, second],
        list_queue(&mut app, &thread_id).await?.data
    );

    decline_approval(&mut app, new_approval_id).await?;
    for _ in 0..3 {
        let completed: TurnCompletedNotification =
            timeout(READ_TIMEOUT, app.read_notification("turn/completed")).await??;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
    }
    assert!(list_queue(&mut app, &thread_id).await?.data.is_empty());

    Ok(())
}

async fn queue_app(responses: Vec<String>) -> Result<(TestAppServer, TempDir, MockServer)> {
    let server = create_mock_responses_server_sequence(responses).await;
    queue_app_with_server(server).await
}

async fn queue_app_with_server(server: MockServer) -> Result<(TestAppServer, TempDir, MockServer)> {
    let codex_home = TempDir::new()?;
    let config = MockResponsesConfig::new(&server.uri())
        .with_approval_policy("on-request")
        .with_root_config(r#"approvals_reviewer = "user""#);
    config.write(codex_home.path())?;
    let app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .build_initialized()
        .await?;
    Ok((app, codex_home, server))
}

fn blocked_turn_response() -> Result<String> {
    #[cfg(target_os = "windows")]
    let shell_command = vec![
        "powershell".to_string(),
        "-Command".to_string(),
        "Start-Sleep -Seconds 10".to_string(),
    ];
    #[cfg(not(target_os = "windows"))]
    let shell_command = vec![
        "python3".to_string(),
        "-c".to_string(),
        "import time; time.sleep(10)".to_string(),
    ];

    create_escalated_command_execution_sse_response(
        shell_command,
        /*workdir*/ None,
        /*timeout_ms*/ Some(10_000),
        "queue-blocked-command",
    )
}

async fn start_blocked_turn(
    app: &mut TestAppServer,
    thread_id: &str,
) -> Result<(String, RequestId)> {
    let started: TurnStartResponse = app
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread_id.to_string(),
                input: vec![text("start an approval-blocked turn")],
                ..Default::default()
            },
        })
        .await?;
    let approval = timeout(READ_TIMEOUT, app.read_stream_until_request_message()).await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, .. } = approval else {
        anyhow::bail!("active turn did not request command approval");
    };
    Ok((started.turn.id, request_id))
}

async fn queue_item(
    app: &mut TestAppServer,
    params: ThreadQueueAddParams,
) -> Result<QueuedSubmission> {
    let response: ThreadQueueAddResponse = app
        .request(|request_id| ClientRequest::ThreadQueueAdd { request_id, params })
        .await?;
    Ok(response.queued_submission)
}

async fn list_queue(app: &mut TestAppServer, thread_id: &str) -> Result<ThreadQueueListResponse> {
    let mut data = Vec::new();
    let mut cursor = None;
    loop {
        let page: ThreadQueueListResponse = app
            .request(|request_id| ClientRequest::ThreadQueueList {
                request_id,
                params: ThreadQueueListParams {
                    thread_id: thread_id.to_string(),
                    cursor,
                    limit: None,
                },
            })
            .await?;
        data.extend(page.data);
        cursor = page.next_cursor;
        if cursor.is_none() {
            return Ok(ThreadQueueListResponse {
                data,
                next_cursor: None,
            });
        }
    }
}

async fn decline_approval(app: &mut TestAppServer, request_id: RequestId) -> Result<()> {
    app.send_response(
        request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Decline,
        })?,
    )
    .await
}

fn submission(thread_id: &str, value: &str) -> ThreadQueueAddParams {
    ThreadQueueAddParams {
        thread_id: thread_id.to_string(),
        input: vec![text(value)],
        client_user_message_id: format!("queued-{value}"),
    }
}

fn text(value: &str) -> UserInput {
    UserInput::Text {
        text: value.to_string(),
        text_elements: Vec::new(),
    }
}
