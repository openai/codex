//! # Transport
//! The transport type must implemented [`Transport`] trait, which allow it send message concurrently and receive message sequentially.
//！
//! ## Standard Transport Types
//! There are 3 pairs of standard transport types:
//!
//! | transport         | client                                                    | server                                                |
//! |:-:                |:-:                                                        |:-:                                                    |
//! | std IO            | [`child_process::TokioChildProcess`]                      | [`io::stdio`]                                         |
//! | streamable http   | [`streamable_http_client::StreamableHttpClientTransport`] | [`streamable_http_server::StreamableHttpService`]     |
//! | sse               | [`sse_client::SseClientTransport`]                        | [`sse_server::SseServer`]                             |
//!
//！## Helper Transport Types
//! Thers are several helper transport types that can help you to create transport quickly.
//!
//! ### [Worker Transport](`worker::WorkerTransport`)
//! Which allows you to run a worker and process messages in another tokio task.
//!
//! ### [Async Read/Write Transport](`async_rw::AsyncRwTransport`)
//! You need to enable `transport-async-rw` feature to use this transport.
//!
//! This transport is used to create a transport from a byte stream which implemented [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`].
//!
//! This could be very helpful when you want to create a transport from a byte stream, such as a file or a tcp connection.
//!
//! ### [Sink/Stream Transport](`sink_stream::SinkStreamTransport`)
//! This transport is used to create a transport from a sink and a stream.
//!
//! This could be very helpful when you want to create a transport from a duplex object stream, such as a websocket connection.
//!
//! ## [IntoTransport](`IntoTransport`) trait
//! [`IntoTransport`] is a helper trait that implicitly convert a type into a transport type.
//!
//! ### These types is automatically implemented [`IntoTransport`] trait
//! 1. A type that already implement both [`futures::Sink`] and [`futures::Stream`] trait, or a tuple `(Tx, Rx)`  where `Tx` is [`futures::Sink`] and `Rx` is [`futures::Stream`].
//! 2. A type that implement both [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`] trait. or a tuple `(R, W)` where `R` is [`tokio::io::AsyncRead`] and `W` is [`tokio::io::AsyncWrite`].
//! 3. A type that implement [Worker](`worker::Worker`) trait.
//! 4. A type that implement [`Transport`] trait.
//!
//! ## Examples
//!
//! ```rust
//! # use rmcp::{
//! #     ServiceExt, serve_client, serve_server,
//! # };
//!
//! // create transport from tcp stream
//! async fn client() -> Result<(), Box<dyn std::error::Error>> {
//!     let stream = tokio::net::TcpSocket::new_v4()?
//!         .connect("127.0.0.1:8001".parse()?)
//!         .await?;
//!     let client = ().serve(stream).await?;
//!     let tools = client.peer().list_tools(Default::default()).await?;
//!     println!("{:?}", tools);
//!     Ok(())
//! }
//!
//! // create transport from std io
//! async fn io()  -> Result<(), Box<dyn std::error::Error>> {
//!     let client = ().serve((tokio::io::stdin(), tokio::io::stdout())).await?;
//!     let tools = client.peer().list_tools(Default::default()).await?;
//!     println!("{:?}", tools);
//!     Ok(())
//! }
//! ```

use std::{borrow::Cow, sync::Arc};

use crate::service::{RxJsonRpcMessage, ServiceRole, TxJsonRpcMessage};

pub mod sink_stream;

#[cfg(feature = "transport-async-rw")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-async-rw")))]
pub mod async_rw;

#[cfg(feature = "transport-worker")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-worker")))]
pub mod worker;
#[cfg(feature = "transport-worker")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-worker")))]
pub use worker::WorkerTransport;

#[cfg(feature = "transport-child-process")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-child-process")))]
pub mod child_process;
#[cfg(feature = "transport-child-process")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-child-process")))]
pub use child_process::{ConfigureCommandExt, TokioChildProcess};

#[cfg(feature = "transport-io")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-io")))]
pub mod io;
#[cfg(feature = "transport-io")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-io")))]
pub use io::stdio;

#[cfg(feature = "transport-sse-client")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-sse-client")))]
pub mod sse_client;
#[cfg(feature = "transport-sse-client")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-sse-client")))]
pub use sse_client::SseClientTransport;

#[cfg(feature = "transport-sse-server")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-sse-server")))]
pub mod sse_server;
#[cfg(feature = "transport-sse-server")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-sse-server")))]
pub use sse_server::SseServer;

#[cfg(feature = "auth")]
#[cfg_attr(docsrs, doc(cfg(feature = "auth")))]
pub mod auth;
#[cfg(feature = "auth")]
#[cfg_attr(docsrs, doc(cfg(feature = "auth")))]
pub use auth::{AuthError, AuthorizationManager, AuthorizationSession, AuthorizedHttpClient};

