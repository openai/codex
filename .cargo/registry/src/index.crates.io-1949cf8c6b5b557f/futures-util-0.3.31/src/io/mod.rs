//! Asynchronous I/O.
//!
//! This module is the asynchronous version of `std::io`. It defines four
//! traits, [`AsyncRead`], [`AsyncWrite`], [`AsyncSeek`], and [`AsyncBufRead`],
//! which mirror the `Read`, `Write`, `Seek`, and `BufRead` traits of the
//! standard library. However, these traits integrate with the asynchronous
//! task system, so that if an I/O object isn't ready for reading (or writing),
//! the thread is not blocked, and instead the current task is queued to be
//! woken when I/O is ready.
//!
//! In addition, the [`AsyncReadExt`], [`AsyncWriteExt`], [`AsyncSeekExt`], and
//! [`AsyncBufReadExt`] extension traits offer a variety of useful combinators
//! for operating with asynchronous I/O objects, including ways to work with
//! them using futures, streams and sinks.
//!
//! This module is only available when the `std` feature of this
//! library is activated, and it is activated by default.

#[cfg(feature = "io-compat")]
#[cfg_attr(docsrs, doc(cfg(feature = "io-compat")))]
use crate::compat::Compat;
use crate::future::assert_future;
use crate::stream::assert_stream;
use std::{pin::Pin, ptr, string::String, vec::Vec};

// Re-export some types from `std::io` so that users don't have to deal
// with conflicts when `use`ing `futures::io` and `std::io`.
#[doc(no_inline)]
pub use std::io::{Error, ErrorKind, IoSlice, IoSliceMut, Result, SeekFrom};

pub use futures_io::{AsyncBufRead, AsyncRead, AsyncSeek, AsyncWrite};

// used by `BufReader` and `BufWriter`
// https://github.com/rust-lang/rust/blob/master/src/libstd/sys_common/io.rs#L1
const DEFAULT_BUF_SIZE: usize = 8 * 1024;

/// Initializes a buffer if necessary.
///
/// A buffer is currently always initialized.
#[inline]
unsafe fn initialize<R: AsyncRead>(_reader: &R, buf: &mut [u8]) {
    unsafe { ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len()) }
}

mod allow_std;
pub use self::allow_std::AllowStdIo;

mod buf_reader;
pub use self::buf_reader::{BufReader, SeeKRelative};

mod buf_writer;
pub use self::buf_writer::BufWriter;

mod line_writer;
pub use self::line_writer::LineWriter;

mod chain;
pub use self::chain::Chain;

mod close;
pub use self::close::Close;

mod copy;
pub use self::copy::{copy, Copy};

mod copy_buf;
pub use self::copy_buf::{copy_buf, CopyBuf};

mod copy_buf_abortable;
pub use self::copy_buf_abortable::{copy_buf_abortable, CopyBufAbortable};

mod cursor;
pub use self::cursor::Cursor;

mod empty;
pub use self::empty::{empty, Empty};

mod fill_buf;
pub use self::fill_buf::FillBuf;

mod flush;
pub use self::flush::Flush;

#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
mod into_sink;
#[cfg(feature = "sink")]
#[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
pub use self::into_sink::IntoSink;

mod lines;
pub use self::lines::Lines;

mod read;
pub use self::read::Read;

mod read_vectored;
pub use self::read_vectored::ReadVectored;

mod read_exact;
pub use self::read_exact::ReadExact;

mod read_line;
pub use self::read_line::ReadLine;

mod read_to_end;
pub use self::read_to_end::ReadToEnd;

mod read_to_string;
pub use self::read_to_string::ReadToString;

mod read_until;
pub use self::read_until::ReadUntil;

mod repeat;
pub use self::repeat::{repeat, Repeat};

mod seek;
pub use self::seek::Seek;

mod sink;
pub use self::sink::{sink, Sink};

mod split;
pub use self::split::{ReadHalf, ReuniteError, WriteHalf};

mod take;
pub use self::take::Take;

mod window;
pub use self::window::Window;

mod write;
pub use self::write::Write;

mod write_vectored;
pub use self::write_vectored::WriteVectored;

mod write_all;
pub use self::write_all::WriteAll;

#[cfg(feature = "write-all-vectored")]
mod write_all_vectored;
#[cfg(feature = "write-all-vectored")]
pub use self::write_all_vectored::WriteAllVectored;

