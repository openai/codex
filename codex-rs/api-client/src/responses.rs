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

use crate::api::ApiClient;
use crate::auth::AuthProvider;
use crate::client::PayloadBuilder;
use crate::common::backoff;
use crate::decode::responses::ErrorResponse;
use crate::error::Error;
use crate::error::Result;
use crate::model_provider::ModelProviderInfo;
use crate::prompt::Prompt;
use crate::stream::ResponseEvent;
use crate::stream::ResponseStream;

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
impl ApiClient for ResponsesApiClient {
    type Config = ResponsesApiClientConfig;

    fn new(config: Self::Config) -> Result<Self> {
        Ok(Self { config })
    }

    async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
        if self.config.provider.wire_api != crate::model_provider::WireApi::Responses {
            return Err(Error::UnsupportedOperation(
                "ResponsesApiClient requires a Responses provider".to_string(),
            ));
        }

        let payload_json = crate::payload::responses::ResponsesPayloadBuilder::new(
            self.config.model.clone(),
            self.config.conversation_id,
            self.config.provider.is_azure_responses_endpoint(),
        )
        .build(prompt)?;

        let max_attempts = self.config.provider.request_max_retries();
        for attempt in 0..=max_attempts {
            match self
                .attempt_stream_responses(attempt, prompt, &payload_json)
                .await
            {
                Ok(stream) => return Ok(stream),
                Err(StreamAttemptError::Fatal(err)) => return Err(err),
                Err(retryable) => {
                    if attempt == max_attempts {
                        return Err(retryable.into_error());
                    }

                    tokio::time::sleep(retryable.delay(attempt)).await;
                }
            }
        }

        unreachable!("attempt_stream_responses should always return");
    }
}

impl ResponsesApiClient {
    async fn attempt_stream_responses(
        &self,
        attempt: i64,
        prompt: &Prompt,
        payload_json: &Value,
    ) -> std::result::Result<ResponseStream, StreamAttemptError> {
        let auth = crate::client::http::resolve_auth(&self.config.auth_provider).await;

        trace!(
            "POST to {}: {:?}",
            self.config.provider.get_full_url(auth.as_ref()),
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
            prompt.session_source.as_ref(),
            &extra_headers,
        )
        .await
        .map_err(StreamAttemptError::Fatal)?;

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
            .log_request(attempt as u64, || req_builder.send())
            .await;

        let mut request_id = None;
        if let Ok(resp) = &res {
            request_id = resp
                .headers()
                .get("cf-ray")
                .and_then(|v| v.to_str().ok())
                .map(std::string::ToString::to_string);
        }

        match res {
            Ok(resp) if resp.status().is_success() => {
                let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

                if let Some(snapshot) =
                    crate::client::rate_limits::parse_rate_limit_snapshot(resp.headers())
                    && tx_event
                        .send(Ok(ResponseEvent::RateLimits(snapshot)))
                        .await
                        .is_err()
                {
                    debug!("receiver dropped rate limit snapshot event");
                }

                let stream = resp
                    .bytes_stream()
                    .map_err(move |err| Error::ResponseStreamFailed {
                        source: err,
                        request_id: request_id.clone(),
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

                Ok(ResponseStream { rx_event })
            }
            Ok(resp) => Err(handle_error_response(resp, request_id, &self.config).await),
            Err(err) => Err(StreamAttemptError::RetryableTransportError(Error::Http(
                err,
            ))),
        }
    }
}

// payload building is provided by crate::payload::responses

async fn handle_error_response(
    resp: reqwest::Response,
    request_id: Option<String>,
    _config: &ResponsesApiClientConfig,
) -> StreamAttemptError {
    let status = resp.status();
    let retry_after_secs = resp
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());
    let retry_after = retry_after_secs.map(|secs| {
        let clamped = if secs < 0 { 0 } else { secs as u64 };
        Duration::from_secs(clamped)
    });

    if !(status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::UNAUTHORIZED
        || status.is_server_error())
    {
        let body = resp.text().await.unwrap_or_default();
        return StreamAttemptError::Fatal(Error::UnexpectedStatus { status, body });
    }

    if status == StatusCode::TOO_MANY_REQUESTS {
        let rate_limits = crate::client::rate_limits::parse_rate_limit_snapshot(resp.headers());
        let body = resp.json::<ErrorResponse>().await.ok();
        if let Some(ErrorResponse { error }) = body {
            if error.r#type.as_deref() == Some("usage_limit_reached") {
                return StreamAttemptError::Fatal(Error::UsageLimitReached {
                    plan_type: error.plan_type,
                    resets_at: error.resets_at,
                    rate_limits,
                });
            } else if error.r#type.as_deref() == Some("usage_not_included") {
                return StreamAttemptError::Fatal(Error::Stream(
                    "usage not included".to_string(),
                    None,
                ));
            } else if crate::decode::responses::is_quota_exceeded_error(&error) {
                return StreamAttemptError::Fatal(Error::Stream(
                    "quota exceeded".to_string(),
                    None,
                ));
            }
        }
    }

    StreamAttemptError::RetryableHttpError {
        status,
        retry_after,
        request_id,
    }
}

enum StreamAttemptError {
    RetryableHttpError {
        status: StatusCode,
        retry_after: Option<Duration>,
        request_id: Option<String>,
    },
    RetryableTransportError(Error),
    Fatal(Error),
}

impl StreamAttemptError {
    fn delay(&self, attempt: i64) -> Duration {
        match self {
            StreamAttemptError::RetryableHttpError {
                retry_after: Some(retry_after),
                ..
            } => *retry_after,
            StreamAttemptError::RetryableHttpError {
                retry_after: None, ..
            }
            | StreamAttemptError::RetryableTransportError(..) => backoff(attempt),
            StreamAttemptError::Fatal(..) => Duration::from_millis(0),
        }
    }

    fn into_error(self) -> Error {
        match self {
            StreamAttemptError::RetryableHttpError {
                status, request_id, ..
            } => Error::RetryLimit {
                status: Some(status),
                request_id,
            },
            StreamAttemptError::RetryableTransportError(err) | StreamAttemptError::Fatal(err) => {
                err
            }
        }
    }
}
