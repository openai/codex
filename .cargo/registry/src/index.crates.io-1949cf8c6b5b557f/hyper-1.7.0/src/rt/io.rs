use std::fmt;
use std::mem::MaybeUninit;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};

// New IO traits? What?! Why, are you bonkers?
//
// I mean, yes, probably. But, here's the goals:
//
// 1. Supports poll-based IO operations.
// 2. Opt-in vectored IO.
// 3. Can use an optional buffer pool.
// 4. Able to add completion-based (uring) IO eventually.
//
// Frankly, the last point is the entire reason we're doing this. We want to
// have forwards-compatibility with an eventually stable io-uring runtime. We
// don't need that to work right away. But it must be possible to add in here
// without breaking hyper 1.0.
//
// While in here, if there's small tweaks to poll_read or poll_write that would
// allow even the "slow" path to be faster, such as if someone didn't remember
// to forward along an `is_completion` call.

/// Reads bytes from a source.
///
/// This trait is similar to `std::io::Read`, but supports asynchronous reads.
pub trait Read {
    /// Attempts to read bytes into the `buf`.
    ///
    /// On success, returns `Poll::Ready(Ok(()))` and places data in the
    /// unfilled portion of `buf`. If no data was read (`buf.remaining()` is
    /// unchanged), it implies that EOF has been reached.
    ///
    /// If no data is available for reading, the method returns `Poll::Pending`
    /// and arranges for the current task (via `cx.waker()`) to receive a
    /// notification when the object becomes readable or is closed.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>>;
}

/// Write bytes asynchronously.
///
/// This trait is similar to `std::io::Write`, but for asynchronous writes.
pub trait Write {
    /// Attempt to write bytes from `buf` into the destination.
    ///
    /// On success, returns `Poll::Ready(Ok(num_bytes_written)))`. If
    /// successful, it must be guaranteed that `n <= buf.len()`. A return value
    /// of `0` means that the underlying object is no longer able to accept
    /// bytes, or that the provided buffer is empty.
    ///
    /// If the object is not ready for writing, the method returns
    /// `Poll::Pending` and arranges for the current task (via `cx.waker()`) to
    /// receive a notification when the object becomes writable or is closed.
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>>;

    /// Attempts to flush the object.
    ///
    /// On success, returns `Poll::Ready(Ok(()))`.
    ///
    /// If flushing cannot immediately complete, this method returns
    /// `Poll::Pending` and arranges for the current task (via `cx.waker()`) to
    /// receive a notification when the object can make progress.
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>>;

    /// Attempts to shut down this writer.
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>>;

    /// Returns whether this writer has an efficient `poll_write_vectored`
    /// implementation.
    ///
    /// The default implementation returns `false`.
    fn is_write_vectored(&self) -> bool {
        false
    }

    /// Like `poll_write`, except that it writes from a slice of buffers.
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        let buf = bufs
            .iter()
            .find(|b| !b.is_empty())
            .map_or(&[][..], |b| &**b);
        self.poll_write(cx, buf)
    }
}

/// A wrapper around a byte buffer that is incrementally filled and initialized.
///
/// This type is a sort of "double cursor". It tracks three regions in the
/// buffer: a region at the beginning of the buffer that has been logically
/// filled with data, a region that has been initialized at some point but not
/// yet logically filled, and a region at the end that may be uninitialized.
/// The filled region is guaranteed to be a subset of the initialized region.
///
/// In summary, the contents of the buffer can be visualized as:
///
/// ```not_rust
/// [             capacity              ]
/// [ filled |         unfilled         ]
/// [    initialized    | uninitialized ]
/// ```
///
/// It is undefined behavior to de-initialize any bytes from the uninitialized
/// region, since it is merely unknown whether this region is uninitialized or
/// not, and if part of it turns out to be initialized, it must stay initialized.
pub struct ReadBuf<'a> {
    raw: &'a mut [MaybeUninit<u8>],
    filled: usize,
    init: usize,
}

/// The cursor part of a [`ReadBuf`].
///
/// This is created by calling `ReadBuf::unfilled()`.
#[derive(Debug)]
pub struct ReadBufCursor<'a> {
    buf: &'a mut ReadBuf<'a>,
}

impl<'data> ReadBuf<'data> {
    /// Create a new `ReadBuf` with a slice of initialized bytes.
    #[inline]
    pub fn new(raw: &'data mut [u8]) -> Self {
        let len = raw.len();
        Self {
            // SAFETY: We never de-init the bytes ourselves.
            raw: unsafe { &mut *(raw as *mut [u8] as *mut [MaybeUninit<u8>]) },
            filled: 0,
            init: len,
        }
    }

    /// Create a new `ReadBuf` with a slice of uninitialized bytes.
    #[inline]
    pub fn uninit(raw: &'data mut [MaybeUninit<u8>]) -> Self {
        Self {
            raw,
            filled: 0,
            init: 0,
        }
    }

    /// Get a slice of the buffer that has been filled in with bytes.
    #[inline]
    pub fn filled(&self) -> &[u8] {
        // SAFETY: We only slice the filled part of the buffer, which is always valid
        unsafe { &*(&self.raw[0..self.filled] as *const [MaybeUninit<u8>] as *const [u8]) }
    }

