//! HTTP transport serving and cancellation for expired MCP reply sessions.
//!
//! Only a session-scoped 404 while sending a JSON-RPC reply marks the generation
//! expired and cancels its service. Ordinary request failures keep their existing
//! recovery behavior; closing an expired transport remains bounded.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use rmcp::model::JsonRpcMessage;
use rmcp::service::ClientInitializeError;
use rmcp::service::ClientLifecycleMode;
use rmcp::service::RoleClient;
use rmcp::service::RunningService;
use rmcp::service::RxJsonRpcMessage;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::Transport;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::streamable_http_client::StreamableHttpClientWorker;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::elicitation_client_service::ElicitationClientService;
use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::tool_list_changes::track_tool_list_changes;

// rmcp logs failed responses to server requests without failing the associated
// client request, so a reply 404 must explicitly close the expired service.
struct SessionExpiryTransport<T> {
    inner: T,
    session_expired: CancellationToken,
    service_cancellation: CancellationToken,
}

impl<T> Transport<RoleClient> for SessionExpiryTransport<T>
where
    T: Transport<RoleClient, Error = StreamableHttpError<StreamableHttpClientAdapterError>>
        + 'static,
{
    type Error = T::Error;

    fn send(
        &mut self,
        message: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send + 'static {
        let response = matches!(
            &message,
            JsonRpcMessage::Response(_) | JsonRpcMessage::Error(_)
        );
        let send = self.inner.send(message);
        let session_expired = self.session_expired.clone();
        let service_cancellation = self.service_cancellation.clone();
        async move {
            let result = send.await;
            if response
                && matches!(
                    &result,
                    Err(StreamableHttpError::Client(
                        StreamableHttpClientAdapterError::SessionExpired404
                    ))
                )
            {
                session_expired.cancel();
                service_cancellation.cancel();
            }
            result
        }
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        self.inner.receive().await
    }

    async fn close(&mut self) -> std::result::Result<(), Self::Error> {
        if !self.session_expired.is_cancelled() {
            return self.inner.close().await;
        }

        match time::timeout(Duration::from_secs(/*secs*/ 5), self.inner.close()).await {
            Ok(result) => result,
            Err(_) => {
                warn!("timed out closing expired MCP session");
                Ok(())
            }
        }
    }
}

#[allow(clippy::result_large_err, reason = "rmcp owns this error type")]
pub(super) async fn serve_http_transport<T>(
    service: ElicitationClientService,
    transport: T,
    revision: Arc<AtomicU64>,
    lifecycle: ClientLifecycleMode,
    session_expired: CancellationToken,
) -> std::result::Result<RunningService<RoleClient, ElicitationClientService>, ClientInitializeError>
where
    T: Transport<RoleClient, Error = StreamableHttpError<StreamableHttpClientAdapterError>>
        + 'static,
{
    let service_cancellation = CancellationToken::new();
    rmcp::service::serve_client_with_lifecycle_and_ct(
        service,
        track_tool_list_changes(
            SessionExpiryTransport {
                inner: transport,
                session_expired,
                service_cancellation: service_cancellation.clone(),
            },
            revision,
        ),
        lifecycle,
        service_cancellation,
    )
    .await
}

pub(super) fn spawn_http_transport<C: StreamableHttpClient>(
    client: C,
    config: StreamableHttpClientTransportConfig,
    cancellation: Option<CancellationToken>,
) -> StreamableHttpClientTransport<C> {
    match cancellation {
        Some(cancellation) => StreamableHttpClientTransport::spawn_with_ct(
            StreamableHttpClientWorker::new(client, config),
            cancellation,
        ),
        None => StreamableHttpClientTransport::with_client(client, config),
    }
}
