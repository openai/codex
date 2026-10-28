//! Establishes Responses sockets with caller-owned TLS and proxy configuration.
//! Connector reuse never caches authentication or bypasses per-connection destination policy.

use super::ResponsesWebsocketClient;
use super::ResponsesWebsocketConnection;
use super::connect_websocket;
use super::merge_request_headers;
use crate::ApiError;
use crate::RESPONSES_PATH;
use crate::WebsocketTelemetry;
use codex_websocket_client::WebSocketConnector;
use http::HeaderMap;
use std::sync::Arc;
use std::sync::OnceLock;

impl ResponsesWebsocketClient {
    /// Opens a socket using the caller's reusable connector and this client's current auth.
    /// The caller owns the lifetime of the connector's TLS configuration snapshot.
    #[tracing::instrument(
        name = "responses_websocket.connect",
        level = "info",
        skip_all,
        fields(transport = "responses_websocket", api.path = "/responses")
    )]
    pub async fn connect_with_connector(
        &self,
        connector: &WebSocketConnector,
        extra_headers: HeaderMap,
        default_headers: HeaderMap,
        turn_state: Option<Arc<OnceLock<String>>>,
        telemetry: Option<Arc<dyn WebsocketTelemetry>>,
    ) -> Result<ResponsesWebsocketConnection, ApiError> {
        let ws_url = self
            .provider
            .websocket_url_for_path(RESPONSES_PATH)
            .map_err(|err| ApiError::Stream(format!("failed to build websocket URL: {err}")))?;
        let headers = merge_request_headers(&self.provider.headers, extra_headers, default_headers);
        let headers = self.auth.apply_auth_headers(headers).await?;
        let (stream, _response, server_reasoning_included, server_model) =
            connect_websocket(ws_url, headers, connector, turn_state)
                .await
                .map_err(|failure| *failure.error)?;
        Ok(ResponsesWebsocketConnection::new(
            stream,
            self.provider.stream_idle_timeout,
            server_reasoning_included,
            server_model,
            telemetry,
        ))
    }
}
