use crate::auth::AuthProvider;
use crate::auth::add_auth_headers;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::telemetry::SseTelemetry;
use crate::telemetry::run_with_request_telemetry;
use codex_client::Body;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use codex_client::StreamResponse;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use http::header::ACCEPT;
use http::header::CONTENT_TYPE;
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct StreamingClient<T: HttpTransport, A: AuthProvider> {
    transport: T,
    provider: Provider,
    auth: A,
    request_telemetry: Option<Arc<dyn RequestTelemetry>>,
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

impl<T: HttpTransport, A: AuthProvider> StreamingClient<T, A> {
    pub(crate) fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            transport,
            provider,
            auth,
            request_telemetry: None,
            sse_telemetry: None,
        }
    }

    pub(crate) fn with_telemetry(
        mut self,
        request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        self.request_telemetry = request;
        self.sse_telemetry = sse;
        self
    }

    pub(crate) fn provider(&self) -> &Provider {
        &self.provider
    }

    pub(crate) async fn stream(
        &self,
        path: &str,
        body: Body,
        extra_headers: HeaderMap,
        spawner: fn(StreamResponse, Duration, Option<Arc<dyn SseTelemetry>>) -> ResponseStream,
    ) -> Result<ResponseStream, ApiError> {
        let builder = || {
            let mut req = self.provider.build_request(Method::POST, path);
            req.headers.extend(extra_headers.clone());
            req.headers
                .insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
            req.headers
                .entry(CONTENT_TYPE)
                .or_insert_with(|| HeaderValue::from_static("application/json"));
            req.body = Some(body.clone());
            add_auth_headers(&self.auth, req)
        };

        let stream_response = run_with_request_telemetry(
            self.provider.retry.to_policy(),
            self.request_telemetry.clone(),
            builder,
            |req| self.transport.stream(req),
        )
        .await?;

        Ok(spawner(
            stream_response,
            self.provider.stream_idle_timeout,
            self.sse_telemetry.clone(),
        ))
    }
}
