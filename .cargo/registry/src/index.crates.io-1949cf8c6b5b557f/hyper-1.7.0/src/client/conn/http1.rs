//! HTTP/1 client connections

use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::rt::{Read, Write};
use bytes::Bytes;
use futures_core::ready;
use http::{Request, Response};
use httparse::ParserConfig;

use super::super::dispatch::{self, TrySendError};
use crate::body::{Body, Incoming as IncomingBody};
use crate::proto;

type Dispatcher<T, B> =
    proto::dispatch::Dispatcher<proto::dispatch::Client<B>, B, T, proto::h1::ClientTransaction>;

/// The sender side of an established connection.
pub struct SendRequest<B> {
    dispatch: dispatch::Sender<Request<B>, Response<IncomingBody>>,
}

/// Deconstructed parts of a `Connection`.
///
/// This allows taking apart a `Connection` at a later time, in order to
/// reclaim the IO object, and additional related pieces.
#[derive(Debug)]
#[non_exhaustive]
pub struct Parts<T> {
    /// The original IO object used in the handshake.
    pub io: T,
    /// A buffer of bytes that have been read but not processed as HTTP.
    ///
    /// For instance, if the `Connection` is used for an HTTP upgrade request,
    /// it is possible the server sent back the first bytes of the new protocol
    /// along with the response upgrade.
    ///
    /// You will want to check for any existing bytes if you plan to continue
    /// communicating on the IO object.
    pub read_buf: Bytes,
}

/// A future that processes all HTTP state for the IO object.
///
/// In most cases, this should just be spawned into an executor, so that it
/// can process incoming and outgoing messages, notice hangups, and the like.
///
/// Instances of this type are typically created via the [`handshake`] function
#[must_use = "futures do nothing unless polled"]
pub struct Connection<T, B>
where
    T: Read + Write,
    B: Body + 'static,
{
    inner: Dispatcher<T, B>,
}

impl<T, B> Connection<T, B>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    /// Return the inner IO object, and additional information.
    ///
    /// Only works for HTTP/1 connections. HTTP/2 connections will panic.
    pub fn into_parts(self) -> Parts<T> {
        let (io, read_buf, _) = self.inner.into_inner();
        Parts { io, read_buf }
    }

    /// Poll the connection for completion, but without calling `shutdown`
    /// on the underlying IO.
    ///
    /// This is useful to allow running a connection while doing an HTTP
    /// upgrade. Once the upgrade is completed, the connection would be "done",
    /// but it is not desired to actually shutdown the IO object. Instead you
    /// would take it back using `into_parts`.
    ///
    /// Use [`poll_fn`](https://docs.rs/futures/0.1.25/futures/future/fn.poll_fn.html)
    /// and [`try_ready!`](https://docs.rs/futures/0.1.25/futures/macro.try_ready.html)
    /// to work with this function; or use the `without_shutdown` wrapper.
    pub fn poll_without_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<crate::Result<()>> {
        self.inner.poll_without_shutdown(cx)
    }

    /// Prevent shutdown of the underlying IO object at the end of service the request,
    /// instead run `into_parts`. This is a convenience wrapper over `poll_without_shutdown`.
    pub async fn without_shutdown(self) -> crate::Result<Parts<T>> {
        let mut conn = Some(self);
        crate::common::future::poll_fn(move |cx| -> Poll<crate::Result<Parts<T>>> {
            ready!(conn.as_mut().unwrap().poll_without_shutdown(cx))?;
            Poll::Ready(Ok(conn.take().unwrap().into_parts()))
        })
        .await
    }
}

/// A builder to configure an HTTP connection.
///
/// After setting options, the builder is used to create a handshake future.
///
/// **Note**: The default values of options are *not considered stable*. They
/// are subject to change at any time.
#[derive(Clone, Debug)]
pub struct Builder {
    h09_responses: bool,
    h1_parser_config: ParserConfig,
    h1_writev: Option<bool>,
    h1_title_case_headers: bool,
    h1_preserve_header_case: bool,
    h1_max_headers: Option<usize>,
    #[cfg(feature = "ffi")]
    h1_preserve_header_order: bool,
    h1_read_buf_exact_size: Option<usize>,
    h1_max_buf_size: Option<usize>,
}

/// Returns a handshake future over some IO.
///
/// This is a shortcut for `Builder::new().handshake(io)`.
/// See [`client::conn`](crate::client::conn) for more.
pub async fn handshake<T, B>(io: T) -> crate::Result<(SendRequest<B>, Connection<T, B>)>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    Builder::new().handshake(io).await
}

// ===== impl SendRequest

impl<B> SendRequest<B> {
    /// Polls to determine whether this sender can be used yet for a request.
    ///
    /// If the associated connection is closed, this returns an Error.
    pub fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<crate::Result<()>> {
        self.dispatch.poll_ready(cx)
    }

