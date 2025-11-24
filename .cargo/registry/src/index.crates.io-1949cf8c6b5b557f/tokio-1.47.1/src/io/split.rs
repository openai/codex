//! Split a single value implementing `AsyncRead + AsyncWrite` into separate
//! `AsyncRead` and `AsyncWrite` handles.
//!
//! To restore this read/write object from its `split::ReadHalf` and
//! `split::WriteHalf` use `unsplit`.

use crate::io::{AsyncRead, AsyncWrite, ReadBuf};

use std::fmt;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};

cfg_io_util! {
    /// The readable half of a value returned from [`split`](split()).
    pub struct ReadHalf<T> {
        inner: Arc<Inner<T>>,
    }

    /// The writable half of a value returned from [`split`](split()).
    pub struct WriteHalf<T> {
        inner: Arc<Inner<T>>,
    }

    /// Splits a single value implementing `AsyncRead + AsyncWrite` into separate
    /// `AsyncRead` and `AsyncWrite` handles.
    ///
    /// To restore this read/write object from its `ReadHalf` and
    /// `WriteHalf` use [`unsplit`](ReadHalf::unsplit()).
    pub fn split<T>(stream: T) -> (ReadHalf<T>, WriteHalf<T>)
    where
        T: AsyncRead + AsyncWrite,
    {
        let is_write_vectored = stream.is_write_vectored();

        let inner = Arc::new(Inner {
            stream: Mutex::new(stream),
            is_write_vectored,
        });

        let rd = ReadHalf {
            inner: inner.clone(),
        };

        let wr = WriteHalf { inner };

        (rd, wr)
    }
}

struct Inner<T> {
    stream: Mutex<T>,
    is_write_vectored: bool,
}

impl<T> Inner<T> {
    fn with_lock<R>(&self, f: impl FnOnce(Pin<&mut T>) -> R) -> R {
        let mut guard = self.stream.lock().unwrap();

        // safety: we do not move the stream.
        let stream = unsafe { Pin::new_unchecked(&mut *guard) };

        f(stream)
    }
}

impl<T> ReadHalf<T> {
    /// Checks if this `ReadHalf` and some `WriteHalf` were split from the same
    /// stream.
    pub fn is_pair_of(&self, other: &WriteHalf<T>) -> bool {
        other.is_pair_of(self)
    }

    /// Reunites with a previously split `WriteHalf`.
    ///
    /// # Panics
    ///
    /// If this `ReadHalf` and the given `WriteHalf` do not originate from the
    /// same `split` operation this method will panic.
    /// This can be checked ahead of time by calling [`is_pair_of()`](Self::is_pair_of).
    #[track_caller]
    pub fn unsplit(self, wr: WriteHalf<T>) -> T
    where
        T: Unpin,
    {
        if self.is_pair_of(&wr) {
            drop(wr);

            let inner = Arc::try_unwrap(self.inner)
                .ok()
                .expect("`Arc::try_unwrap` failed");

            inner.stream.into_inner().unwrap()
        } else {
            panic!("Unrelated `split::Write` passed to `split::Read::unsplit`.")
        }
    }
}

impl<T> WriteHalf<T> {
    /// Checks if this `WriteHalf` and some `ReadHalf` were split from the same
    /// stream.
    pub fn is_pair_of(&self, other: &ReadHalf<T>) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: AsyncRead> AsyncRead for ReadHalf<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.inner.with_lock(|stream| stream.poll_read(cx, buf))
    }
}

impl<T: AsyncWrite> AsyncWrite for WriteHalf<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        self.inner.with_lock(|stream| stream.poll_write(cx, buf))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.inner.with_lock(|stream| stream.poll_flush(cx))
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.inner.with_lock(|stream| stream.poll_shutdown(cx))
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        self.inner
            .with_lock(|stream| stream.poll_write_vectored(cx, bufs))
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored
    }
}

unsafe impl<T: Send> Send for ReadHalf<T> {}
unsafe impl<T: Send> Send for WriteHalf<T> {}
unsafe impl<T: Sync> Sync for ReadHalf<T> {}
unsafe impl<T: Sync> Sync for WriteHalf<T> {}

impl<T: fmt::Debug> fmt::Debug for ReadHalf<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("split::ReadHalf").finish()
    }
}

impl<T: fmt::Debug> fmt::Debug for WriteHalf<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("split::WriteHalf").finish()
    }
}