/// An extension trait which adds utility methods to `AsyncRead` types.
pub trait AsyncReadExt: AsyncRead {
    /// Creates an adaptor which will chain this stream with another.
    ///
    /// The returned `AsyncRead` instance will first read all bytes from this object
    /// until EOF is encountered. Afterwards the output is equivalent to the
    /// output of `next`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncReadExt, Cursor};
    ///
    /// let reader1 = Cursor::new([1, 2, 3, 4]);
    /// let reader2 = Cursor::new([5, 6, 7, 8]);
    ///
    /// let mut reader = reader1.chain(reader2);
    /// let mut buffer = Vec::new();
    ///
    /// // read the value into a Vec.
    /// reader.read_to_end(&mut buffer).await?;
    /// assert_eq!(buffer, [1, 2, 3, 4, 5, 6, 7, 8]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn chain<R>(self, next: R) -> Chain<Self, R>
    where
        Self: Sized,
        R: AsyncRead,
    {
        assert_read(Chain::new(self, next))
    }

    /// Tries to read some bytes directly into the given `buf` in asynchronous
    /// manner, returning a future type.
    ///
    /// The returned future will resolve to the number of bytes read once the read
    /// operation is completed.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncReadExt, Cursor};
    ///
    /// let mut reader = Cursor::new([1, 2, 3, 4]);
    /// let mut output = [0u8; 5];
    ///
    /// let bytes = reader.read(&mut output[..]).await?;
    ///
    /// // This is only guaranteed to be 4 because `&[u8]` is a synchronous
    /// // reader. In a real system you could get anywhere from 1 to
    /// // `output.len()` bytes in a single read.
    /// assert_eq!(bytes, 4);
    /// assert_eq!(output, [1, 2, 3, 4, 0]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> Read<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<usize>, _>(Read::new(self, buf))
    }

    /// Creates a future which will read from the `AsyncRead` into `bufs` using vectored
    /// IO operations.
    ///
    /// The returned future will resolve to the number of bytes read once the read
    /// operation is completed.
    fn read_vectored<'a>(&'a mut self, bufs: &'a mut [IoSliceMut<'a>]) -> ReadVectored<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<usize>, _>(ReadVectored::new(self, bufs))
    }

    /// Creates a future which will read exactly enough bytes to fill `buf`,
    /// returning an error if end of file (EOF) is hit sooner.
    ///
    /// The returned future will resolve once the read operation is completed.
    ///
    /// In the case of an error the buffer and the object will be discarded, with
    /// the error yielded.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncReadExt, Cursor};
    ///
    /// let mut reader = Cursor::new([1, 2, 3, 4]);
    /// let mut output = [0u8; 4];
    ///
    /// reader.read_exact(&mut output).await?;
    ///
    /// assert_eq!(output, [1, 2, 3, 4]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    ///
    /// ## EOF is hit before `buf` is filled
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{self, AsyncReadExt, Cursor};
    ///
    /// let mut reader = Cursor::new([1, 2, 3, 4]);
    /// let mut output = [0u8; 5];
    ///
    /// let result = reader.read_exact(&mut output).await;
    ///
    /// assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    /// # });
    /// ```
    fn read_exact<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadExact<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<()>, _>(ReadExact::new(self, buf))
    }

    /// Creates a future which will read all the bytes from this `AsyncRead`.
    ///
    /// On success the total number of bytes read is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncReadExt, Cursor};
    ///
    /// let mut reader = Cursor::new([1, 2, 3, 4]);
    /// let mut output = Vec::with_capacity(4);
    ///
    /// let bytes = reader.read_to_end(&mut output).await?;
    ///
    /// assert_eq!(bytes, 4);
    /// assert_eq!(output, vec![1, 2, 3, 4]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> ReadToEnd<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<usize>, _>(ReadToEnd::new(self, buf))
    }

    /// Creates a future which will read all the bytes from this `AsyncRead`.
    ///
    /// On success the total number of bytes read is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncReadExt, Cursor};
    ///
    /// let mut reader = Cursor::new(&b"1234"[..]);
    /// let mut buffer = String::with_capacity(4);
    ///
    /// let bytes = reader.read_to_string(&mut buffer).await?;
    ///
    /// assert_eq!(bytes, 4);
    /// assert_eq!(buffer, String::from("1234"));
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn read_to_string<'a>(&'a mut self, buf: &'a mut String) -> ReadToString<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<usize>, _>(ReadToString::new(self, buf))
    }

    /// Helper method for splitting this read/write object into two halves.
    ///
    /// The two halves returned implement the `AsyncRead` and `AsyncWrite`
    /// traits, respectively.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{self, AsyncReadExt, Cursor};
    ///
    /// // Note that for `Cursor` the read and write halves share a single
    /// // seek position. This may or may not be true for other types that
    /// // implement both `AsyncRead` and `AsyncWrite`.
    ///
    /// let reader = Cursor::new([1, 2, 3, 4]);
    /// let mut buffer = Cursor::new(vec![0, 0, 0, 0, 5, 6, 7, 8]);
    /// let mut writer = Cursor::new(vec![0u8; 5]);
    ///
    /// {
    ///     let (buffer_reader, mut buffer_writer) = (&mut buffer).split();
    ///     io::copy(reader, &mut buffer_writer).await?;
    ///     io::copy(buffer_reader, &mut writer).await?;
    /// }
    ///
    /// assert_eq!(buffer.into_inner(), [1, 2, 3, 4, 5, 6, 7, 8]);
    /// assert_eq!(writer.into_inner(), [5, 6, 7, 8, 0]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn split(self) -> (ReadHalf<Self>, WriteHalf<Self>)
    where
        Self: AsyncWrite + Sized,
    {
        let (r, w) = split::split(self);
        (assert_read(r), assert_write(w))
    }

    /// Creates an AsyncRead adapter which will read at most `limit` bytes
    /// from the underlying reader.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncReadExt, Cursor};
    ///
    /// let reader = Cursor::new(&b"12345678"[..]);
    /// let mut buffer = [0; 5];
    ///
    /// let mut take = reader.take(4);
    /// let n = take.read(&mut buffer).await?;
    ///
    /// assert_eq!(n, 4);
    /// assert_eq!(&buffer, b"1234\0");
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn take(self, limit: u64) -> Take<Self>
    where
        Self: Sized,
    {
        assert_read(Take::new(self, limit))
    }

    /// Wraps an [`AsyncRead`] in a compatibility wrapper that allows it to be
    /// used as a futures 0.1 / tokio-io 0.1 `AsyncRead`. If the wrapped type
    /// implements [`AsyncWrite`] as well, the result will also implement the
    /// futures 0.1 / tokio 0.1 `AsyncWrite` trait.
    ///
    /// Requires the `io-compat` feature to enable.
    #[cfg(feature = "io-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "io-compat")))]
    fn compat(self) -> Compat<Self>
    where
        Self: Sized + Unpin,
    {
        Compat::new(self)
    }
}