    /// Get a cursor to the unfilled portion of the buffer.
    #[inline]
    pub fn unfilled<'cursor>(&'cursor mut self) -> ReadBufCursor<'cursor> {
        ReadBufCursor {
            // SAFETY: self.buf is never re-assigned, so its safe to narrow
            // the lifetime.
            buf: unsafe {
                std::mem::transmute::<&'cursor mut ReadBuf<'data>, &'cursor mut ReadBuf<'cursor>>(
                    self,
                )
            },
        }
    }

    #[inline]
    #[cfg(all(any(feature = "client", feature = "server"), feature = "http2"))]
    pub(crate) unsafe fn set_init(&mut self, n: usize) {
        self.init = self.init.max(n);
    }

    #[inline]
    #[cfg(all(any(feature = "client", feature = "server"), feature = "http2"))]
    pub(crate) unsafe fn set_filled(&mut self, n: usize) {
        self.filled = self.filled.max(n);
    }

    #[inline]
    #[cfg(all(any(feature = "client", feature = "server"), feature = "http2"))]
    pub(crate) fn len(&self) -> usize {
        self.filled
    }

    #[inline]
    #[cfg(all(any(feature = "client", feature = "server"), feature = "http2"))]
    pub(crate) fn init_len(&self) -> usize {
        self.init
    }

    #[inline]
    fn remaining(&self) -> usize {
        self.capacity() - self.filled
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.raw.len()
    }
}

impl fmt::Debug for ReadBuf<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadBuf")
            .field("filled", &self.filled)
            .field("init", &self.init)
            .field("capacity", &self.capacity())
            .finish()
    }
}

impl ReadBufCursor<'_> {
    /// Access the unfilled part of the buffer.
    ///
    /// # Safety
    ///
    /// The caller must not uninitialize any bytes that may have been
    /// initialized before.
    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        &mut self.buf.raw[self.buf.filled..]
    }

    /// Advance the `filled` cursor by `n` bytes.
    ///
    /// # Safety
    ///
    /// The caller must take care that `n` more bytes have been initialized.
    #[inline]
    pub unsafe fn advance(&mut self, n: usize) {
        self.buf.filled = self.buf.filled.checked_add(n).expect("overflow");
        self.buf.init = self.buf.filled.max(self.buf.init);
    }

    /// Returns the number of bytes that can be written from the current
    /// position until the end of the buffer is reached.
    ///
    /// This value is equal to the length of the slice returned by `as_mut()``.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.buf.remaining()
    }

    /// Transfer bytes into `self`` from `src` and advance the cursor
    /// by the number of bytes written.
    ///
    /// # Panics
    ///
    /// `self` must have enough remaining capacity to contain all of `src`.
    #[inline]
    pub fn put_slice(&mut self, src: &[u8]) {
        assert!(
            self.buf.remaining() >= src.len(),
            "src.len() must fit in remaining()"
        );

        let amt = src.len();
        // Cannot overflow, asserted above
        let end = self.buf.filled + amt;

        // Safety: the length is asserted above
        unsafe {
            self.buf.raw[self.buf.filled..end]
                .as_mut_ptr()
                .cast::<u8>()
                .copy_from_nonoverlapping(src.as_ptr(), amt);
        }

        if self.buf.init < end {
            self.buf.init = end;
        }
        self.buf.filled = end;
    }
}

macro_rules! deref_async_read {
    () => {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: ReadBufCursor<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut **self).poll_read(cx, buf)
        }
    };
}

impl<T: ?Sized + Read + Unpin> Read for Box<T> {
    deref_async_read!();
}

impl<T: ?Sized + Read + Unpin> Read for &mut T {
    deref_async_read!();
}

impl<P> Read for Pin<P>
where
    P: DerefMut,
    P::Target: Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<std::io::Result<()>> {
        pin_as_deref_mut(self).poll_read(cx, buf)
    }
}

macro_rules! deref_async_write {
    () => {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut **self).poll_write(cx, buf)
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut **self).poll_write_vectored(cx, bufs)
        }

        fn is_write_vectored(&self) -> bool {
            (**self).is_write_vectored()
        }

        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Pin::new(&mut **self).poll_flush(cx)
        }

        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut **self).poll_shutdown(cx)
        }
    };
}

impl<T: ?Sized + Write + Unpin> Write for Box<T> {
    deref_async_write!();
}

impl<T: ?Sized + Write + Unpin> Write for &mut T {
    deref_async_write!();
}

impl<P> Write for Pin<P>
where
    P: DerefMut,
    P::Target: Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        pin_as_deref_mut(self).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        pin_as_deref_mut(self).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        (**self).is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        pin_as_deref_mut(self).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        pin_as_deref_mut(self).poll_shutdown(cx)
    }
}

/// Polyfill for Pin::as_deref_mut()
/// TODO: use Pin::as_deref_mut() instead once stabilized
fn pin_as_deref_mut<P: DerefMut>(pin: Pin<&mut Pin<P>>) -> Pin<&mut P::Target> {
    // SAFETY: we go directly from Pin<&mut Pin<P>> to Pin<&mut P::Target>, without moving or
    // giving out the &mut Pin<P> in the process. See Pin::as_deref_mut() for more detail.
    unsafe { pin.get_unchecked_mut() }.as_mut()
}
