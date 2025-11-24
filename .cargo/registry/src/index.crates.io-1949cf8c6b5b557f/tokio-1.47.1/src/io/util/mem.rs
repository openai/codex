//! In-process memory IO types.

use crate::io::{split, AsyncRead, AsyncWrite, ReadBuf, ReadHalf, WriteHalf};
use crate::loom::sync::Mutex;

use bytes::{Buf, BytesMut};
use std::{
    pin::Pin,
    sync::Arc,
    task::{self, ready, Poll, Waker},
};

/// A bidirectional pipe to read and write bytes in memory.
///
/// A pair of `DuplexStream`s are created together, and they act as a "channel"
/// that can be used as in-memory IO types. Writing to one of the pairs will
/// allow that data to be read from the other, and vice versa.
///
/// # Closing a `DuplexStream`
///
/// If one end of the `DuplexStream` channel is dropped, any pending reads on
/// the other side will continue to read data until the buffer is drained, then
/// they will signal EOF by returning 0 bytes. Any writes to the other side,
/// including pending ones (that are waiting for free space in the buffer) will
/// return `Err(BrokenPipe)` immediately.
///
/// # Example
///
/// ```
/// # async fn ex() -> std::io::Result<()> {
/// # use tokio::io::{AsyncReadExt, AsyncWriteExt};
/// let (mut client, mut server) = tokio::io::duplex(64);
///
/// client.write_all(b"ping").await?;
///
/// let mut buf = [0u8; 4];
/// server.read_exact(&mut buf).await?;
/// assert_eq!(&buf, b"ping");
///
/// server.write_all(b"pong").await?;
///
/// client.read_exact(&mut buf).await?;
/// assert_eq!(&buf, b"pong");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
pub struct DuplexStream {
    read: Arc<Mutex<SimplexStream>>,
    write: Arc<Mutex<SimplexStream>>,
}

/// A unidirectional pipe to read and write bytes in memory.
///
/// It can be constructed by [`simplex`] function which will create a pair of
/// reader and writer or by calling [`SimplexStream::new_unsplit`] that will
/// create a handle for both reading and writing.
///
/// # Example
///
/// ```
/// # async fn ex() -> std::io::Result<()> {
/// # use tokio::io::{AsyncReadExt, AsyncWriteExt};
/// let (mut receiver, mut sender) = tokio::io::simplex(64);
///
/// sender.write_all(b"ping").await?;
///
/// let mut buf = [0u8; 4];
/// receiver.read_exact(&mut buf).await?;
/// assert_eq!(&buf, b"ping");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
pub struct SimplexStream {
    /// The buffer storing the bytes written, also read from.
    ///
    /// Using a `BytesMut` because it has efficient `Buf` and `BufMut`
    /// functionality already. Additionally, it can try to copy data in the
    /// same buffer if there read index has advanced far enough.
    buffer: BytesMut,
    /// Determines if the write side has been closed.
    is_closed: bool,
    /// The maximum amount of bytes that can be written before returning
    /// `Poll::Pending`.
    max_buf_size: usize,
    /// If the `read` side has been polled and is pending, this is the waker
    /// for that parked task.
    read_waker: Option<Waker>,
    /// If the `write` side has filled the `max_buf_size` and returned
    /// `Poll::Pending`, this is the waker for that parked task.
    write_waker: Option<Waker>,
}

// ===== impl DuplexStream =====

/// Create a new pair of `DuplexStream`s that act like a pair of connected sockets.
///
/// The `max_buf_size` argument is the maximum amount of bytes that can be
/// written to a side before the write returns `Poll::Pending`.
#[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
pub fn duplex(max_buf_size: usize) -> (DuplexStream, DuplexStream) {
    let one = Arc::new(Mutex::new(SimplexStream::new_unsplit(max_buf_size)));
    let two = Arc::new(Mutex::new(SimplexStream::new_unsplit(max_buf_size)));

    (
        DuplexStream {
            read: one.clone(),
            write: two.clone(),
        },
        DuplexStream {
            read: two,
            write: one,
        },
    )
}

impl AsyncRead for DuplexStream {
    // Previous rustc required this `self` to be `mut`, even though newer
    // versions recognize it isn't needed to call `lock()`. So for
    // compatibility, we include the `mut` and `allow` the lint.
    //
    // See https://github.com/rust-lang/rust/issues/73592
    #[allow(unused_mut)]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.read.lock()).poll_read(cx, buf)
    }
}

impl AsyncWrite for DuplexStream {
    #[allow(unused_mut)]
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut *self.write.lock()).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut *self.write.lock()).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    #[allow(unused_mut)]
    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.write.lock()).poll_flush(cx)
    }

    #[allow(unused_mut)]
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.write.lock()).poll_shutdown(cx)
    }
}

impl Drop for DuplexStream {
    fn drop(&mut self) {
        // notify the other side of the closure
        self.write.lock().close_write();
        self.read.lock().close_read();
    }
}

// ===== impl SimplexStream =====