    /// Waits until the dispatcher is ready
    ///
    /// If the associated connection is closed, this returns an Error.
    pub async fn ready(&mut self) -> crate::Result<()> {
        crate::common::future::poll_fn(|cx| self.poll_ready(cx)).await
    }

    /// Checks if the connection is currently ready to send a request.
    ///
    /// # Note
    ///
    /// This is mostly a hint. Due to inherent latency of networks, it is
    /// possible that even after checking this is ready, sending a request
    /// may still fail because the connection was closed in the meantime.
    pub fn is_ready(&self) -> bool {
        self.dispatch.is_ready()
    }

    /// Checks if the connection side has been closed.
    pub fn is_closed(&self) -> bool {
        self.dispatch.is_closed()
    }
}

impl<B> SendRequest<B>
where
    B: Body + 'static,
{
    /// Sends a `Request` on the associated connection.
    ///
    /// Returns a future that if successful, yields the `Response`.
    ///
    /// `req` must have a `Host` header.
    ///
    /// # Uri
    ///
    /// The `Uri` of the request is serialized as-is.
    ///
    /// - Usually you want origin-form (`/path?query`).
    /// - For sending to an HTTP proxy, you want to send in absolute-form
    ///   (`https://hyper.rs/guides`).
    ///
    /// This is however not enforced or validated and it is up to the user
    /// of this method to ensure the `Uri` is correct for their intended purpose.
    pub fn send_request(
        &mut self,
        req: Request<B>,
    ) -> impl Future<Output = crate::Result<Response<IncomingBody>>> {
        let sent = self.dispatch.send(req);

        async move {
            match sent {
                Ok(rx) => match rx.await {
                    Ok(Ok(resp)) => Ok(resp),
                    Ok(Err(err)) => Err(err),
                    // this is definite bug if it happens, but it shouldn't happen!
                    Err(_canceled) => panic!("dispatch dropped without returning error"),
                },
                Err(_req) => {
                    debug!("connection was not ready");
                    Err(crate::Error::new_canceled().with("connection was not ready"))
                }
            }
        }
    }

    /// Sends a `Request` on the associated connection.
    ///
    /// Returns a future that if successful, yields the `Response`.
    ///
    /// # Error
    ///
    /// If there was an error before trying to serialize the request to the
    /// connection, the message will be returned as part of this error.
    pub fn try_send_request(
        &mut self,
        req: Request<B>,
    ) -> impl Future<Output = Result<Response<IncomingBody>, TrySendError<Request<B>>>> {
        let sent = self.dispatch.try_send(req);
        async move {
            match sent {
                Ok(rx) => match rx.await {
                    Ok(Ok(res)) => Ok(res),
                    Ok(Err(err)) => Err(err),
                    // this is definite bug if it happens, but it shouldn't happen!
                    Err(_) => panic!("dispatch dropped without returning error"),
                },
                Err(req) => {
                    debug!("connection was not ready");
                    let error = crate::Error::new_canceled().with("connection was not ready");
                    Err(TrySendError {
                        error,
                        message: Some(req),
                    })
                }
            }
        }
    }
}

impl<B> fmt::Debug for SendRequest<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendRequest").finish()
    }
}

// ===== impl Connection

impl<T, B> Connection<T, B>
where
    T: Read + Write + Unpin + Send,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    /// Enable this connection to support higher-level HTTP upgrades.
    ///
    /// See [the `upgrade` module](crate::upgrade) for more.
    pub fn with_upgrades(self) -> upgrades::UpgradeableConnection<T, B> {
        upgrades::UpgradeableConnection { inner: Some(self) }
    }
}

impl<T, B> fmt::Debug for Connection<T, B>
where
    T: Read + Write + fmt::Debug,
    B: Body + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}

impl<T, B> Future for Connection<T, B>
where
    T: Read + Write + Unpin,
    B: Body + 'static,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(Pin::new(&mut self.inner).poll(cx))? {
            proto::Dispatched::Shutdown => Poll::Ready(Ok(())),
            proto::Dispatched::Upgrade(pending) => {
                // With no `Send` bound on `I`, we can't try to do
                // upgrades here. In case a user was trying to use
                // `upgrade` with this API, send a special
                // error letting them know about that.
                pending.manual();
                Poll::Ready(Ok(()))
            }
        }
    }
}

// ===== impl Builder

impl Builder {
    /// Creates a new connection builder.
    #[inline]
    pub fn new() -> Builder {
        Builder {
            h09_responses: false,
            h1_writev: None,
            h1_read_buf_exact_size: None,
            h1_parser_config: Default::default(),
            h1_title_case_headers: false,
            h1_preserve_header_case: false,
            h1_max_headers: None,
            #[cfg(feature = "ffi")]
            h1_preserve_header_order: false,
            h1_max_buf_size: None,
        }
    }

