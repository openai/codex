//! Http1 or Http2 connection.

pub mod upgrade;

use hyper::service::HttpService;
use std::future::Future;
use std::marker::PhantomPinned;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{error::Error as StdError, io, time::Duration};

use bytes::Bytes;
use futures_core::ready;
use http::{Request, Response};
use http_body::Body;
use hyper::{
    body::Incoming,
    rt::{Read, ReadBuf, Timer, Write},
    service::Service,
};

#[cfg(feature = "http1")]
use hyper::server::conn::http1;

#[cfg(feature = "http2")]
use hyper::{rt::bounds::Http2ServerConnExec, server::conn::http2};

#[cfg(any(not(feature = "http2"), not(feature = "http1")))]
use std::marker::PhantomData;

use pin_project_lite::pin_project;

use crate::common::rewind::Rewind;

type Error = Box<dyn std::error::Error + Send + Sync>;

type Result<T> = std::result::Result<T, Error>;

const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// Exactly equivalent to [`Http2ServerConnExec`].
#[cfg(feature = "http2")]
pub trait HttpServerConnExec<A, B: Body>: Http2ServerConnExec<A, B> {}

#[cfg(feature = "http2")]
impl<A, B: Body, T: Http2ServerConnExec<A, B>> HttpServerConnExec<A, B> for T {}

/// Exactly equivalent to [`Http2ServerConnExec`].
#[cfg(not(feature = "http2"))]
pub trait HttpServerConnExec<A, B: Body> {}

#[cfg(not(feature = "http2"))]
impl<A, B: Body, T> HttpServerConnExec<A, B> for T {}

/// Http1 or Http2 connection builder.
#[derive(Clone, Debug)]
pub struct Builder<E> {
    #[cfg(feature = "http1")]
    http1: http1::Builder,
    #[cfg(feature = "http2")]
    http2: http2::Builder<E>,
    #[cfg(any(feature = "http1", feature = "http2"))]
    version: Option<Version>,
    #[cfg(not(feature = "http2"))]
    _executor: E,
}

impl<E: Default> Default for Builder<E> {
    fn default() -> Self {
        Self::new(E::default())
    }
}

impl<E> Builder<E> {
    /// Create a new auto connection builder.
    ///
    /// `executor` parameter should be a type that implements
    /// [`Executor`](hyper::rt::Executor) trait.
    ///
    /// # Example
    ///
    /// ```
    /// use hyper_util::{
    ///     rt::TokioExecutor,
    ///     server::conn::auto,
    /// };
    ///
    /// auto::Builder::new(TokioExecutor::new());
    /// ```
    pub fn new(executor: E) -> Self {
        Self {
            #[cfg(feature = "http1")]
            http1: http1::Builder::new(),
            #[cfg(feature = "http2")]
            http2: http2::Builder::new(executor),
            #[cfg(any(feature = "http1", feature = "http2"))]
            version: None,
            #[cfg(not(feature = "http2"))]
            _executor: executor,
        }
    }

