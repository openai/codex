use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::create_shell_command_sse_response;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::QueuedTurnStatus;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadQueueAddParams;
use codex_app_server_protocol::ThreadQueueAddResponse;
use codex_app_server_protocol::ThreadQueueChangedNotification;
use codex_app_server_protocol::ThreadQueueDeleteParams;
use codex_app_server_protocol::ThreadQueueDeleteResponse;
use codex_app_server_protocol::ThreadQueueListParams;
use codex_app_server_protocol::ThreadQueueListResponse;
use codex_app_server_protocol::ThreadQueueReorderParams;
use codex_app_server_protocol::ThreadQueueReorderResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test]
async fn idle_queue_add_dispatches_serialized_turn_and_drains_visible_queue() -> Result<()> {
    let responses = vec![
        create_final_assistant_message_sse_response("queued done")?,
        create_final_assistant_message_sse_response("second queued done")?,
    ];
    let server = create_mock_responses_server_sequence(responses).await;
    let codex_home = TempDir::new()?;
    write_queue_test_config(codex_home.path(), &server.uri(), "never")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_experimental(&mut mcp).await?;
    let thread = start_thread(&mut mcp).await?;

    let add_request_id = mcp
        .send_raw_request(
            "thread/queue/add",
            Some(serde_json::to_value(ThreadQueueAddParams {
                thread_id: thread.id.clone(),
                turn_start_params: text_turn(&thread.id, "queued serialized input"),
            })?),
        )
        .await?;
    let add_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(add_request_id)),
    )
    .await??;
    let ThreadQueueAddResponse { queued_turn } = to_response(add_response)?;
    assert!(matches!(queued_turn.status, QueuedTurnStatus::Pending));
    let add_notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/queue/changed"),
    )
    .await??;
    let add_notification: ThreadQueueChangedNotification = serde_json::from_value(
        add_notification
            .params
            .expect("thread/queue/changed params"),
    )?;
    assert_eq!(add_notification.thread_id, thread.id);
    assert_eq!(add_notification.queued_turns, vec![queued_turn]);

    let drain_notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/queue/changed"),
    )
    .await??;
    let drain_notification: ThreadQueueChangedNotification = serde_json::from_value(
        drain_notification
            .params
            .expect("thread/queue/changed params"),
    )?;
    assert_eq!(drain_notification.thread_id, thread.id);
    assert!(drain_notification.queued_turns.is_empty());

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let list_request_id = mcp
        .send_raw_request(
            "thread/queue/list",
            Some(serde_json::to_value(ThreadQueueListParams {
                thread_id: thread.id.clone(),
                cursor: None,
                limit: None,
            })?),
        )
        .await?;
    let list_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(list_request_id)),
    )
    .await??;
    let ThreadQueueListResponse { data, next_cursor } = to_response(list_response)?;
    assert!(data.is_empty());
    assert_eq!(next_cursor, None);

    queue_turn(&mut mcp, &thread.id, "second queued serialized input").await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert!(list_queue_ids(&mut mcp, &thread.id).await?.is_empty());

    let requests = server
        .received_requests()
        .await
        .expect("failed to fetch received requests");
    assert_eq!(requests.len(), 2);
    assert!(
        String::from_utf8_lossy(&requests[0].body).contains("queued serialized input"),
        "queued turn payload should reach the model request after state round-trip"
    );
    assert!(
        String::from_utf8_lossy(&requests[1].body).contains("second queued serialized input"),
        "a later queued turn should still drain after a fast terminal dispatch"
    );

    Ok(())
}

#[tokio::test]
async fn queue_add_rejects_ephemeral_threads() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("unused")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_queue_test_config(codex_home.path(), &server.uri(), "never")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_experimental(&mut mcp).await?;
    let start_request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(start_response)?;

    let add_request_id = mcp
        .send_raw_request(
            "thread/queue/add",
            Some(serde_json::to_value(ThreadQueueAddParams {
                thread_id: thread.id.clone(),
                turn_start_params: text_turn(&thread.id, "ephemeral queued turn"),
            })?),
        )
        .await?;
    let add_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(add_request_id)),
    )
    .await??;

    assert_eq!(add_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        add_error.error.message,
        format!(
            "ephemeral thread does not support queued turns: {}",
            thread.id
        )
    );

    Ok(())
}

