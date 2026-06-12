use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadCatalogChangedNotification;
use codex_app_server_protocol::ThreadInjectItemsParams;
use codex_app_server_protocol::ThreadInjectItemsResponse;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSetNameResponse;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn catalog_subscription_reports_new_and_injected_thread() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut app = start_app(&codex_home).await?;
    subscribe(&mut app).await?;

    let started = start_thread(&mut app, ThreadStartParams::default()).await?;
    let changed = read_catalog_change(&mut app).await?;
    assert_eq!(changed.thread.id, started.id);
    assert_eq!(changed.thread.preview, "");
    assert!(
        !started
            .path
            .expect("thread path should be present")
            .exists()
    );

    let inject_id = app
        .send_thread_inject_items_request(ThreadInjectItemsParams {
            thread_id: started.id.clone(),
            items: vec![serde_json::json!({
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "injected"}],
            })],
        })
        .await?;
    let _: ThreadInjectItemsResponse = read_response(&mut app, inject_id).await?;
    let injected = read_catalog_change(&mut app).await?;
    assert_eq!(injected.thread.id, started.id);
    assert!(injected.thread.updated_at_ms >= changed.thread.updated_at_ms);

    Ok(())
}

#[tokio::test]
async fn catalog_subscription_reports_thread_outside_loaded_page() -> Result<()> {
    let codex_home = TempDir::new()?;
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
    let mut app = start_app(&codex_home).await?;

    subscribe(&mut app).await?;

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
            parent_thread_id: None,
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
        app.read_stream_until_notification_message("threadCatalog/changed"),
    )
    .await??;
    let raw_params = notification
        .params
        .expect("threadCatalog/changed should have params");
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

#[tokio::test]
async fn catalog_subscription_reports_ephemeral_thread_updates() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut app = start_app(&codex_home).await?;
    subscribe(&mut app).await?;

    let thread = start_thread(
        &mut app,
        ThreadStartParams {
            ephemeral: Some(true),
            ..Default::default()
        },
    )
    .await?;
    let created = read_catalog_change(&mut app).await?;

    let turn_id = app
        .send_turn_start_request(codex_app_server_protocol::TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![codex_app_server_protocol::UserInput::Text {
                text: "Ephemeral catalog preview".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: codex_app_server_protocol::TurnStartResponse = read_response(&mut app, turn_id).await?;
    let changed = read_catalog_change(&mut app).await?;
    assert_eq!(changed.thread.id, thread.id);
    assert_eq!(changed.thread.preview, "Ephemeral catalog preview");
    assert!(changed.thread.updated_at_ms >= created.thread.updated_at_ms);

    Ok(())
}

async fn start_app(codex_home: &TempDir) -> Result<TestAppServer> {
    write_mock_responses_config_toml(
        codex_home.path(),
        "http://localhost:1",
        &Default::default(),
        i64::MAX,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "",
    )?;
    let mut app = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, app.initialize()).await??;
    Ok(app)
}

async fn subscribe(app: &mut TestAppServer) -> Result<()> {
    let subscribe_id = app
        .send_raw_request("threadCatalog/subscribe", /*params*/ None)
        .await?;
    let _: codex_app_server_protocol::ThreadCatalogSubscribeResponse =
        read_response(app, subscribe_id).await?;
    Ok(())
}

async fn start_thread(
    app: &mut TestAppServer,
    params: ThreadStartParams,
) -> Result<codex_app_server_protocol::Thread> {
    let start_id = app.send_thread_start_request(params).await?;
    let started = read_response::<ThreadStartResponse>(app, start_id).await?;
    Ok(started.thread)
}

async fn read_catalog_change(app: &mut TestAppServer) -> Result<ThreadCatalogChangedNotification> {
    let notification: JSONRPCNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_stream_until_notification_message("threadCatalog/changed"),
    )
    .await??;
    serde_json::from_value(
        notification
            .params
            .ok_or_else(|| anyhow::anyhow!("threadCatalog/changed should have params"))?,
    )
    .map_err(Into::into)
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
