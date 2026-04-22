use std::sync::Arc;

use anyhow::Result;
use codex_exec_server::ExecServerClient;
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use reqwest::header::HeaderMap;
use rmcp::transport::streamable_http_client::StreamableHttpClient;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
use sse_stream::Sse;

use crate::streamable_http_local_client::LocalStreamableHttpClient;
use crate::streamable_http_remote_client::RemoteStreamableHttpClient;
use crate::streamable_http_remote_client::RemoteStreamableHttpClientError;

#[derive(Clone)]
pub(crate) struct StreamableHttpTransportClient {
    backend: Arc<dyn StreamableHttpTransportBackend>,
}

#[derive(Clone)]
impl StreamableHttpTransportClient {
    pub(crate) fn new(
        transport_mode: StreamableHttpTransportMode,
        default_headers: HeaderMap,
    ) -> Result<Self> {
        let backend: Arc<dyn StreamableHttpTransportBackend> = match transport_mode {
            StreamableHttpTransportMode::Local => {
                Arc::new(LocalStreamableHttpClient::new(default_headers)?)
            }
            StreamableHttpTransportMode::Remote { exec_client } => Arc::new(
                RemoteStreamableHttpClient::new(exec_client, default_headers),
            ),
        };
        Ok(Self { backend })
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StreamableHttpTransportClientError {
    #[error("streamable HTTP session expired with 404 Not Found")]
    SessionExpired404,
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Remote(#[from] RemoteStreamableHttpClientError),
}

#[derive(Clone)]
pub(crate) enum StreamableHttpTransportMode {
    Local,
    Remote { exec_client: ExecServerClient },
}

/// Backend interface for local and remote Streamable HTTP execution.
///
/// Implementations own the request mechanics, while
/// `StreamableHttpTransportClient` remains the shared transport-facing handle
/// used by RMCP setup and OAuth wiring.
pub(crate) trait StreamableHttpTransportBackend: Send + Sync {
    fn post_message(
        &self,
        uri: Arc<str>,
        message: rmcp::model::ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
    ) -> BoxFuture<
        '_,
        std::result::Result<
            StreamableHttpPostResponse,
            StreamableHttpError<StreamableHttpTransportClientError>,
        >,
    >;

    fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth_token: Option<String>,
    ) -> BoxFuture<
        '_,
        std::result::Result<(), StreamableHttpError<StreamableHttpTransportClientError>>,
    >;

    fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
    ) -> BoxFuture<
        '_,
        std::result::Result<
            BoxStream<'static, std::result::Result<Sse, sse_stream::Error>>,
            StreamableHttpError<StreamableHttpTransportClientError>,
        >,
    >;
}

impl StreamableHttpTransportBackend for RemoteStreamableHttpClient {
    fn post_message(
        &self,
        uri: Arc<str>,
        message: rmcp::model::ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
    ) -> BoxFuture<
        '_,
        std::result::Result<
            StreamableHttpPostResponse,
            StreamableHttpError<StreamableHttpTransportClientError>,
        >,
    > {
        async move {
            StreamableHttpClient::post_message(self, uri, message, session_id, auth_token)
                .await
                .map_err(|error| {
                    map_streamable_http_error(error, StreamableHttpTransportClientError::Remote)
                })
        }
        .boxed()
    }

    fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth_token: Option<String>,
    ) -> BoxFuture<
        '_,
        std::result::Result<(), StreamableHttpError<StreamableHttpTransportClientError>>,
    > {
        async move {
            StreamableHttpClient::delete_session(self, uri, session, auth_token)
                .await
                .map_err(|error| {
                    map_streamable_http_error(error, StreamableHttpTransportClientError::Remote)
                })
        }
        .boxed()
    }

    fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
    ) -> BoxFuture<
        '_,
        std::result::Result<
            BoxStream<'static, std::result::Result<Sse, sse_stream::Error>>,
            StreamableHttpError<StreamableHttpTransportClientError>,
        >,
    > {
        async move {
            StreamableHttpClient::get_stream(self, uri, session_id, last_event_id, auth_token)
                .await
                .map_err(|error| {
                    map_streamable_http_error(error, StreamableHttpTransportClientError::Remote)
                })
        }
        .boxed()
    }
}

impl StreamableHttpClient for StreamableHttpTransportClient {
    type Error = StreamableHttpTransportClientError;

    async fn post_message(
        &self,
        uri: Arc<str>,
        message: rmcp::model::ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_token: Option<String>,
    ) -> std::result::Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>> {
        self.backend
            .post_message(uri, message, session_id, auth_token)
            .await
    }

    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth_token: Option<String>,
    ) -> std::result::Result<(), StreamableHttpError<Self::Error>> {
        self.backend.delete_session(uri, session, auth_token).await
    }

    async fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_token: Option<String>,
    ) -> std::result::Result<
        BoxStream<'static, std::result::Result<Sse, sse_stream::Error>>,
        StreamableHttpError<Self::Error>,
    > {
        self.backend
            .get_stream(uri, session_id, last_event_id, auth_token)
            .await
    }
}

pub(crate) fn map_streamable_http_error<FromError, ToError>(
    error: StreamableHttpError<FromError>,
    map_client_error: fn(FromError) -> ToError,
) -> StreamableHttpError<ToError>
where
    FromError: std::error::Error + Send + Sync + 'static,
    ToError: std::error::Error + Send + Sync + 'static,
{
    match error {
        StreamableHttpError::Sse(error) => StreamableHttpError::Sse(error),
        StreamableHttpError::Io(error) => StreamableHttpError::Io(error),
        StreamableHttpError::Client(error) => StreamableHttpError::Client(map_client_error(error)),
        StreamableHttpError::UnexpectedEndOfStream => StreamableHttpError::UnexpectedEndOfStream,
        StreamableHttpError::UnexpectedServerResponse(error) => {
            StreamableHttpError::UnexpectedServerResponse(error)
        }
        StreamableHttpError::UnexpectedContentType(error) => {
            StreamableHttpError::UnexpectedContentType(error)
        }
        StreamableHttpError::ServerDoesNotSupportSse => {
            StreamableHttpError::ServerDoesNotSupportSse
        }
        StreamableHttpError::ServerDoesNotSupportDeleteSession => {
            StreamableHttpError::ServerDoesNotSupportDeleteSession
        }
        StreamableHttpError::TokioJoinError(error) => StreamableHttpError::TokioJoinError(error),
        StreamableHttpError::Deserialize(error) => StreamableHttpError::Deserialize(error),
        StreamableHttpError::TransportChannelClosed => StreamableHttpError::TransportChannelClosed,
        StreamableHttpError::MissingSessionIdInResponse => {
            StreamableHttpError::MissingSessionIdInResponse
        }
        StreamableHttpError::Auth(error) => StreamableHttpError::Auth(error),
        StreamableHttpError::AuthRequired(error) => StreamableHttpError::AuthRequired(error),
    }
}
