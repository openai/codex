use std::{borrow::Cow, sync::Arc, time::Duration};

use futures::{Stream, StreamExt, future::BoxFuture, stream::BoxStream};
pub use sse_stream::Error as SseError;
use sse_stream::Sse;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use super::common::client_side_sse::{ExponentialBackoff, SseRetryPolicy, SseStreamReconnect};
use crate::{
    RoleClient,
    model::{ClientJsonRpcMessage, ServerJsonRpcMessage},
    transport::{
        common::client_side_sse::SseAutoReconnectStream,
        worker::{Worker, WorkerQuitReason, WorkerSendRequest, WorkerTransport},
    },
};

type BoxedSseStream = BoxStream<'static, Result<Sse, SseError>>;

#[derive(Debug)]
pub struct AuthRequiredError {
    pub www_authenticate_header: String,
}

#[derive(Error, Debug)]
pub enum StreamableHttpError<E: std::error::Error + Send + Sync + 'static> {
    #[error("SSE error: {0}")]
    Sse(#[from] SseError),
    #[error("Io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Client error: {0}")]
    Client(E),
    #[error("unexpected end of stream")]
    UnexpectedEndOfStream,
    #[error("unexpected server response: {0}")]
    UnexpectedServerResponse(Cow<'static, str>),
    #[error("Unexpected content type: {0:?}")]
    UnexpectedContentType(Option<String>),
    #[error("Server does not support SSE")]
    ServerDoesNotSupportSse,
    #[error("Server does not support delete session")]
    ServerDoesNotSupportDeleteSession,
    #[error("Tokio join error: {0}")]
    TokioJoinError(#[from] tokio::task::JoinError),
    #[error("Deserialize error: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error("Transport channel closed")]
    TransportChannelClosed,
    #[error("Missing session id in HTTP response")]
    MissingSessionIdInResponse,
    #[cfg(feature = "auth")]
    #[cfg_attr(docsrs, doc(cfg(feature = "auth")))]
    #[error("Auth error: {0}")]
    Auth(#[from] crate::transport::auth::AuthError),
    #[error("Auth required")]
    AuthRequired(AuthRequiredError),
}

#[derive(Debug, Clone, Error)]
pub enum StreamableHttpProtocolError {
    #[error("Missing session id in response")]
    MissingSessionIdInResponse,
}

#[allow(clippy::large_enum_variant)]
pub enum StreamableHttpPostResponse {
    Accepted,
    Json(ServerJsonRpcMessage, Option<String>),
    Sse(BoxedSseStream, Option<String>),
}

impl std::fmt::Debug for StreamableHttpPostResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accepted => write!(f, "Accepted"),
            Self::Json(arg0, arg1) => f.debug_tuple("Json").field(arg0).field(arg1).finish(),
            Self::Sse(_, arg1) => f.debug_tuple("Sse").field(arg1).finish(),
        }
    }
}

impl StreamableHttpPostResponse {
    pub async fn expect_initialized<E>(
        self,
    ) -> Result<(ServerJsonRpcMessage, Option<String>), StreamableHttpError<E>>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        match self {
            Self::Json(message, session_id) => Ok((message, session_id)),
            Self::Sse(mut stream, session_id) => {
                while let Some(event) = stream.next().await {
                    let event = event?;
                    let payload = event.data.unwrap_or_default();
                    if payload.trim().is_empty() {
                        continue;
                    }

                    let message: ServerJsonRpcMessage = serde_json::from_str(&payload)?;

                    if matches!(message, ServerJsonRpcMessage::Response(_)) {
                        return Ok((message, session_id));
                    }

                    debug!(
                        ?message,
                        "received message before initialize response; continuing to drain stream"
                    );
                }

                Err(StreamableHttpError::UnexpectedServerResponse(
                    "empty sse stream".into(),
                ))
            }
            _ => Err(StreamableHttpError::UnexpectedServerResponse(
                "expect initialized, accepted".into(),
            )),
        }
    }

    pub fn expect_json<E>(self) -> Result<ServerJsonRpcMessage, StreamableHttpError<E>>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        match self {
            Self::Json(message, ..) => Ok(message),
            got => Err(StreamableHttpError::UnexpectedServerResponse(
                format!("expect json, got {got:?}").into(),
            )),
        }
    }

    pub fn expect_accepted<E>(self) -> Result<(), StreamableHttpError<E>>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        match self {
            Self::Accepted => Ok(()),
            got => Err(StreamableHttpError::UnexpectedServerResponse(
                format!("expect accepted, got {got:?}").into(),
            )),
        }
    }
}