    /// Set whether HTTP/0.9 responses should be tolerated.
    ///
    /// Default is false.
    pub fn http09_responses(&mut self, enabled: bool) -> &mut Builder {
        self.h09_responses = enabled;
        self
    }

    /// Set whether HTTP/1 connections will accept spaces between header names
    /// and the colon that follow them in responses.
    ///
    /// You probably don't need this, here is what [RFC 7230 Section 3.2.4.] has
    /// to say about it:
    ///
    /// > No whitespace is allowed between the header field-name and colon. In
    /// > the past, differences in the handling of such whitespace have led to
    /// > security vulnerabilities in request routing and response handling. A
    /// > server MUST reject any received request message that contains
    /// > whitespace between a header field-name and colon with a response code
    /// > of 400 (Bad Request). A proxy MUST remove any such whitespace from a
    /// > response message before forwarding the message downstream.
    ///
    /// Default is false.
    ///
    /// [RFC 7230 Section 3.2.4.]: https://tools.ietf.org/html/rfc7230#section-3.2.4
    pub fn allow_spaces_after_header_name_in_responses(&mut self, enabled: bool) -> &mut Builder {
        self.h1_parser_config
            .allow_spaces_after_header_name_in_responses(enabled);
        self
    }

    /// Set whether HTTP/1 connections will accept obsolete line folding for
    /// header values.
    ///
    /// Newline codepoints (`\r` and `\n`) will be transformed to spaces when
    /// parsing.
    ///
    /// You probably don't need this, here is what [RFC 7230 Section 3.2.4.] has
    /// to say about it:
    ///
    /// > A server that receives an obs-fold in a request message that is not
    /// > within a message/http container MUST either reject the message by
    /// > sending a 400 (Bad Request), preferably with a representation
    /// > explaining that obsolete line folding is unacceptable, or replace
    /// > each received obs-fold with one or more SP octets prior to
    /// > interpreting the field value or forwarding the message downstream.
    ///
    /// > A proxy or gateway that receives an obs-fold in a response message
    /// > that is not within a message/http container MUST either discard the
    /// > message and replace it with a 502 (Bad Gateway) response, preferably
    /// > with a representation explaining that unacceptable line folding was
    /// > received, or replace each received obs-fold with one or more SP
    /// > octets prior to interpreting the field value or forwarding the
    /// > message downstream.
    ///
    /// > A user agent that receives an obs-fold in a response message that is
    /// > not within a message/http container MUST replace each received
    /// > obs-fold with one or more SP octets prior to interpreting the field
    /// > value.
    ///
    /// Default is false.
    ///
    /// [RFC 7230 Section 3.2.4.]: https://tools.ietf.org/html/rfc7230#section-3.2.4
    pub fn allow_obsolete_multiline_headers_in_responses(&mut self, enabled: bool) -> &mut Builder {
        self.h1_parser_config
            .allow_obsolete_multiline_headers_in_responses(enabled);
        self
    }

