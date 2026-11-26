//! Codex's account-scoped store of Statsig evaluations and its policy-aware HTTP adapter.

mod evaluations;
mod telemetry;

pub use codex_backend_client::StatsigBootstrapContext;
pub use evaluations::DynamicConfig;
pub use evaluations::Gate;
pub use evaluations::StatsigIdentity;
pub use evaluations::StatsigRuntime;

use std::time::Duration;

use bytes::Bytes;
use bytes::BytesMut;
use codex_http_client::HttpError;
use codex_http_client::RouteAwareClientPool;
use http::HeaderValue;
use http::Request;
use http::header::ACCEPT_ENCODING;
use http::header::CONTENT_ENCODING;
use statsig_client::HttpClient;
use statsig_client::HttpClientFuture;
use statsig_client::NetworkOptions;
use statsig_client::StatsigError;

/// Sends Statsig traffic through an existing, configured Codex HTTP pool.
#[derive(Debug)]
pub struct StatsigHttpClient {
    client: RouteAwareClientPool,
}

impl StatsigHttpClient {
    /// Reuses the pool's existing proxy, TLS, and network-policy configuration.
    pub fn new(client: RouteAwareClientPool) -> Self {
        Self { client }
    }
}

impl HttpClient for StatsigHttpClient {
    fn send(&self, request: Request<Bytes>, options: NetworkOptions) -> HttpClientFuture<'_> {
        Box::pin(async move {
            let (mut parts, body) = request.into_parts();
            parts
                .headers
                .insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
            let mut response = self
                .client
                .request(parts.method, parts.uri.to_string())
                .version(parts.version)
                .headers(parts.headers)
                .body(body)
                .timeout(options.timeout)
                .send()
                .await
                .map_err(|error| map_error(error, options.timeout))?;
            if response
                .headers()
                .get_all(CONTENT_ENCODING)
                .iter()
                .any(|encoding| !encoding.as_bytes().eq_ignore_ascii_case(b"identity"))
            {
                return Err(StatsigError::Network(
                    "unexpected Statsig response encoding".into(),
                ));
            }
            let mut bytes = BytesMut::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| map_error(error, options.timeout))?
            {
                let actual = bytes.len().saturating_add(chunk.len());
                if let Some(limit) = options.response_body_byte_limit
                    && actual > limit
                {
                    return Err(StatsigError::NetworkResponseTooLarge { limit, actual });
                }
                bytes.extend_from_slice(&chunk);
            }
            let final_url = response.url().clone();
            let mut response = response.into_http_response().map(|_| bytes.freeze());
            response.extensions_mut().insert(final_url);
            Ok(response)
        })
    }
}

fn map_error(error: HttpError, timeout: Duration) -> StatsigError {
    if error.is_timeout() {
        StatsigError::NetworkTimeout(timeout)
    } else if matches!(error, HttpError::Policy(_)) {
        StatsigError::Network(telemetry::POLICY_DENIED.into())
    } else {
        StatsigError::Network(error.without_url().to_string())
    }
}

#[cfg(test)]
#[path = "http_client_tests.rs"]
mod tests;
