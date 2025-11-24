//! Asynchronous I/O
//!
//! This crate contains the `AsyncRead`, `AsyncWrite`, `AsyncSeek`, and
//! `AsyncBufRead` traits, the asynchronous analogs to
//! `std::io::{Read, Write, Seek, BufRead}`. The primary difference is
//! that these traits integrate with the asynchronous task system.
//!
//! All items of this library are only available when the `std` feature of this
//! library is activated, and it is activated by default.

#![no_std]
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms, single_use_lifetimes),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]
#![warn(missing_docs, /* unsafe_op_in_unsafe_fn */)] // unsafe_op_in_unsafe_fn requires Rust 1.52
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
mod if_std {
    use std::boxed::Box;
    use std::io;
    use std::ops::DerefMut;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::vec::Vec;

    // Re-export some types from `std::io` so that users don't have to deal
    // with conflicts when `use`ing `futures::io` and `std::io`.
    #[allow(unreachable_pub)] // https://github.com/rust-lang/rust/issues/57411
    #[doc(no_inline)]
    pub use io::{Error, ErrorKind, IoSlice, IoSliceMut, Result, SeekFrom};

    /// Read bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Read` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_read`
    /// method, unlike `Read::read`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncRead {
        /// Attempt to read from the `AsyncRead` into `buf`.
        ///
        /// On success, returns `Poll::Ready(Ok(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<usize>>;

