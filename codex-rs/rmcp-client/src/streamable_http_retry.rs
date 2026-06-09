use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use anyhow::anyhow;
use codex_exec_server::ExecServerError;
use reqwest::StatusCode;
use rmcp::service::RoleClient;
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use tokio::time;
use tracing::warn;

use crate::elicitation_client_service::ElicitationClientService;
use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::oauth::OAuthPersistor;

use super::PendingTransport;
use super::RmcpClient;

const JSON_RPC_INTERNAL_ERROR_CODE: i64 = -32603;
const STREAMABLE_HTTP_RETRY_DELAYS_MS: [u64; 2] = [250, 1_000];

impl RmcpClient {
    pub(super) async fn connect_pending_transport_with_initialize_retries(
        &self,
        initial_transport: PendingTransport,
        client_service: ElicitationClientService,
        timeout: Option<Duration>,
    ) -> Result<(
        Arc<RunningService<RoleClient, ElicitationClientService>>,
        Option<OAuthPersistor>,
    )> {
        let should_retry = match &initial_transport {
            PendingTransport::InProcess { .. } | PendingTransport::Stdio { .. } => false,
            PendingTransport::StreamableHttp { .. }
            | PendingTransport::StreamableHttpWithOAuth { .. } => true,
        };
        let retry_deadline = timeout.map(|duration| Instant::now() + duration);
        let mut pending_transport = Some(initial_transport);

        for (attempt, retry_delay_ms) in STREAMABLE_HTTP_RETRY_DELAYS_MS
            .iter()
            .copied()
            .map(Some)
            .chain(std::iter::once(None))
            .enumerate()
        {
            let transport = match pending_transport.take() {
                Some(transport) => transport,
                None => {
                    let remaining = remaining_initialize_timeout(timeout, retry_deadline)?;
                    match remaining {
                        Some(remaining) => time::timeout(
                            remaining,
                            Self::create_pending_transport(&self.transport_recipe),
                        )
                        .await
                        .map_err(|_| initialize_timeout_error(timeout, remaining))??,
                        None => Self::create_pending_transport(&self.transport_recipe).await?,
                    }
                }
            };
            let attempt_timeout = remaining_initialize_timeout(timeout, retry_deadline)?;

            match Self::connect_pending_transport(
                transport,
                client_service.clone(),
                attempt_timeout,
            )
            .await
            {
                Ok(result) => return Ok(result),
                Err(error) if should_retry && Self::is_retryable_initialize_error(&error) => {
                    let Some(retry_delay_ms) = retry_delay_ms else {
                        return Err(error);
                    };
                    let delay = Duration::from_millis(retry_delay_ms);
                    warn!(
                        attempt = attempt + 1,
                        max_attempts = STREAMABLE_HTTP_RETRY_DELAYS_MS.len() + 1,
                        delay_ms = delay.as_millis(),
                        error = %error,
                        "streamable HTTP MCP initialize failed with a retryable error; retrying"
                    );
                    if !sleep_with_retry_deadline(delay, retry_deadline).await {
                        let duration = timeout.unwrap_or(delay);
                        return Err(anyhow!(
                            "timed out handshaking with MCP server after {duration:?}"
                        ));
                    }
                }
                Err(error) => return Err(error),
            }
        }

        unreachable!("initialize retry loop should return on success or final error")
    }

    fn is_retryable_initialize_error(error: &anyhow::Error) -> bool {
        error.chain().any(|source| {
            source
                .downcast_ref::<HandshakeError>()
                .is_some_and(|error| Self::is_retryable_client_initialize_error(&error.source))
                || source
                    .downcast_ref::<rmcp::service::ClientInitializeError>()
                    .is_some_and(Self::is_retryable_client_initialize_error)
        })
    }

    fn is_retryable_client_initialize_error(error: &rmcp::service::ClientInitializeError) -> bool {
        match error {
            rmcp::service::ClientInitializeError::TransportError { error, context }
                if matches!(
                    context.as_ref(),
                    "send initialize request" | "send initialized notification"
                ) =>
            {
                error
                    .error
                    .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
                    .is_some_and(Self::is_retryable_streamable_http_error)
            }
            _ => false,
        }
    }

    fn is_retryable_streamable_http_error(
        error: &StreamableHttpError<StreamableHttpClientAdapterError>,
    ) -> bool {
        match error {
            StreamableHttpError::Client(StreamableHttpClientAdapterError::HttpRequest(
                ExecServerError::HttpRequest(_),
            )) => true,
            StreamableHttpError::Client(StreamableHttpClientAdapterError::HttpRequest(
                ExecServerError::Server { code, message },
            )) => {
                *code == JSON_RPC_INTERNAL_ERROR_CODE && message.starts_with("http/request failed:")
            }
            StreamableHttpError::Client(StreamableHttpClientAdapterError::HttpRequest(
                ExecServerError::Protocol(message),
            )) => message.starts_with("http response stream `") && message.contains("` failed:"),
            StreamableHttpError::UnexpectedServerResponse(message) => {
                is_retryable_unexpected_server_response(message.as_ref())
            }
            StreamableHttpError::AuthRequired(_)
            | StreamableHttpError::InsufficientScope(_)
            | StreamableHttpError::SessionExpired
            | StreamableHttpError::UnexpectedContentType(_)
            | StreamableHttpError::ServerDoesNotSupportSse
            | StreamableHttpError::Deserialize(_)
            | StreamableHttpError::Client(StreamableHttpClientAdapterError::SessionExpired404)
            | StreamableHttpError::Client(StreamableHttpClientAdapterError::Header(_)) => false,
            _ => false,
        }
    }
}

fn is_retryable_unexpected_server_response(message: &str) -> bool {
    let Some(message) = message.strip_prefix("HTTP ") else {
        return false;
    };
    let status_code = message
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    let Ok(status) = status_code.parse::<u16>() else {
        return false;
    };
    let Ok(status) = StatusCode::from_u16(status) else {
        return false;
    };
    is_retryable_http_status(status)
}

fn is_retryable_http_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn remaining_initialize_timeout(
    timeout: Option<Duration>,
    deadline: Option<Instant>,
) -> Result<Option<Duration>> {
    let Some(deadline) = deadline else {
        return Ok(None);
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        Err(initialize_timeout_error(timeout, remaining))
    } else {
        Ok(Some(remaining))
    }
}

fn initialize_timeout_error(timeout: Option<Duration>, fallback: Duration) -> anyhow::Error {
    let duration = timeout.unwrap_or(fallback);
    anyhow!("timed out handshaking with MCP server after {duration:?}")
}

async fn sleep_with_retry_deadline(delay: Duration, deadline: Option<Instant>) -> bool {
    if let Some(deadline) = deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        time::timeout(remaining, time::sleep(delay)).await.is_ok()
    } else {
        time::sleep(delay).await;
        true
    }
}

#[derive(Debug, thiserror::Error)]
#[error("handshaking with MCP server failed: {source}")]
pub(super) struct HandshakeError {
    #[source]
    pub(super) source: rmcp::service::ClientInitializeError,
}

#[cfg(test)]
#[path = "streamable_http_retry_tests.rs"]
mod tests;
