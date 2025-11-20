use crate::auth::AuthProvider;
use crate::auth::add_auth_headers;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::provider::WireApi;
use crate::telemetry::run_with_request_telemetry;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use codex_protocol::models::ResponseItem;
use http::HeaderMap;
use http::Method;
use serde::Deserialize;
use std::sync::Arc;

pub struct CompactClient<T: HttpTransport, A: AuthProvider> {
    transport: T,
    provider: Provider,
    auth: A,
    request_telemetry: Option<Arc<dyn RequestTelemetry>>,
}

impl<T: HttpTransport, A: AuthProvider> CompactClient<T, A> {
    pub fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            transport,
            provider,
            auth,
            request_telemetry: None,
        }
    }

    pub fn with_telemetry(mut self, request: Option<Arc<dyn RequestTelemetry>>) -> Self {
        self.request_telemetry = request;
        self
    }

    fn path(&self) -> &'static str {
        match self.provider.wire {
            WireApi::Compact | WireApi::Responses => "responses/compact",
            WireApi::Chat => "chat/completions",
        }
    }

    pub async fn compact(
        &self,
        body: serde_json::Value,
        extra_headers: HeaderMap,
    ) -> Result<Vec<ResponseItem>, ApiError> {
        let builder = || {
            let mut req = self.provider.build_request(Method::POST, self.path());
            req.headers.extend(extra_headers.clone());
            req.body = Some(body.clone());
            add_auth_headers(&self.auth, &mut req)
        };

        let resp =
            run_with_request_telemetry(self.provider.retry.to_policy(), self.request_telemetry.clone(), builder, |req| {
                self.transport.execute(req)
            })
            .await?;
        let parsed: CompactHistoryResponse =
            serde_json::from_slice(&resp.body).map_err(|e| ApiError::Stream(e.to_string()))?;
        Ok(parsed.output)
    }
}

#[derive(Debug, Deserialize)]
struct CompactHistoryResponse {
    output: Vec<ResponseItem>,
}
