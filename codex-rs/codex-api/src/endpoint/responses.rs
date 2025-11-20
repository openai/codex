use crate::auth::AuthProvider;
use crate::auth::add_auth_headers;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::provider::WireApi;
use crate::requests::ResponsesRequest;
use crate::sse::spawn_response_stream;
use crate::telemetry::SseTelemetry;
use crate::telemetry::run_with_request_telemetry;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use http::HeaderMap;
use http::Method;
use serde_json::Value;
use std::sync::Arc;

pub struct ResponsesClient<T: HttpTransport, A: AuthProvider> {
    transport: T,
    provider: Provider,
    auth: A,
    request_telemetry: Option<Arc<dyn RequestTelemetry>>,
    sse_telemetry: Option<Arc<dyn SseTelemetry>>,
}

impl<T: HttpTransport, A: AuthProvider> ResponsesClient<T, A> {
    pub fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            transport,
            provider,
            auth,
            request_telemetry: None,
            sse_telemetry: None,
        }
    }

    pub fn with_telemetry(
        mut self,
        request: Option<Arc<dyn RequestTelemetry>>,
        sse: Option<Arc<dyn SseTelemetry>>,
    ) -> Self {
        self.request_telemetry = request;
        self.sse_telemetry = sse;
        self
    }

    pub async fn stream_request(
        &self,
        request: ResponsesRequest,
    ) -> Result<ResponseStream, ApiError> {
        self.stream(request.body, request.headers).await
    }

    fn path(&self) -> &'static str {
        match self.provider.wire {
            WireApi::Responses | WireApi::Compact => "responses",
            WireApi::Chat => "chat/completions",
        }
    }

    pub async fn stream(
        &self,
        body: Value,
        extra_headers: HeaderMap,
    ) -> Result<ResponseStream, ApiError> {
        let request_builder = || {
            let mut req = self.provider.build_request(Method::POST, self.path());
            req.headers.extend(extra_headers.clone());
            req.headers.insert(
                http::header::ACCEPT,
                http::HeaderValue::from_static("text/event-stream"),
            );
            req.body = Some(body.clone());
            add_auth_headers(&self.auth, &mut req)
        };

        let stream_response = run_with_request_telemetry(
            self.provider.retry.to_policy(),
            self.request_telemetry.clone(),
            request_builder,
            |req| self.transport.stream(req),
        )
        .await?;

        Ok(spawn_response_stream(
            stream_response,
            self.provider.stream_idle_timeout,
            self.sse_telemetry.clone(),
        ))
    }
}
