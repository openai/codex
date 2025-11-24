use std::error::Error as StdError;
use std::future::Future;
use std::io::{Cursor, IoSlice};
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes};
use futures_core::ready;
use h2::{Reason, RecvStream, SendStream};
use http::header::{HeaderName, CONNECTION, TE, TRANSFER_ENCODING, UPGRADE};
use http::HeaderMap;
use pin_project_lite::pin_project;

use crate::body::Body;
use crate::proto::h2::ping::Recorder;
use crate::rt::{Read, ReadBufCursor, Write};

pub(crate) mod ping;

cfg_client! {
    pub(crate) mod client;
    pub(crate) use self::client::ClientTask;
}

cfg_server! {
    pub(crate) mod server;
    pub(crate) use self::server::Server;
}

/// Default initial stream window size defined in HTTP2 spec.
pub(crate) const SPEC_WINDOW_SIZE: u32 = 65_535;

// List of connection headers from RFC 9110 Section 7.6.1
//
// TE headers are allowed in HTTP/2 requests as long as the value is "trailers", so they're
// tested separately.
static CONNECTION_HEADERS: [HeaderName; 4] = [
    HeaderName::from_static("keep-alive"),
    HeaderName::from_static("proxy-connection"),
    TRANSFER_ENCODING,
    UPGRADE,
];

fn strip_connection_headers(headers: &mut HeaderMap, is_request: bool) {
    for header in &CONNECTION_HEADERS {
        if headers.remove(header).is_some() {
            warn!("Connection header illegal in HTTP/2: {}", header.as_str());
        }
    }

    if is_request {
        if headers
            .get(TE)
            .map_or(false, |te_header| te_header != "trailers")
        {
            warn!("TE headers not set to \"trailers\" are illegal in HTTP/2 requests");
            headers.remove(TE);
        }
    } else if headers.remove(TE).is_some() {
        warn!("TE headers illegal in HTTP/2 responses");
    }

    if let Some(header) = headers.remove(CONNECTION) {
        warn!(
            "Connection header illegal in HTTP/2: {}",
            CONNECTION.as_str()
        );
        let header_contents = header.to_str().unwrap();

        // A `Connection` header may have a comma-separated list of names of other headers that
        // are meant for only this specific connection.
        //
        // Iterate these names and remove them as headers. Connection-specific headers are
        // forbidden in HTTP2, as that information has been moved into frame types of the h2
        // protocol.
        for name in header_contents.split(',') {
            let name = name.trim();
            headers.remove(name);
        }
    }
}

// body adapters used by both Client and Server

pin_project! {
    pub(crate) struct PipeToSendStream<S>
    where
        S: Body,
    {
        body_tx: SendStream<SendBuf<S::Data>>,
        data_done: bool,
        #[pin]
        stream: S,
    }
}

impl<S> PipeToSendStream<S>
where
    S: Body,
{
    fn new(stream: S, tx: SendStream<SendBuf<S::Data>>) -> PipeToSendStream<S> {
        PipeToSendStream {
            body_tx: tx,
            data_done: false,
            stream,
        }
    }
}

