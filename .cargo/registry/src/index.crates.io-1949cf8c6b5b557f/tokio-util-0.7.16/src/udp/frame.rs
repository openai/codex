use crate::codec::{Decoder, Encoder};

use futures_core::Stream;
use tokio::{io::ReadBuf, net::UdpSocket};

use bytes::{BufMut, BytesMut};
use futures_sink::Sink;
use std::io;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::{
    borrow::Borrow,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

/// A unified [`Stream`] and [`Sink`] interface to an underlying `UdpSocket`, using
/// the `Encoder` and `Decoder` traits to encode and decode frames.
///
/// Raw UDP sockets work with datagrams, but higher-level code usually wants to
/// batch these into meaningful chunks, called "frames". This method layers
/// framing on top of this socket by using the `Encoder` and `Decoder` traits to
/// handle encoding and decoding of messages frames. Note that the incoming and
/// outgoing frame types may be distinct.
///
/// This function returns a *single* object that is both [`Stream`] and [`Sink`];
/// grouping this into a single object is often useful for layering things which
/// require both read and write access to the underlying object.
///
/// If you want to work more directly with the streams and sink, consider
/// calling [`split`] on the `UdpFramed` returned by this method, which will break
/// them into separate objects, allowing them to interact more easily.
///
/// [`Stream`]: futures_core::Stream
/// [`Sink`]: futures_sink::Sink
/// [`split`]: https://docs.rs/futures/0.3/futures/stream/trait.StreamExt.html#method.split
#[must_use = "sinks do nothing unless polled"]
#[derive(Debug)]
pub struct UdpFramed<C, T = UdpSocket> {
    socket: T,
    codec: C,
    rd: BytesMut,
    wr: BytesMut,
    out_addr: SocketAddr,
    flushed: bool,
    is_readable: bool,
    current_addr: Option<SocketAddr>,
}

const INITIAL_RD_CAPACITY: usize = 64 * 1024;
const INITIAL_WR_CAPACITY: usize = 8 * 1024;

impl<C, T> Unpin for UdpFramed<C, T> {}

impl<C, T> Stream for UdpFramed<C, T>
where
    T: Borrow<UdpSocket>,
    C: Decoder,
{
    type Item = Result<(C::Item, SocketAddr), C::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        pin.rd.reserve(INITIAL_RD_CAPACITY);

        loop {
            // Are there still bytes left in the read buffer to decode?
            if pin.is_readable {
                if let Some(frame) = pin.codec.decode_eof(&mut pin.rd)? {
                    let current_addr = pin
                        .current_addr
                        .expect("will always be set before this line is called");

                    return Poll::Ready(Some(Ok((frame, current_addr))));
                }

                // if this line has been reached then decode has returned `None`.
                pin.is_readable = false;
                pin.rd.clear();
            }

            // We're out of data. Try and fetch more data to decode
            let addr = {
                // Safety: `chunk_mut()` returns a `&mut UninitSlice`, and `UninitSlice` is a
                // transparent wrapper around `[MaybeUninit<u8>]`.
                let buf = unsafe { pin.rd.chunk_mut().as_uninit_slice_mut() };
                let mut read = ReadBuf::uninit(buf);
                let ptr = read.filled().as_ptr();
                let res = ready!(pin.socket.borrow().poll_recv_from(cx, &mut read));

                assert_eq!(ptr, read.filled().as_ptr());
                let addr = res?;

                let filled = read.filled().len();
                // Safety: This is guaranteed to be the number of initialized (and read) bytes due
                // to the invariants provided by `ReadBuf::filled`.
                unsafe { pin.rd.advance_mut(filled) };

                addr
            };

            pin.current_addr = Some(addr);
            pin.is_readable = true;
        }
    }
}

impl<I, C, T> Sink<(I, SocketAddr)> for UdpFramed<C, T>
where
    T: Borrow<UdpSocket>,
    C: Encoder<I>,
{
    type Error = C::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if !self.flushed {
            match self.poll_flush(cx)? {
                Poll::Ready(()) => {}
                Poll::Pending => return Poll::Pending,
            }
        }

        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: (I, SocketAddr)) -> Result<(), Self::Error> {
        let (frame, out_addr) = item;

        let pin = self.get_mut();

        pin.codec.encode(frame, &mut pin.wr)?;
        pin.out_addr = out_addr;
        pin.flushed = false;

        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.flushed {
            return Poll::Ready(Ok(()));
        }

        let Self {
            ref socket,
            ref mut out_addr,
            ref mut wr,
            ..
        } = *self;

        let n = ready!(socket.borrow().poll_send_to(cx, wr, *out_addr))?;

        let wrote_all = n == self.wr.len();
        self.wr.clear();
        self.flushed = true;

        let res = if wrote_all {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to write entire datagram to socket",
            )
            .into())
        };

        Poll::Ready(res)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }
}

impl<C, T> UdpFramed<C, T>
where
    T: Borrow<UdpSocket>,
{
    /// Create a new `UdpFramed` backed by the given socket and codec.
    ///
    /// See struct level documentation for more details.
    pub fn new(socket: T, codec: C) -> UdpFramed<C, T> {
        Self {
            socket,
            codec,
            out_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)),
            rd: BytesMut::with_capacity(INITIAL_RD_CAPACITY),
            wr: BytesMut::with_capacity(INITIAL_WR_CAPACITY),
            flushed: true,
            is_readable: false,
            current_addr: None,
        }
    }

    /// Returns a reference to the underlying I/O stream wrapped by `Framed`.
    ///
    /// # Note
    ///
    /// Care should be taken to not tamper with the underlying stream of data
    /// coming in as it may corrupt the stream of frames otherwise being worked
    /// with.
    pub fn get_ref(&self) -> &T {
        &self.socket
    }

    /// Returns a mutable reference to the underlying I/O stream wrapped by `Framed`.
    ///
    /// # Note
    ///
    /// Care should be taken to not tamper with the underlying stream of data
    /// coming in as it may corrupt the stream of frames otherwise being worked
    /// with.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.socket
    }

    /// Returns a reference to the underlying codec wrapped by
    /// `Framed`.
    ///
    /// Note that care should be taken to not tamper with the underlying codec
    /// as it may corrupt the stream of frames otherwise being worked with.
    pub fn codec(&self) -> &C {
        &self.codec
    }

    /// Returns a mutable reference to the underlying codec wrapped by
    /// `UdpFramed`.
    ///
    /// Note that care should be taken to not tamper with the underlying codec
    /// as it may corrupt the stream of frames otherwise being worked with.
    pub fn codec_mut(&mut self) -> &mut C {
        &mut self.codec
    }

    /// Returns a reference to the read buffer.
    pub fn read_buffer(&self) -> &BytesMut {
        &self.rd
    }

    /// Returns a mutable reference to the read buffer.
    pub fn read_buffer_mut(&mut self) -> &mut BytesMut {
        &mut self.rd
    }

    /// Consumes the `Framed`, returning its underlying I/O stream.
    pub fn into_inner(self) -> T {
        self.socket
    }
}