/// Creates unidirectional buffer that acts like in memory pipe.
///
/// The `max_buf_size` argument is the maximum amount of bytes that can be
/// written to a buffer before the it returns `Poll::Pending`.
///
/// # Unify reader and writer
///
/// The reader and writer half can be unified into a single structure
/// of `SimplexStream` that supports both reading and writing or
/// the `SimplexStream` can be already created as unified structure
/// using [`SimplexStream::new_unsplit()`].
///
/// ```
/// # async fn ex() -> std::io::Result<()> {
/// # use tokio::io::{AsyncReadExt, AsyncWriteExt};
/// let (reader, writer) = tokio::io::simplex(64);
/// let mut simplex_stream = reader.unsplit(writer);
/// simplex_stream.write_all(b"hello").await?;
///
/// let mut buf = [0u8; 5];
/// simplex_stream.read_exact(&mut buf).await?;
/// assert_eq!(&buf, b"hello");
/// # Ok(())
/// # }
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
pub fn simplex(max_buf_size: usize) -> (ReadHalf<SimplexStream>, WriteHalf<SimplexStream>) {
    split(SimplexStream::new_unsplit(max_buf_size))
}

impl SimplexStream {
    /// Creates unidirectional buffer that acts like in memory pipe. To create split
    /// version with separate reader and writer you can use [`simplex`] function.
    ///
    /// The `max_buf_size` argument is the maximum amount of bytes that can be
    /// written to a buffer before the it returns `Poll::Pending`.
    #[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
    pub fn new_unsplit(max_buf_size: usize) -> SimplexStream {
        SimplexStream {
            buffer: BytesMut::new(),
            is_closed: false,
            max_buf_size,
            read_waker: None,
            write_waker: None,
        }
    }

    fn close_write(&mut self) {
        self.is_closed = true;
        // needs to notify any readers that no more data will come
        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }
    }

    fn close_read(&mut self) {
        self.is_closed = true;
        // needs to notify any writers that they have to abort
        if let Some(waker) = self.write_waker.take() {
            waker.wake();
        }
    }

    fn poll_read_internal(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.buffer.has_remaining() {
            let max = self.buffer.remaining().min(buf.remaining());
            buf.put_slice(&self.buffer[..max]);
            self.buffer.advance(max);
            if max > 0 {
                // The passed `buf` might have been empty, don't wake up if
                // no bytes have been moved.
                if let Some(waker) = self.write_waker.take() {
                    waker.wake();
                }
            }
            Poll::Ready(Ok(()))
        } else if self.is_closed {
            Poll::Ready(Ok(()))
        } else {
            self.read_waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    fn poll_write_internal(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.is_closed {
            return Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()));
        }
        let avail = self.max_buf_size - self.buffer.len();
        if avail == 0 {
            self.write_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        let len = buf.len().min(avail);
        self.buffer.extend_from_slice(&buf[..len]);
        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }
        Poll::Ready(Ok(len))
    }

    fn poll_write_vectored_internal(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        if self.is_closed {
            return Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into()));
        }
        let avail = self.max_buf_size - self.buffer.len();
        if avail == 0 {
            self.write_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        let mut rem = avail;
        for buf in bufs {
            if rem == 0 {
                break;
            }

            let len = buf.len().min(rem);
            self.buffer.extend_from_slice(&buf[..len]);
            rem -= len;
        }

        if let Some(waker) = self.read_waker.take() {
            waker.wake();
        }
        Poll::Ready(Ok(avail - rem))
    }
}

impl AsyncRead for SimplexStream {
    cfg_coop! {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            ready!(crate::trace::trace_leaf(cx));
            let coop = ready!(crate::task::coop::poll_proceed(cx));

            let ret = self.poll_read_internal(cx, buf);
            if ret.is_ready() {
                coop.made_progress();
            }
            ret
        }
    }

    cfg_not_coop! {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            ready!(crate::trace::trace_leaf(cx));
            self.poll_read_internal(cx, buf)
        }
    }
}

impl AsyncWrite for SimplexStream {
    cfg_coop! {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            ready!(crate::trace::trace_leaf(cx));
            let coop = ready!(crate::task::coop::poll_proceed(cx));

            let ret = self.poll_write_internal(cx, buf);
            if ret.is_ready() {
                coop.made_progress();
            }
            ret
        }
    }

    cfg_not_coop! {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            ready!(crate::trace::trace_leaf(cx));
            self.poll_write_internal(cx, buf)
        }
    }

    cfg_coop! {
        fn poll_write_vectored(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> Poll<Result<usize, std::io::Error>> {
            ready!(crate::trace::trace_leaf(cx));
            let coop = ready!(crate::task::coop::poll_proceed(cx));

            let ret = self.poll_write_vectored_internal(cx, bufs);
            if ret.is_ready() {
                coop.made_progress();
            }
            ret
        }
    }

    cfg_not_coop! {
        fn poll_write_vectored(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> Poll<Result<usize, std::io::Error>> {
            ready!(crate::trace::trace_leaf(cx));
            self.poll_write_vectored_internal(cx, bufs)
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut task::Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        _: &mut task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.close_write();
        Poll::Ready(Ok(()))
    }
}
