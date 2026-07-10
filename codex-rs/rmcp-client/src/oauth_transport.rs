//! Codex-owned OAuth policy for RMCP Streamable HTTP traffic.
//!
//! RMCP remains responsible for transport mechanics and bearer-token injection. Its authorization
//! manager receives only request-safe credentials, so it cannot independently refresh outside
//! Codex's serialized transaction.
//!
//! Client-originated requests retain their outer `RmcpClient` recovery, which owns caller
//! deadlines and replay decisions. RMCP-owned responses, SSE GET/reconnects, and session DELETEs
//! have no public caller; this transport reports their exact rejected token to the parent
//! `RmcpClient` and stops RMCP's unbounded SSE reconnect loop. The parent then owns any refresh
//! and session rebuild before the next public operation.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;

use oauth2::AccessToken;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::model::JsonRpcMessage;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::common::client_side_sse::ExponentialBackoff;
use rmcp::transport::common::client_side_sse::SseRetryPolicy;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;

use crate::http_client_adapter::StreamableHttpClientAdapter;
use crate::http_client_adapter::StreamableHttpClientAdapterError;

type TransportResult<T> =
    std::result::Result<T, StreamableHttpError<StreamableHttpClientAdapterError>>;

#[derive(Clone)]
pub(crate) struct OAuthTransportClient {
    auth_client: AuthClient<StreamableHttpClientAdapter>,
    failure_state: OAuthTransportFailureState,
}

impl OAuthTransportClient {
    pub(crate) fn new(
        auth_client: AuthClient<StreamableHttpClientAdapter>,
        failure_state: OAuthTransportFailureState,
    ) -> Self {
        Self {
            auth_client,
            failure_state,
        }
    }
}

/// Shared state between RMCP's bearer-only transport and Codex's OAuth session owner.
///
/// RMCP may issue GET reconnects, DELETE cleanup, and server-response POSTs outside a public
/// `RmcpClient` operation. Those requests may report which access token was rejected, but they
/// must not refresh it: Codex owns the credential transaction and transport rebuild. The state
/// also stops RMCP's unbounded SSE reconnect policy after an auth failure so it cannot repeatedly
/// re-enter this transport with a rejected token while Codex is recovering the session.
#[derive(Clone, Debug, Default)]
pub(crate) struct OAuthTransportFailureState {
    inner: Arc<OAuthTransportFailureStateInner>,
}

#[derive(Debug, Default)]
struct OAuthTransportFailureStateInner {
    pending_rejected_access_token: Mutex<Option<AccessToken>>,
}

impl OAuthTransportFailureState {
    pub(crate) fn record_rejected_access_token(&self, rejected_access_token: AccessToken) {
        *self
            .inner
            .pending_rejected_access_token
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(rejected_access_token);
    }

    pub(crate) fn pending_rejected_access_token(&self) -> Option<AccessToken> {
        self.inner
            .pending_rejected_access_token
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn finish_recovery(&self, rejected_access_token: &AccessToken) {
        let mut pending = self
            .inner
            .pending_rejected_access_token
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if pending
            .as_ref()
            .is_some_and(|pending| pending.secret() == rejected_access_token.secret())
        {
            *pending = None;
        }
    }

    pub(crate) fn retry_policy(&self) -> OAuthSseRetryPolicy {
        OAuthSseRetryPolicy {
            failure_state: self.clone(),
            fallback: ExponentialBackoff::default(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct OAuthSseRetryPolicy {
    failure_state: OAuthTransportFailureState,
    fallback: ExponentialBackoff,
}

impl SseRetryPolicy for OAuthSseRetryPolicy {
    fn retry(&self, current_times: usize) -> Option<Duration> {
        if self
            .failure_state
            .inner
            .pending_rejected_access_token
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
        {
            None
        } else {
            self.fallback.retry(current_times)
        }
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
        let result = self
            .auth_client
            .post_message(uri, message, session_id, auth_token, custom_headers)
            .await;

        // Client-originated requests retain their outer `RmcpClient` recovery boundary, which
        // owns caller deadlines and replay decisions. Server responses have no public caller, so
        // surface their rejected token to the Codex session owner instead of refreshing here.
        if is_rmcp_owned_response
            && let Some(rejected_access_token) =
                result.as_ref().err().and_then(rejected_access_token)
        {
            self.failure_state
                .record_rejected_access_token(rejected_access_token);
        }
        result
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> TransportResult<()> {
        let result = self
            .auth_client
            .delete_session(uri, session_id, auth_token, custom_headers)
            .await;
        if let Some(rejected_access_token) = result.as_ref().err().and_then(rejected_access_token) {
            self.failure_state
                .record_rejected_access_token(rejected_access_token);
        }
        result
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
        let result = self
            .auth_client
            .get_stream(uri, session_id, last_event_id, auth_token, custom_headers)
            .await;
        if let Some(rejected_access_token) = result.as_ref().err().and_then(rejected_access_token) {
            self.failure_state
                .record_rejected_access_token(rejected_access_token);
        }
        result
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

#[cfg(test)]
#[path = "oauth_transport_tests.rs"]
mod tests;
