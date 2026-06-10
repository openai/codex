use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;

use crate::ExecServerClient;
use crate::ExecServerError;
use crate::HttpClient;
use crate::client_api::ExecServerTransportParams;
use crate::protocol::EnvironmentInfo;

pub(crate) type RemoteWebSocketUrlProvider =
    Arc<dyn Fn() -> BoxFuture<'static, Result<String, ExecServerError>> + Send + Sync>;

#[derive(Clone)]
pub(crate) enum RemoteConnectionSource {
    Fixed(ExecServerTransportParams),
    RefreshingWebSocket(RemoteWebSocketUrlProvider),
}

impl RemoteConnectionSource {
    pub(crate) fn configured_url(&self) -> Option<&str> {
        match self {
            Self::Fixed(ExecServerTransportParams::WebSocketUrl { websocket_url, .. }) => {
                Some(websocket_url)
            }
            Self::Fixed(ExecServerTransportParams::StdioCommand { .. })
            | Self::RefreshingWebSocket(_) => None,
        }
    }

    fn is_reconnectable(&self) -> bool {
        matches!(
            self,
            Self::Fixed(ExecServerTransportParams::WebSocketUrl { .. })
                | Self::RefreshingWebSocket(_)
        )
    }

    async fn connect(&self) -> Result<ExecServerClient, ExecServerError> {
        let transport_params = match self {
            Self::Fixed(transport_params) => transport_params.clone(),
            Self::RefreshingWebSocket(provider) => {
                ExecServerTransportParams::websocket_url(provider().await?)
            }
        };
        ExecServerClient::connect_for_transport(transport_params).await
    }
}

#[derive(Clone)]
pub(crate) struct LazyRemoteExecServerClient {
    source: RemoteConnectionSource,
    state: Arc<StdMutex<ConnectionState>>,
}

#[derive(Default)]
struct ConnectionState {
    client: Option<ExecServerClient>,
    in_flight: Option<InFlightConnection>,
    next_attempt_id: u64,
}

struct InFlightConnection {
    attempt_id: u64,
    attempt: SharedConnectionAttempt,
}

type SharedConnectionAttempt =
    Shared<BoxFuture<'static, Result<ExecServerClient, ExecServerError>>>;

impl LazyRemoteExecServerClient {
    pub(crate) fn new(source: RemoteConnectionSource) -> Self {
        Self {
            source,
            state: Arc::new(StdMutex::new(ConnectionState::default())),
        }
    }

    pub(crate) async fn get(&self) -> Result<ExecServerClient, ExecServerError> {
        let (attempt_id, attempt) = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(client) = state.client.as_ref()
                && (!client.is_disconnected() || !self.source.is_reconnectable())
            {
                return Ok(client.clone());
            }

            if let Some(in_flight) = state.in_flight.as_ref() {
                (in_flight.attempt_id, in_flight.attempt.clone())
            } else {
                let attempt_id = state.next_attempt_id;
                state.next_attempt_id = state.next_attempt_id.wrapping_add(1);
                let source = self.source.clone();
                let attempt = async move { source.connect().await }.boxed().shared();
                state.in_flight = Some(InFlightConnection {
                    attempt_id,
                    attempt: attempt.clone(),
                });
                (attempt_id, attempt)
            }
        };

        let result = attempt.await;
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state
                .in_flight
                .as_ref()
                .is_some_and(|in_flight| in_flight.attempt_id == attempt_id)
            {
                if let Ok(connected_client) = &result {
                    state.client = Some(connected_client.clone());
                }
                state.in_flight = None;
            }
        }
        result
    }

    pub(crate) async fn environment_info(&self) -> Result<EnvironmentInfo, ExecServerError> {
        self.get().await?.environment_info().await
    }
}

impl HttpClient for LazyRemoteExecServerClient {
    fn http_request(
        &self,
        params: crate::HttpRequestParams,
    ) -> BoxFuture<'_, Result<crate::HttpRequestResponse, ExecServerError>> {
        async move { self.get().await?.http_request(params).await }.boxed()
    }

    fn http_request_stream(
        &self,
        params: crate::HttpRequestParams,
    ) -> BoxFuture<
        '_,
        Result<(crate::HttpRequestResponse, crate::HttpResponseBodyStream), ExecServerError>,
    > {
        async move { self.get().await?.http_request_stream(params).await }.boxed()
    }
}

#[cfg(test)]
#[path = "lazy_remote_tests.rs"]
mod tests;
