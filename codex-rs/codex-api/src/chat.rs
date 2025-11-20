use crate::auth::AuthProvider;
use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::provider::Provider;
use crate::provider::WireApi;
use crate::rate_limits::parse_rate_limit;
use crate::responses::add_auth_headers;
use crate::responses::process_sse;
use codex_client::HttpTransport;
use codex_client::run_with_retry;
use http::HeaderMap;
use http::Method;
use serde_json::Value;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct ChatClient<T: HttpTransport, A: AuthProvider> {
    transport: T,
    provider: Provider,
    auth: A,
}

impl<T: HttpTransport, A: AuthProvider> ChatClient<T, A> {
    pub fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            transport,
            provider,
            auth,
        }
    }

    fn path(&self) -> &'static str {
        match self.provider.wire {
            WireApi::Chat => "chat/completions",
            _ => "responses",
        }
    }

    pub async fn stream(
        &self,
        body: Value,
        extra_headers: HeaderMap,
    ) -> Result<ResponseStream, ApiError> {
        let builder = || {
            let mut req = self.provider.build_request(Method::POST, self.path());
            req.headers.extend(extra_headers.clone());
            req.headers.insert(
                http::header::ACCEPT,
                http::HeaderValue::from_static("text/event-stream"),
            );
            req.body = Some(body.clone());
            add_auth_headers(&self.auth, &mut req)
        };
        let policy = self.provider.retry.to_policy();
        let (headers, stream) = run_with_retry(policy, builder, |r| async {
            self.transport.stream(r).await
        })
        .await?;

        let rate_limits = parse_rate_limit(&headers);
        let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);
        let idle = self.provider.stream_idle_timeout;
        tokio::spawn(async move {
            if let Some(snapshot) = rate_limits {
                let _ = tx_event.send(Ok(ResponseEvent::RateLimits(snapshot))).await;
            }
            process_sse(stream, tx_event, idle).await;
        });

        Ok(ResponseStream { rx_event })
    }
}