impl<R: AsyncRead + ?Sized> AsyncReadExt for R {}

/// An extension trait which adds utility methods to `AsyncWrite` types.
pub trait AsyncWriteExt: AsyncWrite {
    /// Creates a future which will entirely flush this `AsyncWrite`.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AllowStdIo, AsyncWriteExt};
    /// use std::io::{BufWriter, Cursor};
    ///
    /// let mut output = vec![0u8; 5];
    ///
    /// {
    ///     let writer = Cursor::new(&mut output);
    ///     let mut buffered = AllowStdIo::new(BufWriter::new(writer));
    ///     buffered.write_all(&[1, 2]).await?;
    ///     buffered.write_all(&[3, 4]).await?;
    ///     buffered.flush().await?;
    /// }
    ///
    /// assert_eq!(output, [1, 2, 3, 4, 0]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn flush(&mut self) -> Flush<'_, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<()>, _>(Flush::new(self))
    }

    /// Creates a future which will entirely close this `AsyncWrite`.
    fn close(&mut self) -> Close<'_, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<()>, _>(Close::new(self))
    }

    /// Creates a future which will write bytes from `buf` into the object.
    ///
    /// The returned future will resolve to the number of bytes written once the write
    /// operation is completed.
    fn write<'a>(&'a mut self, buf: &'a [u8]) -> Write<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<usize>, _>(Write::new(self, buf))
    }

    /// Creates a future which will write bytes from `bufs` into the object using vectored
    /// IO operations.
    ///
    /// The returned future will resolve to the number of bytes written once the write
    /// operation is completed.
    fn write_vectored<'a>(&'a mut self, bufs: &'a [IoSlice<'a>]) -> WriteVectored<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<usize>, _>(WriteVectored::new(self, bufs))
    }

    /// Write data into this object.
    ///
    /// Creates a future that will write the entire contents of the buffer `buf` into
    /// this `AsyncWrite`.
    ///
    /// The returned future will not complete until all the data has been written.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncWriteExt, Cursor};
    ///
    /// let mut writer = Cursor::new(vec![0u8; 5]);
    ///
    /// writer.write_all(&[1, 2, 3, 4]).await?;
    ///
    /// assert_eq!(writer.into_inner(), [1, 2, 3, 4, 0]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteAll<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<()>, _>(WriteAll::new(self, buf))
    }

    /// Attempts to write multiple buffers into this writer.
    ///
    /// Creates a future that will write the entire contents of `bufs` into this
    /// `AsyncWrite` using [vectored writes].
    ///
    /// The returned future will not complete until all the data has been
    /// written.
    ///
    /// [vectored writes]: std::io::Write::write_vectored
    ///
    /// # Notes
    ///
    /// Unlike `io::Write::write_vectored`, this takes a *mutable* reference to
    /// a slice of `IoSlice`s, not an immutable one. That's because we need to
    /// modify the slice to keep track of the bytes already written.
    ///
    /// Once this futures returns, the contents of `bufs` are unspecified, as
    /// this depends on how many calls to `write_vectored` were necessary. It is
    /// best to understand this function as taking ownership of `bufs` and to
    /// not use `bufs` afterwards. The underlying buffers, to which the
    /// `IoSlice`s point (but not the `IoSlice`s themselves), are unchanged and
    /// can be reused.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::AsyncWriteExt;
    /// use futures_util::io::Cursor;
    /// use std::io::IoSlice;
    ///
    /// let mut writer = Cursor::new(Vec::new());
    /// let bufs = &mut [
    ///     IoSlice::new(&[1]),
    ///     IoSlice::new(&[2, 3]),
    ///     IoSlice::new(&[4, 5, 6]),
    /// ];
    ///
    /// writer.write_all_vectored(bufs).await?;
    /// // Note: the contents of `bufs` is now unspecified, see the Notes section.
    ///
    /// assert_eq!(writer.into_inner(), &[1, 2, 3, 4, 5, 6]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    #[cfg(feature = "write-all-vectored")]
    fn write_all_vectored<'a>(
        &'a mut self,
        bufs: &'a mut [IoSlice<'a>],
    ) -> WriteAllVectored<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<()>, _>(WriteAllVectored::new(self, bufs))
    }

    /// Wraps an [`AsyncWrite`] in a compatibility wrapper that allows it to be
    /// used as a futures 0.1 / tokio-io 0.1 `AsyncWrite`.
    /// Requires the `io-compat` feature to enable.
    #[cfg(feature = "io-compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "io-compat")))]
    fn compat_write(self) -> Compat<Self>
    where
        Self: Sized + Unpin,
    {
        Compat::new(self)
    }

    /// Allow using an [`AsyncWrite`] as a [`Sink`](futures_sink::Sink)`<Item: AsRef<[u8]>>`.
    ///
    /// This adapter produces a sink that will write each value passed to it
    /// into the underlying writer.
    ///
    /// Note that this function consumes the given writer, returning a wrapped
    /// version.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::AsyncWriteExt;
    /// use futures::stream::{self, StreamExt};
    ///
    /// let stream = stream::iter(vec![Ok([1, 2, 3]), Ok([4, 5, 6])]);
    ///
    /// let mut writer = vec![];
    ///
    /// stream.forward((&mut writer).into_sink()).await?;
    ///
    /// assert_eq!(writer, vec![1, 2, 3, 4, 5, 6]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// # })?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[cfg(feature = "sink")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sink")))]
    fn into_sink<Item: AsRef<[u8]>>(self) -> IntoSink<Self, Item>
    where
        Self: Sized,
    {
        crate::sink::assert_sink::<Item, Error, _>(IntoSink::new(self))
    }
}