    /// Set whether HTTP/1 connections will silently ignored malformed header lines.
    ///
    /// If this is enabled and a header line does not start with a valid header
    /// name, or does not include a colon at all, the line will be silently ignored
    /// and no error will be reported.
    ///
    /// Default is false.
    pub fn ignore_invalid_headers_in_responses(&mut self, enabled: bool) -> &mut Builder {
        self.h1_parser_config
            .ignore_invalid_headers_in_responses(enabled);
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
    pub fn writev(&mut self, enabled: bool) -> &mut Builder {
        self.h1_writev = Some(enabled);
        self
    }

    /// Set whether HTTP/1 connections will write header names as title case at
    /// the socket level.
    ///
    /// Default is false.
    pub fn title_case_headers(&mut self, enabled: bool) -> &mut Builder {
        self.h1_title_case_headers = enabled;
        self
    }

    /// Set whether to support preserving original header cases.
    ///
    /// Currently, this will record the original cases received, and store them
    /// in a private extension on the `Response`. It will also look for and use
    /// such an extension in any provided `Request`.
    ///
    /// Since the relevant extension is still private, there is no way to
    /// interact with the original cases. The only effect this can have now is
    /// to forward the cases in a proxy-like fashion.
    ///
    /// Default is false.
    pub fn preserve_header_case(&mut self, enabled: bool) -> &mut Builder {
        self.h1_preserve_header_case = enabled;
        self
    }

    /// Set the maximum number of headers.
    ///
    /// When a response is received, the parser will reserve a buffer to store headers for optimal
    /// performance.
    ///
    /// If client receives more headers than the buffer size, the error "message header too large"
    /// is returned.
    ///
    /// Note that headers is allocated on the stack by default, which has higher performance. After
    /// setting this value, headers will be allocated in heap memory, that is, heap memory
    /// allocation will occur for each response, and there will be a performance drop of about 5%.
    ///
    /// Default is 100.
    pub fn max_headers(&mut self, val: usize) -> &mut Self {
        self.h1_max_headers = Some(val);
        self
    }

    /// Set whether to support preserving original header order.
    ///
    /// Currently, this will record the order in which headers are received, and store this
    /// ordering in a private extension on the `Response`. It will also look for and use
    /// such an extension in any provided `Request`.
    ///
    /// Default is false.
    #[cfg(feature = "ffi")]
    pub fn preserve_header_order(&mut self, enabled: bool) -> &mut Builder {
        self.h1_preserve_header_order = enabled;
        self
    }

    /// Sets the exact size of the read buffer to *always* use.
    ///
    /// Note that setting this option unsets the `max_buf_size` option.
    ///
    /// Default is an adaptive read buffer.
    pub fn read_buf_exact_size(&mut self, sz: Option<usize>) -> &mut Builder {
        self.h1_read_buf_exact_size = sz;
        self.h1_max_buf_size = None;
        self
    }

    /// Set the maximum buffer size for the connection.
    ///
    /// Default is ~400kb.
    ///
    /// Note that setting this option unsets the `read_exact_buf_size` option.
    ///
    /// # Panics
    ///
    /// The minimum value allowed is 8192. This method panics if the passed `max` is less than the minimum.
    pub fn max_buf_size(&mut self, max: usize) -> &mut Self {
        assert!(
            max >= proto::h1::MINIMUM_MAX_BUFFER_SIZE,
            "the max_buf_size cannot be smaller than the minimum that h1 specifies."
        );

        self.h1_max_buf_size = Some(max);
        self.h1_read_buf_exact_size = None;
        self
    }

    /// Constructs a connection with the configured options and IO.
    /// See [`client::conn`](crate::client::conn) for more.
    ///
    /// Note, if [`Connection`] is not `await`-ed, [`SendRequest`] will
    /// do nothing.
    pub fn handshake<T, B>(
        &self,
        io: T,
    ) -> impl Future<Output = crate::Result<(SendRequest<B>, Connection<T, B>)>>
    where
        T: Read + Write + Unpin,
        B: Body + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        let opts = self.clone();

        async move {
            trace!("client handshake HTTP/1");

            let (tx, rx) = dispatch::channel();
            let mut conn = proto::Conn::new(io);
            conn.set_h1_parser_config(opts.h1_parser_config);
            if let Some(writev) = opts.h1_writev {
                if writev {
                    conn.set_write_strategy_queue();
                } else {
                    conn.set_write_strategy_flatten();
                }
            }
            if opts.h1_title_case_headers {
                conn.set_title_case_headers();
            }
            if opts.h1_preserve_header_case {
                conn.set_preserve_header_case();
            }
            if let Some(max_headers) = opts.h1_max_headers {
                conn.set_http1_max_headers(max_headers);
            }
            #[cfg(feature = "ffi")]
            if opts.h1_preserve_header_order {
                conn.set_preserve_header_order();
            }

            if opts.h09_responses {
                conn.set_h09_responses();
            }

            if let Some(sz) = opts.h1_read_buf_exact_size {
                conn.set_read_buf_exact_size(sz);
            }
            if let Some(max) = opts.h1_max_buf_size {
                conn.set_max_buf_size(max);
            }
            let cd = proto::h1::dispatch::Client::new(rx);
            let proto = proto::h1::Dispatcher::new(cd, conn);

            Ok((SendRequest { dispatch: tx }, Connection { inner: proto }))
        }
    }
}

mod upgrades {
    use crate::upgrade::Upgraded;

    use super::*;

    // A future binding a connection with a Service with Upgrade support.
    //
    // This type is unnameable outside the crate.
    #[must_use = "futures do nothing unless polled"]
    #[allow(missing_debug_implementations)]
    pub struct UpgradeableConnection<T, B>
    where
        T: Read + Write + Unpin + Send + 'static,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        pub(super) inner: Option<Connection<T, B>>,
    }

    impl<I, B> Future for UpgradeableConnection<I, B>
    where
        I: Read + Write + Unpin + Send + 'static,
        B: Body + 'static,
        B::Data: Send,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
    {
        type Output = crate::Result<()>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            match ready!(Pin::new(&mut self.inner.as_mut().unwrap().inner).poll(cx)) {
                Ok(proto::Dispatched::Shutdown) => Poll::Ready(Ok(())),
                Ok(proto::Dispatched::Upgrade(pending)) => {
                    let Parts { io, read_buf } = self.inner.take().unwrap().into_parts();
                    pending.fulfill(Upgraded::new(io, read_buf));
                    Poll::Ready(Ok(()))
                }
                Err(e) => Poll::Ready(Err(e)),
            }
        }
    }
}
