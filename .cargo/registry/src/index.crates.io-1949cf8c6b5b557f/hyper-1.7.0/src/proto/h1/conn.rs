use std::fmt;
#[cfg(feature = "server")]
use std::future::Future;
use std::io;
use std::marker::{PhantomData, Unpin};
use std::pin::Pin;
use std::task::{Context, Poll};
#[cfg(feature = "server")]
use std::time::{Duration, Instant};

use crate::rt::{Read, Write};
use bytes::{Buf, Bytes};
use futures_core::ready;
use http::header::{HeaderValue, CONNECTION, TE};
use http::{HeaderMap, Method, Version};
use http_body::Frame;
use httparse::ParserConfig;

use super::io::Buffered;
use super::{Decoder, Encode, EncodedBuf, Encoder, Http1Transaction, ParseContext, Wants};
use crate::body::DecodedLength;
#[cfg(feature = "server")]
use crate::common::time::Time;
use crate::headers;
use crate::proto::{BodyLength, MessageHead};
#[cfg(feature = "server")]
use crate::rt::Sleep;

const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// This handles a connection, which will have been established over an
/// `Read + Write` (like a socket), and will likely include multiple
/// `Transaction`s over HTTP.
///
/// The connection will determine when a message begins and ends as well as
/// determine if this connection can be kept alive after the message,
/// or if it is complete.
pub(crate) struct Conn<I, B, T> {
    io: Buffered<I, EncodedBuf<B>>,
    state: State,
    _marker: PhantomData<fn(T)>,
}