impl<W: AsyncWrite + ?Sized> AsyncWriteExt for W {}

/// An extension trait which adds utility methods to `AsyncSeek` types.
pub trait AsyncSeekExt: AsyncSeek {
    /// Creates a future which will seek an IO object, and then yield the
    /// new position in the object and the object itself.
    ///
    /// In the case of an error the buffer and the object will be discarded, with
    /// the error yielded.
    fn seek(&mut self, pos: SeekFrom) -> Seek<'_, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<u64>, _>(Seek::new(self, pos))
    }

    /// Creates a future which will return the current seek position from the
    /// start of the stream.
    ///
    /// This is equivalent to `self.seek(SeekFrom::Current(0))`.
    fn stream_position(&mut self) -> Seek<'_, Self>
    where
        Self: Unpin,
    {
        self.seek(SeekFrom::Current(0))
    }
}

impl<S: AsyncSeek + ?Sized> AsyncSeekExt for S {}

/// An extension trait which adds utility methods to `AsyncBufRead` types.
pub trait AsyncBufReadExt: AsyncBufRead {
    /// Creates a future which will wait for a non-empty buffer to be available from this I/O
    /// object or EOF to be reached.
    ///
    /// This method is the async equivalent to [`BufRead::fill_buf`](std::io::BufRead::fill_buf).
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use futures::{io::AsyncBufReadExt as _, stream::{iter, TryStreamExt as _}};
    ///
    /// let mut stream = iter(vec![Ok(vec![1, 2, 3]), Ok(vec![4, 5, 6])]).into_async_read();
    ///
    /// assert_eq!(stream.fill_buf().await?, vec![1, 2, 3]);
    /// stream.consume_unpin(2);
    ///
    /// assert_eq!(stream.fill_buf().await?, vec![3]);
    /// stream.consume_unpin(1);
    ///
    /// assert_eq!(stream.fill_buf().await?, vec![4, 5, 6]);
    /// stream.consume_unpin(3);
    ///
    /// assert_eq!(stream.fill_buf().await?, vec![]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn fill_buf(&mut self) -> FillBuf<'_, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<&[u8]>, _>(FillBuf::new(self))
    }

    /// A convenience for calling [`AsyncBufRead::consume`] on [`Unpin`] IO types.
    ///
    /// ```rust
    /// # futures::executor::block_on(async {
    /// use futures::{io::AsyncBufReadExt as _, stream::{iter, TryStreamExt as _}};
    ///
    /// let mut stream = iter(vec![Ok(vec![1, 2, 3])]).into_async_read();
    ///
    /// assert_eq!(stream.fill_buf().await?, vec![1, 2, 3]);
    /// stream.consume_unpin(2);
    ///
    /// assert_eq!(stream.fill_buf().await?, vec![3]);
    /// stream.consume_unpin(1);
    ///
    /// assert_eq!(stream.fill_buf().await?, vec![]);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn consume_unpin(&mut self, amt: usize)
    where
        Self: Unpin,
    {
        Pin::new(self).consume(amt)
    }

    /// Creates a future which will read all the bytes associated with this I/O
    /// object into `buf` until the delimiter `byte` or EOF is reached.
    /// This method is the async equivalent to [`BufRead::read_until`](std::io::BufRead::read_until).
    ///
    /// This function will read bytes from the underlying stream until the
    /// delimiter or EOF is found. Once found, all bytes up to, and including,
    /// the delimiter (if found) will be appended to `buf`.
    ///
    /// The returned future will resolve to the number of bytes read once the read
    /// operation is completed.
    ///
    /// In the case of an error the buffer and the object will be discarded, with
    /// the error yielded.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncBufReadExt, Cursor};
    ///
    /// let mut cursor = Cursor::new(b"lorem-ipsum");
    /// let mut buf = vec![];
    ///
    /// // cursor is at 'l'
    /// let num_bytes = cursor.read_until(b'-', &mut buf).await?;
    /// assert_eq!(num_bytes, 6);
    /// assert_eq!(buf, b"lorem-");
    /// buf.clear();
    ///
    /// // cursor is at 'i'
    /// let num_bytes = cursor.read_until(b'-', &mut buf).await?;
    /// assert_eq!(num_bytes, 5);
    /// assert_eq!(buf, b"ipsum");
    /// buf.clear();
    ///
    /// // cursor is at EOF
    /// let num_bytes = cursor.read_until(b'-', &mut buf).await?;
    /// assert_eq!(num_bytes, 0);
    /// assert_eq!(buf, b"");
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn read_until<'a>(&'a mut self, byte: u8, buf: &'a mut Vec<u8>) -> ReadUntil<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<usize>, _>(ReadUntil::new(self, byte, buf))
    }

    /// Creates a future which will read all the bytes associated with this I/O
    /// object into `buf` until a newline (the 0xA byte) or EOF is reached,
    /// This method is the async equivalent to [`BufRead::read_line`](std::io::BufRead::read_line).
    ///
    /// This function will read bytes from the underlying stream until the
    /// newline delimiter (the 0xA byte) or EOF is found. Once found, all bytes
    /// up to, and including, the delimiter (if found) will be appended to
    /// `buf`.
    ///
    /// The returned future will resolve to the number of bytes read once the read
    /// operation is completed.
    ///
    /// In the case of an error the buffer and the object will be discarded, with
    /// the error yielded.
    ///
    /// # Errors
    ///
    /// This function has the same error semantics as [`read_until`] and will
    /// also return an error if the read bytes are not valid UTF-8. If an I/O
    /// error is encountered then `buf` may contain some bytes already read in
    /// the event that all data read so far was valid UTF-8.
    ///
    /// [`read_until`]: AsyncBufReadExt::read_until
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncBufReadExt, Cursor};
    ///
    /// let mut cursor = Cursor::new(b"foo\nbar");
    /// let mut buf = String::new();
    ///
    /// // cursor is at 'f'
    /// let num_bytes = cursor.read_line(&mut buf).await?;
    /// assert_eq!(num_bytes, 4);
    /// assert_eq!(buf, "foo\n");
    /// buf.clear();
    ///
    /// // cursor is at 'b'
    /// let num_bytes = cursor.read_line(&mut buf).await?;
    /// assert_eq!(num_bytes, 3);
    /// assert_eq!(buf, "bar");
    /// buf.clear();
    ///
    /// // cursor is at EOF
    /// let num_bytes = cursor.read_line(&mut buf).await?;
    /// assert_eq!(num_bytes, 0);
    /// assert_eq!(buf, "");
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn read_line<'a>(&'a mut self, buf: &'a mut String) -> ReadLine<'a, Self>
    where
        Self: Unpin,
    {
        assert_future::<Result<usize>, _>(ReadLine::new(self, buf))
    }

    /// Returns a stream over the lines of this reader.
    /// This method is the async equivalent to [`BufRead::lines`](std::io::BufRead::lines).
    ///
    /// The stream returned from this function will yield instances of
    /// [`io::Result`]`<`[`String`]`>`. Each string returned will *not* have a newline
    /// byte (the 0xA byte) or CRLF (0xD, 0xA bytes) at the end.
    ///
    /// [`io::Result`]: std::io::Result
    /// [`String`]: String
    ///
    /// # Errors
    ///
    /// Each line of the stream has the same error semantics as [`AsyncBufReadExt::read_line`].
    ///
    /// [`AsyncBufReadExt::read_line`]: AsyncBufReadExt::read_line
    ///
    /// # Examples
    ///
    /// ```
    /// # futures::executor::block_on(async {
    /// use futures::io::{AsyncBufReadExt, Cursor};
    /// use futures::stream::StreamExt;
    ///
    /// let cursor = Cursor::new(b"lorem\nipsum\xc2\r\ndolor");
    ///
    /// let mut lines_stream = cursor.lines().map(|l| l.unwrap_or(String::from("invalid UTF_8")));
    /// assert_eq!(lines_stream.next().await, Some(String::from("lorem")));
    /// assert_eq!(lines_stream.next().await, Some(String::from("invalid UTF_8")));
    /// assert_eq!(lines_stream.next().await, Some(String::from("dolor")));
    /// assert_eq!(lines_stream.next().await, None);
    /// # Ok::<(), Box<dyn std::error::Error>>(()) }).unwrap();
    /// ```
    fn lines(self) -> Lines<Self>
    where
        Self: Sized,
    {
        assert_stream::<Result<String>, _>(Lines::new(self))
    }
}

impl<R: AsyncBufRead + ?Sized> AsyncBufReadExt for R {}

// Just a helper function to ensure the reader we're returning all have the
// right implementations.
pub(crate) fn assert_read<R>(reader: R) -> R
where
    R: AsyncRead,
{
    reader
}
// Just a helper function to ensure the writer we're returning all have the
// right implementations.
pub(crate) fn assert_write<W>(writer: W) -> W
where
    W: AsyncWrite,
{
    writer
}
