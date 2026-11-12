//! Keeps MCP HTTP requests cancellable while OAuth exchanges have an independent owner.
//! Session cleanup uses only cached access credentials and never rotates a refresh token.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use futures::stream::BoxStream;
use http::HeaderName;
use http::HeaderValue;
use rmcp::model::ClientJsonRpcMessage;
use rmcp::transport::auth::AuthError;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use sse_stream::Sse;

use super::OAuthRuntime;
use crate::http_client_adapter::StreamableHttpClientAdapter;
use crate::http_client_adapter::StreamableHttpClientAdapterError;

#[derive(Clone)]
pub(crate) struct OAuthStreamableHttpClient {
    http_client: StreamableHttpClientAdapter,
    runtime: OAuthRuntime,
}

impl OAuthStreamableHttpClient {
    pub(crate) fn new(http_client: StreamableHttpClientAdapter, runtime: OAuthRuntime) -> Self {
        Self {
            http_client,
            runtime,
        }
    }

    async fn call_with_auth<T, F, Fut>(
        &self,
        auth_token: Option<String>,
        call: F,
    ) -> Result<T, StreamableHttpError<StreamableHttpClientAdapterError>>
    where
        F: Fn(Option<String>) -> Fut,
        Fut: Future<Output = Result<T, StreamableHttpError<StreamableHttpClientAdapterError>>>,
    {
        // Match RMCP AuthClient's reactive behavior, with owned authentication tasks.
        let auth_token = match auth_token {
            None => match self.runtime.get_access_token().await {
                Ok(token) => Some(token),
                Err(AuthError::AuthorizationRequired) => None,
                Err(error) => return Err(error.into()),
            },
            token => token,
        };
        match call(auth_token.clone()).await {
            Err(StreamableHttpError::AuthRequired(challenge)) => {
                let Some(sent_token) = auth_token else {
                    return Err(StreamableHttpError::AuthRequired(challenge));
                };
                match self.runtime.refresh_access_token(&sent_token).await {
                    Ok(fresh_token) if fresh_token != sent_token => call(Some(fresh_token)).await,
                    Ok(_) | Err(AuthError::AuthorizationRequired) => {
                        Err(StreamableHttpError::AuthRequired(challenge))
                    }
                    Err(error) => Err(error.into()),
                }
            }
            result => result,
        }
    }
}

impl StreamableHttpClient for OAuthStreamableHttpClient {
    type Error = StreamableHttpClientAdapterError;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.call_with_auth(auth_token, |token| {
            self.http_client.post_message(
                uri.clone(),
                message.clone(),
                session_id.clone(),
                token,
                custom_headers.clone(),
            )
        })
        .await
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Option<Arc<str>>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<Sse, sse_stream::Error>>, StreamableHttpError<Self::Error>>
    {
        self.call_with_auth(auth_token, |token| {
            self.http_client.get_stream(
                uri.clone(),
                session_id.clone(),
                last_event_id.clone(),
                token,
                custom_headers.clone(),
            )
        })
        .await
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_token: Option<String>,
        custom_headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), StreamableHttpError<Self::Error>> {
        let token = match auth_token {
            Some(token) => Some(token),
            None => self.runtime.cleanup_access_token().await,
        };
        self.http_client
            .delete_session(uri, session_id, token, custom_headers)
            .await
    }
}
