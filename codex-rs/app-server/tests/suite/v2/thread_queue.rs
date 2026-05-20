use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::QueuedTurnStatus;
use codex_app_server_protocol::RequestId;
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
use codex_app_server_protocol::TurnSubmission;
use codex_app_server_protocol::UserInput as V2UserInput;
use std::collections::BTreeMap;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[tokio::test]
async fn queue_add_persists_turn_params_and_emits_snapshot() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("unused")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_queue_test_config(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_experimental(&mut mcp).await?;
    let thread = start_thread(&mut mcp).await?;

    let add_request_id = mcp
        .send_raw_request(
            "thread/queue/add",
            Some(serde_json::to_value(ThreadQueueAddParams {
                thread_id: thread.id.clone(),
                submission: text_submission("queued serialized input").into(),
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

    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("thread/queue/changed"),
    )
    .await??;
    let notification: ThreadQueueChangedNotification =
        serde_json::from_value(notification.params.expect("thread/queue/changed params"))?;
    assert_eq!(notification.thread_id, thread.id);
    assert_eq!(notification.queued_turns, vec![queued_turn.clone()]);

    let ThreadQueueListResponse { data, next_cursor } = list_queue_page(
        &mut mcp, &thread.id, /*cursor*/ None, /*limit*/ None,
    )
    .await?;
    assert_eq!(data, vec![queued_turn]);
    assert_eq!(next_cursor, None);

    Ok(())
}

#[tokio::test]
async fn queue_add_rejects_ephemeral_threads() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("unused")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_queue_test_config(codex_home.path(), &server.uri())?;

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
                submission: text_submission("ephemeral queued turn").into(),
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
async fn queue_add_rejects_requests_when_feature_is_disabled() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("unused")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_queue_test_config_without_feature(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_experimental(&mut mcp).await?;
    let thread = start_thread(&mut mcp).await?;

    let add_request_id = mcp
        .send_raw_request(
            "thread/queue/add",
            Some(serde_json::to_value(ThreadQueueAddParams {
                thread_id: thread.id,
                submission: text_submission("disabled queued turn").into(),
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
        "app-server queue feature is disabled"
    );

    Ok(())
}

#[tokio::test]
async fn runtime_feature_enablement_controls_queue_access_without_deleting_rows() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("unused")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_queue_test_config_without_feature(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_experimental(&mut mcp).await?;
    let thread = start_thread(&mut mcp).await?;

    set_queue_feature(&mut mcp, /*enabled*/ true).await?;
    let queued_turn_id = queue_turn(&mut mcp, &thread.id, "durable queued turn").await?;

    set_queue_feature(&mut mcp, /*enabled*/ false).await?;
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
    let list_error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(list_request_id)),
    )
    .await??;
    assert_eq!(list_error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        list_error.error.message,
        "app-server queue feature is disabled"
    );

    set_queue_feature(&mut mcp, /*enabled*/ true).await?;
    assert_eq!(
        list_queue_ids(&mut mcp, &thread.id).await?,
        vec![queued_turn_id]
    );

    Ok(())
}

#[tokio::test]
async fn visible_queue_rows_support_pagination_reorder_and_delete() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("unused")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    write_queue_test_config(codex_home.path(), &server.uri())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    initialize_experimental(&mut mcp).await?;
    let thread = start_thread(&mut mcp).await?;

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
                submission: text_submission(text).into(),
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

async fn set_queue_feature(mcp: &mut McpProcess, enabled: bool) -> Result<()> {
    let request_id = mcp
        .send_experimental_feature_enablement_set_request(ExperimentalFeatureEnablementSetParams {
            enablement: BTreeMap::from([("app_server_queue".to_string(), enabled)]),
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ExperimentalFeatureEnablementSetResponse { enablement } = to_response(response)?;
    assert_eq!(
        enablement,
        BTreeMap::from([("app_server_queue".to_string(), enabled)])
    );
    Ok(())
}

fn text_submission(text: &str) -> TurnSubmission {
    TurnSubmission {
        input: vec![V2UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    }
}

fn write_queue_test_config(codex_home: &std::path::Path, server_uri: &str) -> std::io::Result<()> {
    write_queue_test_config_with_feature(codex_home, server_uri, /*app_server_queue*/ true)
}

fn write_queue_test_config_with_feature(
    codex_home: &std::path::Path,
    server_uri: &str,
    app_server_queue: bool,
) -> std::io::Result<()> {
    write_queue_test_config_with_optional_feature(codex_home, server_uri, Some(app_server_queue))
}

fn write_queue_test_config_without_feature(
    codex_home: &std::path::Path,
    server_uri: &str,
) -> std::io::Result<()> {
    write_queue_test_config_with_optional_feature(
        codex_home, server_uri, /*app_server_queue*/ None,
    )
}

fn write_queue_test_config_with_optional_feature(
    codex_home: &std::path::Path,
    server_uri: &str,
    app_server_queue: Option<bool>,
) -> std::io::Result<()> {
    let feature_config = app_server_queue
        .map(|enabled| format!("\n[features]\napp_server_queue = {enabled}\n"))
        .unwrap_or_default();
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
model_provider = "mock_provider"
{feature_config}

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