    /// Http1 configuration.
    #[cfg(feature = "http1")]
    pub fn http1(&mut self) -> Http1Builder<'_, E> {
        Http1Builder { inner: self }
    }

    /// Http2 configuration.
    #[cfg(feature = "http2")]
    pub fn http2(&mut self) -> Http2Builder<'_, E> {
        Http2Builder { inner: self }
    }

    /// Only accepts HTTP/2
    ///
    /// Does not do anything if used with [`serve_connection_with_upgrades`]
    ///
    /// [`serve_connection_with_upgrades`]: Builder::serve_connection_with_upgrades
    #[cfg(feature = "http2")]
    pub fn http2_only(mut self) -> Self {
        assert!(self.version.is_none());
        self.version = Some(Version::H2);
        self
    }

    /// Only accepts HTTP/1
    ///
    /// Does not do anything if used with [`serve_connection_with_upgrades`]
    ///
    /// [`serve_connection_with_upgrades`]: Builder::serve_connection_with_upgrades
    #[cfg(feature = "http1")]
    pub fn http1_only(mut self) -> Self {
        assert!(self.version.is_none());
        self.version = Some(Version::H1);
        self
    }

    /// Returns `true` if this builder can serve an HTTP/1.1-based connection.
    pub fn is_http1_available(&self) -> bool {
        match self.version {
            #[cfg(feature = "http1")]
            Some(Version::H1) => true,
            #[cfg(feature = "http2")]
            Some(Version::H2) => false,
            #[cfg(any(feature = "http1", feature = "http2"))]
            _ => true,
        }
    }

    /// Returns `true` if this builder can serve an HTTP/2-based connection.
    pub fn is_http2_available(&self) -> bool {
        match self.version {
            #[cfg(feature = "http1")]
            Some(Version::H1) => false,
            #[cfg(feature = "http2")]
            Some(Version::H2) => true,
            #[cfg(any(feature = "http1", feature = "http2"))]
            _ => true,
        }
    }

    /// Set whether HTTP/1 connections will write header names as title case at
    /// the socket level.
    ///
    /// This setting only affects HTTP/1 connections. HTTP/2 connections are
    /// not affected by this setting.
    ///
    /// Default is false.
    ///
    /// # Example
    ///
    /// ```
    /// use hyper_util::{
    ///     rt::TokioExecutor,
    ///     server::conn::auto,
    /// };
    ///
    /// auto::Builder::new(TokioExecutor::new())
    ///     .title_case_headers(true);
    /// ```
    #[cfg(feature = "http1")]
    pub fn title_case_headers(mut self, enabled: bool) -> Self {
        self.http1.title_case_headers(enabled);
        self
    }

    /// Set whether HTTP/1 connections will preserve the original case of header names.
    ///
    /// This setting only affects HTTP/1 connections. HTTP/2 connections are
    /// not affected by this setting.
    ///
    /// Default is false.
    ///
    /// # Example
    ///
    /// ```
    /// use hyper_util::{
    ///     rt::TokioExecutor,
    ///     server::conn::auto,
    /// };
    ///
    /// auto::Builder::new(TokioExecutor::new())
    ///     .preserve_header_case(true);
    /// ```
    #[cfg(feature = "http1")]
    pub fn preserve_header_case(mut self, enabled: bool) -> Self {
        self.http1.preserve_header_case(enabled);
        self
    }

    /// Bind a connection together with a [`Service`].
    pub fn serve_connection<I, S, B>(&self, io: I, service: S) -> Connection<'_, I, S, E>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        let state = match self.version {
            #[cfg(feature = "http1")]
            Some(Version::H1) => {
                let io = Rewind::new_buffered(io, Bytes::new());
                let conn = self.http1.serve_connection(io, service);
                ConnState::H1 { conn }
            }
            #[cfg(feature = "http2")]
            Some(Version::H2) => {
                let io = Rewind::new_buffered(io, Bytes::new());
                let conn = self.http2.serve_connection(io, service);
                ConnState::H2 { conn }
            }
            #[cfg(any(feature = "http1", feature = "http2"))]
            _ => ConnState::ReadVersion {
                read_version: read_version(io),
                builder: Cow::Borrowed(self),
                service: Some(service),
            },
        };

        Connection { state }
    }

    /// Bind a connection together with a [`Service`], with the ability to
    /// handle HTTP upgrades. This requires that the IO object implements
    /// `Send`.
    ///
    /// Note that if you ever want to use [`hyper::upgrade::Upgraded::downcast`]
    /// with this crate, you'll need to use [`hyper_util::server::conn::auto::upgrade::downcast`]
    /// instead. See the documentation of the latter to understand why.
    ///
    /// [`hyper_util::server::conn::auto::upgrade::downcast`]: crate::server::conn::auto::upgrade::downcast
    pub fn serve_connection_with_upgrades<I, S, B>(
        &self,
        io: I,
        service: S,
    ) -> UpgradeableConnection<'_, I, S, E>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + Send + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        UpgradeableConnection {
            state: UpgradeableConnState::ReadVersion {
                read_version: read_version(io),
                builder: Cow::Borrowed(self),
                service: Some(service),
            },
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum Version {
    H1,
    H2,
}

impl Version {
    #[must_use]
    #[cfg(any(not(feature = "http2"), not(feature = "http1")))]
    pub fn unsupported(self) -> Error {
        match self {
            Version::H1 => Error::from("HTTP/1 is not supported"),
            Version::H2 => Error::from("HTTP/2 is not supported"),
        }
    }
}

fn read_version<I>(io: I) -> ReadVersion<I>
where
    I: Read + Unpin,
{
    ReadVersion {
        io: Some(io),
        buf: [MaybeUninit::uninit(); 24],
        filled: 0,
        version: Version::H2,
        cancelled: false,
        _pin: PhantomPinned,
    }
}

pin_project! {
    struct ReadVersion<I> {
        io: Option<I>,
        buf: [MaybeUninit<u8>; 24],
        // the amount of `buf` thats been filled
        filled: usize,
        version: Version,
        cancelled: bool,
        // Make this future `!Unpin` for compatibility with async trait methods.
        #[pin]
        _pin: PhantomPinned,
    }
}

impl<I> ReadVersion<I> {
    pub fn cancel(self: Pin<&mut Self>) {
        *self.project().cancelled = true;
    }
}

