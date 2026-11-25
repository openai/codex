//! Regression coverage for cancellation while RMCP holds a recovery-session DELETE claim.

#![cfg(test)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use codex_exec_server::RouteAwareHttpClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::future::BoxFuture;
use http::HeaderMap;
use pretty_assertions::assert_eq;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use tokio::sync::Notify;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

use super::RecoveryHandshakeGuard;
use super::StreamableHttpClientAdapter;
use crate::http_client_adapter::StreamableHttpRedirectMode;

struct GatedDeleteClient {
    inner: RouteAwareHttpClient,
    started: Notify,
    release: CancellationToken,
    completed: Notify,
}

impl HttpClient for GatedDeleteClient {
    fn http_request(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        Box::pin(async move {
            self.started.notify_one();
            self.release.cancelled().await;
            let result = self.inner.http_request(params).await;
            self.completed.notify_one();
            result
        })
    }

    fn http_request_stream(
        &self,
        params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        self.inner.http_request_stream(params)
    }
}

#[tokio::test]
async fn cancelling_sdk_delete_waiter_does_not_strand_the_cleanup_claim() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let http = Arc::new(GatedDeleteClient {
        inner: RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        )),
        started: Notify::new(),
        release: CancellationToken::new(),
        completed: Notify::new(),
    });
    let mut adapter = StreamableHttpClientAdapter::new(
        http.clone(),
        HeaderMap::new(),
        /*auth_provider*/ None,
        /*has_configured_headers*/ false,
        StreamableHttpRedirectMode::Legacy,
        Arc::default(),
    );
    let guard = RecoveryHandshakeGuard::new();
    let cleanup = guard.handle();
    let _cancellation = adapter.enable_recovery_cancellation(cleanup.clone());
    let uri: Arc<str> = Arc::from(server.uri());
    let session: Arc<str> = Arc::from("allocated-session");
    cleanup.register(
        adapter.clone(),
        Arc::clone(&uri),
        Arc::clone(&session),
        Some("access-token".into()),
        HashMap::new(),
    );
    let sdk_delete = tokio::spawn(async move {
        adapter
            .delete_session(uri, session, Some("access-token".into()), HashMap::new())
            .await
    });
    timeout(Duration::from_secs(2), http.started.notified()).await?;
    sdk_delete.abort();
    assert!(sdk_delete.await.unwrap_err().is_cancelled());
    drop(guard);
    http.release.cancel();
    timeout(Duration::from_secs(2), http.completed.notified()).await?;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].headers["mcp-session-id"], "allocated-session");
    assert_eq!(requests[0].headers["authorization"], "Bearer access-token");
    Ok(())
}