#[tokio::test]
async fn busy_thread_queue_rows_support_list_reorder_and_delete_before_drain() -> Result<()> {
    let responses = vec![
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            /*workdir*/ None,
            Some(5000),
            "queue-blocker",
        )?,
        create_final_assistant_message_sse_response("active turn done")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    write_queue_test_config(codex_home.path(), &server.uri(), "untrusted")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_experimental(&mut mcp).await?;
    let thread = start_thread(&mut mcp).await?;

    let active_turn_request_id = mcp
        .send_turn_start_request(text_turn(&thread.id, "keep the thread running"))
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(active_turn_request_id)),
    )
    .await??;

    let approval_request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, .. } = approval_request else {
        panic!("expected command approval to keep the active turn open");
    };

    let first = queue_turn(&mut mcp, &thread.id, "first queued").await?;
    let second = queue_turn(&mut mcp, &thread.id, "second queued").await?;
    assert_eq!(
        list_queue_ids(&mut mcp, &thread.id).await?,
        vec![first.clone(), second.clone()]
    );
    let first_page = list_queue_page(&mut mcp, &thread.id, /*cursor*/ None, Some(1)).await?;
    assert_eq!(
        first_page
            .data
            .into_iter()
            .map(|queued_turn| queued_turn.id)
            .collect::<Vec<_>>(),
        vec![first.clone()]
    );
    let second_page =
        list_queue_page(&mut mcp, &thread.id, first_page.next_cursor, Some(1)).await?;
    assert_eq!(
        second_page
            .data
            .into_iter()
            .map(|queued_turn| queued_turn.id)
            .collect::<Vec<_>>(),
        vec![second.clone()]
    );
    assert_eq!(second_page.next_cursor, None);

    let reorder_request_id = mcp
        .send_raw_request(
            "thread/queue/reorder",
            Some(serde_json::to_value(ThreadQueueReorderParams {
                thread_id: thread.id.clone(),
                queued_turn_ids: vec![second.clone(), first.clone()],
            })?),
        )
        .await?;
    let reorder_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(reorder_request_id)),
    )
    .await??;
    let ThreadQueueReorderResponse { queued_turns } = to_response(reorder_response)?;
    assert_eq!(
        queued_turns
            .into_iter()
            .map(|queued_turn| queued_turn.id)
            .collect::<Vec<_>>(),
        vec![second.clone(), first.clone()]
    );

    delete_queue_turn(&mut mcp, &thread.id, &second).await?;
    delete_queue_turn(&mut mcp, &thread.id, &first).await?;
    assert!(list_queue_ids(&mut mcp, &thread.id).await?.is_empty());

    mcp.send_response(
        request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Accept,
        })?,
    )
    .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn queued_turns_stay_serial_after_the_first_dispatch_starts() -> Result<()> {
    let responses = vec![
        create_shell_command_sse_response(
            vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(42)".to_string(),
            ],
            /*workdir*/ None,
            Some(5000),
            "queued-serial-blocker",
        )?,
        create_final_assistant_message_sse_response("first queued turn done")?,
        create_final_assistant_message_sse_response("second queued turn done")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    write_queue_test_config(codex_home.path(), &server.uri(), "untrusted")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_experimental(&mut mcp).await?;
    let thread = start_thread(&mut mcp).await?;

    queue_turn(&mut mcp, &thread.id, "first queued").await?;
    let approval_request = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::CommandExecutionRequestApproval { request_id, .. } = approval_request else {
        panic!("expected queued turn approval request to keep the first dispatch active");
    };

    let second = queue_turn(&mut mcp, &thread.id, "second queued").await?;
    assert_eq!(list_queue_ids(&mut mcp, &thread.id).await?, vec![second]);

    mcp.send_response(
        request_id,
        serde_json::to_value(CommandExecutionRequestApprovalResponse {
            decision: CommandExecutionApprovalDecision::Accept,
        })?,
    )
    .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert!(list_queue_ids(&mut mcp, &thread.id).await?.is_empty());

    let requests = server
        .received_requests()
        .await
        .expect("failed to fetch received requests");
    assert_eq!(requests.len(), 3);
    assert!(
        String::from_utf8_lossy(&requests[2].body).contains("second queued"),
        "second queued follow-up should become its own later model request"
    );

    Ok(())
}

async fn initialize_experimental(mcp: &mut McpProcess) -> Result<()> {
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.initialize_with_capabilities(
            ClientInfo {
                name: "thread-queue-tests".to_string(),
                title: None,
                version: "0.0.0".to_string(),
            },
            Some(InitializeCapabilities {
                experimental_api: true,
                opt_out_notification_methods: None,
                request_attestation: false,
            }),
        ),
    )
    .await??;
    Ok(())
}

async fn start_thread(mcp: &mut McpProcess) -> Result<codex_app_server_protocol::Thread> {
    let request_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(response)?;
    Ok(thread)
}

async fn queue_turn(mcp: &mut McpProcess, thread_id: &str, text: &str) -> Result<String> {
    let request_id = mcp
        .send_raw_request(
            "thread/queue/add",
            Some(serde_json::to_value(ThreadQueueAddParams {
                thread_id: thread_id.to_string(),
                turn_start_params: text_turn(thread_id, text),
            })?),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadQueueAddResponse { queued_turn } = to_response(response)?;
    Ok(queued_turn.id)
}

async fn list_queue_ids(mcp: &mut McpProcess, thread_id: &str) -> Result<Vec<String>> {
    let ThreadQueueListResponse { data, .. } =
        list_queue_page(mcp, thread_id, /*cursor*/ None, /*limit*/ None).await?;
    Ok(data.into_iter().map(|queued_turn| queued_turn.id).collect())
}

async fn list_queue_page(
    mcp: &mut McpProcess,
    thread_id: &str,
    cursor: Option<String>,
    limit: Option<u32>,
) -> Result<ThreadQueueListResponse> {
    let request_id = mcp
        .send_raw_request(
            "thread/queue/list",
            Some(serde_json::to_value(ThreadQueueListParams {
                thread_id: thread_id.to_string(),
                cursor,
                limit,
            })?),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

async fn delete_queue_turn(
    mcp: &mut McpProcess,
    thread_id: &str,
    queued_turn_id: &str,
) -> Result<()> {
    let request_id = mcp
        .send_raw_request(
            "thread/queue/delete",
            Some(serde_json::to_value(ThreadQueueDeleteParams {
                thread_id: thread_id.to_string(),
                queued_turn_id: queued_turn_id.to_string(),
            })?),
        )
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadQueueDeleteResponse { deleted } = to_response(response)?;
    assert!(deleted);
    Ok(())
}

fn text_turn(thread_id: &str, text: &str) -> TurnStartParams {
    TurnStartParams {
        thread_id: thread_id.to_string(),
        input: vec![V2UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    }
}

fn write_queue_test_config(
    codex_home: &std::path::Path,
    server_uri: &str,
    approval_policy: &str,
) -> std::io::Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "{approval_policy}"
sandbox_mode = "read-only"
model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
"#
        ),
    )
}
