use super::connection_handling_websocket::DEFAULT_READ_TIMEOUT;
use super::connection_handling_websocket::WsClient;
use super::connection_handling_websocket::assert_no_message;
use super::connection_handling_websocket::connect_websocket;
use super::connection_handling_websocket::create_config_toml;
use super::connection_handling_websocket::read_notification_for_method;
use super::connection_handling_websocket::read_response_and_notification_for_method;
use super::connection_handling_websocket::read_response_for_id;
use super::connection_handling_websocket::send_initialize_request;
use super::connection_handling_websocket::send_request;
use super::connection_handling_websocket::spawn_websocket_server;
use anyhow::Context;
use anyhow::Result;
use app_test_support::create_fake_rollout_with_text_elements;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ThreadNameUpdatedNotification;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSetNameResponse;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn thread_name_updated_broadcasts_for_loaded_threads() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let conversation_id = create_rollout(codex_home.path(), "2025-01-05T12-00-00")?;

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;

    let result = async {
        let mut ws1 = connect_websocket(bind_addr).await?;
        let mut ws2 = connect_websocket(bind_addr).await?;
        initialize_both_clients(&mut ws1, &mut ws2).await?;

        send_request(
            &mut ws1,
            "thread/resume",
            10,
            Some(serde_json::to_value(ThreadResumeParams {
                thread_id: conversation_id.clone(),
                ..Default::default()
            })?),
        )
        .await?;
        let resume_resp: JSONRPCResponse = read_response_for_id(&mut ws1, 10).await?;
        let resume: ThreadResumeResponse = to_response::<ThreadResumeResponse>(resume_resp)?;
        assert_eq!(resume.thread.id, conversation_id);

        let renamed = "Loaded rename";
        send_request(
            &mut ws1,
            "thread/name/set",
            11,
            Some(serde_json::to_value(ThreadSetNameParams {
                thread_id: conversation_id.clone(),
                name: renamed.to_string(),
            })?),
        )
        .await?;
        let (rename_resp, ws1_notification) =
            read_response_and_notification_for_method(&mut ws1, 11, "thread/name/updated").await?;
        let _: ThreadSetNameResponse = to_response::<ThreadSetNameResponse>(rename_resp)?;
        assert_thread_name_updated(ws1_notification, &conversation_id, renamed)?;

        let ws2_notification =
            read_notification_for_method(&mut ws2, "thread/name/updated").await?;
        assert_thread_name_updated(ws2_notification, &conversation_id, renamed)?;

        assert_no_message(&mut ws1, Duration::from_millis(250)).await?;
        assert_no_message(&mut ws2, Duration::from_millis(250)).await?;
        Ok(())
    }
    .await;

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    result
}

#[tokio::test]
async fn thread_name_updated_broadcasts_for_not_loaded_threads() -> Result<()> {
    let server = create_mock_responses_server_repeating_assistant("Done").await;
    let codex_home = TempDir::new()?;
    create_config_toml(codex_home.path(), &server.uri(), "never")?;
    let conversation_id = create_rollout(codex_home.path(), "2025-01-05T12-05-00")?;

    let (mut process, bind_addr) = spawn_websocket_server(codex_home.path()).await?;

    let result = async {
        let mut ws1 = connect_websocket(bind_addr).await?;
        let mut ws2 = connect_websocket(bind_addr).await?;
        initialize_both_clients(&mut ws1, &mut ws2).await?;

        let renamed = "Stored rename";
        send_request(
            &mut ws1,
            "thread/name/set",
            20,
            Some(serde_json::to_value(ThreadSetNameParams {
                thread_id: conversation_id.clone(),
                name: renamed.to_string(),
            })?),
        )
        .await?;
        let (rename_resp, ws1_notification) =
            read_response_and_notification_for_method(&mut ws1, 20, "thread/name/updated").await?;
        let _: ThreadSetNameResponse = to_response::<ThreadSetNameResponse>(rename_resp)?;
        assert_thread_name_updated(ws1_notification, &conversation_id, renamed)?;

        let ws2_notification =
            read_notification_for_method(&mut ws2, "thread/name/updated").await?;
        assert_thread_name_updated(ws2_notification, &conversation_id, renamed)?;

        assert_no_message(&mut ws1, Duration::from_millis(250)).await?;
        assert_no_message(&mut ws2, Duration::from_millis(250)).await?;
        Ok(())
    }
    .await;

    process
        .kill()
        .await
        .context("failed to stop websocket app-server process")?;
    result
}

async fn initialize_both_clients(ws1: &mut WsClient, ws2: &mut WsClient) -> Result<()> {
    send_initialize_request(ws1, 1, "ws_client_one").await?;
    timeout(DEFAULT_READ_TIMEOUT, read_response_for_id(ws1, 1)).await??;

    send_initialize_request(ws2, 2, "ws_client_two").await?;
    timeout(DEFAULT_READ_TIMEOUT, read_response_for_id(ws2, 2)).await??;
    Ok(())
}

fn create_rollout(codex_home: &std::path::Path, filename_ts: &str) -> Result<String> {
    create_fake_rollout_with_text_elements(
        codex_home,
        filename_ts,
        "2025-01-05T12:00:00Z",
        "Saved user message",
        Vec::new(),
        Some("mock_provider"),
        None,
    )
}

fn assert_thread_name_updated(
    notification: JSONRPCNotification,
    thread_id: &str,
    thread_name: &str,
) -> Result<()> {
    let notification: ThreadNameUpdatedNotification =
        serde_json::from_value(notification.params.context("thread/name/updated params")?)?;
    assert_eq!(notification.thread_id, thread_id);
    assert_eq!(notification.thread_name.as_deref(), Some(thread_name));
    Ok(())
}
