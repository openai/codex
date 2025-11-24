//! Asynchronous TLS/SSL streams for Tokio using [Rustls](https://github.com/rustls/rustls).
//!
//! # Why do I need to call `poll_flush`?
//!
//! Most TLS implementations will have an internal buffer to improve throughput,
//! and rustls is no exception.
//!
//! When we write data to `TlsStream`, we always write rustls buffer first,
//! then take out rustls encrypted data packet, and write it to data channel (like TcpStream).
//! When data channel is pending, some data may remain in rustls buffer.
//!
//! `tokio-rustls` To keep it simple and correct, [TlsStream] will behave like `BufWriter`.
//! For `TlsStream<TcpStream>`, this means that data written by `poll_write` is not guaranteed to be written to `TcpStream`.
//! You must call `poll_flush` to ensure that it is written to `TcpStream`.
//!
//! You should call `poll_flush` at the appropriate time,
//! such as when a period of `poll_write` write is complete and there is no more data to write.
//!
//! ## Why don't we write during `poll_read`?
//!
//! We did this in the early days of `tokio-rustls`, but it caused some bugs.
//! We can solve these bugs through some solutions, but this will cause performance degradation (reverse false wakeup).
//!
//! And reverse write will also prevent us implement full duplex in the future.
//!
//! see <https://github.com/tokio-rs/tls/issues/40>
//!
//! ## Why can't we handle it like `native-tls`?
//!
//! When data channel returns to pending, `native-tls` will falsely report the number of bytes it consumes.
//! This means that if data written by `poll_write` is not actually written to data channel, it will not return `Ready`.
//! Thus avoiding the call of `poll_flush`.
//!
//! but which does not conform to convention of `AsyncWrite` trait.
//! This means that if you give inconsistent data in two `poll_write`, it may cause unexpected behavior.
//!
//! see <https://github.com/tokio-rs/tls/issues/41>

use std::future::Future;
use std::io;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, RawSocket};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

pub use rustls;

use rustls::pki_types::ServerName;
use rustls::server::AcceptedAlert;
use rustls::{ClientConfig, ClientConnection, CommonState, ServerConfig, ServerConnection};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, ReadBuf};

macro_rules! ready {
    ( $e:expr ) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}

pub mod client;
mod common;
use common::{MidHandshake, TlsState};
pub mod server;

/// A wrapper around a `rustls::ClientConfig`, providing an async `connect` method.
#[derive(Clone)]
pub struct TlsConnector {
    inner: Arc<ClientConfig>,
    #[cfg(feature = "early-data")]
    early_data: bool,
}

/// A wrapper around a `rustls::ServerConfig`, providing an async `accept` method.
#[derive(Clone)]
pub struct TlsAcceptor {
    inner: Arc<ServerConfig>,
}

impl From<Arc<ClientConfig>> for TlsConnector {
    fn from(inner: Arc<ClientConfig>) -> TlsConnector {
        TlsConnector {
            inner,
            #[cfg(feature = "early-data")]
            early_data: false,
        }
    }
}

impl From<Arc<ServerConfig>> for TlsAcceptor {
    fn from(inner: Arc<ServerConfig>) -> TlsAcceptor {
        TlsAcceptor { inner }
    }
}

impl TlsConnector {
    /// Enable 0-RTT.
    ///
    /// If you want to use 0-RTT,
    /// You must also set `ClientConfig.enable_early_data` to `true`.
    #[cfg(feature = "early-data")]
    pub fn early_data(mut self, flag: bool) -> TlsConnector {
        self.early_data = flag;
        self
    }

    #[inline]
    pub fn connect<IO>(&self, domain: ServerName<'static>, stream: IO) -> Connect<IO>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        self.connect_with(domain, stream, |_| ())
    }

