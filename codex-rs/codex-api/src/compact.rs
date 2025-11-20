use crate::auth::AuthProvider;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::provider::WireApi;
use crate::responses::add_auth_headers;
use codex_client::HttpTransport;
use codex_client::run_with_retry;
use codex_protocol::models::ResponseItem;
use http::HeaderMap;
use http::Method;
use serde::Deserialize;

#[derive(Debug)]
pub struct CompactClient<T: HttpTransport, A: AuthProvider> {
    transport: T,
    provider: Provider,
    auth: A,
}

impl<T: HttpTransport, A: AuthProvider> CompactClient<T, A> {
    pub fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            transport,
            provider,
            auth,
        }
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
        let policy = self.provider.retry.to_policy();
        let resp = run_with_retry(policy, builder, |r| async {
            self.transport.execute(r).await
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
