use crate::auth::SharedAuthProvider;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::telemetry::run_with_request_telemetry;
use codex_client::HttpTransport;
use codex_client::Request;
use codex_client::RequestBody;
use codex_client::RequestTelemetry;
use codex_client::Response;
use codex_client::StreamResponse;
use codex_client::TransportError;
use http::HeaderMap;
use http::Method;
use serde_json::Value;
use std::sync::Arc;
use tracing::instrument;

pub(crate) struct EndpointSession<T: HttpTransport> {
    transport: T,
    provider: Provider,
    auth: SharedAuthProvider,
    request_telemetry: Option<Arc<dyn RequestTelemetry>>,
}

impl<T: HttpTransport> EndpointSession<T> {
    pub(crate) fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            transport,
            provider,
            auth,
            request_telemetry: None,
        }
    }

    pub(crate) fn with_request_telemetry(
        mut self,
        request: Option<Arc<dyn RequestTelemetry>>,
    ) -> Self {
        self.request_telemetry = request;
        self
    }

    pub(crate) fn provider(&self) -> &Provider {
        &self.provider
    }

    fn make_request(
        &self,
        method: &Method,
        path: &str,
        extra_headers: &HeaderMap,
        body: Option<&Value>,
    ) -> Request {
        let mut req = self.provider.build_request(method.clone(), path);
        req.headers.extend(extra_headers.clone());
        if let Some(body) = body {
            req.body = Some(RequestBody::Json(body.clone()));
        }
        req
    }

    pub(crate) async fn execute(
        &self,
        method: Method,
        path: &str,
        extra_headers: HeaderMap,
        body: Option<Value>,
    ) -> Result<Response, ApiError> {
        self.execute_with(method, path, extra_headers, body, |_| {})
            .await
    }

    #[instrument(
        name = "endpoint_session.execute_with",
        level = "info",
        skip_all,
        fields(http.method = %method, api.path = path)
    )]
    pub(crate) async fn execute_with<C>(
        &self,
        method: Method,
        path: &str,
        extra_headers: HeaderMap,
        body: Option<Value>,
        configure: C,
    ) -> Result<Response, ApiError>
    where
        C: Fn(&mut Request),
    {
        let make_request = || {
            let mut req = self.make_request(&method, path, &extra_headers, body.as_ref());
            configure(&mut req);
            req
        };

        let response = run_with_request_telemetry(
            self.provider.retry.to_policy(),
            self.request_telemetry.clone(),
            make_request,
            |req| {
                let auth = self.auth.clone();
                let transport = &self.transport;
                async move {
                    let req = auth.apply_auth(req).await.map_err(TransportError::from)?;
                    let request_url = req.url.clone();
                    let request_headers = req.headers.clone();
                    let response = transport.execute(req).await;
                    observe_auth_response_headers(
                        auth.as_ref(),
                        &request_url,
                        &request_headers,
                        &response,
                    );
                    response
                }
            },
        )
        .await?;

        Ok(response)
    }

    #[instrument(
        name = "endpoint_session.stream_with",
        level = "info",
        skip_all,
        fields(http.method = %method, api.path = path)
    )]
    pub(crate) async fn stream_with<C>(
        &self,
        method: Method,
        path: &str,
        extra_headers: HeaderMap,
        body: Option<Value>,
        configure: C,
    ) -> Result<StreamResponse, ApiError>
    where
        C: Fn(&mut Request),
    {
        let make_request = || {
            let mut req = self.make_request(&method, path, &extra_headers, body.as_ref());
            configure(&mut req);
            req
        };

        let stream = run_with_request_telemetry(
            self.provider.retry.to_policy(),
            self.request_telemetry.clone(),
            make_request,
            |req| {
                let auth = self.auth.clone();
                let transport = &self.transport;
                async move {
                    let req = auth.apply_auth(req).await.map_err(TransportError::from)?;
                    let request_url = req.url.clone();
                    let request_headers = req.headers.clone();
                    let response = transport.stream(req).await;
                    observe_auth_response_headers(
                        auth.as_ref(),
                        &request_url,
                        &request_headers,
                        &response,
                    );
                    response
                }
            },
        )
        .await?;

        Ok(stream)
    }
}

fn observe_auth_response_headers<T>(
    auth: &dyn crate::auth::AuthProvider,
    request_url: &str,
    request_headers: &HeaderMap,
    response: &Result<T, TransportError>,
) where
    T: ResponseHeaders,
{
    match response {
        Ok(response) => {
            auth.observe_response_headers(request_url, request_headers, response.headers())
        }
        Err(TransportError::Http {
            headers: Some(headers),
            ..
        }) => auth.observe_response_headers(request_url, request_headers, headers),
        Err(_) => {}
    }
}

/// Exposes transport response headers so auth providers can observe request-scoped updates.
trait ResponseHeaders {
    fn headers(&self) -> &HeaderMap;
}

impl ResponseHeaders for Response {
    fn headers(&self) -> &HeaderMap {
        &self.headers
    }
}

impl ResponseHeaders for StreamResponse {
    fn headers(&self) -> &HeaderMap {
        &self.headers
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