    pub fn connect_with<IO, F>(&self, domain: ServerName<'static>, stream: IO, f: F) -> Connect<IO>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
        F: FnOnce(&mut ClientConnection),
    {
        let mut session = match ClientConnection::new(self.inner.clone(), domain) {
            Ok(session) => session,
            Err(error) => {
                return Connect(MidHandshake::Error {
                    io: stream,
                    // TODO(eliza): should this really return an `io::Error`?
                    // Probably not...
                    error: io::Error::new(io::ErrorKind::Other, error),
                });
            }
        };
        f(&mut session);

        Connect(MidHandshake::Handshaking(client::TlsStream {
            io: stream,

            #[cfg(not(feature = "early-data"))]
            state: TlsState::Stream,

            #[cfg(feature = "early-data")]
            state: if self.early_data && session.early_data().is_some() {
                TlsState::EarlyData(0, Vec::new())
            } else {
                TlsState::Stream
            },

            #[cfg(feature = "early-data")]
            early_waker: None,

            session,
        }))
    }

    /// Get a read-only reference to underlying config
    pub fn config(&self) -> &Arc<ClientConfig> {
        &self.inner
    }
}

impl TlsAcceptor {
    #[inline]
    pub fn accept<IO>(&self, stream: IO) -> Accept<IO>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        self.accept_with(stream, |_| ())
    }

    pub fn accept_with<IO, F>(&self, stream: IO, f: F) -> Accept<IO>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
        F: FnOnce(&mut ServerConnection),
    {
        let mut session = match ServerConnection::new(self.inner.clone()) {
            Ok(session) => session,
            Err(error) => {
                return Accept(MidHandshake::Error {
                    io: stream,
                    // TODO(eliza): should this really return an `io::Error`?
                    // Probably not...
                    error: io::Error::new(io::ErrorKind::Other, error),
                });
            }
        };
        f(&mut session);

        Accept(MidHandshake::Handshaking(server::TlsStream {
            session,
            io: stream,
            state: TlsState::Stream,
        }))
    }

    /// Get a read-only reference to underlying config
    pub fn config(&self) -> &Arc<ServerConfig> {
        &self.inner
    }
}

pub struct LazyConfigAcceptor<IO> {
    acceptor: rustls::server::Acceptor,
    io: Option<IO>,
    alert: Option<(rustls::Error, AcceptedAlert)>,
}

impl<IO> LazyConfigAcceptor<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    pub fn new(acceptor: rustls::server::Acceptor, io: IO) -> Self {
        Self {
            acceptor,
            io: Some(io),
            alert: None,
        }
    }

    /// Takes back the client connection. Will return `None` if called more than once or if the
    /// connection has been accepted.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # fn choose_server_config(
    /// #     _: rustls::server::ClientHello,
    /// # ) -> std::sync::Arc<rustls::ServerConfig> {
    /// #     unimplemented!();
    /// # }
    /// # #[allow(unused_variables)]
    /// # async fn listen() {
    /// use tokio::io::AsyncWriteExt;
    /// let listener = tokio::net::TcpListener::bind("127.0.0.1:4443").await.unwrap();
    /// let (stream, _) = listener.accept().await.unwrap();
    ///
    /// let acceptor = tokio_rustls::LazyConfigAcceptor::new(rustls::server::Acceptor::default(), stream);
    /// tokio::pin!(acceptor);
    ///
    /// match acceptor.as_mut().await {
    ///     Ok(start) => {
    ///         let clientHello = start.client_hello();
    ///         let config = choose_server_config(clientHello);
    ///         let stream = start.into_stream(config).await.unwrap();
    ///         // Proceed with handling the ServerConnection...
    ///     }
    ///     Err(err) => {
    ///         if let Some(mut stream) = acceptor.take_io() {
    ///             stream
    ///                 .write_all(
    ///                     format!("HTTP/1.1 400 Invalid Input\r\n\r\n\r\n{:?}\n", err)
    ///                         .as_bytes()
    ///                 )
    ///                 .await
    ///                 .unwrap();
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    pub fn take_io(&mut self) -> Option<IO> {
        self.io.take()
    }
}