pub trait StreamableHttpClient: Clone + Send + 'static {
    type Error: std::error::Error + Send + Sync + 'static;
    fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session_id: Option<Arc<str>>,
        auth_header: Option<String>,
    ) -> impl Future<Output = Result<StreamableHttpPostResponse, StreamableHttpError<Self::Error>>>
    + Send
    + '_;
    fn delete_session(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        auth_header: Option<String>,
    ) -> impl Future<Output = Result<(), StreamableHttpError<Self::Error>>> + Send + '_;
    fn get_stream(
        &self,
        uri: Arc<str>,
        session_id: Arc<str>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
    ) -> impl Future<
        Output = Result<
            BoxStream<'static, Result<Sse, SseError>>,
            StreamableHttpError<Self::Error>,
        >,
    > + Send
    + '_;
}

pub struct RetryConfig {
    pub max_times: Option<usize>,
    pub min_duration: Duration,
}

struct StreamableHttpClientReconnect<C> {
    pub client: C,
    pub session_id: Arc<str>,
    pub uri: Arc<str>,
    pub auth_header: Option<String>,
}

impl<C: StreamableHttpClient> SseStreamReconnect for StreamableHttpClientReconnect<C> {
    type Error = StreamableHttpError<C::Error>;
    type Future = BoxFuture<'static, Result<BoxedSseStream, Self::Error>>;
    fn retry_connection(&mut self, last_event_id: Option<&str>) -> Self::Future {
        let client = self.client.clone();
        let uri = self.uri.clone();
        let session_id = self.session_id.clone();
        let auth_header = self.auth_header.clone();
        let last_event_id = last_event_id.map(|s| s.to_owned());
        Box::pin(async move {
            client
                .get_stream(uri, session_id, last_event_id, auth_header)
                .await
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct StreamableHttpClientWorker<C: StreamableHttpClient> {
    pub client: C,
    pub config: StreamableHttpClientTransportConfig,
}

impl<C: StreamableHttpClient + Default> StreamableHttpClientWorker<C> {
    pub fn new_simple(url: impl Into<Arc<str>>) -> Self {
        Self {
            client: C::default(),
            config: StreamableHttpClientTransportConfig {
                uri: url.into(),
                ..Default::default()
            },
        }
    }
}

impl<C: StreamableHttpClient> StreamableHttpClientWorker<C> {
    pub fn new(client: C, config: StreamableHttpClientTransportConfig) -> Self {
        Self { client, config }
    }
}

impl<C: StreamableHttpClient> StreamableHttpClientWorker<C> {
    async fn execute_sse_stream(
        sse_stream: impl Stream<Item = Result<ServerJsonRpcMessage, StreamableHttpError<C::Error>>>
        + Send
        + 'static,
        sse_worker_tx: tokio::sync::mpsc::Sender<ServerJsonRpcMessage>,
        close_on_response: bool,
        ct: CancellationToken,
    ) -> Result<(), StreamableHttpError<C::Error>> {
        let mut sse_stream = std::pin::pin!(sse_stream);
        loop {
            let message = tokio::select! {
                event = sse_stream.next() => {
                    event
                }
                _ = ct.cancelled() => {
                    tracing::debug!("cancelled");
                    break;
                }
            };
            let Some(message) = message.transpose()? else {
                break;
            };
            let is_response = matches!(message, ServerJsonRpcMessage::Response(_));
            let yield_result = sse_worker_tx.send(message).await;
            if yield_result.is_err() {
                tracing::trace!("streamable http transport worker dropped, exiting");
                break;
            }
            if close_on_response && is_response {
                tracing::debug!("got response, closing sse stream");
                break;
            }
        }
        Ok(())
    }
}

impl<C: StreamableHttpClient> Worker for StreamableHttpClientWorker<C> {
    type Role = RoleClient;
    type Error = StreamableHttpError<C::Error>;
    fn err_closed() -> Self::Error {
        StreamableHttpError::TransportChannelClosed
    }
    fn err_join(e: tokio::task::JoinError) -> Self::Error {
        StreamableHttpError::TokioJoinError(e)
    }
    fn config(&self) -> super::worker::WorkerConfig {
        super::worker::WorkerConfig {
            name: Some("StreamableHttpClientWorker".into()),
            channel_buffer_capacity: self.config.channel_buffer_capacity,
        }
    }
    async fn run(
        self,
        mut context: super::worker::WorkerContext<Self>,
    ) -> Result<(), WorkerQuitReason<Self::Error>> {
        let channel_buffer_capacity = self.config.channel_buffer_capacity;
        let (sse_worker_tx, mut sse_worker_rx) =
            tokio::sync::mpsc::channel::<ServerJsonRpcMessage>(channel_buffer_capacity);
        let config = self.config.clone();
        let transport_task_ct = context.cancellation_token.clone();
        let _drop_guard = transport_task_ct.clone().drop_guard();
        let WorkerSendRequest {
            responder,
            message: initialize_request,
        } = context.recv_from_handler().await?;
        let (message, session_id) = match self
            .client
            .post_message(
                config.uri.clone(),
                initialize_request,
                None,
                self.config.auth_header,
            )
            .await
        {
            Ok(res) => {
                let _ = responder.send(Ok(()));
                res.expect_initialized::<C::Error>().await.map_err(
                    WorkerQuitReason::fatal_context("process initialize response"),
                )?
            }
            Err(err) => {
                let msg = format!("{:?}", err);
                let _ = responder.send(Err(err));
                return Err(WorkerQuitReason::fatal(
                    StreamableHttpError::TransportChannelClosed,
                    msg,
                ));
            }
        };
        let session_id: Option<Arc<str>> = if let Some(session_id) = session_id {
            Some(session_id.into())
        } else {
            if !self.config.allow_stateless {
                return Err(WorkerQuitReason::fatal(
                    StreamableHttpError::<C::Error>::MissingSessionIdInResponse,
                    "process initialize response",
                ));
            }
            None
        };
        // Store session info for cleanup when run() exits (not spawned, so cleanup completes before close() returns)
        let session_cleanup_info = session_id.as_ref().map(|sid| {
            (
                self.client.clone(),
                config.uri.clone(),
                sid.clone(),
                config.auth_header.clone(),
            )
        });

        context.send_to_handler(message).await?;
        let initialized_notification = context.recv_from_handler().await?;
        // expect a initialized response
        self.client
            .post_message(
                config.uri.clone(),
                initialized_notification.message,
                session_id.clone(),
                config.auth_header.clone(),
            )
            .await
            .map_err(WorkerQuitReason::fatal_context(
                "send initialized notification",
            ))?
            .expect_accepted::<C::Error>()
            .map_err(WorkerQuitReason::fatal_context(
                "process initialized notification response",
            ))?;
        let _ = initialized_notification.responder.send(Ok(()));
        #[allow(clippy::large_enum_variant)]
        enum Event<W: Worker, E: std::error::Error + Send + Sync + 'static> {
            ClientMessage(WorkerSendRequest<W>),
            ServerMessage(ServerJsonRpcMessage),
            StreamResult(Result<(), StreamableHttpError<E>>),
        }
        let mut streams = tokio::task::JoinSet::new();
        if let Some(session_id) = &session_id {
            let client = self.client.clone();
            let uri = config.uri.clone();
            let session_id = session_id.clone();
            let auth_header = config.auth_header.clone();
            let retry_config = self.config.retry_config.clone();
            let sse_worker_tx = sse_worker_tx.clone();
            let transport_task_ct = transport_task_ct.clone();
            let config_uri = config.uri.clone();
            let config_auth_header = config.auth_header.clone();

            streams.spawn(async move {
                match client
                    .get_stream(uri.clone(), session_id.clone(), None, auth_header.clone())
                    .await
                {
                    Ok(stream) => {
                        let sse_stream = SseAutoReconnectStream::new(
                            stream,
                            StreamableHttpClientReconnect {
                                client: client.clone(),
                                session_id: session_id.clone(),
                                uri: config_uri,
                                auth_header: config_auth_header,
                            },
                            retry_config,
                        );
                        Self::execute_sse_stream(
                            sse_stream,
                            sse_worker_tx,
                            false,
                            transport_task_ct.child_token(),
                        )
                        .await
                    }
                    Err(StreamableHttpError::ServerDoesNotSupportSse) => {
                        tracing::debug!("server doesn't support sse, skip common stream");
                        Ok(())
                    }
                    Err(e) => {
                        // fail to get common stream
                        tracing::error!("fail to get common stream: {e}");
                        Err(e)
                    }
                }
            });
        }
        // Main event loop - capture exit reason so we can do cleanup before returning
        let loop_result: Result<(), WorkerQuitReason<Self::Error>> = 'main_loop: loop {
            let event = tokio::select! {
                _ = transport_task_ct.cancelled() => {
                    tracing::debug!("cancelled");
                    break 'main_loop Err(WorkerQuitReason::Cancelled);
                }
                message = context.recv_from_handler() => {
                    match message {
                        Ok(msg) => Event::ClientMessage(msg),
                        Err(e) => break 'main_loop Err(e),
                    }
                },
                message = sse_worker_rx.recv() => {
                    let Some(message) = message else {
                        tracing::trace!("transport dropped, exiting");
                        break 'main_loop Err(WorkerQuitReason::HandlerTerminated);
                    };
                    Event::ServerMessage(message)
                },
                terminated_stream = streams.join_next(), if !streams.is_empty() => {
                    match terminated_stream {
                        Some(result) => {
                            Event::StreamResult(result.map_err(StreamableHttpError::TokioJoinError).and_then(std::convert::identity))
                        }
                        None => {
                            continue
                        }
                    }
                }
            };
            match event {
                Event::ClientMessage(send_request) => {
                    let WorkerSendRequest { message, responder } = send_request;
                    let response = self
                        .client
                        .post_message(
                            config.uri.clone(),
                            message,
                            session_id.clone(),
                            config.auth_header.clone(),
                        )
                        .await;
                    let send_result = match response {
                        Err(e) => Err(e),
                        Ok(StreamableHttpPostResponse::Accepted) => {
                            tracing::trace!("client message accepted");
                            Ok(())
                        }
                        Ok(StreamableHttpPostResponse::Json(message, ..)) => {
                            context.send_to_handler(message).await?;
                            Ok(())
                        }
                        Ok(StreamableHttpPostResponse::Sse(stream, ..)) => {
                            if let Some(session_id) = &session_id {
                                let sse_stream = SseAutoReconnectStream::new(
                                    stream,
                                    StreamableHttpClientReconnect {
                                        client: self.client.clone(),
                                        session_id: session_id.clone(),
                                        uri: config.uri.clone(),
                                        auth_header: config.auth_header.clone(),
                                    },
                                    self.config.retry_config.clone(),
                                );
                                streams.spawn(Self::execute_sse_stream(
                                    sse_stream,
                                    sse_worker_tx.clone(),
                                    true,
                                    transport_task_ct.child_token(),
                                ));
                            } else {
                                let sse_stream = SseAutoReconnectStream::never_reconnect(
                                    stream,
                                    StreamableHttpError::<C::Error>::UnexpectedEndOfStream,
                                );
                                streams.spawn(Self::execute_sse_stream(
                                    sse_stream,
                                    sse_worker_tx.clone(),
                                    true,
                                    transport_task_ct.child_token(),
                                ));
                            }
                            tracing::trace!("got new sse stream");
                            Ok(())
                        }
                    };
                    let _ = responder.send(send_result);
                }
                Event::ServerMessage(json_rpc_message) => {
                    // send the message to the handler
                    if let Err(e) = context.send_to_handler(json_rpc_message).await {
                        break 'main_loop Err(e);
                    }
                }
                Event::StreamResult(result) => {
                    if result.is_err() {
                        tracing::warn!(
                            "sse client event stream terminated with error: {:?}",
                            result
                        );
                    }
                }
            }
        };

        // Cleanup session before returning (ensures close() waits for session deletion)
        // Use a timeout to prevent indefinite hangs if the server is unresponsive
        if let Some((client, url, session_id, auth_header)) = session_cleanup_info {
            const SESSION_CLEANUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
            match tokio::time::timeout(
                SESSION_CLEANUP_TIMEOUT,
                client.delete_session(url, session_id.clone(), auth_header),
            )
            .await
            {
                Ok(Ok(_)) => {
                    tracing::info!(session_id = session_id.as_ref(), "delete session success")
                }
                Ok(Err(StreamableHttpError::ServerDoesNotSupportDeleteSession)) => {
                    tracing::info!(
                        session_id = session_id.as_ref(),
                        "server doesn't support delete session"
                    )
                }
                Ok(Err(e)) => {
                    tracing::error!(
                        session_id = session_id.as_ref(),
                        "fail to delete session: {e}"
                    );
                }
                Err(_elapsed) => {
                    tracing::warn!(
                        session_id = session_id.as_ref(),
                        "session cleanup timed out after {:?}",
                        SESSION_CLEANUP_TIMEOUT
                    );
                }
            }
        }

        loop_result
    }
}

/// A client-agnostic HTTP transport for RMCP that supports streaming responses.
///
/// This transport allows you to choose your preferred HTTP client implementation
/// by implementing the [`StreamableHttpClient`] trait. The transport handles
/// session management, SSE streaming, and automatic reconnection.
///
/// # Usage
///
/// ## Using reqwest
///
/// ```rust,no_run
/// use rmcp::transport::StreamableHttpClientTransport;
///
/// // Enable the reqwest feature in Cargo.toml:
/// // rmcp = { version = "0.5", features = ["transport-streamable-http-client-reqwest"] }
///
/// let transport = StreamableHttpClientTransport::from_uri("http://localhost:8000/mcp");
/// ```
///
/// ## Using a custom HTTP client
///
/// ```rust,no_run
/// use rmcp::transport::streamable_http_client::{
///     StreamableHttpClient,
///     StreamableHttpClientTransport,
///     StreamableHttpClientTransportConfig
/// };
/// use std::sync::Arc;
/// use futures::stream::BoxStream;
/// use rmcp::model::ClientJsonRpcMessage;
/// use sse_stream::{Sse, Error as SseError};
///
/// #[derive(Clone)]
/// struct MyHttpClient;
///
/// #[derive(Debug, thiserror::Error)]
/// struct MyError;
///
/// impl std::fmt::Display for MyError {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
///         write!(f, "MyError")
///     }
/// }
///
/// impl StreamableHttpClient for MyHttpClient {
///     type Error = MyError;
///
///     async fn post_message(
///         &self,
///         _uri: Arc<str>,
///         _message: ClientJsonRpcMessage,
///         _session_id: Option<Arc<str>>,
///         _auth_header: Option<String>,
///     ) -> Result<rmcp::transport::streamable_http_client::StreamableHttpPostResponse, rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>> {
///         todo!()
///     }
///
///     async fn delete_session(
///         &self,
///         _uri: Arc<str>,
///         _session_id: Arc<str>,
///         _auth_header: Option<String>,
///     ) -> Result<(), rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>> {
///         todo!()
///     }
///
///     async fn get_stream(
///         &self,
///         _uri: Arc<str>,
///         _session_id: Arc<str>,
///         _last_event_id: Option<String>,
///         _auth_header: Option<String>,
///     ) -> Result<BoxStream<'static, Result<Sse, SseError>>, rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>> {
///         todo!()
///     }
/// }
///
/// let transport = StreamableHttpClientTransport::with_client(
///     MyHttpClient,
///     StreamableHttpClientTransportConfig::with_uri("http://localhost:8000/mcp")
/// );
/// ```
///
/// # Feature Flags
///
/// - `transport-streamable-http-client`: Base feature providing the generic transport infrastructure
/// - `transport-streamable-http-client-reqwest`: Includes reqwest HTTP client support with convenience methods
pub type StreamableHttpClientTransport<C> = WorkerTransport<StreamableHttpClientWorker<C>>;

impl<C: StreamableHttpClient> StreamableHttpClientTransport<C> {
    /// Creates a new transport with a custom HTTP client implementation.
    ///
    /// This method allows you to use any HTTP client that implements the [`StreamableHttpClient`] trait.
    /// Use this when you want to use a custom HTTP client or when the reqwest feature is not enabled.
    ///
    /// # Arguments
    ///
    /// * `client` - Your HTTP client implementation
    /// * `config` - Transport configuration including the server URI
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rmcp::transport::streamable_http_client::{
    ///     StreamableHttpClient,
    ///     StreamableHttpClientTransport,
    ///     StreamableHttpClientTransportConfig
    /// };
    /// use std::sync::Arc;
    /// use futures::stream::BoxStream;
    /// use rmcp::model::ClientJsonRpcMessage;
    /// use sse_stream::{Sse, Error as SseError};
    ///
    /// // Define your custom client
    /// #[derive(Clone)]
    /// struct MyHttpClient;
    ///
    /// #[derive(Debug, thiserror::Error)]
    /// struct MyError;
    ///
    /// impl std::fmt::Display for MyError {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "MyError")
    ///     }
    /// }
    ///
    /// impl StreamableHttpClient for MyHttpClient {
    ///     type Error = MyError;
    ///
    ///     async fn post_message(
    ///         &self,
    ///         _uri: Arc<str>,
    ///         _message: ClientJsonRpcMessage,
    ///         _session_id: Option<Arc<str>>,
    ///         _auth_header: Option<String>,
    ///     ) -> Result<rmcp::transport::streamable_http_client::StreamableHttpPostResponse, rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>> {
    ///         todo!()
    ///     }
    ///
    ///     async fn delete_session(
    ///         &self,
    ///         _uri: Arc<str>,
    ///         _session_id: Arc<str>,
    ///         _auth_header: Option<String>,
    ///     ) -> Result<(), rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>> {
    ///         todo!()
    ///     }
    ///
    ///     async fn get_stream(
    ///         &self,
    ///         _uri: Arc<str>,
    ///         _session_id: Arc<str>,
    ///         _last_event_id: Option<String>,
    ///         _auth_header: Option<String>,
    ///     ) -> Result<BoxStream<'static, Result<Sse, SseError>>, rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>> {
    ///         todo!()
    ///     }
    /// }
    ///
    /// let transport = StreamableHttpClientTransport::with_client(
    ///     MyHttpClient,
    ///     StreamableHttpClientTransportConfig::with_uri("http://localhost:8000/mcp")
    /// );
    /// ```
    pub fn with_client(client: C, config: StreamableHttpClientTransportConfig) -> Self {
        let worker = StreamableHttpClientWorker::new(client, config);
        WorkerTransport::spawn(worker)
    }
}
#[derive(Debug, Clone)]
pub struct StreamableHttpClientTransportConfig {
    pub uri: Arc<str>,
    pub retry_config: Arc<dyn SseRetryPolicy>,
    pub channel_buffer_capacity: usize,
    /// if true, the transport will not require a session to be established
    pub allow_stateless: bool,
    /// The value to send in the authorization header
    pub auth_header: Option<String>,
}

impl StreamableHttpClientTransportConfig {
    pub fn with_uri(uri: impl Into<Arc<str>>) -> Self {
        Self {
            uri: uri.into(),
            ..Default::default()
        }
    }

    /// Set the authorization header to send with requests
    ///
    /// # Arguments
    ///
    /// * `value` - A bearer token without the `Bearer ` prefix
    pub fn auth_header<T: Into<String>>(mut self, value: T) -> Self {
        // set our authorization header
        self.auth_header = Some(value.into());
        self
    }
}

impl Default for StreamableHttpClientTransportConfig {
    fn default() -> Self {
        Self {
            uri: "localhost".into(),
            retry_config: Arc::new(ExponentialBackoff::default()),
            channel_buffer_capacity: 16,
            allow_stateless: true,
            auth_header: None,
        }
    }
}