impl<I, B, T> Conn<I, B, T>
where
    I: Read + Write + Unpin,
    B: Buf,
    T: Http1Transaction,
{
    pub(crate) fn new(io: I) -> Conn<I, B, T> {
        Conn {
            io: Buffered::new(io),
            state: State {
                allow_half_close: false,
                cached_headers: None,
                error: None,
                keep_alive: KA::Busy,
                method: None,
                h1_parser_config: ParserConfig::default(),
                h1_max_headers: None,
                #[cfg(feature = "server")]
                h1_header_read_timeout: None,
                #[cfg(feature = "server")]
                h1_header_read_timeout_fut: None,
                #[cfg(feature = "server")]
                h1_header_read_timeout_running: false,
                #[cfg(feature = "server")]
                date_header: true,
                #[cfg(feature = "server")]
                timer: Time::Empty,
                preserve_header_case: false,
                #[cfg(feature = "ffi")]
                preserve_header_order: false,
                title_case_headers: false,
                h09_responses: false,
                #[cfg(feature = "client")]
                on_informational: None,
                notify_read: false,
                reading: Reading::Init,
                writing: Writing::Init,
                upgrade: None,
                // We assume a modern world where the remote speaks HTTP/1.1.
                // If they tell us otherwise, we'll downgrade in `read_head`.
                version: Version::HTTP_11,
                allow_trailer_fields: false,
            },
            _marker: PhantomData,
        }
    }

    #[cfg(feature = "server")]
    pub(crate) fn set_timer(&mut self, timer: Time) {
        self.state.timer = timer;
    }

    #[cfg(feature = "server")]
    pub(crate) fn set_flush_pipeline(&mut self, enabled: bool) {
        self.io.set_flush_pipeline(enabled);
    }

    pub(crate) fn set_write_strategy_queue(&mut self) {
        self.io.set_write_strategy_queue();
    }

    pub(crate) fn set_max_buf_size(&mut self, max: usize) {
        self.io.set_max_buf_size(max);
    }

    #[cfg(feature = "client")]
    pub(crate) fn set_read_buf_exact_size(&mut self, sz: usize) {
        self.io.set_read_buf_exact_size(sz);
    }

    pub(crate) fn set_write_strategy_flatten(&mut self) {
        self.io.set_write_strategy_flatten();
    }

    pub(crate) fn set_h1_parser_config(&mut self, parser_config: ParserConfig) {
        self.state.h1_parser_config = parser_config;
    }

    pub(crate) fn set_title_case_headers(&mut self) {
        self.state.title_case_headers = true;
    }

    pub(crate) fn set_preserve_header_case(&mut self) {
        self.state.preserve_header_case = true;
    }

    #[cfg(feature = "ffi")]
    pub(crate) fn set_preserve_header_order(&mut self) {
        self.state.preserve_header_order = true;
    }

    #[cfg(feature = "client")]
    pub(crate) fn set_h09_responses(&mut self) {
        self.state.h09_responses = true;
    }

    pub(crate) fn set_http1_max_headers(&mut self, val: usize) {
        self.state.h1_max_headers = Some(val);
    }

    #[cfg(feature = "server")]
    pub(crate) fn set_http1_header_read_timeout(&mut self, val: Duration) {
        self.state.h1_header_read_timeout = Some(val);
    }

    #[cfg(feature = "server")]
    pub(crate) fn set_allow_half_close(&mut self) {
        self.state.allow_half_close = true;
    }

    #[cfg(feature = "server")]
    pub(crate) fn disable_date_header(&mut self) {
        self.state.date_header = false;
    }

    pub(crate) fn into_inner(self) -> (I, Bytes) {
        self.io.into_inner()
    }

    pub(crate) fn pending_upgrade(&mut self) -> Option<crate::upgrade::Pending> {
        self.state.upgrade.take()
    }

    pub(crate) fn is_read_closed(&self) -> bool {
        self.state.is_read_closed()
    }

    pub(crate) fn is_write_closed(&self) -> bool {
        self.state.is_write_closed()
    }

    pub(crate) fn can_read_head(&self) -> bool {
        if !matches!(self.state.reading, Reading::Init) {
            return false;
        }

        if T::should_read_first() {
            return true;
        }

        !matches!(self.state.writing, Writing::Init)
    }

    pub(crate) fn can_read_body(&self) -> bool {
        matches!(
            self.state.reading,
            Reading::Body(..) | Reading::Continue(..)
        )
    }

    #[cfg(feature = "server")]
    pub(crate) fn has_initial_read_write_state(&self) -> bool {
        matches!(self.state.reading, Reading::Init)
            && matches!(self.state.writing, Writing::Init)
            && self.io.read_buf().is_empty()
    }

    fn should_error_on_eof(&self) -> bool {
        // If we're idle, it's probably just the connection closing gracefully.
        T::should_error_on_parse_eof() && !self.state.is_idle()
    }

    fn has_h2_prefix(&self) -> bool {
        let read_buf = self.io.read_buf();
        read_buf.len() >= 24 && read_buf[..24] == *H2_PREFACE
    }

    pub(super) fn poll_read_head(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<crate::Result<(MessageHead<T::Incoming>, DecodedLength, Wants)>>> {
        debug_assert!(self.can_read_head());
        trace!("Conn::read_head");

        #[cfg(feature = "server")]
        if !self.state.h1_header_read_timeout_running {
            if let Some(h1_header_read_timeout) = self.state.h1_header_read_timeout {
                let deadline = Instant::now() + h1_header_read_timeout;
                self.state.h1_header_read_timeout_running = true;
                match self.state.h1_header_read_timeout_fut {
                    Some(ref mut h1_header_read_timeout_fut) => {
                        trace!("resetting h1 header read timeout timer");
                        self.state.timer.reset(h1_header_read_timeout_fut, deadline);
                    }
                    None => {
                        trace!("setting h1 header read timeout timer");
                        self.state.h1_header_read_timeout_fut =
                            Some(self.state.timer.sleep_until(deadline));
                    }
                }
            }
        }

        let msg = match self.io.parse::<T>(
            cx,
            ParseContext {
                cached_headers: &mut self.state.cached_headers,
                req_method: &mut self.state.method,
                h1_parser_config: self.state.h1_parser_config.clone(),
                h1_max_headers: self.state.h1_max_headers,
                preserve_header_case: self.state.preserve_header_case,
                #[cfg(feature = "ffi")]
                preserve_header_order: self.state.preserve_header_order,
                h09_responses: self.state.h09_responses,
                #[cfg(feature = "client")]
                on_informational: &mut self.state.on_informational,
            },
        ) {
            Poll::Ready(Ok(msg)) => msg,
            Poll::Ready(Err(e)) => return self.on_read_head_error(e),
            Poll::Pending => {
                #[cfg(feature = "server")]
                if self.state.h1_header_read_timeout_running {
                    if let Some(ref mut h1_header_read_timeout_fut) =
                        self.state.h1_header_read_timeout_fut
                    {
                        if Pin::new(h1_header_read_timeout_fut).poll(cx).is_ready() {
                            self.state.h1_header_read_timeout_running = false;

                            warn!("read header from client timeout");
                            return Poll::Ready(Some(Err(crate::Error::new_header_timeout())));
                        }
                    }
                }

                return Poll::Pending;
            }
        };

        #[cfg(feature = "server")]
        {
            self.state.h1_header_read_timeout_running = false;
            self.state.h1_header_read_timeout_fut = None;
        }

        // Note: don't deconstruct `msg` into local variables, it appears
        // the optimizer doesn't remove the extra copies.

        debug!("incoming body is {}", msg.decode);

        // Prevent accepting HTTP/0.9 responses after the initial one, if any.
        self.state.h09_responses = false;

        // Drop any OnInformational callbacks, we're done there!
        #[cfg(feature = "client")]
        {
            self.state.on_informational = None;
        }

        self.state.busy();
        self.state.keep_alive &= msg.keep_alive;
        self.state.version = msg.head.version;

        let mut wants = if msg.wants_upgrade {
            Wants::UPGRADE
        } else {
            Wants::EMPTY
        };

        if msg.decode == DecodedLength::ZERO {
            if msg.expect_continue {
                debug!("ignoring expect-continue since body is empty");
            }
            self.state.reading = Reading::KeepAlive;
            if !T::should_read_first() {
                self.try_keep_alive(cx);
            }
        } else if msg.expect_continue && msg.head.version.gt(&Version::HTTP_10) {
            let h1_max_header_size = None; // TODO: remove this when we land h1_max_header_size support
            self.state.reading = Reading::Continue(Decoder::new(
                msg.decode,
                self.state.h1_max_headers,
                h1_max_header_size,
            ));
            wants = wants.add(Wants::EXPECT);
        } else {
            let h1_max_header_size = None; // TODO: remove this when we land h1_max_header_size support
            self.state.reading = Reading::Body(Decoder::new(
                msg.decode,
                self.state.h1_max_headers,
                h1_max_header_size,
            ));
        }

        self.state.allow_trailer_fields = msg
            .head
            .headers
            .get(TE)
            .map_or(false, |te_header| te_header == "trailers");

        Poll::Ready(Some(Ok((msg.head, msg.decode, wants))))
    }

    fn on_read_head_error<Z>(&mut self, e: crate::Error) -> Poll<Option<crate::Result<Z>>> {
        // If we are currently waiting on a message, then an empty
        // message should be reported as an error. If not, it is just
        // the connection closing gracefully.
        let must_error = self.should_error_on_eof();
        self.close_read();
        self.io.consume_leading_lines();
        let was_mid_parse = e.is_parse() || !self.io.read_buf().is_empty();
        if was_mid_parse || must_error {
            // We check if the buf contains the h2 Preface
            debug!(
                "parse error ({}) with {} bytes",
                e,
                self.io.read_buf().len()
            );
            match self.on_parse_error(e) {
                Ok(()) => Poll::Pending, // XXX: wat?
                Err(e) => Poll::Ready(Some(Err(e))),
            }
        } else {
            debug!("read eof");
            self.close_write();
            Poll::Ready(None)
        }
    }

    pub(crate) fn poll_read_body(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<io::Result<Frame<Bytes>>>> {
        debug_assert!(self.can_read_body());

        let (reading, ret) = match self.state.reading {
            Reading::Body(ref mut decoder) => {
                match ready!(decoder.decode(cx, &mut self.io)) {
                    Ok(frame) => {
                        if frame.is_data() {
                            let slice = frame.data_ref().unwrap_or_else(|| unreachable!());
                            let (reading, maybe_frame) = if decoder.is_eof() {
                                debug!("incoming body completed");
                                (
                                    Reading::KeepAlive,
                                    if !slice.is_empty() {
                                        Some(Ok(frame))
                                    } else {
                                        None
                                    },
                                )
                            } else if slice.is_empty() {
                                error!("incoming body unexpectedly ended");
                                // This should be unreachable, since all 3 decoders
                                // either set eof=true or return an Err when reading
                                // an empty slice...
                                (Reading::Closed, None)
                            } else {
                                return Poll::Ready(Some(Ok(frame)));
                            };
                            (reading, Poll::Ready(maybe_frame))
                        } else if frame.is_trailers() {
                            (Reading::Closed, Poll::Ready(Some(Ok(frame))))
                        } else {
                            trace!("discarding unknown frame");
                            (Reading::Closed, Poll::Ready(None))
                        }
                    }
                    Err(e) => {
                        debug!("incoming body decode error: {}", e);
                        (Reading::Closed, Poll::Ready(Some(Err(e))))
                    }
                }
            }
            Reading::Continue(ref decoder) => {
                // Write the 100 Continue if not already responded...
                if let Writing::Init = self.state.writing {
                    trace!("automatically sending 100 Continue");
                    let cont = b"HTTP/1.1 100 Continue\r\n\r\n";
                    self.io.headers_buf().extend_from_slice(cont);
                }

                // And now recurse once in the Reading::Body state...
                self.state.reading = Reading::Body(decoder.clone());
                return self.poll_read_body(cx);
            }
            _ => unreachable!("poll_read_body invalid state: {:?}", self.state.reading),
        };

        self.state.reading = reading;
        self.try_keep_alive(cx);
        ret
    }

    pub(crate) fn wants_read_again(&mut self) -> bool {
        let ret = self.state.notify_read;
        self.state.notify_read = false;
        ret
    }

    pub(crate) fn poll_read_keep_alive(&mut self, cx: &mut Context<'_>) -> Poll<crate::Result<()>> {
        debug_assert!(!self.can_read_head() && !self.can_read_body());

        if self.is_read_closed() {
            Poll::Pending
        } else if self.is_mid_message() {
            self.mid_message_detect_eof(cx)
        } else {
            self.require_empty_read(cx)
        }
    }

    fn is_mid_message(&self) -> bool {
        !matches!(
            (&self.state.reading, &self.state.writing),
            (&Reading::Init, &Writing::Init)
        )
    }

    // This will check to make sure the io object read is empty.
    //
    // This should only be called for Clients wanting to enter the idle
    // state.
    fn require_empty_read(&mut self, cx: &mut Context<'_>) -> Poll<crate::Result<()>> {
        debug_assert!(!self.can_read_head() && !self.can_read_body() && !self.is_read_closed());
        debug_assert!(!self.is_mid_message());
        debug_assert!(T::is_client());

        if !self.io.read_buf().is_empty() {
            debug!("received an unexpected {} bytes", self.io.read_buf().len());
            return Poll::Ready(Err(crate::Error::new_unexpected_message()));
        }

        let num_read = ready!(self.force_io_read(cx)).map_err(crate::Error::new_io)?;

        if num_read == 0 {
            let ret = if self.should_error_on_eof() {
                trace!("found unexpected EOF on busy connection: {:?}", self.state);
                Poll::Ready(Err(crate::Error::new_incomplete()))
            } else {
                trace!("found EOF on idle connection, closing");
                Poll::Ready(Ok(()))
            };

            // order is important: should_error needs state BEFORE close_read
            self.state.close_read();
            return ret;
        }

        debug!(
            "received unexpected {} bytes on an idle connection",
            num_read
        );
        Poll::Ready(Err(crate::Error::new_unexpected_message()))
    }

    fn mid_message_detect_eof(&mut self, cx: &mut Context<'_>) -> Poll<crate::Result<()>> {
        debug_assert!(!self.can_read_head() && !self.can_read_body() && !self.is_read_closed());
        debug_assert!(self.is_mid_message());

        if self.state.allow_half_close || !self.io.read_buf().is_empty() {
            return Poll::Pending;
        }

        let num_read = ready!(self.force_io_read(cx)).map_err(crate::Error::new_io)?;

        if num_read == 0 {
            trace!("found unexpected EOF on busy connection: {:?}", self.state);
            self.state.close_read();
            Poll::Ready(Err(crate::Error::new_incomplete()))
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn force_io_read(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<usize>> {
        debug_assert!(!self.state.is_read_closed());

        let result = ready!(self.io.poll_read_from_io(cx));
        Poll::Ready(result.map_err(|e| {
            trace!(error = %e, "force_io_read; io error");
            self.state.close();
            e
        }))
    }

    fn maybe_notify(&mut self, cx: &mut Context<'_>) {
        // its possible that we returned NotReady from poll() without having
        // exhausted the underlying Io. We would have done this when we
        // determined we couldn't keep reading until we knew how writing
        // would finish.

        match self.state.reading {
            Reading::Continue(..) | Reading::Body(..) | Reading::KeepAlive | Reading::Closed => {
                return
            }
            Reading::Init => (),
        };

        match self.state.writing {
            Writing::Body(..) => return,
            Writing::Init | Writing::KeepAlive | Writing::Closed => (),
        }

        if !self.io.is_read_blocked() {
            if self.io.read_buf().is_empty() {
                match self.io.poll_read_from_io(cx) {
                    Poll::Ready(Ok(n)) => {
                        if n == 0 {
                            trace!("maybe_notify; read eof");
                            if self.state.is_idle() {
                                self.state.close();
                            } else {
                                self.close_read()
                            }
                            return;
                        }
                    }
                    Poll::Pending => {
                        trace!("maybe_notify; read_from_io blocked");
                        return;
                    }
                    Poll::Ready(Err(e)) => {
                        trace!("maybe_notify; read_from_io error: {}", e);
                        self.state.close();
                        self.state.error = Some(crate::Error::new_io(e));
                    }
                }
            }
            self.state.notify_read = true;
        }
    }

    fn try_keep_alive(&mut self, cx: &mut Context<'_>) {
        self.state.try_keep_alive::<T>();
        self.maybe_notify(cx);
    }

    pub(crate) fn can_write_head(&self) -> bool {
        if !T::should_read_first() && matches!(self.state.reading, Reading::Closed) {
            return false;
        }

        match self.state.writing {
            Writing::Init => self.io.can_headers_buf(),
            _ => false,
        }
    }

    pub(crate) fn can_write_body(&self) -> bool {
        match self.state.writing {
            Writing::Body(..) => true,
            Writing::Init | Writing::KeepAlive | Writing::Closed => false,
        }
    }

    pub(crate) fn can_buffer_body(&self) -> bool {
        self.io.can_buffer()
    }

    pub(crate) fn write_head(&mut self, head: MessageHead<T::Outgoing>, body: Option<BodyLength>) {
        if let Some(encoder) = self.encode_head(head, body) {
            self.state.writing = if !encoder.is_eof() {
                Writing::Body(encoder)
            } else if encoder.is_last() {
                Writing::Closed
            } else {
                Writing::KeepAlive
            };
        }
    }

    fn encode_head(
        &mut self,
        mut head: MessageHead<T::Outgoing>,
        body: Option<BodyLength>,
    ) -> Option<Encoder> {
        debug_assert!(self.can_write_head());

        if !T::should_read_first() {
            self.state.busy();
        }

        self.enforce_version(&mut head);

        let buf = self.io.headers_buf();
        match super::role::encode_headers::<T>(
            Encode {
                head: &mut head,
                body,
                #[cfg(feature = "server")]
                keep_alive: self.state.wants_keep_alive(),
                req_method: &mut self.state.method,
                title_case_headers: self.state.title_case_headers,
                #[cfg(feature = "server")]
                date_header: self.state.date_header,
            },
            buf,
        ) {
            Ok(encoder) => {
                debug_assert!(self.state.cached_headers.is_none());
                debug_assert!(head.headers.is_empty());
                self.state.cached_headers = Some(head.headers);

                #[cfg(feature = "client")]
                {
                    self.state.on_informational =
                        head.extensions.remove::<crate::ext::OnInformational>();
                }

                Some(encoder)
            }
            Err(err) => {
                self.state.error = Some(err);
                self.state.writing = Writing::Closed;
                None
            }
        }
    }

    // Fix keep-alive when Connection: keep-alive header is not present
    fn fix_keep_alive(&mut self, head: &mut MessageHead<T::Outgoing>) {
        let outgoing_is_keep_alive = head
            .headers
            .get(CONNECTION)
            .map_or(false, headers::connection_keep_alive);

        if !outgoing_is_keep_alive {
            match head.version {
                // If response is version 1.0 and keep-alive is not present in the response,
                // disable keep-alive so the server closes the connection
                Version::HTTP_10 => self.state.disable_keep_alive(),
                // If response is version 1.1 and keep-alive is wanted, add
                // Connection: keep-alive header when not present
                Version::HTTP_11 => {
                    if self.state.wants_keep_alive() {
                        head.headers
                            .insert(CONNECTION, HeaderValue::from_static("keep-alive"));
                    }
                }
                _ => (),
            }
        }
    }

    // If we know the remote speaks an older version, we try to fix up any messages
    // to work with our older peer.
    fn enforce_version(&mut self, head: &mut MessageHead<T::Outgoing>) {
        match self.state.version {
            Version::HTTP_10 => {
                // Fixes response or connection when keep-alive header is not present
                self.fix_keep_alive(head);
                // If the remote only knows HTTP/1.0, we should force ourselves
                // to do only speak HTTP/1.0 as well.
                head.version = Version::HTTP_10;
            }
            Version::HTTP_11 => {
                if let KA::Disabled = self.state.keep_alive.status() {
                    head.headers
                        .insert(CONNECTION, HeaderValue::from_static("close"));
                }
            }
            _ => (),
        }
        // If the remote speaks HTTP/1.1, then it *should* be fine with
        // both HTTP/1.0 and HTTP/1.1 from us. So again, we just let
        // the user's headers be.
    }

    pub(crate) fn write_body(&mut self, chunk: B) {
        debug_assert!(self.can_write_body() && self.can_buffer_body());
        // empty chunks should be discarded at Dispatcher level
        debug_assert!(chunk.remaining() != 0);

        let state = match self.state.writing {
            Writing::Body(ref mut encoder) => {
                self.io.buffer(encoder.encode(chunk));

                if !encoder.is_eof() {
                    return;
                }

                if encoder.is_last() {
                    Writing::Closed
                } else {
                    Writing::KeepAlive
                }
            }
            _ => unreachable!("write_body invalid state: {:?}", self.state.writing),
        };

        self.state.writing = state;
    }

    pub(crate) fn write_trailers(&mut self, trailers: HeaderMap) {
        if T::is_server() && !self.state.allow_trailer_fields {
            debug!("trailers not allowed to be sent");
            return;
        }
        debug_assert!(self.can_write_body() && self.can_buffer_body());

        match self.state.writing {
            Writing::Body(ref encoder) => {
                if let Some(enc_buf) =
                    encoder.encode_trailers(trailers, self.state.title_case_headers)
                {
                    self.io.buffer(enc_buf);

                    self.state.writing = if encoder.is_last() || encoder.is_close_delimited() {
                        Writing::Closed
                    } else {
                        Writing::KeepAlive
                    };
                }
            }
            _ => unreachable!("write_trailers invalid state: {:?}", self.state.writing),
        }
    }

    pub(crate) fn write_body_and_end(&mut self, chunk: B) {
        debug_assert!(self.can_write_body() && self.can_buffer_body());
        // empty chunks should be discarded at Dispatcher level
        debug_assert!(chunk.remaining() != 0);

        let state = match self.state.writing {
            Writing::Body(ref encoder) => {
                let can_keep_alive = encoder.encode_and_end(chunk, self.io.write_buf());
                if can_keep_alive {
                    Writing::KeepAlive
                } else {
                    Writing::Closed
                }
            }
            _ => unreachable!("write_body invalid state: {:?}", self.state.writing),
        };

        self.state.writing = state;
    }

    pub(crate) fn end_body(&mut self) -> crate::Result<()> {
        debug_assert!(self.can_write_body());

        let encoder = match self.state.writing {
            Writing::Body(ref mut enc) => enc,
            _ => return Ok(()),
        };

        // end of stream, that means we should try to eof
        match encoder.end() {
            Ok(end) => {
                if let Some(end) = end {
                    self.io.buffer(end);
                }

                self.state.writing = if encoder.is_last() || encoder.is_close_delimited() {
                    Writing::Closed
                } else {
                    Writing::KeepAlive
                };

                Ok(())
            }
            Err(not_eof) => {
                self.state.writing = Writing::Closed;
                Err(crate::Error::new_body_write_aborted().with(not_eof))
            }
        }
    }

    // When we get a parse error, depending on what side we are, we might be able
    // to write a response before closing the connection.
    //
    // - Client: there is nothing we can do
    // - Server: if Response hasn't been written yet, we can send a 4xx response
    fn on_parse_error(&mut self, err: crate::Error) -> crate::Result<()> {
        if let Writing::Init = self.state.writing {
            if self.has_h2_prefix() {
                return Err(crate::Error::new_version_h2());
            }
            if let Some(msg) = T::on_error(&err) {
                // Drop the cached headers so as to not trigger a debug
                // assert in `write_head`...
                self.state.cached_headers.take();
                self.write_head(msg, None);
                self.state.error = Some(err);
                return Ok(());
            }
        }

        // fallback is pass the error back up
        Err(err)
    }

    pub(crate) fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(Pin::new(&mut self.io).poll_flush(cx))?;
        self.try_keep_alive(cx);
        trace!("flushed({}): {:?}", T::LOG, self.state);
        Poll::Ready(Ok(()))
    }

    pub(crate) fn poll_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match ready!(Pin::new(self.io.io_mut()).poll_shutdown(cx)) {
            Ok(()) => {
                trace!("shut down IO complete");
                Poll::Ready(Ok(()))
            }
            Err(e) => {
                debug!("error shutting down IO: {}", e);
                Poll::Ready(Err(e))
            }
        }
    }

    /// If the read side can be cheaply drained, do so. Otherwise, close.
    pub(super) fn poll_drain_or_close_read(&mut self, cx: &mut Context<'_>) {
        if let Reading::Continue(ref decoder) = self.state.reading {
            // skip sending the 100-continue
            // just move forward to a read, in case a tiny body was included
            self.state.reading = Reading::Body(decoder.clone());
        }

        let _ = self.poll_read_body(cx);

        // If still in Reading::Body, just give up
        match self.state.reading {
            Reading::Init | Reading::KeepAlive => {
                trace!("body drained")
            }
            _ => self.close_read(),
        }
    }

    pub(crate) fn close_read(&mut self) {
        self.state.close_read();
    }

    pub(crate) fn close_write(&mut self) {
        self.state.close_write();
    }

    #[cfg(feature = "server")]
    pub(crate) fn disable_keep_alive(&mut self) {
        if self.state.is_idle() {
            trace!("disable_keep_alive; closing idle connection");
            self.state.close();
        } else {
            trace!("disable_keep_alive; in-progress connection");
            self.state.disable_keep_alive();
        }
    }

    pub(crate) fn take_error(&mut self) -> crate::Result<()> {
        if let Some(err) = self.state.error.take() {
            Err(err)
        } else {
            Ok(())
        }
    }

    pub(super) fn on_upgrade(&mut self) -> crate::upgrade::OnUpgrade {
        trace!("{}: prepare possible HTTP upgrade", T::LOG);
        self.state.prepare_upgrade()
    }
}

impl<I, B: Buf, T> fmt::Debug for Conn<I, B, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Conn")
            .field("state", &self.state)
            .field("io", &self.io)
            .finish()
    }
}

// B and T are never pinned
impl<I: Unpin, B, T> Unpin for Conn<I, B, T> {}

struct State {
    allow_half_close: bool,
    /// Re-usable HeaderMap to reduce allocating new ones.
    cached_headers: Option<HeaderMap>,
    /// If an error occurs when there wasn't a direct way to return it
    /// back to the user, this is set.
    error: Option<crate::Error>,
    /// Current keep-alive status.
    keep_alive: KA,
    /// If mid-message, the HTTP Method that started it.
    ///
    /// This is used to know things such as if the message can include
    /// a body or not.
    method: Option<Method>,
    h1_parser_config: ParserConfig,
    h1_max_headers: Option<usize>,
    #[cfg(feature = "server")]
    h1_header_read_timeout: Option<Duration>,
    #[cfg(feature = "server")]
    h1_header_read_timeout_fut: Option<Pin<Box<dyn Sleep>>>,
    #[cfg(feature = "server")]
    h1_header_read_timeout_running: bool,
    #[cfg(feature = "server")]
    date_header: bool,
    #[cfg(feature = "server")]
    timer: Time,
    preserve_header_case: bool,
    #[cfg(feature = "ffi")]
    preserve_header_order: bool,
    title_case_headers: bool,
    h09_responses: bool,
    /// If set, called with each 1xx informational response received for
    /// the current request. MUST be unset after a non-1xx response is
    /// received.
    #[cfg(feature = "client")]
    on_informational: Option<crate::ext::OnInformational>,
    /// Set to true when the Dispatcher should poll read operations
    /// again. See the `maybe_notify` method for more.
    notify_read: bool,
    /// State of allowed reads
    reading: Reading,
    /// State of allowed writes
    writing: Writing,
    /// An expected pending HTTP upgrade.
    upgrade: Option<crate::upgrade::Pending>,
    /// Either HTTP/1.0 or 1.1 connection
    version: Version,
    /// Flag to track if trailer fields are allowed to be sent
    allow_trailer_fields: bool,
}

#[derive(Debug)]
enum Reading {
    Init,
    Continue(Decoder),
    Body(Decoder),
    KeepAlive,
    Closed,
}

enum Writing {
    Init,
    Body(Encoder),
    KeepAlive,
    Closed,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = f.debug_struct("State");
        builder
            .field("reading", &self.reading)
            .field("writing", &self.writing)
            .field("keep_alive", &self.keep_alive);

        // Only show error field if it's interesting...
        if let Some(ref error) = self.error {
            builder.field("error", error);
        }

        if self.allow_half_close {
            builder.field("allow_half_close", &true);
        }

        // Purposefully leaving off other fields..

        builder.finish()
    }
}