impl<IO> Future for LazyConfigAcceptor<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    type Output = Result<StartHandshake<IO>, io::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            let io = match this.io.as_mut() {
                Some(io) => io,
                None => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::Other,
                        "acceptor cannot be polled after acceptance",
                    )))
                }
            };

            if let Some((err, mut alert)) = this.alert.take() {
                match alert.write(&mut common::SyncWriteAdapter { io, cx }) {
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        this.alert = Some((err, alert));
                        return Poll::Pending;
                    }
                    Ok(0) | Err(_) => {
                        return Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidData, err)))
                    }
                    Ok(_) => {
                        this.alert = Some((err, alert));
                        continue;
                    }
                };
            }

            let mut reader = common::SyncReadAdapter { io, cx };
            match this.acceptor.read_tls(&mut reader) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()).into(),
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Poll::Pending,
                Err(e) => return Err(e).into(),
            }

            match this.acceptor.accept() {
                Ok(Some(accepted)) => {
                    let io = this.io.take().unwrap();
                    return Poll::Ready(Ok(StartHandshake { accepted, io }));
                }
                Ok(None) => {}
                Err((err, alert)) => {
                    this.alert = Some((err, alert));
                }
            }
        }
    }
}

pub struct StartHandshake<IO> {
    accepted: rustls::server::Accepted,
    io: IO,
}

impl<IO> StartHandshake<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    pub fn client_hello(&self) -> rustls::server::ClientHello<'_> {
        self.accepted.client_hello()
    }

    pub fn into_stream(self, config: Arc<ServerConfig>) -> Accept<IO> {
        self.into_stream_with(config, |_| ())
    }

    pub fn into_stream_with<F>(self, config: Arc<ServerConfig>, f: F) -> Accept<IO>
    where
        F: FnOnce(&mut ServerConnection),
    {
        let mut conn = match self.accepted.into_connection(config) {
            Ok(conn) => conn,
            Err((error, alert)) => {
                return Accept(MidHandshake::SendAlert {
                    io: self.io,
                    alert,
                    // TODO(eliza): should this really return an `io::Error`?
                    // Probably not...
                    error: io::Error::new(io::ErrorKind::InvalidData, error),
                });
            }
        };
        f(&mut conn);

        Accept(MidHandshake::Handshaking(server::TlsStream {
            session: conn,
            io: self.io,
            state: TlsState::Stream,
        }))
    }
}

/// Future returned from `TlsConnector::connect` which will resolve
/// once the connection handshake has finished.
pub struct Connect<IO>(MidHandshake<client::TlsStream<IO>>);

/// Future returned from `TlsAcceptor::accept` which will resolve
/// once the accept handshake has finished.
pub struct Accept<IO>(MidHandshake<server::TlsStream<IO>>);

/// Like [Connect], but returns `IO` on failure.
pub struct FallibleConnect<IO>(MidHandshake<client::TlsStream<IO>>);

/// Like [Accept], but returns `IO` on failure.
pub struct FallibleAccept<IO>(MidHandshake<server::TlsStream<IO>>);

impl<IO> Connect<IO> {
    #[inline]
    pub fn into_fallible(self) -> FallibleConnect<IO> {
        FallibleConnect(self.0)
    }