impl<I> Future for ReadVersion<I>
where
    I: Read + Unpin,
{
    type Output = io::Result<(Version, Rewind<I>)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if *this.cancelled {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled")));
        }

        let mut buf = ReadBuf::uninit(&mut *this.buf);
        // SAFETY: `this.filled` tracks how many bytes have been read (and thus initialized) and
        // we're only advancing by that many.
        unsafe {
            buf.unfilled().advance(*this.filled);
        };

        // We start as H2 and switch to H1 as soon as we don't have the preface.
        while buf.filled().len() < H2_PREFACE.len() {
            let len = buf.filled().len();
            ready!(Pin::new(this.io.as_mut().unwrap()).poll_read(cx, buf.unfilled()))?;
            *this.filled = buf.filled().len();

            // We starts as H2 and switch to H1 when we don't get the preface.
            if buf.filled().len() == len
                || buf.filled()[len..] != H2_PREFACE[len..buf.filled().len()]
            {
                *this.version = Version::H1;
                break;
            }
        }

        let io = this.io.take().unwrap();
        let buf = buf.filled().to_vec();
        Poll::Ready(Ok((
            *this.version,
            Rewind::new_buffered(io, Bytes::from(buf)),
        )))
    }
}

pin_project! {
    /// A [`Future`](core::future::Future) representing an HTTP/1 connection, returned from
    /// [`Builder::serve_connection`](struct.Builder.html#method.serve_connection).
    ///
    /// To drive HTTP on this connection this future **must be polled**, typically with
    /// `.await`. If it isn't polled, no progress will be made on this connection.
    #[must_use = "futures do nothing unless polled"]
    pub struct Connection<'a, I, S, E>
    where
        S: HttpService<Incoming>,
    {
        #[pin]
        state: ConnState<'a, I, S, E>,
    }
}

// A custom COW, since the libstd is has ToOwned bounds that are too eager.
enum Cow<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}

impl<T> std::ops::Deref for Cow<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Cow::Borrowed(t) => &*t,
            Cow::Owned(ref t) => t,
        }
    }
}

#[cfg(feature = "http1")]
type Http1Connection<I, S> = hyper::server::conn::http1::Connection<Rewind<I>, S>;

#[cfg(not(feature = "http1"))]
type Http1Connection<I, S> = (PhantomData<I>, PhantomData<S>);

#[cfg(feature = "http2")]
type Http2Connection<I, S, E> = hyper::server::conn::http2::Connection<Rewind<I>, S, E>;

#[cfg(not(feature = "http2"))]
type Http2Connection<I, S, E> = (PhantomData<I>, PhantomData<S>, PhantomData<E>);

pin_project! {
    #[project = ConnStateProj]
    enum ConnState<'a, I, S, E>
    where
        S: HttpService<Incoming>,
    {
        ReadVersion {
            #[pin]
            read_version: ReadVersion<I>,
            builder: Cow<'a, Builder<E>>,
            service: Option<S>,
        },
        H1 {
            #[pin]
            conn: Http1Connection<I, S>,
        },
        H2 {
            #[pin]
            conn: Http2Connection<I, S, E>,
        },
    }
}

impl<I, S, E, B> Connection<'_, I, S, E>
where
    S: HttpService<Incoming, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: HttpServerConnExec<S::Future, B>,
{
    /// Start a graceful shutdown process for this connection.
    ///
    /// This `Connection` should continue to be polled until shutdown can finish.
    ///
    /// # Note
    ///
    /// This should only be called while the `Connection` future is still pending. If called after
    /// `Connection::poll` has resolved, this does nothing.
    pub fn graceful_shutdown(self: Pin<&mut Self>) {
        match self.project().state.project() {
            ConnStateProj::ReadVersion { read_version, .. } => read_version.cancel(),
            #[cfg(feature = "http1")]
            ConnStateProj::H1 { conn } => conn.graceful_shutdown(),
            #[cfg(feature = "http2")]
            ConnStateProj::H2 { conn } => conn.graceful_shutdown(),
            #[cfg(any(not(feature = "http1"), not(feature = "http2")))]
            _ => unreachable!(),
        }
    }

    /// Make this Connection static, instead of borrowing from Builder.
    pub fn into_owned(self) -> Connection<'static, I, S, E>
    where
        Builder<E>: Clone,
    {
        Connection {
            state: match self.state {
                ConnState::ReadVersion {
                    read_version,
                    builder,
                    service,
                } => ConnState::ReadVersion {
                    read_version,
                    service,
                    builder: Cow::Owned(builder.clone()),
                },
                #[cfg(feature = "http1")]
                ConnState::H1 { conn } => ConnState::H1 { conn },
                #[cfg(feature = "http2")]
                ConnState::H2 { conn } => ConnState::H2 { conn },
                #[cfg(any(not(feature = "http1"), not(feature = "http2")))]
                _ => unreachable!(),
            },
        }
    }
}

