use std::sync::Arc;

use codex_app_server_protocol::AuthMode;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::ConversationId;
use codex_protocol::protocol::RateLimitSnapshot;
use futures::TryStreamExt;
use reqwest::StatusCode;
use serde_json::Value;
use tracing::debug;
use tracing::trace;

use crate::auth::AuthProvider;
use crate::error::Error;
use crate::error::Result;
use crate::stream::WireResponseStream;
use codex_provider_config::ModelProviderInfo;

#[derive(Clone)]
/// Configuration for the OpenAI Responses API client (`/v1/responses`).
///
/// - `http_client`: Reqwest client used for HTTP requests.
/// - `provider`: Provider configuration (base URL, headers, retries, etc.).
/// - `conversation_id`: Used to set conversation/session headers and cache keys.
/// - `auth_provider`: Optional provider of auth context (e.g., ChatGPT login token).
/// - `otel_event_manager`: Telemetry event manager for request/stream instrumentation.
pub struct ResponsesApiClientConfig {
    pub http_client: reqwest::Client,
    pub provider: ModelProviderInfo,
    pub conversation_id: ConversationId,
    pub auth_provider: Option<Arc<dyn AuthProvider>>,
    pub otel_event_manager: OtelEventManager,
    pub extra_headers: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct ResponsesApiClient {
    config: ResponsesApiClientConfig,
}

impl ResponsesApiClient {
    pub fn new(config: ResponsesApiClientConfig) -> Result<Self> {
        Ok(Self { config })
    }
}

impl ResponsesApiClient {
    pub async fn stream_payload_wire(&self, payload_json: &Value) -> Result<WireResponseStream> {
        if self.config.provider.wire_api != codex_provider_config::WireApi::Responses {
            return Err(Error::UnsupportedOperation(
                "ResponsesApiClient requires a Responses provider".to_string(),
            ));
        }

        let mut owned_headers: Vec<(String, String)> = vec![
            (
                "conversation_id".to_string(),
                self.config.conversation_id.to_string(),
            ),
            (
                "session_id".to_string(),
                self.config.conversation_id.to_string(),
            ),
        ];
        owned_headers.extend(self.config.extra_headers.iter().cloned());

        let mut refreshed_auth = false;
        loop {
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

            let extra_headers = crate::client::http::header_pairs(&owned_headers);
            let mut req_builder = crate::client::http::build_request(
                &self.config.http_client,
                &self.config.provider,
                &auth,
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

            let status = res.status();
            let snapshot = crate::client::rate_limits::parse_rate_limit_snapshot(res.headers());

            if !status.is_success() {
                if status == StatusCode::UNAUTHORIZED
                    && !refreshed_auth
                    && self.config.auth_provider.is_some()
                    && let Some(provider) = &self.config.auth_provider {
                        provider.refresh_token().await?;
                        refreshed_auth = true;
                        continue;
                    }

                let body = res
                    .text()
                    .await
                    .unwrap_or_else(|err| format!("<failed to read body: {err}>"));
                return Err(map_error_response(status, &body, snapshot));
            }

            let stream = res
                .bytes_stream()
                .map_err(|err| Error::ResponseStreamFailed {
                    source: err,
                    request_id: None,
                });

            let (tx_event, rx_event) = crate::client::sse::spawn_wire_stream(
                stream,
                &self.config.provider,
                self.config.otel_event_manager.clone(),
                crate::decode_wire::responses::WireResponsesSseDecoder,
            );
            if let Some(snapshot) = snapshot
                && tx_event
                    .send(Ok(crate::stream::WireEvent::RateLimits(snapshot)))
                    .await
                    .is_err()
            {
                debug!("receiver dropped rate limit snapshot event");
            }

            return Ok(rx_event);
        }
    }
}

fn map_error_response(
    status: StatusCode,
    body: &str,
    rate_limits: Option<RateLimitSnapshot>,
) -> Error {
    if let Ok(value) = serde_json::from_str::<Value>(body)
        && let Some(error) = value.get("error") {
            let error_code = error
                .get("type")
                .or_else(|| error.get("code"))
                .and_then(|value| value.as_str())
                .map(str::to_lowercase);
            if let Some(code) = error_code.as_deref() {
                match code {
                    "usage_limit_reached" => {
                        let plan_type = extract_string_field(
                            error,
                            &[
                                &["plan_type"],
                                &["metadata", "plan_type"],
                                &["details", "plan_type"],
                            ],
                        );
                        let resets_at = extract_i64_field(
                            error,
                            &[
                                &["resets_at"],
                                &["metadata", "resets_at"],
                                &["details", "resets_at"],
                            ],
                        );
                        return Error::UsageLimitReached {
                            plan_type,
                            resets_at,
                            rate_limits,
                        };
                    }
                    "usage_not_included" => {
                        return Error::UsageNotIncluded;
                    }
                    "quota_exceeded" | "insufficient_quota" => {
                        return Error::QuotaExceeded;
                    }
                    _ => {}
                }
            }

            if let Some(message) = error.get("message").and_then(|v| v.as_str())
                && !message.is_empty() {
                    return Error::Stream(message.to_string(), None);
                }
        }

    Error::UnexpectedStatus {
        status,
        body: body.to_string(),
    }
}

fn extract_string_field(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .filter_map(|path| nested_value(value, path))
        .find_map(|candidate| candidate.as_str().map(std::string::ToString::to_string))
}

fn extract_i64_field(value: &Value, paths: &[&[&str]]) -> Option<i64> {
    paths
        .iter()
        .filter_map(|path| nested_value(value, path))
        .find_map(|candidate| match candidate {
            Value::Number(num) => num.as_i64(),
            Value::String(text) => text.parse::<i64>().ok(),
            _ => None,
        })
}

fn nested_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(segment)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::RateLimitWindow;
    use serde_json::json;

    fn snapshot() -> RateLimitSnapshot {
        RateLimitSnapshot {
            primary: Some(RateLimitWindow {
                used_percent: 40.0,
                window_minutes: Some(15),
                resets_at: Some(1_704_067_200),
            }),
            secondary: None,
        }
    }

    #[test]
    fn usage_limit_error_includes_metadata() {
        let body = json!({
            "error": {
                "type": "usage_limit_reached",
                "message": "limit",
                "plan_type": "pro",
                "resets_at": 1704,
            }
        })
        .to_string();

        let err = map_error_response(StatusCode::TOO_MANY_REQUESTS, &body, Some(snapshot()));
        match err {
            Error::UsageLimitReached {
                plan_type,
                resets_at,
                rate_limits,
            } => {
                assert_eq!(plan_type.as_deref(), Some("pro"));
                assert_eq!(resets_at, Some(1704));
                assert!(rate_limits.is_some());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn usage_not_included_maps_to_specific_variant() {
        let body = json!({
            "error": {
                "code": "usage_not_included",
                "message": "upgrade",
            }
        })
        .to_string();

        let err = map_error_response(StatusCode::PAYMENT_REQUIRED, &body, None);
        assert!(matches!(err, Error::UsageNotIncluded));
    }

    #[test]
    fn unexpected_status_falls_back_to_generic_error() {
        let err = map_error_response(StatusCode::BAD_REQUEST, "oops", None);
        match err {
            Error::UnexpectedStatus { status, body } => {
                assert_eq!(status, StatusCode::BAD_REQUEST);
                assert_eq!(body, "oops");
            }
            other => panic!("expected UnexpectedStatus, got {other:?}"),
        }
    }
}
