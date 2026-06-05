use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use anyhow::anyhow;
use codex_exec_server::ExecServerError;
use rmcp::service::RoleClient;
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use tokio::time;
use tracing::warn;

use crate::elicitation_client_service::ElicitationClientService;
use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::oauth::OAuthPersistor;

use super::ElicitationPauseState;
use super::PendingTransport;
use super::RmcpClient;
use super::active_time_timeout;

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

        let retry_schedule = STREAMABLE_HTTP_RETRY_DELAYS_MS
            .iter()
            .copied()
            .map(Some)
            .chain(std::iter::once(None));

        for (attempt, retry_delay_ms) in retry_schedule.enumerate() {
            let attempt_count = attempt + 1;
            let attempt_timeout =
                retry_deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
            if let Some(remaining) = attempt_timeout
                && remaining.is_zero()
            {
                let duration = timeout.unwrap_or(remaining);
                return Err(anyhow!(
                    "timed out handshaking with MCP server after {duration:?}"
                ));
            }

            let transport = match pending_transport.take() {
                Some(transport) => transport,
                None => match Self::create_pending_transport(&self.transport_recipe).await {
                    Ok(transport) => transport,
                    Err(error) => return Err(error),
                },
            };

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
                        attempt = attempt_count,
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

    pub(super) async fn run_service_operation<T, F, Fut>(
        &self,
        label: &str,
        timeout: Option<Duration>,
        operation: F,
    ) -> Result<T>
    where
        F: Fn(Arc<RunningService<RoleClient, ElicitationClientService>>) -> Fut,
        Fut: Future<Output = std::result::Result<T, rmcp::service::ServiceError>>,
    {
        let mut session_recovery_attempted = false;
        let mut retry_attempt = 0;
        let retry_deadline = timeout.map(|duration| Instant::now() + duration);

        loop {
            let service = self.service().await?;
            let attempt_timeout =
                retry_deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
            if let Some(remaining) = attempt_timeout
                && remaining.is_zero()
            {
                let duration = timeout.unwrap_or(remaining);
                return Err(ClientOperationError::Timeout {
                    label: label.to_string(),
                    duration,
                }
                .into());
            }

            match Self::run_service_operation_once(
                Arc::clone(&service),
                label,
                attempt_timeout,
                self.elicitation_pause_state.clone(),
                &operation,
            )
            .await
            {
                Ok(result) => return Ok(result),
                Err(error)
                    if !session_recovery_attempted && Self::is_session_expired_404(&error) =>
                {
                    session_recovery_attempted = true;
                    self.reinitialize_after_session_expiry(&service).await?;
                }
                Err(error)
                    if Self::should_retry_tools_list_operation(label, retry_attempt, &error) =>
                {
                    let delay =
                        Duration::from_millis(STREAMABLE_HTTP_RETRY_DELAYS_MS[retry_attempt]);
                    retry_attempt += 1;
                    warn!(
                        label,
                        attempt = retry_attempt,
                        max_attempts = STREAMABLE_HTTP_RETRY_DELAYS_MS.len() + 1,
                        delay_ms = delay.as_millis(),
                        error = %error,
                        "MCP service operation failed with a retryable error; retrying"
                    );
                    if !sleep_with_retry_deadline(delay, retry_deadline).await {
                        let duration = timeout.unwrap_or(delay);
                        return Err(ClientOperationError::Timeout {
                            label: label.to_string(),
                            duration,
                        }
                        .into());
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    async fn run_service_operation_once<T, F, Fut>(
        service: Arc<RunningService<RoleClient, ElicitationClientService>>,
        label: &str,
        timeout: Option<Duration>,
        pause_state: ElicitationPauseState,
        operation: &F,
    ) -> std::result::Result<T, ClientOperationError>
    where
        F: Fn(Arc<RunningService<RoleClient, ElicitationClientService>>) -> Fut,
        Fut: Future<Output = std::result::Result<T, rmcp::service::ServiceError>>,
    {
        match timeout {
            Some(duration) => {
                active_time_timeout(duration, pause_state.subscribe(), operation(service))
                    .await
                    .map_err(|_| ClientOperationError::Timeout {
                        label: label.to_string(),
                        duration,
                    })?
                    .map_err(ClientOperationError::from)
            }
            None => operation(service).await.map_err(ClientOperationError::from),
        }
    }

    fn is_session_expired_404(error: &ClientOperationError) -> bool {
        let ClientOperationError::Service(rmcp::service::ServiceError::TransportSend(error)) =
            error
        else {
            return false;
        };

        error
            .error
            .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
            .is_some_and(|error| {
                matches!(
                    error,
                    StreamableHttpError::Client(
                        StreamableHttpClientAdapterError::SessionExpired404
                    )
                )
            })
    }

    fn should_retry_tools_list_operation(
        label: &str,
        retry_attempt: usize,
        error: &ClientOperationError,
    ) -> bool {
        label == "tools/list"
            && retry_attempt < STREAMABLE_HTTP_RETRY_DELAYS_MS.len()
            && Self::is_retryable_service_operation_error(error)
    }

    fn is_retryable_service_operation_error(error: &ClientOperationError) -> bool {
        let ClientOperationError::Service(rmcp::service::ServiceError::TransportSend(error)) =
            error
        else {
            return false;
        };

        error
            .error
            .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
            .is_some_and(Self::is_retryable_streamable_http_error)
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
            StreamableHttpError::Client(
                StreamableHttpClientAdapterError::RetryableHttpStatus(_)
                | StreamableHttpClientAdapterError::HttpRequest(ExecServerError::HttpRequest(_)),
            ) => true,
            StreamableHttpError::Client(StreamableHttpClientAdapterError::HttpRequest(
                ExecServerError::Server { code, message },
            )) => {
                *code == JSON_RPC_INTERNAL_ERROR_CODE && message.starts_with("http/request failed:")
            }
            StreamableHttpError::Client(StreamableHttpClientAdapterError::HttpRequest(
                ExecServerError::Protocol(message),
            )) => message.starts_with("http response stream `") && message.contains("` failed:"),
            _ => false,
        }
    }
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
enum ClientOperationError {
    #[error(transparent)]
    Service(#[from] rmcp::service::ServiceError),
    #[error("timed out awaiting {label} after {duration:?}")]
    Timeout { label: String, duration: Duration },
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