impl<I, S, E, B> Future for Connection<'_, I, S, E>
where
    S: Service<Request<Incoming>, Response = Response<B>>,
    S::Future: 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin + 'static,
    E: HttpServerConnExec<S::Future, B>,
{
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();

            match this.state.as_mut().project() {
                ConnStateProj::ReadVersion {
                    read_version,
                    builder,
                    service,
                } => {
                    let (version, io) = ready!(read_version.poll(cx))?;
                    let service = service.take().unwrap();
                    match version {
                        #[cfg(feature = "http1")]
                        Version::H1 => {
                            let conn = builder.http1.serve_connection(io, service);
                            this.state.set(ConnState::H1 { conn });
                        }
                        #[cfg(feature = "http2")]
                        Version::H2 => {
                            let conn = builder.http2.serve_connection(io, service);
                            this.state.set(ConnState::H2 { conn });
                        }
                        #[cfg(any(not(feature = "http1"), not(feature = "http2")))]
                        _ => return Poll::Ready(Err(version.unsupported())),
                    }
                }
                #[cfg(feature = "http1")]
                ConnStateProj::H1 { conn } => {
                    return conn.poll(cx).map_err(Into::into);
                }
                #[cfg(feature = "http2")]
                ConnStateProj::H2 { conn } => {
                    return conn.poll(cx).map_err(Into::into);
                }
                #[cfg(any(not(feature = "http1"), not(feature = "http2")))]
                _ => unreachable!(),
            }
        }
    }
}

pin_project! {
    /// An upgradable [`Connection`], returned by
    /// [`Builder::serve_upgradable_connection`](struct.Builder.html#method.serve_connection_with_upgrades).
    ///
    /// To drive HTTP on this connection this future **must be polled**, typically with
    /// `.await`. If it isn't polled, no progress will be made on this connection.
    #[must_use = "futures do nothing unless polled"]
    pub struct UpgradeableConnection<'a, I, S, E>
    where
        S: HttpService<Incoming>,
    {
        #[pin]
        state: UpgradeableConnState<'a, I, S, E>,
    }
}

#[cfg(feature = "http1")]
type Http1UpgradeableConnection<I, S> = hyper::server::conn::http1::UpgradeableConnection<I, S>;

#[cfg(not(feature = "http1"))]
type Http1UpgradeableConnection<I, S> = (PhantomData<I>, PhantomData<S>);

pin_project! {
    #[project = UpgradeableConnStateProj]
    enum UpgradeableConnState<'a, I, S, E>
    where
        S: HttpService<Incoming>,
    {
        ReadVersion {
            #[pin]
            read_version: ReadVersion<I>,
            builder: Cow<'a, Builder<E>>,
            service: Option<S>,
        },
        H1 {
            #[pin]
            conn: Http1UpgradeableConnection<Rewind<I>, S>,
        },
        H2 {
            #[pin]
            conn: Http2Connection<I, S, E>,
        },
    }
}

impl<I, S, E, B> UpgradeableConnection<'_, I, S, E>
where
    S: HttpService<Incoming, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: HttpServerConnExec<S::Future, B>,
{
    /// Start a graceful shutdown process for this connection.
    ///
    /// This `UpgradeableConnection` should continue to be polled until shutdown can finish.
    ///
    /// # Note
    ///
    /// This should only be called while the `Connection` future is still nothing. pending. If
    /// called after `UpgradeableConnection::poll` has resolved, this does nothing.
    pub fn graceful_shutdown(self: Pin<&mut Self>) {
        match self.project().state.project() {
            UpgradeableConnStateProj::ReadVersion { read_version, .. } => read_version.cancel(),
            #[cfg(feature = "http1")]
            UpgradeableConnStateProj::H1 { conn } => conn.graceful_shutdown(),
            #[cfg(feature = "http2")]
            UpgradeableConnStateProj::H2 { conn } => conn.graceful_shutdown(),
            #[cfg(any(not(feature = "http1"), not(feature = "http2")))]
            _ => unreachable!(),
        }
    }

    /// Make this Connection static, instead of borrowing from Builder.
    pub fn into_owned(self) -> UpgradeableConnection<'static, I, S, E>
    where
        Builder<E>: Clone,
    {
        UpgradeableConnection {
            state: match self.state {
                UpgradeableConnState::ReadVersion {
                    read_version,
                    builder,
                    service,
                } => UpgradeableConnState::ReadVersion {
                    read_version,
                    service,
                    builder: Cow::Owned(builder.clone()),
                },
                #[cfg(feature = "http1")]
                UpgradeableConnState::H1 { conn } => UpgradeableConnState::H1 { conn },
                #[cfg(feature = "http2")]
                UpgradeableConnState::H2 { conn } => UpgradeableConnState::H2 { conn },
                #[cfg(any(not(feature = "http1"), not(feature = "http2")))]
                _ => unreachable!(),
            },
        }
    }
}

