//! Explicit external time requires a native clock on the app-server.
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use tempfile::TempDir;

#[tokio::test]
async fn external_time_without_native_provider_fails_thread_start() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_extra_config(
            "[features.current_time_reminder]\nenabled = true\nclock_source = \"external\"",
        )
        .write(home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    let id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let error = app_server
        .read_stream_until_error_message(RequestId::Integer(id))
        .await?;
    assert!(
        error
            .error
            .message
            .contains("no external current-time provider"),
        "{}",
        error.error.message
    );
    Ok(())
}
