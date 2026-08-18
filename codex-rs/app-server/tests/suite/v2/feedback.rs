use std::time::Duration;

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

#[tokio::test]
async fn feedback_upload_reports_transport_failure_as_json_rpc_error() -> Result<()> {
    let proxy = MockServer::start().await;
    Mock::given(method("CONNECT"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&proxy)
        .await;
    let proxy_uri = proxy.uri();
    let mut app_server = TestAppServer::builder()
        .with_env_overrides(&[
            ("HTTPS_PROXY", Some(proxy_uri.as_str())),
            ("https_proxy", Some(proxy_uri.as_str())),
            ("NO_PROXY", Some("")),
            ("no_proxy", Some("")),
        ])
        .build_initialized()
        .await?;

    let request_id = app_server
        .send_raw_request(
            "feedback/upload",
            Some(json!({ "classification": "bug", "includeLogs": false })),
        )
        .await?;
    let error = timeout(
        Duration::from_secs(15),
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32603);
    assert!(error.error.message.contains("failed to upload feedback"));
    Ok(())
}