impl<I, S, E, B> Future for UpgradeableConnection<'_, I, S, E>
where
    S: Service<Request<Incoming>, Response = Response<B>>,
    S::Future: 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin + Send + 'static,
    E: HttpServerConnExec<S::Future, B>,
{
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();

            match this.state.as_mut().project() {
                UpgradeableConnStateProj::ReadVersion {
                    read_version,
                    builder,
                    service,
                } => {
                    let (version, io) = ready!(read_version.poll(cx))?;
                    let service = service.take().unwrap();
                    match version {
                        #[cfg(feature = "http1")]
                        Version::H1 => {
                            let conn = builder.http1.serve_connection(io, service).with_upgrades();
                            this.state.set(UpgradeableConnState::H1 { conn });
                        }
                        #[cfg(feature = "http2")]
                        Version::H2 => {
                            let conn = builder.http2.serve_connection(io, service);
                            this.state.set(UpgradeableConnState::H2 { conn });
                        }
                        #[cfg(any(not(feature = "http1"), not(feature = "http2")))]
                        _ => return Poll::Ready(Err(version.unsupported())),
                    }
                }
                #[cfg(feature = "http1")]
                UpgradeableConnStateProj::H1 { conn } => {
                    return conn.poll(cx).map_err(Into::into);
                }
                #[cfg(feature = "http2")]
                UpgradeableConnStateProj::H2 { conn } => {
                    return conn.poll(cx).map_err(Into::into);
                }
                #[cfg(any(not(feature = "http1"), not(feature = "http2")))]
                _ => unreachable!(),
            }
        }
    }
}

/// Http1 part of builder.
#[cfg(feature = "http1")]
pub struct Http1Builder<'a, E> {
    inner: &'a mut Builder<E>,
}

#[cfg(feature = "http1")]
impl<E> Http1Builder<'_, E> {
    /// Http2 configuration.
    #[cfg(feature = "http2")]
    pub fn http2(&mut self) -> Http2Builder<'_, E> {
        Http2Builder { inner: self.inner }
    }

    /// Set whether the `date` header should be included in HTTP responses.
    ///
    /// Note that including the `date` header is recommended by RFC 7231.
    ///
    /// Default is true.
    pub fn auto_date_header(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.auto_date_header(enabled);
        self
    }

    /// Set whether HTTP/1 connections should support half-closures.
    ///
    /// Clients can chose to shutdown their write-side while waiting
    /// for the server to respond. Setting this to `true` will
    /// prevent closing the connection immediately if `read`
    /// detects an EOF in the middle of a request.
    ///
    /// Default is `false`.
    pub fn half_close(&mut self, val: bool) -> &mut Self {
        self.inner.http1.half_close(val);
        self
    }

    /// Enables or disables HTTP/1 keep-alive.
    ///
    /// Default is true.
    pub fn keep_alive(&mut self, val: bool) -> &mut Self {
        self.inner.http1.keep_alive(val);
        self
    }

    /// Set whether HTTP/1 connections will write header names as title case at
    /// the socket level.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is false.
    pub fn title_case_headers(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.title_case_headers(enabled);
        self
    }

    /// Set whether HTTP/1 connections will silently ignored malformed header lines.
    ///
    /// If this is enabled and a header line does not start with a valid header
    /// name, or does not include a colon at all, the line will be silently ignored
    /// and no error will be reported.
    ///
    /// Default is false.
    pub fn ignore_invalid_headers(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.ignore_invalid_headers(enabled);
        self
    }

    /// Set whether to support preserving original header cases.
    ///
    /// Currently, this will record the original cases received, and store them
    /// in a private extension on the `Request`. It will also look for and use
    /// such an extension in any provided `Response`.
    ///
    /// Since the relevant extension is still private, there is no way to
    /// interact with the original cases. The only effect this can have now is
    /// to forward the cases in a proxy-like fashion.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is false.
    pub fn preserve_header_case(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.preserve_header_case(enabled);
        self
    }

    /// Set the maximum number of headers.
    ///
    /// When a request is received, the parser will reserve a buffer to store headers for optimal
    /// performance.
    ///
    /// If server receives more headers than the buffer size, it responds to the client with
    /// "431 Request Header Fields Too Large".
    ///
    /// The headers is allocated on the stack by default, which has higher performance. After
    /// setting this value, headers will be allocated in heap memory, that is, heap memory
    /// allocation will occur for each request, and there will be a performance drop of about 5%.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is 100.
    pub fn max_headers(&mut self, val: usize) -> &mut Self {
        self.inner.http1.max_headers(val);
        self
    }

    /// Set a timeout for reading client request headers. If a client does not
    /// transmit the entire header within this time, the connection is closed.
    ///
    /// Requires a [`Timer`] set by [`Http1Builder::timer`] to take effect. Panics if `header_read_timeout` is configured
    /// without a [`Timer`].
    ///
    /// Pass `None` to disable.
    ///
    /// Default is currently 30 seconds, but do not depend on that.
    pub fn header_read_timeout(&mut self, read_timeout: impl Into<Option<Duration>>) -> &mut Self {
        self.inner.http1.header_read_timeout(read_timeout);
        self
    }

    /// Set whether HTTP/1 connections should try to use vectored writes,
    /// or always flatten into a single buffer.
    ///
    /// Note that setting this to false may mean more copies of body data,
    /// but may also improve performance when an IO transport doesn't
    /// support vectored writes well, such as most TLS implementations.
    ///
    /// Setting this to true will force hyper to use queued strategy
    /// which may eliminate unnecessary cloning on some TLS backends
    ///
    /// Default is `auto`. In this mode hyper will try to guess which
    /// mode to use
    pub fn writev(&mut self, val: bool) -> &mut Self {
        self.inner.http1.writev(val);
        self
    }

    /// Set the maximum buffer size for the connection.
    ///
    /// Default is ~400kb.
    ///
    /// # Panics
    ///
    /// The minimum value allowed is 8192. This method panics if the passed `max` is less than the minimum.
    pub fn max_buf_size(&mut self, max: usize) -> &mut Self {
        self.inner.http1.max_buf_size(max);
        self
    }

    /// Aggregates flushes to better support pipelined responses.
    ///
    /// Experimental, may have bugs.
    ///
    /// Default is false.
    pub fn pipeline_flush(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.pipeline_flush(enabled);
        self
    }

    /// Set the timer used in background tasks.
    pub fn timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Send + Sync + 'static,
    {
        self.inner.http1.timer(timer);
        self
    }

    /// Bind a connection together with a [`Service`].
    #[cfg(feature = "http2")]
    pub async fn serve_connection<I, S, B>(&self, io: I, service: S) -> Result<()>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        self.inner.serve_connection(io, service).await
    }

    /// Bind a connection together with a [`Service`].
    #[cfg(not(feature = "http2"))]
    pub async fn serve_connection<I, S, B>(&self, io: I, service: S) -> Result<()>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + 'static,
    {
        self.inner.serve_connection(io, service).await
    }

    /// Bind a connection together with a [`Service`], with the ability to
    /// handle HTTP upgrades. This requires that the IO object implements
    /// `Send`.
    #[cfg(feature = "http2")]
    pub fn serve_connection_with_upgrades<I, S, B>(
        &self,
        io: I,
        service: S,
    ) -> UpgradeableConnection<'_, I, S, E>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + Send + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        self.inner.serve_connection_with_upgrades(io, service)
    }
}