impl<S> Future for PipeToSendStream<S>
where
    S: Body,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Output = crate::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        loop {
            // we don't have the next chunk of data yet, so just reserve 1 byte to make
            // sure there's some capacity available. h2 will handle the capacity management
            // for the actual body chunk.
            me.body_tx.reserve_capacity(1);

            if me.body_tx.capacity() == 0 {
                loop {
                    match ready!(me.body_tx.poll_capacity(cx)) {
                        Some(Ok(0)) => {}
                        Some(Ok(_)) => break,
                        Some(Err(e)) => return Poll::Ready(Err(crate::Error::new_body_write(e))),
                        None => {
                            // None means the stream is no longer in a
                            // streaming state, we either finished it
                            // somehow, or the remote reset us.
                            return Poll::Ready(Err(crate::Error::new_body_write(
                                "send stream capacity unexpectedly closed",
                            )));
                        }
                    }
                }
            } else if let Poll::Ready(reason) = me
                .body_tx
                .poll_reset(cx)
                .map_err(crate::Error::new_body_write)?
            {
                debug!("stream received RST_STREAM: {:?}", reason);
                return Poll::Ready(Err(crate::Error::new_body_write(::h2::Error::from(reason))));
            }

            match ready!(me.stream.as_mut().poll_frame(cx)) {
                Some(Ok(frame)) => {
                    if frame.is_data() {
                        let chunk = frame.into_data().unwrap_or_else(|_| unreachable!());
                        let is_eos = me.stream.is_end_stream();
                        trace!(
                            "send body chunk: {} bytes, eos={}",
                            chunk.remaining(),
                            is_eos,
                        );

                        let buf = SendBuf::Buf(chunk);
                        me.body_tx
                            .send_data(buf, is_eos)
                            .map_err(crate::Error::new_body_write)?;

                        if is_eos {
                            return Poll::Ready(Ok(()));
                        }
                    } else if frame.is_trailers() {
                        // no more DATA, so give any capacity back
                        me.body_tx.reserve_capacity(0);
                        me.body_tx
                            .send_trailers(frame.into_trailers().unwrap_or_else(|_| unreachable!()))
                            .map_err(crate::Error::new_body_write)?;
                        return Poll::Ready(Ok(()));
                    } else {
                        trace!("discarding unknown frame");
                        // loop again
                    }
                }
                Some(Err(e)) => return Poll::Ready(Err(me.body_tx.on_user_err(e))),
                None => {
                    // no more frames means we're done here
                    // but at this point, we haven't sent an EOS DATA, or
                    // any trailers, so send an empty EOS DATA.
                    return Poll::Ready(me.body_tx.send_eos_frame());
                }
            }
        }
    }
}

trait SendStreamExt {
    fn on_user_err<E>(&mut self, err: E) -> crate::Error
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>;
    fn send_eos_frame(&mut self) -> crate::Result<()>;
}

impl<B: Buf> SendStreamExt for SendStream<SendBuf<B>> {
    fn on_user_err<E>(&mut self, err: E) -> crate::Error
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let err = crate::Error::new_user_body(err);
        debug!("send body user stream error: {}", err);
        self.send_reset(err.h2_reason());
        err
    }

    fn send_eos_frame(&mut self) -> crate::Result<()> {
        trace!("send body eos");
        self.send_data(SendBuf::None, true)
            .map_err(crate::Error::new_body_write)
    }
}

#[repr(usize)]
enum SendBuf<B> {
    Buf(B),
    Cursor(Cursor<Box<[u8]>>),
    None,
}

impl<B: Buf> Buf for SendBuf<B> {
    #[inline]
    fn remaining(&self) -> usize {
        match *self {
            Self::Buf(ref b) => b.remaining(),
            Self::Cursor(ref c) => Buf::remaining(c),
            Self::None => 0,
        }
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        match *self {
            Self::Buf(ref b) => b.chunk(),
            Self::Cursor(ref c) => c.chunk(),
            Self::None => &[],
        }
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        match *self {
            Self::Buf(ref mut b) => b.advance(cnt),
            Self::Cursor(ref mut c) => c.advance(cnt),
            Self::None => {}
        }
    }

    fn chunks_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        match *self {
            Self::Buf(ref b) => b.chunks_vectored(dst),
            Self::Cursor(ref c) => c.chunks_vectored(dst),
            Self::None => 0,
        }
    }
}

struct H2Upgraded<B>
where
    B: Buf,
{
    ping: Recorder,
    send_stream: UpgradedSendStream<B>,
    recv_stream: RecvStream,
    buf: Bytes,
}

impl<B> Read for H2Upgraded<B>
where
    B: Buf,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut read_buf: ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        if self.buf.is_empty() {
            self.buf = loop {
                match ready!(self.recv_stream.poll_data(cx)) {
                    None => return Poll::Ready(Ok(())),
                    Some(Ok(buf)) if buf.is_empty() && !self.recv_stream.is_end_stream() => {
                        continue
                    }
                    Some(Ok(buf)) => {
                        self.ping.record_data(buf.len());
                        break buf;
                    }
                    Some(Err(e)) => {
                        return Poll::Ready(match e.reason() {
                            Some(Reason::NO_ERROR) | Some(Reason::CANCEL) => Ok(()),
                            Some(Reason::STREAM_CLOSED) => {
                                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, e))
                            }
                            _ => Err(h2_to_io_error(e)),
                        })
                    }
                }
            };
        }
        let cnt = std::cmp::min(self.buf.len(), read_buf.remaining());
        read_buf.put_slice(&self.buf[..cnt]);
        self.buf.advance(cnt);
        let _ = self.recv_stream.flow_control().release_capacity(cnt);
        Poll::Ready(Ok(()))
    }
}