    pub fn get_ref(&self) -> Option<&IO> {
        match &self.0 {
            MidHandshake::Handshaking(sess) => Some(sess.get_ref().0),
            MidHandshake::SendAlert { io, .. } => Some(io),
            MidHandshake::Error { io, .. } => Some(io),
            MidHandshake::End => None,
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut IO> {
        match &mut self.0 {
            MidHandshake::Handshaking(sess) => Some(sess.get_mut().0),
            MidHandshake::SendAlert { io, .. } => Some(io),
            MidHandshake::Error { io, .. } => Some(io),
            MidHandshake::End => None,
        }
    }
}

impl<IO> Accept<IO> {
    #[inline]
    pub fn into_fallible(self) -> FallibleAccept<IO> {
        FallibleAccept(self.0)
    }

    pub fn get_ref(&self) -> Option<&IO> {
        match &self.0 {
            MidHandshake::Handshaking(sess) => Some(sess.get_ref().0),
            MidHandshake::SendAlert { io, .. } => Some(io),
            MidHandshake::Error { io, .. } => Some(io),
            MidHandshake::End => None,
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut IO> {
        match &mut self.0 {
            MidHandshake::Handshaking(sess) => Some(sess.get_mut().0),
            MidHandshake::SendAlert { io, .. } => Some(io),
            MidHandshake::Error { io, .. } => Some(io),
            MidHandshake::End => None,
        }
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> Future for Connect<IO> {
    type Output = io::Result<client::TlsStream<IO>>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx).map_err(|(err, _)| err)
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> Future for Accept<IO> {
    type Output = io::Result<server::TlsStream<IO>>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx).map_err(|(err, _)| err)
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> Future for FallibleConnect<IO> {
    type Output = Result<client::TlsStream<IO>, (io::Error, IO)>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> Future for FallibleAccept<IO> {
    type Output = Result<server::TlsStream<IO>, (io::Error, IO)>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// Unified TLS stream type
///
/// This abstracts over the inner `client::TlsStream` and `server::TlsStream`, so you can use
/// a single type to keep both client- and server-initiated TLS-encrypted connections.
#[allow(clippy::large_enum_variant)] // https://github.com/rust-lang/rust-clippy/issues/9798
#[derive(Debug)]
pub enum TlsStream<T> {
    Client(client::TlsStream<T>),
    Server(server::TlsStream<T>),
}

impl<T> TlsStream<T> {
    pub fn get_ref(&self) -> (&T, &CommonState) {
        use TlsStream::*;
        match self {
            Client(io) => {
                let (io, session) = io.get_ref();
                (io, session)
            }
            Server(io) => {
                let (io, session) = io.get_ref();
                (io, session)
            }
        }
    }

    pub fn get_mut(&mut self) -> (&mut T, &mut CommonState) {
        use TlsStream::*;
        match self {
            Client(io) => {
                let (io, session) = io.get_mut();
                (io, &mut *session)
            }
            Server(io) => {
                let (io, session) = io.get_mut();
                (io, &mut *session)
            }
        }
    }
}

impl<T> From<client::TlsStream<T>> for TlsStream<T> {
    fn from(s: client::TlsStream<T>) -> Self {
        Self::Client(s)
    }
}

impl<T> From<server::TlsStream<T>> for TlsStream<T> {
    fn from(s: server::TlsStream<T>) -> Self {
        Self::Server(s)
    }
}

#[cfg(unix)]
impl<S> AsRawFd for TlsStream<S>
where
    S: AsRawFd,
{
    fn as_raw_fd(&self) -> RawFd {
        self.get_ref().0.as_raw_fd()
    }
}

#[cfg(windows)]
impl<S> AsRawSocket for TlsStream<S>
where
    S: AsRawSocket,
{
    fn as_raw_socket(&self) -> RawSocket {
        self.get_ref().0.as_raw_socket()
    }
}

impl<T> AsyncRead for TlsStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            TlsStream::Client(x) => Pin::new(x).poll_read(cx, buf),
            TlsStream::Server(x) => Pin::new(x).poll_read(cx, buf),
        }
    }
}

impl<T> AsyncBufRead for TlsStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        match self.get_mut() {
            TlsStream::Client(x) => Pin::new(x).poll_fill_buf(cx),
            TlsStream::Server(x) => Pin::new(x).poll_fill_buf(cx),
        }
    }

    #[inline]
    fn consume(self: Pin<&mut Self>, amt: usize) {
        match self.get_mut() {
            TlsStream::Client(x) => Pin::new(x).consume(amt),
            TlsStream::Server(x) => Pin::new(x).consume(amt),
        }
    }
}

impl<T> AsyncWrite for TlsStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            TlsStream::Client(x) => Pin::new(x).poll_write(cx, buf),
            TlsStream::Server(x) => Pin::new(x).poll_write(cx, buf),
        }
    }

    #[inline]
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            TlsStream::Client(x) => Pin::new(x).poll_write_vectored(cx, bufs),
            TlsStream::Server(x) => Pin::new(x).poll_write_vectored(cx, bufs),
        }
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        match self {
            TlsStream::Client(x) => x.is_write_vectored(),
            TlsStream::Server(x) => x.is_write_vectored(),
        }
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            TlsStream::Client(x) => Pin::new(x).poll_flush(cx),
            TlsStream::Server(x) => Pin::new(x).poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            TlsStream::Client(x) => Pin::new(x).poll_shutdown(cx),
            TlsStream::Server(x) => Pin::new(x).poll_shutdown(cx),
        }
    }
}
