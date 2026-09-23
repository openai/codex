//! Routes AWS credential and region HTTP through the application-owned client.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use aws_smithy_runtime_api::client::http::HttpConnector;
use aws_smithy_runtime_api::client::http::HttpConnectorFuture;
use aws_smithy_runtime_api::client::http::SharedHttpClient;
use aws_smithy_runtime_api::client::http::SharedHttpConnector;
use aws_smithy_runtime_api::client::http::http_client_fn;
use aws_smithy_runtime_api::client::orchestrator::HttpRequest;
use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
use aws_smithy_runtime_api::client::result::ConnectorError;
use aws_smithy_types::body::SdkBody;
use bytes::Bytes;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientBuilder;
use codex_http_client::HttpClientFactory;
use codex_http_client::HttpError;
use codex_http_client::RouteAwareClientPool;
use http_body_util::BodyExt;

pub(crate) fn http_client(factory: HttpClientFactory) -> SharedHttpClient {
    let connectors = Mutex::new(HashMap::new());
    http_client_fn(move |settings, _| {
        let timeouts = (settings.connect_timeout(), settings.read_timeout());
        connectors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(timeouts)
            .or_insert_with(|| {
                // Smithy's default connector does not follow redirects. Keep that behavior
                // so signed headers and metadata tokens cannot be sent to another origin.
                let mut builder = HttpClientBuilder::new()
                    .without_redirects()
                    .without_request_logging();
                if let Some(timeout) = timeouts.0 {
                    builder = builder.connect_timeout(timeout);
                }
                SharedHttpConnector::new(CredentialConnector {
                    client: RouteAwareClientPool::with_builder(
                        factory.clone(),
                        ClientRouteClass::Auth,
                        builder,
                    )
                    .into_client(),
                    read_timeout: timeouts.1,
                })
            })
            .clone()
    })
}

#[derive(Debug)]
struct CredentialConnector {
    client: HttpClient,
    read_timeout: Option<Duration>,
}

impl HttpConnector for CredentialConnector {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let client = self.client.clone();
        let read_timeout = self.read_timeout;
        HttpConnectorFuture::new(async move {
            let request = request
                .try_into_http1x()
                .map_err(|error| ConnectorError::user(Box::new(error)))?;
            let (parts, body) = request.into_parts();
            let mut request = client
                .request(parts.method, parts.uri.to_string())
                .headers(parts.headers)
                .version(parts.version);
            if let Some(timeout) = read_timeout {
                request = request.timeout(timeout);
            }
            request = match body.bytes() {
                Some(bytes) => request.body(Bytes::copy_from_slice(bytes)),
                None => request.body_stream(body.into_data_stream()),
            };
            let response = request.send().await.map_err(|error| match error {
                HttpError::Policy(error) => ConnectorError::user(Box::new(error)),
                error if error.is_timeout() => ConnectorError::timeout(Box::new(error)),
                error if error.is_connect() => ConnectorError::io(Box::new(error)),
                error => ConnectorError::user(Box::new(error)),
            })?;
            HttpResponse::try_from(
                response
                    .into_http_response()
                    .map(|body| SdkBody::from_body_1_x(SyncBody(Mutex::new(Box::pin(body))))),
            )
            .map_err(|error| ConnectorError::user(Box::new(error)))
        })
    }
}

// Smithy requires Sync bodies; the shared client's cancellation future is only Send.
// Polling stays synchronous, while the body retains policy checks for every frame.
struct SyncBody<B>(Mutex<Pin<Box<B>>>);

impl<B: http_body::Body> http_body::Body for SyncBody<B> {
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
            .poll_frame(cx)
    }
}