// #[cfg(feature = "transport-ws")]
// #[cfg_attr(docsrs, doc(cfg(feature = "transport-ws")))]
// pub mod ws;
#[cfg(feature = "transport-streamable-http-server-session")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-streamable-http-server-session")))]
pub mod streamable_http_server;
#[cfg(feature = "transport-streamable-http-server")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-streamable-http-server")))]
pub use streamable_http_server::tower::{StreamableHttpServerConfig, StreamableHttpService};

#[cfg(feature = "transport-streamable-http-client")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-streamable-http-client")))]
pub mod streamable_http_client;
#[cfg(feature = "transport-streamable-http-client")]
#[cfg_attr(docsrs, doc(cfg(feature = "transport-streamable-http-client")))]
pub use streamable_http_client::StreamableHttpClientTransport;

/// Common use codes
pub mod common;

pub trait Transport<R>: Send
where
    R: ServiceRole,
{
    type Error: std::error::Error + Send + Sync + 'static;
    fn name() -> Cow<'static, str> {
        std::any::type_name::<Self>().into()
    }
    /// Send a message to the transport
    ///
    /// Notice that the future returned by this function should be `Send` and `'static`.
    /// It's because the sending message could be executed concurrently.
    ///
    fn send(
        &mut self,
        item: TxJsonRpcMessage<R>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static;

    /// Receive a message from the transport, this operation is sequential.
    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<R>>> + Send;

    /// Close the transport
    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

pub trait IntoTransport<R, E, A>: Send + 'static
where
    R: ServiceRole,
    E: std::error::Error + Send + 'static,
{
    fn into_transport(self) -> impl Transport<R, Error = E> + 'static;
}

pub enum TransportAdapterIdentity {}
impl<R, T, E> IntoTransport<R, E, TransportAdapterIdentity> for T
where
    T: Transport<R, Error = E> + Send + 'static,
    R: ServiceRole,
    E: std::error::Error + Send + Sync + 'static,
{
    fn into_transport(self) -> impl Transport<R, Error = E> + 'static {
        self
    }
}

/// A transport that can send a single message and then close itself
pub struct OneshotTransport<R>
where
    R: ServiceRole,
{
    message: Option<RxJsonRpcMessage<R>>,
    sender: tokio::sync::mpsc::Sender<TxJsonRpcMessage<R>>,
    finished_signal: Arc<tokio::sync::Notify>,
}

impl<R> OneshotTransport<R>
where
    R: ServiceRole,
{
    pub fn new(
        message: RxJsonRpcMessage<R>,
    ) -> (Self, tokio::sync::mpsc::Receiver<TxJsonRpcMessage<R>>) {
        let (sender, receiver) = tokio::sync::mpsc::channel(16);
        (
            Self {
                message: Some(message),
                sender,
                finished_signal: Arc::new(tokio::sync::Notify::new()),
            },
            receiver,
        )
    }
}

impl<R> Transport<R> for OneshotTransport<R>
where
    R: ServiceRole,
{
    type Error = tokio::sync::mpsc::error::SendError<TxJsonRpcMessage<R>>;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<R>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let sender = self.sender.clone();
        let terminate = matches!(item, TxJsonRpcMessage::<R>::Response(_))
            || matches!(item, TxJsonRpcMessage::<R>::Error(_));
        let signal = self.finished_signal.clone();
        async move {
            sender.send(item).await?;
            if terminate {
                signal.notify_waiters();
            }
            Ok(())
        }
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<R>> {
        if self.message.is_none() {
            self.finished_signal.notified().await;
        }
        self.message.take()
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.message.take();
        std::future::ready(Ok(()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Transport [{transport_name}] error: {error}")]
pub struct DynamicTransportError {
    pub transport_name: Cow<'static, str>,
    pub transport_type_id: std::any::TypeId,
    #[source]
    pub error: Box<dyn std::error::Error + Send + Sync>,
}

impl DynamicTransportError {
    pub fn new<T: Transport<R> + 'static, R: ServiceRole>(e: T::Error) -> Self {
        Self {
            transport_name: T::name(),
            transport_type_id: std::any::TypeId::of::<T>(),
            error: Box::new(e),
        }
    }
    pub fn downcast<T: Transport<R> + 'static, R: ServiceRole>(self) -> Result<T::Error, Self> {
        if !self.is::<T, R>() {
            Err(self)
        } else {
            Ok(self
                .error
                .downcast::<T::Error>()
                .map(|e| *e)
                .expect("type is checked"))
        }
    }
    pub fn is<T: Transport<R> + 'static, R: ServiceRole>(&self) -> bool {
        self.error.is::<T::Error>() && self.transport_type_id == std::any::TypeId::of::<T>()
    }
}
