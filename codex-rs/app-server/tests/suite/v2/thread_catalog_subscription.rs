use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::create_fake_rollout;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadCatalogChangedNotification;
use codex_app_server_protocol::ThreadCatalogSubscribeResponse;
use codex_app_server_protocol::ThreadDeleteParams;
use codex_app_server_protocol::ThreadDeleteResponse;
use codex_app_server_protocol::ThreadSetNameParams;
use codex_app_server_protocol::ThreadSetNameResponse;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

const READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn catalog_subscription_tracks_threads_the_client_never_listed() -> Result<()> {
    let codex_home = TempDir::new()?;
    let thread_id = create_fake_rollout(
        codex_home.path(),
        "2025-01-05T12-00-00",
        "2025-01-05T12:00:00Z",
        "Unlisted thread",
        Some("mock_provider"),
        /*git_info*/ None,
    )?;
    let mut app = start_app(&codex_home).await?;

    let subscribe_id = app
        .send_raw_request("threadCatalog/subscribe", /*params*/ None)
        .await?;
    let _: ThreadCatalogSubscribeResponse = read_response(&mut app, subscribe_id).await?;

    let rename_id = app
        .send_thread_set_name_request(ThreadSetNameParams {
            thread_id: thread_id.clone(),
            name: "Renamed without listing".to_string(),
        })
        .await?;
    let _: ThreadSetNameResponse = read_response(&mut app, rename_id).await?;
    let changed_raw: JSONRPCNotification = timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("threadCatalog/changed"),
    )
    .await??;
    let changed_params = changed_raw.params.expect("catalog change params");
    assert_eq!(changed_params["thread"].get("turns"), None);
    assert_eq!(changed_params["thread"].get("status"), None);
    let ThreadCatalogChangedNotification::Upsert { thread } =
        serde_json::from_value(changed_params)?
    else {
        panic!("expected catalog upsert");
    };
    assert_eq!(thread.id, thread_id);
    assert_eq!(thread.name.as_deref(), Some("Renamed without listing"));
    assert_eq!(
        thread.recency_at_ms.map(|value| value / 1_000),
        thread.recency_at
    );

    let delete_id = app
        .send_thread_delete_request(ThreadDeleteParams {
            thread_id: thread_id.clone(),
        })
        .await?;
    let _: ThreadDeleteResponse = read_response(&mut app, delete_id).await?;
    let deleted_raw: JSONRPCNotification = timeout(
        READ_TIMEOUT,
        app.read_stream_until_notification_message("threadCatalog/changed"),
    )
    .await??;
    let deleted: ThreadCatalogChangedNotification =
        serde_json::from_value(deleted_raw.params.expect("catalog deletion params"))?;
    assert_eq!(
        deleted,
        ThreadCatalogChangedNotification::Delete { thread_id }
    );

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
    let mut app = TestAppServer::new_with_auto_env(codex_home.path()).await?;
    timeout(READ_TIMEOUT, app.initialize()).await??;
    Ok(app)
}

async fn read_response<T: DeserializeOwned>(app: &mut TestAppServer, id: i64) -> Result<T> {
    let response: JSONRPCResponse = timeout(
        READ_TIMEOUT,
        app.read_stream_until_response_message(RequestId::Integer(id)),
    )
    .await??;
    to_response(response)
}
