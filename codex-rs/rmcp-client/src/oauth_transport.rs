//! Codex-owned OAuth policy for RMCP Streamable HTTP traffic.
//!
//! RMCP remains responsible for transport mechanics and bearer-token injection. Codex owns the
//! credential lifecycle: every POST, SSE GET/reconnect, and session DELETE receives proactive
//! refresh from its owning Codex layer, and each path has at most one 401 recovery. The
//! authorization manager only receives request-safe credentials, so it cannot independently
//! refresh outside Codex's serialized transaction.
//!
//! POST recovery is split at an intentional ownership boundary. Client-originated requests and
//! notifications retain their outer `RmcpClient` recovery, which knows the startup/tool deadline
//! and can avoid replaying a request after its caller timed out. RMCP-owned responses to
//! server-initiated requests have no such outer operation, so they recover here. GET/reconnect and
//! DELETE are always RMCP-owned and also recover here.

use std::collections::HashMap;
use std::sync::Arc;

use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::JsonRpcMessage;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use tracing::debug;

use crate::http_client_adapter::StreamableHttpClientAdapter;
use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::oauth::OAuthPersistor;

type TransportResult<T> =
    std::result::Result<T, StreamableHttpError<StreamableHttpClientAdapterError>>;

#[derive(Clone)]
pub(crate) struct OAuthTransportClient {
    auth_client: AuthClient<StreamableHttpClientAdapter>,
    persistor: OAuthPersistor,
}

impl OAuthTransportClient {
    pub(crate) fn new(
        auth_client: AuthClient<StreamableHttpClientAdapter>,
        persistor: OAuthPersistor,
    ) -> Self {
        Self {
            auth_client,
            persistor,
        }
    }

    pub(crate) fn persistor(&self) -> OAuthPersistor {
        self.persistor.clone()
    }

    async fn preflight(&self, operation: &'static str) -> TransportResult<()> {
        debug!(
            operation,
            "checking MCP OAuth credentials before transport request"
        );
        self.persistor
            .refresh_if_needed()
            .await
            .map_err(oauth_transport_error)
    }

    async fn recover_after_unauthorized(
        &self,
        operation: &'static str,
        rejected_access_token: Option<oauth2::AccessToken>,
    ) -> TransportResult<bool> {
        let Some(rejected_access_token) = rejected_access_token else {
            return Ok(false);
        };

        debug!(
            operation,
            "recovering once after MCP transport rejected an OAuth access token"
        );
        self.persistor
            .refresh_after_unauthorized(rejected_access_token)
            .await
            .map_err(oauth_transport_error)?;
        Ok(true)
    }
}

impl StreamableHttpClient for OAuthTransportClient {
    type Error = StreamableHttpClientAdapterError;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> TransportResult<StreamableHttpPostResponse> {
        let is_rmcp_owned_response = matches!(
            message,
            JsonRpcMessage::Response(_) | JsonRpcMessage::Error(_)
        );
        if is_rmcp_owned_response {
            self.preflight("post_message").await?;
        }
        let result = self
            .auth_client
            .post_message(
                Arc::clone(&uri),
                message.clone(),
                session_id.clone(),
                auth_token.clone(),
                custom_headers.clone(),
            )
            .await;

        // RMCP queues client-originated requests independently of the caller waiting on them. If
        // recovery happened here, a timed-out public tool call could still be replayed after its
        // refresh finished. The outer RmcpClient path owns those deadlines. Responses to
        // server-initiated requests have no outer operation and therefore recover here.
        if !is_rmcp_owned_response {
            return result;
        }
        let rejected_access_token = result.as_ref().err().and_then(rejected_access_token);
        if self
            .recover_after_unauthorized("post_message", rejected_access_token)
            .await?
        {
            self.auth_client
                .post_message(uri, message, session_id, auth_token, custom_headers)
                .await
        } else {
            result
        }
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> TransportResult<()> {
        self.preflight("delete_session").await?;
        let result = self
            .auth_client
            .delete_session(
                Arc::clone(&uri),
                Arc::clone(&session_id),
                auth_token.clone(),
                custom_headers.clone(),
            )
            .await;
        let rejected_access_token = result.as_ref().err().and_then(rejected_access_token);
        if self
            .recover_after_unauthorized("delete_session", rejected_access_token)
            .await?
        {
            self.auth_client
                .delete_session(uri, session_id, auth_token, custom_headers)
                .await
        } else {
            result
        }
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> TransportResult<
        futures::stream::BoxStream<'static, Result<sse_stream::Sse, sse_stream::Error>>,
    > {
        self.preflight("get_stream").await?;
        let result = self
            .auth_client
            .get_stream(
                Arc::clone(&uri),
                Arc::clone(&session_id),
                last_event_id.clone(),
                auth_token.clone(),
                custom_headers.clone(),
            )
            .await;
        let rejected_access_token = result.as_ref().err().and_then(rejected_access_token);
        if self
            .recover_after_unauthorized("get_stream", rejected_access_token)
            .await?
        {
            self.auth_client
                .get_stream(uri, session_id, last_event_id, auth_token, custom_headers)
                .await
        } else {
            result
        }
    }
}

fn rejected_access_token(
    error: &StreamableHttpError<StreamableHttpClientAdapterError>,
) -> Option<oauth2::AccessToken> {
    match error {
        StreamableHttpError::Client(StreamableHttpClientAdapterError::AccessTokenRejected {
            rejected_access_token,
        }) => Some(rejected_access_token.clone()),
        _ => None,
    }
}

fn oauth_transport_error(
    error: anyhow::Error,
) -> StreamableHttpError<StreamableHttpClientAdapterError> {
    StreamableHttpError::Client(StreamableHttpClientAdapterError::OAuth(error))
}

#[cfg(test)]
#[path = "oauth_transport_tests.rs"]
mod tests;
