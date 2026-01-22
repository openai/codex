use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ListConversationsParams;
use codex_app_server_protocol::ListConversationsResponse;
use codex_app_server_protocol::NewConversationParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ResumeConversationParams;
use codex_app_server_protocol::ResumeConversationResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionConfiguredNotification;
use codex_core::protocol::EventMsg;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_and_resume_conversations() -> Result<()> {
    // Prepare a temporary CODEX_HOME with a few fake rollout files.
    let codex_home = TempDir::new()?;
    create_fake_rollout(
        codex_home.path(),
        "2025-01-02T12-00-00",
        "2025-01-02T12:00:00Z",
        "Hello A",
        Some("openai"),
        None,
    )?;
    create_fake_rollout(
        codex_home.path(),
        "2025-01-01T13-00-00",
        "2025-01-01T13:00:00Z",
        "Hello B",
        Some("openai"),
        None,
    )?;
    create_fake_rollout(
        codex_home.path(),
        "2025-01-01T12-00-00",
        "2025-01-01T12:00:00Z",
        "Hello C",
        None,
        None,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    // Request first page with size 2
    let req_id = mcp
        .send_list_conversations_request(ListConversationsParams {
            page_size: Some(2),
            cursor: None,
            model_providers: None,
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ListConversationsResponse { items, next_cursor } =
        to_response::<ListConversationsResponse>(resp)?;

    assert_eq!(items.len(), 2);
    // Newest first; preview text should match
    assert_eq!(items[0].preview, "Hello A");
    assert_eq!(items[1].preview, "Hello B");
    assert_eq!(items[0].model_provider, "openai");
    assert_eq!(items[1].model_provider, "openai");
    assert!(items[0].path.is_absolute());
    assert!(next_cursor.is_some());

    // Request the next page using the cursor
    let req_id2 = mcp
        .send_list_conversations_request(ListConversationsParams {
            page_size: Some(2),
            cursor: next_cursor,
            model_providers: None,
        })
        .await?;
    let resp2: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id2)),
    )
    .await??;
    let ListConversationsResponse {
        items: items2,
        next_cursor: next2,
        ..
    } = to_response::<ListConversationsResponse>(resp2)?;
    assert_eq!(items2.len(), 1);
    assert_eq!(items2[0].preview, "Hello C");
    assert_eq!(items2[0].model_provider, "openai");
    assert_eq!(next2, None);

    // Add a conversation with an explicit non-OpenAI provider for filter tests.
    create_fake_rollout(
        codex_home.path(),
        "2025-01-01T11-30-00",
        "2025-01-01T11:30:00Z",
        "Hello TP",
        Some("test-provider"),
        None,
    )?;

    // Filtering by model provider should return only matching sessions.
    let filter_req_id = mcp
        .send_list_conversations_request(ListConversationsParams {
            page_size: Some(10),
            cursor: None,
            model_providers: Some(vec!["test-provider".to_string()]),
        })
        .await?;
    let filter_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(filter_req_id)),
    )
    .await??;
    let ListConversationsResponse {
        items: filtered_items,
        next_cursor: filtered_next,
    } = to_response::<ListConversationsResponse>(filter_resp)?;
    assert_eq!(filtered_items.len(), 1);
    assert_eq!(filtered_next, None);
    assert_eq!(filtered_items[0].preview, "Hello TP");
    assert_eq!(filtered_items[0].model_provider, "test-provider");

    // Empty filter should include every session regardless of provider metadata.
    let unfiltered_req_id = mcp
        .send_list_conversations_request(ListConversationsParams {
            page_size: Some(10),
            cursor: None,
            model_providers: Some(Vec::new()),
        })
        .await?;
    let unfiltered_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(unfiltered_req_id)),
    )
    .await??;
    let ListConversationsResponse {
        items: unfiltered_items,
        next_cursor: unfiltered_next,
    } = to_response::<ListConversationsResponse>(unfiltered_resp)?;
    assert_eq!(unfiltered_items.len(), 4);
    assert!(unfiltered_next.is_none());

    let empty_req_id = mcp
        .send_list_conversations_request(ListConversationsParams {
            page_size: Some(10),
            cursor: None,
            model_providers: Some(vec!["other".to_string()]),
        })
        .await?;
    let empty_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(empty_req_id)),
    )
    .await??;
    let ListConversationsResponse {
        items: empty_items,
        next_cursor: empty_next,
    } = to_response::<ListConversationsResponse>(empty_resp)?;
    assert!(empty_items.is_empty());
    assert!(empty_next.is_none());

    let first_item = &items[0];

    // Now resume one of the sessions from an explicit rollout path.
    let resume_req_id = mcp
        .send_resume_conversation_request(ResumeConversationParams {
            path: Some(first_item.path.clone()),
            conversation_id: None,
            history: None,
            overrides: Some(NewConversationParams {
                model: Some("o3".to_string()),
                ..Default::default()
            }),
        })
        .await?;

    // Expect a codex/event notification with msg.type == sessionConfigured
    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("sessionConfigured"),
    )
    .await??;
    let session_configured: ServerNotification = notification.try_into()?;
    let ServerNotification::SessionConfigured(SessionConfiguredNotification {
        model,
        rollout_path,
        initial_messages: session_initial_messages,
        ..
    }) = session_configured
    else {
        unreachable!("expected sessionConfigured notification");
    };
    assert_eq!(model, "o3");
    assert_eq!(rollout_path, first_item.path.clone());
    let session_initial_messages = session_initial_messages
        .expect("expected initial messages when resuming from rollout path");
    match session_initial_messages.as_slice() {
        [EventMsg::UserMessage(message)] => {
            assert_eq!(message.message, first_item.preview.clone());
        }
        other => panic!("unexpected initial messages from rollout resume: {other:#?}"),
    }

    // Then the response for resumeConversation
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_req_id)),
    )
    .await??;
    let ResumeConversationResponse {
        conversation_id,
        model: resume_model,
        initial_messages: response_initial_messages,
        ..
    } = to_response::<ResumeConversationResponse>(resume_resp)?;
    // conversation id should be a valid UUID
    assert!(!conversation_id.to_string().is_empty());
    assert_eq!(resume_model, "o3");
    let response_initial_messages =
        response_initial_messages.expect("expected initial messages in resume response");
    match response_initial_messages.as_slice() {
        [EventMsg::UserMessage(message)] => {
            assert_eq!(message.message, first_item.preview.clone());
        }
        other => panic!("unexpected initial messages in resume response: {other:#?}"),
    }

    // Resuming with only a conversation id should locate the rollout automatically.
    let resume_by_id_req_id = mcp
        .send_resume_conversation_request(ResumeConversationParams {
            path: None,
            conversation_id: Some(first_item.conversation_id),
            history: None,
            overrides: Some(NewConversationParams {
                model: Some("o3".to_string()),
                ..Default::default()
            }),
        })
        .await?;
    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("sessionConfigured"),
    )
    .await??;
    let session_configured: ServerNotification = notification.try_into()?;
    let ServerNotification::SessionConfigured(SessionConfiguredNotification {
        model,
        rollout_path,
        initial_messages: session_initial_messages,
        ..
    }) = session_configured
    else {
        unreachable!("expected sessionConfigured notification");
    };
    assert_eq!(model, "o3");
    assert_eq!(rollout_path, first_item.path.clone());
    let session_initial_messages = session_initial_messages
        .expect("expected initial messages when resuming from conversation id");
    match session_initial_messages.as_slice() {
        [EventMsg::UserMessage(message)] => {
            assert_eq!(message.message, first_item.preview.clone());
        }
        other => panic!("unexpected initial messages from conversation id resume: {other:#?}"),
    }
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_by_id_req_id)),
    )
    .await??;
    let ResumeConversationResponse {
        conversation_id: by_id_conversation_id,
        model: by_id_model,
        initial_messages: by_id_initial_messages,
        ..
    } = to_response::<ResumeConversationResponse>(resume_resp)?;
    assert!(!by_id_conversation_id.to_string().is_empty());
    assert_eq!(by_id_model, "o3");
    let by_id_initial_messages = by_id_initial_messages
        .expect("expected initial messages when resuming from conversation id response");
    match by_id_initial_messages.as_slice() {
        [EventMsg::UserMessage(message)] => {
            assert_eq!(message.message, first_item.preview.clone());
        }
        other => {
            panic!("unexpected initial messages in conversation id resume response: {other:#?}")
        }
    }

    // Resuming with explicit history should succeed even without a stored rollout.
    let fork_history_text = "Hello from history";
    let history = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: fork_history_text.to_string(),
        }],
        end_turn: None,
    }];
    let resume_with_history_req_id = mcp
        .send_resume_conversation_request(ResumeConversationParams {
            path: None,
            conversation_id: None,
            history: Some(history),
            overrides: Some(NewConversationParams {
                model: Some("o3".to_string()),
                ..Default::default()
            }),
        })
        .await?;
    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("sessionConfigured"),
    )
    .await??;
    let session_configured: ServerNotification = notification.try_into()?;
    let ServerNotification::SessionConfigured(SessionConfiguredNotification {
        model,
        initial_messages: session_initial_messages,
        ..
    }) = session_configured
    else {
        unreachable!("expected sessionConfigured notification");
    };
    assert_eq!(model, "o3");
    assert!(
        session_initial_messages.as_ref().is_none_or(Vec::is_empty),
        "expected no initial messages when resuming from explicit history but got {session_initial_messages:#?}"
    );
    let resume_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(resume_with_history_req_id)),
    )
    .await??;
    let ResumeConversationResponse {
        conversation_id: history_conversation_id,
        model: history_model,
        initial_messages: history_initial_messages,
        ..
    } = to_response::<ResumeConversationResponse>(resume_resp)?;
    assert!(!history_conversation_id.to_string().is_empty());
    assert_eq!(history_model, "o3");
    assert!(
        history_initial_messages.as_ref().is_none_or(Vec::is_empty),
        "expected no initial messages in resume response when history is provided but got {history_initial_messages:#?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_conversations_fetches_through_filtered_pages() -> Result<()> {
    let codex_home = TempDir::new()?;

    // Only the last 3 conversations match the provider filter; request 3 and
    // ensure pagination keeps fetching past non-matching pages.
    let cases = [
        (
            "2025-03-04T12-00-00",
            "2025-03-04T12:00:00Z",
            "skip_provider",
        ),
        (
            "2025-03-03T12-00-00",
            "2025-03-03T12:00:00Z",
            "skip_provider",
        ),
        (
            "2025-03-02T12-00-00",
            "2025-03-02T12:00:00Z",
            "target_provider",
        ),
        (
            "2025-03-01T12-00-00",
            "2025-03-01T12:00:00Z",
            "target_provider",
        ),
        (
            "2025-02-28T12-00-00",
            "2025-02-28T12:00:00Z",
            "target_provider",
        ),
    ];

    for (ts_file, ts_rfc, provider) in cases {
        create_fake_rollout(
            codex_home.path(),
            ts_file,
            ts_rfc,
            "Hello",
            Some(provider),
            None,
        )?;
    }

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let req_id = mcp
        .send_list_conversations_request(ListConversationsParams {
            page_size: Some(3),
            cursor: None,
            model_providers: Some(vec!["target_provider".to_string()]),
        })
        .await?;
    let resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(req_id)),
    )
    .await??;
    let ListConversationsResponse { items, next_cursor } =
        to_response::<ListConversationsResponse>(resp)?;

    assert_eq!(
        items.len(),
        3,
        "should fetch across pages to satisfy the limit"
    );
    assert!(
        items
            .iter()
            .all(|item| item.model_provider == "target_provider")
    );
    assert_eq!(next_cursor, None);

    Ok(())
}
