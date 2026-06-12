use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadCatalogChangedNotification;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSetNameResponse;
use codex_app_server_protocol::ThreadSortKey;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn catalog_subscription_reports_thread_outside_loaded_page() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        "http://localhost:1",
        &Default::default(),
        i64::MAX,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "",
    )?;
    let older_thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Older thread",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let newer_thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-05-00",
        "2025-01-05T12:05:00Z",
        "Newer thread",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;

    let mut app = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, app.initialize()).await??;

    let subscribe_id = app
        .send_raw_request("thread/catalog/subscribe", /*params*/ None)
        .await?;
    let _: codex_app_server_protocol::ThreadCatalogSubscribeResponse =
        read_response(&mut app, subscribe_id).await?;

    let list_id = app
        .send_thread_list_request(ThreadListParams {
            cursor: None,
            limit: Some(1),
            sort_key: Some(ThreadSortKey::CreatedAt),
            sort_direction: None,
            model_providers: Some(vec!["mock_provider".to_string()]),
            source_kinds: None,
            archived: None,
            cwd: None,
            use_state_db_only: false,
            search_term: None,
        })
        .await?;
    let page = read_response::<ThreadListResponse>(&mut app, list_id).await?;
    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].id, newer_thread_id);

    rename_thread(
        &mut app,
        older_thread_id.clone(),
        "Renamed outside first page",
    )
    .await?;

    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_stream_until_notification_message("thread/catalog/changed"),
    )
    .await??;
    let raw_params = notification
        .params
        .expect("thread/catalog/changed should have params");
    assert_eq!(raw_params["thread"].get("turns"), None);

    let changed: ThreadCatalogChangedNotification = serde_json::from_value(raw_params)?;
    assert_eq!(changed.thread.id, older_thread_id);
    assert_eq!(
        changed.thread.name.as_deref(),
        Some("Renamed outside first page")
    );
    assert_eq!(changed.thread.archived_at, None);

    Ok(())
}

async fn rename_thread(app: &mut TestAppServer, thread_id: String, name: &str) -> Result<()> {
    let rename_id = app
        .send_thread_set_name_request(ThreadSetNameParams {
            thread_id,
            name: name.to_string(),
        })
        .await?;
    let _: ThreadSetNameResponse = read_response(app, rename_id).await?;
    Ok(())
}

async fn read_response<T: DeserializeOwned>(app: &mut TestAppServer, id: i64) -> Result<T> {
    let response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_stream_until_response_message(RequestId::Integer(id)),
    )
    .await??;
    to_response(response)
}