impl<B> Write for H2Upgraded<B>
where
    B: Buf,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        self.send_stream.reserve_capacity(buf.len());

        // We ignore all errors returned by `poll_capacity` and `write`, as we
        // will get the correct from `poll_reset` anyway.
        let cnt = match ready!(self.send_stream.poll_capacity(cx)) {
            None => Some(0),
            Some(Ok(cnt)) => self
                .send_stream
                .write(&buf[..cnt], false)
                .ok()
                .map(|()| cnt),
            Some(Err(_)) => None,
        };

        if let Some(cnt) = cnt {
            return Poll::Ready(Ok(cnt));
        }

        Poll::Ready(Err(h2_to_io_error(
            match ready!(self.send_stream.poll_reset(cx)) {
                Ok(Reason::NO_ERROR) | Ok(Reason::CANCEL) | Ok(Reason::STREAM_CLOSED) => {
                    return Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()))
                }
                Ok(reason) => reason.into(),
                Err(e) => e,
            },
        )))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        if self.send_stream.write(&[], true).is_ok() {
            return Poll::Ready(Ok(()));
        }

        Poll::Ready(Err(h2_to_io_error(
            match ready!(self.send_stream.poll_reset(cx)) {
                Ok(Reason::NO_ERROR) => return Poll::Ready(Ok(())),
                Ok(Reason::CANCEL) | Ok(Reason::STREAM_CLOSED) => {
                    return Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()))
                }
                Ok(reason) => reason.into(),
                Err(e) => e,
            },
        )))
    }
}

fn h2_to_io_error(e: h2::Error) -> std::io::Error {
    if e.is_io() {
        e.into_io().unwrap()
    } else {
        std::io::Error::new(std::io::ErrorKind::Other, e)
    }
}

struct UpgradedSendStream<B>(SendStream<SendBuf<Neutered<B>>>);

impl<B> UpgradedSendStream<B>
where
    B: Buf,
{
    unsafe fn new(inner: SendStream<SendBuf<B>>) -> Self {
        assert_eq!(mem::size_of::<B>(), mem::size_of::<Neutered<B>>());
        Self(mem::transmute(inner))
    }

    fn reserve_capacity(&mut self, cnt: usize) {
        unsafe { self.as_inner_unchecked().reserve_capacity(cnt) }
    }

    fn poll_capacity(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<usize, h2::Error>>> {
        unsafe { self.as_inner_unchecked().poll_capacity(cx) }
    }

    fn poll_reset(&mut self, cx: &mut Context<'_>) -> Poll<Result<h2::Reason, h2::Error>> {
        unsafe { self.as_inner_unchecked().poll_reset(cx) }
    }

    fn write(&mut self, buf: &[u8], end_of_stream: bool) -> Result<(), std::io::Error> {
        let send_buf = SendBuf::Cursor(Cursor::new(buf.into()));
        unsafe {
            self.as_inner_unchecked()
                .send_data(send_buf, end_of_stream)
                .map_err(h2_to_io_error)
        }
    }

    unsafe fn as_inner_unchecked(&mut self) -> &mut SendStream<SendBuf<B>> {
        &mut *(&mut self.0 as *mut _ as *mut _)
    }
}

#[repr(transparent)]
struct Neutered<B> {
    _inner: B,
    impossible: Impossible,
}

enum Impossible {}

unsafe impl<B> Send for Neutered<B> {}

impl<B> Buf for Neutered<B> {
    fn remaining(&self) -> usize {
        match self.impossible {}
    }

    fn chunk(&self) -> &[u8] {
        match self.impossible {}
    }

    fn advance(&mut self, _cnt: usize) {
        match self.impossible {}
    }
}