/// Http2 part of builder.
#[cfg(feature = "http2")]
pub struct Http2Builder<'a, E> {
    inner: &'a mut Builder<E>,
}

#[cfg(feature = "http2")]
impl<E> Http2Builder<'_, E> {
    #[cfg(feature = "http1")]
    /// Http1 configuration.
    pub fn http1(&mut self) -> Http1Builder<'_, E> {
        Http1Builder { inner: self.inner }
    }

    /// Configures the maximum number of pending reset streams allowed before a GOAWAY will be sent.
    ///
    /// This will default to the default value set by the [`h2` crate](https://crates.io/crates/h2).
    /// As of v0.4.0, it is 20.
    ///
    /// See <https://github.com/hyperium/hyper/issues/2877> for more information.
    pub fn max_pending_accept_reset_streams(&mut self, max: impl Into<Option<usize>>) -> &mut Self {
        self.inner.http2.max_pending_accept_reset_streams(max);
        self
    }

    /// Configures the maximum number of local reset streams allowed before a GOAWAY will be sent.
    ///
    /// If not set, hyper will use a default, currently of 1024.
    ///
    /// If `None` is supplied, hyper will not apply any limit.
    /// This is not advised, as it can potentially expose servers to DOS vulnerabilities.
    ///
    /// See <https://rustsec.org/advisories/RUSTSEC-2024-0003.html> for more information.
    pub fn max_local_error_reset_streams(&mut self, max: impl Into<Option<usize>>) -> &mut Self {
        self.inner.http2.max_local_error_reset_streams(max);
        self
    }

    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    ///
    /// [spec]: https://http2.github.io/http2-spec/#SETTINGS_INITIAL_WINDOW_SIZE
    pub fn initial_stream_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.inner.http2.initial_stream_window_size(sz);
        self
    }

    /// Sets the max connection-level flow control for HTTP2.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    pub fn initial_connection_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.inner.http2.initial_connection_window_size(sz);
        self
    }

    /// Sets whether to use an adaptive flow control.
    ///
    /// Enabling this will override the limits set in
    /// `http2_initial_stream_window_size` and
    /// `http2_initial_connection_window_size`.
    pub fn adaptive_window(&mut self, enabled: bool) -> &mut Self {
        self.inner.http2.adaptive_window(enabled);
        self
    }

    /// Sets the maximum frame size to use for HTTP2.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    pub fn max_frame_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.inner.http2.max_frame_size(sz);
        self
    }

    /// Sets the [`SETTINGS_MAX_CONCURRENT_STREAMS`][spec] option for HTTP2
    /// connections.
    ///
    /// Default is 200. Passing `None` will remove any limit.
    ///
    /// [spec]: https://http2.github.io/http2-spec/#SETTINGS_MAX_CONCURRENT_STREAMS
    pub fn max_concurrent_streams(&mut self, max: impl Into<Option<u32>>) -> &mut Self {
        self.inner.http2.max_concurrent_streams(max);
        self
    }

    /// Sets an interval for HTTP2 Ping frames should be sent to keep a
    /// connection alive.
    ///
    /// Pass `None` to disable HTTP2 keep-alive.
    ///
    /// Default is currently disabled.
    ///
    /// # Cargo Feature
    ///
    pub fn keep_alive_interval(&mut self, interval: impl Into<Option<Duration>>) -> &mut Self {
        self.inner.http2.keep_alive_interval(interval);
        self
    }

    /// Sets a timeout for receiving an acknowledgement of the keep-alive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will
    /// be closed. Does nothing if `http2_keep_alive_interval` is disabled.
    ///
    /// Default is 20 seconds.
    ///
    /// # Cargo Feature
    ///
    pub fn keep_alive_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.inner.http2.keep_alive_timeout(timeout);
        self
    }

    /// Set the maximum write buffer size for each HTTP/2 stream.
    ///
    /// Default is currently ~400KB, but may change.
    ///
    /// # Panics
    ///
    /// The value must be no larger than `u32::MAX`.
    pub fn max_send_buf_size(&mut self, max: usize) -> &mut Self {
        self.inner.http2.max_send_buf_size(max);
        self
    }

    /// Enables the [extended CONNECT protocol].
    ///
    /// [extended CONNECT protocol]: https://datatracker.ietf.org/doc/html/rfc8441#section-4
    pub fn enable_connect_protocol(&mut self) -> &mut Self {
        self.inner.http2.enable_connect_protocol();
        self
    }

    /// Sets the max size of received header frames.
    ///
    /// Default is currently ~16MB, but may change.
    pub fn max_header_list_size(&mut self, max: u32) -> &mut Self {
        self.inner.http2.max_header_list_size(max);
        self
    }

    /// Set the timer used in background tasks.
    pub fn timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Send + Sync + 'static,
    {
        self.inner.http2.timer(timer);
        self
    }

    /// Set whether the `date` header should be included in HTTP responses.
    ///
    /// Note that including the `date` header is recommended by RFC 7231.
    ///
    /// Default is true.
    pub fn auto_date_header(&mut self, enabled: bool) -> &mut Self {
        self.inner.http2.auto_date_header(enabled);
        self
    }

    /// Bind a connection together with a [`Service`].
    pub async fn serve_connection<I, S, B>(&self, io: I, service: S) -> Result<()>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        self.inner.serve_connection(io, service).await
    }

    /// Bind a connection together with a [`Service`], with the ability to
    /// handle HTTP upgrades. This requires that the IO object implements
    /// `Send`.
    pub fn serve_connection_with_upgrades<I, S, B>(
        &self,
        io: I,
        service: S,
    ) -> UpgradeableConnection<'_, I, S, E>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + Send + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        self.inner.serve_connection_with_upgrades(io, service)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        rt::{TokioExecutor, TokioIo},
        server::conn::auto,
    };
    use http::{Request, Response};
    use http_body::Body;
    use http_body_util::{BodyExt, Empty, Full};
    use hyper::{body, body::Bytes, client, service::service_fn};
    use std::{convert::Infallible, error::Error as StdError, net::SocketAddr, time::Duration};
    use tokio::{
        net::{TcpListener, TcpStream},
        pin,
    };

    const BODY: &[u8] = b"Hello, world!";

    #[test]
    fn configuration() {
        // One liner.
        auto::Builder::new(TokioExecutor::new())
            .http1()
            .keep_alive(true)
            .http2()
            .keep_alive_interval(None);
        //  .serve_connection(io, service);

        // Using variable.
        let mut builder = auto::Builder::new(TokioExecutor::new());

        builder.http1().keep_alive(true);
        builder.http2().keep_alive_interval(None);
        // builder.serve_connection(io, service);
    }

    #[test]
    #[cfg(feature = "http1")]
    fn title_case_headers_configuration() {
        // Test title_case_headers can be set on the main builder
        auto::Builder::new(TokioExecutor::new()).title_case_headers(true);

        // Can be combined with other configuration
        auto::Builder::new(TokioExecutor::new())
            .title_case_headers(true)
            .http1_only();
    }

    #[test]
    #[cfg(feature = "http1")]
    fn preserve_header_case_configuration() {
        // Test preserve_header_case can be set on the main builder
        auto::Builder::new(TokioExecutor::new()).preserve_header_case(true);

        // Can be combined with other configuration
        auto::Builder::new(TokioExecutor::new())
            .preserve_header_case(true)
            .http1_only();
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn http1() {
        let addr = start_server(false, false).await;
        let mut sender = connect_h1(addr).await;

        let response = sender
            .send_request(Request::new(Empty::<Bytes>::new()))
            .await
            .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();

        assert_eq!(body, BODY);
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn http2() {
        let addr = start_server(false, false).await;
        let mut sender = connect_h2(addr).await;

        let response = sender
            .send_request(Request::new(Empty::<Bytes>::new()))
            .await
            .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();

        assert_eq!(body, BODY);
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn http2_only() {
        let addr = start_server(false, true).await;
        let mut sender = connect_h2(addr).await;

        let response = sender
            .send_request(Request::new(Empty::<Bytes>::new()))
            .await
            .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();

        assert_eq!(body, BODY);
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn http2_only_fail_if_client_is_http1() {
        let addr = start_server(false, true).await;
        let mut sender = connect_h1(addr).await;

        let _ = sender
            .send_request(Request::new(Empty::<Bytes>::new()))
            .await
            .expect_err("should fail");
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn http1_only() {
        let addr = start_server(true, false).await;
        let mut sender = connect_h1(addr).await;

        let response = sender
            .send_request(Request::new(Empty::<Bytes>::new()))
            .await
            .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();

        assert_eq!(body, BODY);
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn http1_only_fail_if_client_is_http2() {
        let addr = start_server(true, false).await;
        let mut sender = connect_h2(addr).await;

        let _ = sender
            .send_request(Request::new(Empty::<Bytes>::new()))
            .await
            .expect_err("should fail");
    }

    #[cfg(not(miri))]
    #[tokio::test]
    async fn graceful_shutdown() {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();

        let listener_addr = listener.local_addr().unwrap();

        // Spawn the task in background so that we can connect there
        let listen_task = tokio::spawn(async move { listener.accept().await.unwrap() });
        // Only connect a stream, do not send headers or anything
        let _stream = TcpStream::connect(listener_addr).await.unwrap();

        let (stream, _) = listen_task.await.unwrap();
        let stream = TokioIo::new(stream);
        let builder = auto::Builder::new(TokioExecutor::new());
        let connection = builder.serve_connection(stream, service_fn(hello));

        pin!(connection);

        connection.as_mut().graceful_shutdown();

        let connection_error = tokio::time::timeout(Duration::from_millis(200), connection)
            .await
            .expect("Connection should have finished in a timely manner after graceful shutdown.")
            .expect_err("Connection should have been interrupted.");

        let connection_error = connection_error
            .downcast_ref::<std::io::Error>()
            .expect("The error should have been `std::io::Error`.");
        assert_eq!(connection_error.kind(), std::io::ErrorKind::Interrupted);
    }

    async fn connect_h1<B>(addr: SocketAddr) -> client::conn::http1::SendRequest<B>
    where
        B: Body + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        let stream = TokioIo::new(TcpStream::connect(addr).await.unwrap());
        let (sender, connection) = client::conn::http1::handshake(stream).await.unwrap();

        tokio::spawn(connection);

        sender
    }

    async fn connect_h2<B>(addr: SocketAddr) -> client::conn::http2::SendRequest<B>
    where
        B: Body + Unpin + Send + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        let stream = TokioIo::new(TcpStream::connect(addr).await.unwrap());
        let (sender, connection) = client::conn::http2::Builder::new(TokioExecutor::new())
            .handshake(stream)
            .await
            .unwrap();

        tokio::spawn(connection);

        sender
    }

    async fn start_server(h1_only: bool, h2_only: bool) -> SocketAddr {
        let addr: SocketAddr = ([127, 0, 0, 1], 0).into();
        let listener = TcpListener::bind(addr).await.unwrap();

        let local_addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let stream = TokioIo::new(stream);
                tokio::task::spawn(async move {
                    let mut builder = auto::Builder::new(TokioExecutor::new());
                    if h1_only {
                        builder = builder.http1_only();
                        builder.serve_connection(stream, service_fn(hello)).await
                    } else if h2_only {
                        builder = builder.http2_only();
                        builder.serve_connection(stream, service_fn(hello)).await
                    } else {
                        builder
                            .http2()
                            .max_header_list_size(4096)
                            .serve_connection_with_upgrades(stream, service_fn(hello))
                            .await
                    }
                    .unwrap();
                });
            }
        });

        local_addr
    }

    async fn hello(_req: Request<body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
        Ok(Response::new(Full::new(Bytes::from(BODY))))
    }
}
