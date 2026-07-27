use std::collections::HashMap;
use std::sync::Arc;

use codex_exec_server::Environment;
use reqwest::header::HeaderMap;
use rmcp::ErrorData;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::ClientResult;
use rmcp::model::RequestId;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::StreamableHttpClientAdapter;

#[tokio::test]
async fn accepts_empty_ok_for_one_way_client_messages() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "application/json"))
        .expect(2)
        .mount(&server)
        .await;

    let adapter = StreamableHttpClientAdapter::new(
        Environment::default_for_tests().get_http_client(),
        HeaderMap::new(),
        /*auth_provider*/ None,
    );
    let messages = [
        (
            "response",
            ClientJsonRpcMessage::response(
                ClientResult::empty(()),
                RequestId::String("response-id".into()),
            ),
        ),
        (
            "error",
            ClientJsonRpcMessage::error(
                ErrorData::internal_error("test error", /*data*/ None),
                Some(RequestId::String("error-id".into())),
            ),
        ),
    ];

    for (message_kind, message) in messages {
        let result = adapter
            .post_message(
                Arc::from(format!("{}/mcp", server.uri())),
                message,
                /*session_id*/ None,
                /*auth_token*/ None,
                HashMap::new(),
            )
            .await;

        assert!(
            matches!(result, Ok(StreamableHttpPostResponse::Accepted)),
            "expected empty response for {message_kind} to be accepted, got {result:?}"
        );
    }
}