        /// Attempt to read from the `AsyncRead` into `bufs` using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_read`, but allows data to be read
        /// into multiple buffers using a single operation.
        ///
        /// On success, returns `Poll::Ready(Ok(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
        /// readable or is closed.
        /// By default, this method delegates to using `poll_read` on the first
        /// nonempty buffer in `bufs`, or an empty one if none exists. Objects which
        /// support vectored IO should override this method.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_read_vectored(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &mut [IoSliceMut<'_>],
        ) -> Poll<Result<usize>> {
            for b in bufs {
                if !b.is_empty() {
                    return self.poll_read(cx, b);
                }
            }

            self.poll_read(cx, &mut [])
        }
    }

    /// Write bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Write` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_write`
    /// method, unlike `Write::write`, will automatically queue the current task
    /// for wakeup and return if the writer cannot take more data, rather than blocking
    /// the calling thread.
    pub trait AsyncWrite {
        /// Attempt to write bytes from `buf` into the object.
        ///
        /// On success, returns `Poll::Ready(Ok(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
        /// writable or is closed.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        ///
        /// `poll_write` must try to make progress by flushing the underlying object if
        /// that is the only way the underlying object can become writable again.
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize>>;

        /// Attempt to write bytes from `bufs` into the object using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_write`, but allows data from multiple buffers to be written
        /// using a single operation.
        ///
        /// On success, returns `Poll::Ready(Ok(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
        /// writable or is closed.
        ///
        /// By default, this method delegates to using `poll_write` on the first
        /// nonempty buffer in `bufs`, or an empty one if none exists. Objects which
        /// support vectored IO should override this method.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_write_vectored(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<Result<usize>> {
            for b in bufs {
                if !b.is_empty() {
                    return self.poll_write(cx, b);
                }
            }

            self.poll_write(cx, &[])
        }

        /// Attempt to flush the object, ensuring that any buffered data reach
        /// their destination.
        ///
        /// On success, returns `Poll::Ready(Ok(()))`.
        ///
        /// If flushing cannot immediately complete, this method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object can make
        /// progress towards flushing.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        ///
        /// It only makes sense to do anything here if you actually buffer data.
        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>>;

        /// Attempt to close the object.
        ///
        /// On success, returns `Poll::Ready(Ok(()))`.
        ///
        /// If closing cannot immediately complete, this function returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object can make
        /// progress towards closing.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>>;
    }

    /// Seek bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Seek` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_seek`
    /// method, unlike `Seek::seek`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncSeek {
        /// Attempt to seek to an offset, in bytes, in a stream.
        ///
        /// A seek beyond the end of a stream is allowed, but behavior is defined
        /// by the implementation.
        ///
        /// If the seek operation completed successfully,
        /// this method returns the new position from the start of the stream.
        /// That position can be used later with [`SeekFrom::Start`].
        ///
        /// # Errors
        ///
        /// Seeking to a negative offset is considered an error.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_seek(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            pos: SeekFrom,
        ) -> Poll<Result<u64>>;
    }

    /// Read bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::BufRead` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_fill_buf`
    /// method, unlike `BufRead::fill_buf`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncBufRead: AsyncRead {
        /// Attempt to return the contents of the internal buffer, filling it with more data
        /// from the inner reader if it is empty.
        ///
        /// On success, returns `Poll::Ready(Ok(buf))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Poll::Pending` and arranges for the current task (via
        /// `cx.waker().wake_by_ref()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// This function is a lower-level call. It needs to be paired with the
        /// [`consume`] method to function properly. When calling this
        /// method, none of the contents will be "read" in the sense that later
        /// calling [`poll_read`] may return the same contents. As such, [`consume`] must
        /// be called with the number of bytes that are consumed from this buffer to
        /// ensure that the bytes are never returned twice.
        ///
        /// [`poll_read`]: AsyncRead::poll_read
        /// [`consume`]: AsyncBufRead::consume
        ///
        /// An empty buffer returned indicates that the stream has reached EOF.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Poll::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>>;

        /// Tells this buffer that `amt` bytes have been consumed from the buffer,
        /// so they should no longer be returned in calls to [`poll_read`].
        ///
        /// This function is a lower-level call. It needs to be paired with the
        /// [`poll_fill_buf`] method to function properly. This function does
        /// not perform any I/O, it simply informs this object that some amount of
        /// its buffer, returned from [`poll_fill_buf`], has been consumed and should
        /// no longer be returned. As such, this function may do odd things if
        /// [`poll_fill_buf`] isn't called before calling it.
        ///
        /// The `amt` must be `<=` the number of bytes in the buffer returned by
        /// [`poll_fill_buf`].
        ///
        /// [`poll_read`]: AsyncRead::poll_read
        /// [`poll_fill_buf`]: AsyncBufRead::poll_fill_buf
        fn consume(self: Pin<&mut Self>, amt: usize);
    }

    macro_rules! deref_async_read {
        () => {
            fn poll_read(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &mut [u8],
            ) -> Poll<Result<usize>> {
                Pin::new(&mut **self).poll_read(cx, buf)
            }

            fn poll_read_vectored(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                bufs: &mut [IoSliceMut<'_>],
            ) -> Poll<Result<usize>> {
                Pin::new(&mut **self).poll_read_vectored(cx, bufs)
            }
        };
    }

    impl<T: ?Sized + AsyncRead + Unpin> AsyncRead for Box<T> {
        deref_async_read!();
    }

    impl<T: ?Sized + AsyncRead + Unpin> AsyncRead for &mut T {
        deref_async_read!();
    }

    impl<P> AsyncRead for Pin<P>
    where
        P: DerefMut + Unpin,
        P::Target: AsyncRead,
    {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<usize>> {
            self.get_mut().as_mut().poll_read(cx, buf)
        }

        fn poll_read_vectored(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &mut [IoSliceMut<'_>],
        ) -> Poll<Result<usize>> {
            self.get_mut().as_mut().poll_read_vectored(cx, bufs)
        }
    }

    macro_rules! delegate_async_read_to_stdio {
        () => {
            fn poll_read(
                mut self: Pin<&mut Self>,
                _: &mut Context<'_>,
                buf: &mut [u8],
            ) -> Poll<Result<usize>> {
                Poll::Ready(io::Read::read(&mut *self, buf))
            }

            fn poll_read_vectored(
                mut self: Pin<&mut Self>,
                _: &mut Context<'_>,
                bufs: &mut [IoSliceMut<'_>],
            ) -> Poll<Result<usize>> {
                Poll::Ready(io::Read::read_vectored(&mut *self, bufs))
            }
        };
    }

    impl AsyncRead for &[u8] {
        delegate_async_read_to_stdio!();
    }

    macro_rules! deref_async_write {
        () => {
            fn poll_write(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buf: &[u8],
            ) -> Poll<Result<usize>> {
                Pin::new(&mut **self).poll_write(cx, buf)
            }

            fn poll_write_vectored(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                bufs: &[IoSlice<'_>],
            ) -> Poll<Result<usize>> {
                Pin::new(&mut **self).poll_write_vectored(cx, bufs)
            }

            fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
                Pin::new(&mut **self).poll_flush(cx)
            }

            fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
                Pin::new(&mut **self).poll_close(cx)
            }
        };
    }

    impl<T: ?Sized + AsyncWrite + Unpin> AsyncWrite for Box<T> {
        deref_async_write!();
    }

    impl<T: ?Sized + AsyncWrite + Unpin> AsyncWrite for &mut T {
        deref_async_write!();
    }

    impl<P> AsyncWrite for Pin<P>
    where
        P: DerefMut + Unpin,
        P::Target: AsyncWrite,
    {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize>> {
            self.get_mut().as_mut().poll_write(cx, buf)
        }

        fn poll_write_vectored(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<Result<usize>> {
            self.get_mut().as_mut().poll_write_vectored(cx, bufs)
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            self.get_mut().as_mut().poll_flush(cx)
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            self.get_mut().as_mut().poll_close(cx)
        }
    }

    macro_rules! delegate_async_write_to_stdio {
        () => {
            fn poll_write(
                mut self: Pin<&mut Self>,
                _: &mut Context<'_>,
                buf: &[u8],
            ) -> Poll<Result<usize>> {
                Poll::Ready(io::Write::write(&mut *self, buf))
            }

            fn poll_write_vectored(
                mut self: Pin<&mut Self>,
                _: &mut Context<'_>,
                bufs: &[IoSlice<'_>],
            ) -> Poll<Result<usize>> {
                Poll::Ready(io::Write::write_vectored(&mut *self, bufs))
            }

            fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
                Poll::Ready(io::Write::flush(&mut *self))
            }

            fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
                self.poll_flush(cx)
            }
        };
    }

    impl AsyncWrite for Vec<u8> {
        delegate_async_write_to_stdio!();
    }

    macro_rules! deref_async_seek {
        () => {
            fn poll_seek(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                pos: SeekFrom,
            ) -> Poll<Result<u64>> {
                Pin::new(&mut **self).poll_seek(cx, pos)
            }
        };
    }

    impl<T: ?Sized + AsyncSeek + Unpin> AsyncSeek for Box<T> {
        deref_async_seek!();
    }

    impl<T: ?Sized + AsyncSeek + Unpin> AsyncSeek for &mut T {
        deref_async_seek!();
    }

    impl<P> AsyncSeek for Pin<P>
    where
        P: DerefMut + Unpin,
        P::Target: AsyncSeek,
    {
        fn poll_seek(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            pos: SeekFrom,
        ) -> Poll<Result<u64>> {
            self.get_mut().as_mut().poll_seek(cx, pos)
        }
    }

    macro_rules! deref_async_buf_read {
        () => {
            fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
                Pin::new(&mut **self.get_mut()).poll_fill_buf(cx)
            }

            fn consume(mut self: Pin<&mut Self>, amt: usize) {
                Pin::new(&mut **self).consume(amt)
            }
        };
    }

    impl<T: ?Sized + AsyncBufRead + Unpin> AsyncBufRead for Box<T> {
        deref_async_buf_read!();
    }

    impl<T: ?Sized + AsyncBufRead + Unpin> AsyncBufRead for &mut T {
        deref_async_buf_read!();
    }

    impl<P> AsyncBufRead for Pin<P>
    where
        P: DerefMut + Unpin,
        P::Target: AsyncBufRead,
    {
        fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
            self.get_mut().as_mut().poll_fill_buf(cx)
        }

        fn consume(self: Pin<&mut Self>, amt: usize) {
            self.get_mut().as_mut().consume(amt)
        }
    }

    macro_rules! delegate_async_buf_read_to_stdio {
        () => {
            fn poll_fill_buf(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<&[u8]>> {
                Poll::Ready(io::BufRead::fill_buf(self.get_mut()))
            }

            fn consume(self: Pin<&mut Self>, amt: usize) {
                io::BufRead::consume(self.get_mut(), amt)
            }
        };
    }

    impl AsyncBufRead for &[u8] {
        delegate_async_buf_read_to_stdio!();
    }
}

#[cfg(feature = "std")]
pub use self::if_std::*;
