use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use anyhow::anyhow;
use codex_exec_server::ExecServerError;
use oauth2::AccessToken;
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
pub(super) const STREAMABLE_HTTP_RETRY_DELAYS_MS: [u64; 2] = [250, 1_000];

#[derive(Default)]
struct InitializeAttemptContext {
    oauth_persistor: Option<OAuthPersistor>,
}

impl RmcpClient {
    pub(super) async fn connect_pending_transport_with_oauth_recovery(
        &self,
        initial_transport: PendingTransport,
        client_service: ElicitationClientService,
        timeout: Option<Duration>,
        initialize_deadline: &mut Option<Instant>,
    ) -> Result<(
        Arc<RunningService<RoleClient, ElicitationClientService>>,
        Option<OAuthPersistor>,
    )> {
        let mut attempt_context = InitializeAttemptContext::default();
        match self
            .connect_pending_transport_with_initialize_retries(
                initial_transport,
                client_service.clone(),
                timeout,
                initialize_deadline,
                &mut attempt_context,
            )
            .await
        {
            Ok(result) => Ok(result),
            Err(error) => {
                let Some(rejected_access_token) =
                    Self::rejected_access_token_from_initialize_error(&error)
                else {
                    return Err(error);
                };
                let Some(oauth_persistor) = attempt_context.oauth_persistor else {
                    return Err(error);
                };
                // Initialization gets one OAuth refresh and one reconstructed transport. Reusing
                // this wrapper for the retry would turn persistent 401s into a refresh loop. The
                // startup deadline gates whether recovery starts and bounds transport setup plus
                // the retry handshake, but the refresh transaction has its own bounds and is
                // deliberately excluded from the startup budget.
                remaining_initialize_timeout(timeout, *initialize_deadline)?;
                let refresh_started_at = Instant::now();
                let refresh_result = oauth_persistor
                    .refresh_after_unauthorized(rejected_access_token)
                    .await;
                if let Some(deadline) = initialize_deadline.as_mut() {
                    *deadline += refresh_started_at.elapsed();
                }
                refresh_result?;
                let remaining = remaining_initialize_timeout(timeout, *initialize_deadline)?;
                let transport = match remaining {
                    Some(remaining) => time::timeout(
                        remaining,
                        Self::create_pending_transport(&self.transport_recipe),
                    )
                    .await
                    .map_err(|_| initialize_timeout_error(timeout, remaining))??,
                    None => Self::create_pending_transport(&self.transport_recipe).await?,
                };
                let mut retry_context = InitializeAttemptContext::default();
                self.connect_pending_transport_with_initialize_retries(
                    transport,
                    client_service,
                    timeout,
                    initialize_deadline,
                    &mut retry_context,
                )
                .await
            }
        }
    }

    async fn connect_pending_transport_with_initialize_retries(
        &self,
        initial_transport: PendingTransport,
        client_service: ElicitationClientService,
        timeout: Option<Duration>,
        initialize_deadline: &mut Option<Instant>,
        attempt_context: &mut InitializeAttemptContext,
    ) -> Result<(
        Arc<RunningService<RoleClient, ElicitationClientService>>,
        Option<OAuthPersistor>,
    )> {
        let should_retry = match &initial_transport {
            PendingTransport::InProcess { .. } | PendingTransport::Stdio { .. } => false,
            PendingTransport::StreamableHttp { .. }
            | PendingTransport::StreamableHttpWithOAuth { .. } => true,
        };
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
                    let remaining = remaining_initialize_timeout(timeout, *initialize_deadline)?;
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
            // Keep the persistor paired with the transport attempt that returned 401. Rebuilt
            // transports reuse the recipe's lifecycle-pinned credential source, and this pairing
            // also keeps the authorization manager and snapshot aligned with the failed attempt.
            attempt_context.oauth_persistor = match &transport {
                PendingTransport::StreamableHttpWithOAuth {
                    oauth_persistor, ..
                } => Some(oauth_persistor.clone()),
                PendingTransport::InProcess { .. }
                | PendingTransport::Stdio { .. }
                | PendingTransport::StreamableHttp { .. } => None,
            };
            match Self::connect_pending_transport(
                transport,
                client_service.clone(),
                timeout,
                initialize_deadline,
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
                    if !sleep_with_retry_deadline(delay, *initialize_deadline).await {
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

    fn rejected_access_token_from_initialize_error(error: &anyhow::Error) -> Option<AccessToken> {
        error.chain().find_map(|source| {
            source
                .downcast_ref::<HandshakeError>()
                .and_then(|error| {
                    Self::rejected_access_token_from_client_initialize_error(&error.source)
                })
                .or_else(|| {
                    source
                        .downcast_ref::<rmcp::service::ClientInitializeError>()
                        .and_then(Self::rejected_access_token_from_client_initialize_error)
                })
        })
    }

    fn rejected_access_token_from_client_initialize_error(
        error: &rmcp::service::ClientInitializeError,
    ) -> Option<AccessToken> {
        match error {
            rmcp::service::ClientInitializeError::TransportError { error, .. } => error
                .error
                .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
                .and_then(Self::rejected_access_token),
            _ => None,
        }
    }

    fn is_retryable_client_initialize_error(error: &rmcp::service::ClientInitializeError) -> bool {
        match error {
            rmcp::service::ClientInitializeError::TransportError { error, context }
                if context.as_ref() == "send initialize request" =>
            {
                error
                    .error
                    .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
                    .is_some_and(Self::is_retryable_streamable_http_error)
            }
            rmcp::service::ClientInitializeError::TransportError { error, context }
                if context.as_ref() == "send initialized notification" =>
            {
                error
                    .error
                    .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
                    .is_some_and(|error| {
                        matches!(error, StreamableHttpError::TransportChannelClosed)
                            || Self::is_retryable_streamable_http_error(error)
                    })
            }
            _ => false,
        }
    }

    pub(super) fn is_retryable_streamable_http_error(
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
            | StreamableHttpError::Client(
                StreamableHttpClientAdapterError::AccessTokenRejected { .. },
            )
            | StreamableHttpError::Client(StreamableHttpClientAdapterError::OAuth(_))
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

pub(super) fn remaining_initialize_timeout(
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

pub(super) fn initialize_timeout_error(
    timeout: Option<Duration>,
    fallback: Duration,
) -> anyhow::Error {
    let duration = timeout.unwrap_or(fallback);
    anyhow!("timed out handshaking with MCP server after {duration:?}")
}

pub(super) async fn sleep_with_retry_deadline(delay: Duration, deadline: Option<Instant>) -> bool {
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
