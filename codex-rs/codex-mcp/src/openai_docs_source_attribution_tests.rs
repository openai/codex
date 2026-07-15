use std::sync::Arc;
use std::sync::Mutex;

use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use futures::FutureExt;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;

use super::OPENAI_DEVELOPER_DOCS_MCP_CODEX_URL;
use super::OPENAI_DEVELOPER_DOCS_MCP_URL;
use super::maybe_with_openai_docs_source_attribution;

#[derive(Default)]
struct RecordingHttpClient {
    urls: Mutex<Vec<String>>,
}

impl HttpClient for RecordingHttpClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        self.urls.lock().unwrap().push(params.url);
        async { Err(ExecServerError::HttpRequest("test response".to_string())) }.boxed()
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        self.urls.lock().unwrap().push(params.url);
        async { Err(ExecServerError::HttpRequest("test response".to_string())) }.boxed()
    }
}

fn request(url: &str) -> HttpRequestParams {
    HttpRequestParams {
        method: "POST".to_string(),
        url: url.to_string(),
        headers: Vec::new(),
        body: None,
        timeout_ms: None,
        redirect_policy: HttpRedirectPolicy::Follow,
        request_id: "test-request".to_string(),
        stream_response: true,
    }
}

#[tokio::test]
async fn attributes_only_docs_mcp_requests() {
    let recording_client = Arc::new(RecordingHttpClient::default());
    let http_client = maybe_with_openai_docs_source_attribution(
        OPENAI_DEVELOPER_DOCS_MCP_URL,
        recording_client.clone(),
    );

    let _ = http_client
        .http_request_stream(request(OPENAI_DEVELOPER_DOCS_MCP_URL))
        .await;
    let _ = http_client
        .http_request(request(
            "https://developers.openai.com/.well-known/oauth-protected-resource/mcp",
        ))
        .await;

    assert_eq!(
        recording_client.urls.lock().unwrap().as_slice(),
        [
            OPENAI_DEVELOPER_DOCS_MCP_CODEX_URL,
            "https://developers.openai.com/.well-known/oauth-protected-resource/mcp",
        ]
    );
}

#[test]
fn leaves_other_mcp_clients_unwrapped() {
    let recording_client = Arc::new(RecordingHttpClient::default());
    let http_client = maybe_with_openai_docs_source_attribution(
        "https://example.com/mcp",
        recording_client.clone(),
    );

    assert!(Arc::ptr_eq(
        &http_client,
        &(recording_client as Arc<dyn HttpClient>)
    ));
}
