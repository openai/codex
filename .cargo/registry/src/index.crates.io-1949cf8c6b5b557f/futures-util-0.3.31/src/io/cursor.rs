use futures_core::task::{Context, Poll};
use futures_io::{AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite, IoSlice, IoSliceMut, SeekFrom};
use std::boxed::Box;
use std::io;
use std::pin::Pin;
use std::vec::Vec;

/// A `Cursor` wraps an in-memory buffer and provides it with a
/// [`AsyncSeek`] implementation.
///
/// `Cursor`s are used with in-memory buffers, anything implementing
/// `AsRef<[u8]>`, to allow them to implement [`AsyncRead`] and/or [`AsyncWrite`],
/// allowing these buffers to be used anywhere you might use a reader or writer
/// that does actual I/O.
///
/// This library implements some I/O traits on various types which
/// are commonly used as a buffer, like `Cursor<`[`Vec`]`<u8>>` and
/// `Cursor<`[`&[u8]`][bytes]`>`.
///
/// [`AsyncSeek`]: trait.AsyncSeek.html
/// [`AsyncRead`]: trait.AsyncRead.html
/// [`AsyncWrite`]: trait.AsyncWrite.html
/// [bytes]: https://doc.rust-lang.org/std/primitive.slice.html
#[derive(Clone, Debug, Default)]
pub struct Cursor<T> {
    inner: io::Cursor<T>,
}

impl<T> Cursor<T> {
    /// Creates a new cursor wrapping the provided underlying in-memory buffer.
    ///
    /// Cursor initial position is `0` even if underlying buffer (e.g., `Vec`)
    /// is not empty. So writing to cursor starts with overwriting `Vec`
    /// content, not with appending to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    /// ```
    pub fn new(inner: T) -> Self {
        Self { inner: io::Cursor::new(inner) }
    }

    /// Consumes this cursor, returning the underlying value.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let vec = buff.into_inner();
    /// ```
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }

    /// Gets a reference to the underlying value in this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let reference = buff.get_ref();
    /// ```
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    /// Gets a mutable reference to the underlying value in this cursor.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying value as it may corrupt this cursor's position.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::io::Cursor;
    ///
    /// let mut buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let reference = buff.get_mut();
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    /// Returns the current position of this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncSeekExt, Cursor, SeekFrom};
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.position(), 0);
    ///
    /// buff.seek(SeekFrom::Current(2)).await?;
    /// assert_eq!(buff.position(), 2);
    ///
    /// buff.seek(SeekFrom::Current(-1)).await?;
    /// assert_eq!(buff.position(), 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    pub fn position(&self) -> u64 {
        self.inner.position()
    }

    /// Sets the position of this cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use futures::io::Cursor;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.position(), 0);
    ///
    /// buff.set_position(2);
    /// assert_eq!(buff.position(), 2);
    ///
    /// buff.set_position(4);
    /// assert_eq!(buff.position(), 4);
    /// ```
    pub fn set_position(&mut self, pos: u64) {
        self.inner.set_position(pos)
    }
}

impl<T> AsyncSeek for Cursor<T>
where
    T: AsRef<[u8]> + Unpin,
{
    fn poll_seek(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        Poll::Ready(io::Seek::seek(&mut self.inner, pos))
    }
}

impl<T: AsRef<[u8]> + Unpin> AsyncRead for Cursor<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(io::Read::read(&mut self.inner, buf))
    }

    fn poll_read_vectored(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(io::Read::read_vectored(&mut self.inner, bufs))
    }
}

impl<T> AsyncBufRead for Cursor<T>
where
    T: AsRef<[u8]> + Unpin,
{
    fn poll_fill_buf(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        Poll::Ready(io::BufRead::fill_buf(&mut self.get_mut().inner))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        io::BufRead::consume(&mut self.inner, amt)
    }
}

macro_rules! delegate_async_write_to_stdio {
    () => {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(io::Write::write(&mut self.inner, buf))
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(io::Write::write_vectored(&mut self.inner, bufs))
        }

        fn poll_flush(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(io::Write::flush(&mut self.inner))
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            self.poll_flush(cx)
        }
    };
}

impl AsyncWrite for Cursor<&mut [u8]> {
    delegate_async_write_to_stdio!();
}

impl AsyncWrite for Cursor<&mut Vec<u8>> {
    delegate_async_write_to_stdio!();
}

impl AsyncWrite for Cursor<Vec<u8>> {
    delegate_async_write_to_stdio!();
}

impl AsyncWrite for Cursor<Box<[u8]>> {
    delegate_async_write_to_stdio!();
}