impl fmt::Debug for Writing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Writing::Init => f.write_str("Init"),
            Writing::Body(ref enc) => f.debug_tuple("Body").field(enc).finish(),
            Writing::KeepAlive => f.write_str("KeepAlive"),
            Writing::Closed => f.write_str("Closed"),
        }
    }
}

impl std::ops::BitAndAssign<bool> for KA {
    fn bitand_assign(&mut self, enabled: bool) {
        if !enabled {
            trace!("remote disabling keep-alive");
            *self = KA::Disabled;
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
enum KA {
    Idle,
    #[default]
    Busy,
    Disabled,
}

impl KA {
    fn idle(&mut self) {
        *self = KA::Idle;
    }

    fn busy(&mut self) {
        *self = KA::Busy;
    }

    fn disable(&mut self) {
        *self = KA::Disabled;
    }

    fn status(&self) -> KA {
        *self
    }
}

impl State {
    fn close(&mut self) {
        trace!("State::close()");
        self.reading = Reading::Closed;
        self.writing = Writing::Closed;
        self.keep_alive.disable();
    }

    fn close_read(&mut self) {
        trace!("State::close_read()");
        self.reading = Reading::Closed;
        self.keep_alive.disable();
    }

    fn close_write(&mut self) {
        trace!("State::close_write()");
        self.writing = Writing::Closed;
        self.keep_alive.disable();
    }

    fn wants_keep_alive(&self) -> bool {
        !matches!(self.keep_alive.status(), KA::Disabled)
    }

    fn try_keep_alive<T: Http1Transaction>(&mut self) {
        match (&self.reading, &self.writing) {
            (&Reading::KeepAlive, &Writing::KeepAlive) => {
                if let KA::Busy = self.keep_alive.status() {
                    self.idle::<T>();
                } else {
                    trace!(
                        "try_keep_alive({}): could keep-alive, but status = {:?}",
                        T::LOG,
                        self.keep_alive
                    );
                    self.close();
                }
            }
            (&Reading::Closed, &Writing::KeepAlive) | (&Reading::KeepAlive, &Writing::Closed) => {
                self.close()
            }
            _ => (),
        }
    }

    fn disable_keep_alive(&mut self) {
        self.keep_alive.disable()
    }

    fn busy(&mut self) {
        if let KA::Disabled = self.keep_alive.status() {
            return;
        }
        self.keep_alive.busy();
    }

    fn idle<T: Http1Transaction>(&mut self) {
        debug_assert!(!self.is_idle(), "State::idle() called while idle");

        self.method = None;
        self.keep_alive.idle();

        if !self.is_idle() {
            self.close();
            return;
        }

        self.reading = Reading::Init;
        self.writing = Writing::Init;

        // !T::should_read_first() means Client.
        //
        // If Client connection has just gone idle, the Dispatcher
        // should try the poll loop one more time, so as to poll the
        // pending requests stream.
        if !T::should_read_first() {
            self.notify_read = true;
        }

        #[cfg(feature = "server")]
        if self.h1_header_read_timeout.is_some() {
            // Next read will start and poll the header read timeout,
            // so we can close the connection if another header isn't
            // received in a timely manner.
            self.notify_read = true;
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self.keep_alive.status(), KA::Idle)
    }

    fn is_read_closed(&self) -> bool {
        matches!(self.reading, Reading::Closed)
    }

    fn is_write_closed(&self) -> bool {
        matches!(self.writing, Writing::Closed)
    }

    fn prepare_upgrade(&mut self) -> crate::upgrade::OnUpgrade {
        let (tx, rx) = crate::upgrade::pending();
        self.upgrade = Some(tx);
        rx
    }
}

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "nightly", not(miri)))]
    #[bench]
    fn bench_read_head_short(b: &mut ::test::Bencher) {
        use super::*;
        use crate::common::io::Compat;
        let s = b"GET / HTTP/1.1\r\nHost: localhost:8080\r\n\r\n";
        let len = s.len();
        b.bytes = len as u64;

        // an empty IO, we'll be skipping and using the read buffer anyways
        let io = Compat(tokio_test::io::Builder::new().build());
        let mut conn = Conn::<_, bytes::Bytes, crate::proto::h1::ServerTransaction>::new(io);
        *conn.io.read_buf_mut() = ::bytes::BytesMut::from(&s[..]);
        conn.state.cached_headers = Some(HeaderMap::with_capacity(2));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        b.iter(|| {
            rt.block_on(futures_util::future::poll_fn(|cx| {
                match conn.poll_read_head(cx) {
                    Poll::Ready(Some(Ok(x))) => {
                        ::test::black_box(&x);
                        let mut headers = x.0.headers;
                        headers.clear();
                        conn.state.cached_headers = Some(headers);
                    }
                    f => panic!("expected Ready(Some(Ok(..))): {:?}", f),
                }

                conn.io.read_buf_mut().reserve(1);
                unsafe {
                    conn.io.read_buf_mut().set_len(len);
                }
                conn.state.reading = Reading::Init;
                Poll::Ready(())
            }));
        });
    }

    /*
    //TODO: rewrite these using dispatch... someday...
    use futures::{Async, Future, Stream, Sink};
    use futures::future;

    use proto::{self, ClientTransaction, MessageHead, ServerTransaction};
    use super::super::Encoder;
    use mock::AsyncIo;

    use super::{Conn, Decoder, Reading, Writing};
    use ::uri::Uri;

    use std::str::FromStr;

    #[test]
    fn test_conn_init_read() {
        let good_message = b"GET / HTTP/1.1\r\n\r\n".to_vec();
        let len = good_message.len();
        let io = AsyncIo::new_buf(good_message, len);
        let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);

        match conn.poll().unwrap() {
            Async::Ready(Some(Frame::Message { message, body: false })) => {
                assert_eq!(message, MessageHead {
                    subject: ::proto::RequestLine(::Get, Uri::from_str("/").unwrap()),
                    .. MessageHead::default()
                })
            },
            f => panic!("frame is not Frame::Message: {:?}", f)
        }
    }

    #[test]
    fn test_conn_parse_partial() {
        let _: Result<(), ()> = future::lazy(|| {
            let good_message = b"GET / HTTP/1.1\r\nHost: foo.bar\r\n\r\n".to_vec();
            let io = AsyncIo::new_buf(good_message, 10);
            let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
            assert!(conn.poll().unwrap().is_not_ready());
            conn.io.io_mut().block_in(50);
            let async = conn.poll().unwrap();
            assert!(async.is_ready());
            match async {
                Async::Ready(Some(Frame::Message { .. })) => (),
                f => panic!("frame is not Message: {:?}", f),
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_init_read_eof_idle() {
        let io = AsyncIo::new_buf(vec![], 1);
        let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
        conn.state.idle();

        match conn.poll().unwrap() {
            Async::Ready(None) => {},
            other => panic!("frame is not None: {:?}", other)
        }
    }

    #[test]
    fn test_conn_init_read_eof_idle_partial_parse() {
        let io = AsyncIo::new_buf(b"GET / HTTP/1.1".to_vec(), 100);
        let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
        conn.state.idle();

        match conn.poll() {
            Err(ref err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {},
            other => panic!("unexpected frame: {:?}", other)
        }
    }

    #[test]
    fn test_conn_init_read_eof_busy() {
        let _: Result<(), ()> = future::lazy(|| {
            // server ignores
            let io = AsyncIo::new_eof();
            let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
            conn.state.busy();

            match conn.poll().unwrap() {
                Async::Ready(None) => {},
                other => panic!("unexpected frame: {:?}", other)
            }

            // client
            let io = AsyncIo::new_eof();
            let mut conn = Conn::<_, proto::Bytes, ClientTransaction>::new(io);
            conn.state.busy();

            match conn.poll() {
                Err(ref err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {},
                other => panic!("unexpected frame: {:?}", other)
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_body_finish_read_eof() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_eof();
            let mut conn = Conn::<_, proto::Bytes, ClientTransaction>::new(io);
            conn.state.busy();
            conn.state.writing = Writing::KeepAlive;
            conn.state.reading = Reading::Body(Decoder::length(0));

            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Body { chunk: None }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            // conn eofs, but tokio-proto will call poll() again, before calling flush()
            // the conn eof in this case is perfectly fine

            match conn.poll() {
                Ok(Async::Ready(None)) => (),
                other => panic!("unexpected frame: {:?}", other)
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_message_empty_body_read_eof() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(), 1024);
            let mut conn = Conn::<_, proto::Bytes, ClientTransaction>::new(io);
            conn.state.busy();
            conn.state.writing = Writing::KeepAlive;

            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Message { body: false, .. }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            // conn eofs, but tokio-proto will call poll() again, before calling flush()
            // the conn eof in this case is perfectly fine

            match conn.poll() {
                Ok(Async::Ready(None)) => (),
                other => panic!("unexpected frame: {:?}", other)
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_read_body_end() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\n12345".to_vec(), 1024);
            let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
            conn.state.busy();

            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Message { body: true, .. }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Body { chunk: Some(_) }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            // When the body is done, `poll` MUST return a `Body` frame with chunk set to `None`
            match conn.poll() {
                Ok(Async::Ready(Some(Frame::Body { chunk: None }))) => (),
                other => panic!("unexpected frame: {:?}", other)
            }

            match conn.poll() {
                Ok(Async::NotReady) => (),
                other => panic!("unexpected frame: {:?}", other)
            }
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_closed_read() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
        conn.state.close();

        match conn.poll().unwrap() {
            Async::Ready(None) => {},
            other => panic!("frame is not None: {:?}", other)
        }
    }

    #[test]
    fn test_conn_body_write_length() {
        let _ = pretty_env_logger::try_init();
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 0);
            let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
            let max = super::super::io::DEFAULT_MAX_BUFFER_SIZE + 4096;
            conn.state.writing = Writing::Body(Encoder::length((max * 2) as u64));

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'a'; max].into()) }).unwrap().is_ready());
            assert!(!conn.can_buffer_body());

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'b'; 1024 * 8].into()) }).unwrap().is_not_ready());

            conn.io.io_mut().block_in(1024 * 3);
            assert!(conn.poll_complete().unwrap().is_not_ready());
            conn.io.io_mut().block_in(1024 * 3);
            assert!(conn.poll_complete().unwrap().is_not_ready());
            conn.io.io_mut().block_in(max * 2);
            assert!(conn.poll_complete().unwrap().is_ready());

            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'c'; 1024 * 8].into()) }).unwrap().is_ready());
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_body_write_chunked() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
            conn.state.writing = Writing::Body(Encoder::chunked());

            assert!(conn.start_send(Frame::Body { chunk: Some("headers".into()) }).unwrap().is_ready());
            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'x'; 8192].into()) }).unwrap().is_ready());
            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_body_flush() {
        let _: Result<(), ()> = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 1024 * 1024 * 5);
            let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
            conn.state.writing = Writing::Body(Encoder::length(1024 * 1024));
            assert!(conn.start_send(Frame::Body { chunk: Some(vec![b'a'; 1024 * 1024].into()) }).unwrap().is_ready());
            assert!(!conn.can_buffer_body());
            conn.io.io_mut().block_in(1024 * 1024 * 5);
            assert!(conn.poll_complete().unwrap().is_ready());
            assert!(conn.can_buffer_body());
            assert!(conn.io.io_mut().flushed());

            Ok(())
        }).wait();
    }

    #[test]
    fn test_conn_parking() {
        use std::sync::Arc;
        use futures::executor::Notify;
        use futures::executor::NotifyHandle;

        struct Car {
            permit: bool,
        }
        impl Notify for Car {
            fn notify(&self, _id: usize) {
                assert!(self.permit, "unparked without permit");
            }
        }

        fn car(permit: bool) -> NotifyHandle {
            Arc::new(Car {
                permit: permit,
            }).into()
        }

        // test that once writing is done, unparks
        let f = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
            conn.state.reading = Reading::KeepAlive;
            assert!(conn.poll().unwrap().is_not_ready());

            conn.state.writing = Writing::KeepAlive;
            assert!(conn.poll_complete().unwrap().is_ready());
            Ok::<(), ()>(())
        });
        ::futures::executor::spawn(f).poll_future_notify(&car(true), 0).unwrap();


        // test that flushing when not waiting on read doesn't unpark
        let f = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
            conn.state.writing = Writing::KeepAlive;
            assert!(conn.poll_complete().unwrap().is_ready());
            Ok::<(), ()>(())
        });
        ::futures::executor::spawn(f).poll_future_notify(&car(false), 0).unwrap();


        // test that flushing and writing isn't done doesn't unpark
        let f = future::lazy(|| {
            let io = AsyncIo::new_buf(vec![], 4096);
            let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
            conn.state.reading = Reading::KeepAlive;
            assert!(conn.poll().unwrap().is_not_ready());
            conn.state.writing = Writing::Body(Encoder::length(5_000));
            assert!(conn.poll_complete().unwrap().is_ready());
            Ok::<(), ()>(())
        });
        ::futures::executor::spawn(f).poll_future_notify(&car(false), 0).unwrap();
    }

    #[test]
    fn test_conn_closed_write() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
        conn.state.close();

        match conn.start_send(Frame::Body { chunk: Some(b"foobar".to_vec().into()) }) {
            Err(_e) => {},
            other => panic!("did not return Err: {:?}", other)
        }

        assert!(conn.state.is_write_closed());
    }

    #[test]
    fn test_conn_write_empty_chunk() {
        let io = AsyncIo::new_buf(vec![], 0);
        let mut conn = Conn::<_, proto::Bytes, ServerTransaction>::new(io);
        conn.state.writing = Writing::KeepAlive;

        assert!(conn.start_send(Frame::Body { chunk: None }).unwrap().is_ready());
        assert!(conn.start_send(Frame::Body { chunk: Some(Vec::new().into()) }).unwrap().is_ready());
        conn.start_send(Frame::Body { chunk: Some(vec![b'a'].into()) }).unwrap_err();
    }
    */
}
