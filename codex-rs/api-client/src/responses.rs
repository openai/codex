use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use codex_app_server_protocol::AuthMode;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use futures::TryStreamExt;
use reqwest::StatusCode;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::trace;

use crate::api::PayloadClient;
use crate::auth::AuthProvider;
use crate::common::backoff;
use crate::decode::responses::ErrorResponse;
use crate::error::Error;
use crate::error::Result;
use crate::stream::ResponseEvent;
use crate::stream::ResponseStream;
use codex_provider_config::ModelProviderInfo;

#[derive(Clone)]
/// Configuration for the OpenAI Responses API client (`/v1/responses`).
///
/// - `http_client`: Reqwest client used for HTTP requests.
/// - `provider`: Provider configuration (base URL, headers, retries, etc.).
/// - `model`: Model identifier to use.
/// - `conversation_id`: Used to set conversation/session headers and cache keys.
/// - `auth_provider`: Optional provider of auth context (e.g., ChatGPT login token).
/// - `otel_event_manager`: Telemetry event manager for request/stream instrumentation.
pub struct ResponsesApiClientConfig {
    pub http_client: reqwest::Client,
    pub provider: ModelProviderInfo,
    pub model: String,
    pub conversation_id: ConversationId,
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    pub otel_event_manager: OtelEventManager,
}

#[derive(Clone)]
pub struct ResponsesApiClient {
    config: ResponsesApiClientConfig,
}


#[async_trait]
impl PayloadClient for ResponsesApiClient {
    type Config = ResponsesApiClientConfig;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(Self { config })
    }

    async fn stream_payload(
        &self,
        payload_json: &Value,
        session_source: Option<&codex_protocol::protocol::SessionSource>,
    ) -> Result<ResponseStream> {
        if self.config.provider.wire_api != codex_provider_config::WireApi::Responses {
            return Err(Error::UnsupportedOperation(
                "ResponsesApiClient requires a Responses provider".to_string(),
            ));
        }

        let auth = crate::client::http::resolve_auth(&self.config.auth_provider).await;

        trace!(
            "POST to {}: {:?}",
            self.config.provider.get_full_url(
                auth.as_ref()
                    .map(|a| codex_provider_config::AuthContext {
                        mode: a.mode,
                        bearer_token: a.bearer_token.clone(),
                        account_id: a.account_id.clone(),
                    })
                    .as_ref()
            ),
            serde_json::to_string(payload_json)
                .unwrap_or_else(|_| "<unable to serialize payload>".to_string())
        );

        let extra_headers = vec![
            ("conversation_id", self.config.conversation_id.to_string()),
            ("session_id", self.config.conversation_id.to_string()),
        ];
        let mut req_builder = crate::client::http::build_request(
            &self.config.http_client,
            &self.config.provider,
            &auth,
            session_source,
            &extra_headers,
        )
        .await?;

        req_builder = req_builder
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(payload_json);

        if let Some(auth_ctx) = auth.as_ref()
            && auth_ctx.mode == AuthMode::ChatGPT
            && let Some(account_id) = auth_ctx.account_id.clone()
        {
            req_builder = req_builder.header("chatgpt-account-id", account_id);
        }

        let res = self
            .config
            .otel_event_manager
            .log_request(0, || req_builder.send())
            .await
            .map_err(|source| Error::ResponseStreamFailed {
                source,
                request_id: None,
            })?;

        let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);
        if let Some(snapshot) = crate::client::rate_limits::parse_rate_limit_snapshot(res.headers())
            && tx_event
                .send(Ok(ResponseEvent::RateLimits(snapshot)))
                .await
                .is_err()
        {
            debug!("receiver dropped rate limit snapshot event");
        }

        let stream = res
            .bytes_stream()
            .map_err(|err| Error::ResponseStreamFailed {
                source: err,
                request_id: None,
            });
        let idle_timeout = self.config.provider.stream_idle_timeout();
        let otel = self.config.otel_event_manager.clone();
        tokio::spawn(crate::client::sse::process_sse(
            stream,
            tx_event,
            idle_timeout,
            otel,
            crate::decode::responses::ResponsesSseDecoder,
        ));

        Ok(crate::stream::EventStream::from_receiver(rx_event))
    }
}

