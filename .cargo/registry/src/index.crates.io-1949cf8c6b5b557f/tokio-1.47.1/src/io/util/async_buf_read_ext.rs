use crate::io::util::fill_buf::{fill_buf, FillBuf};
use crate::io::util::lines::{lines, Lines};
use crate::io::util::read_line::{read_line, ReadLine};
use crate::io::util::read_until::{read_until, ReadUntil};
use crate::io::util::split::{split, Split};
use crate::io::AsyncBufRead;

cfg_io_util! {
    /// An extension trait which adds utility methods to [`AsyncBufRead`] types.
    ///
    /// [`AsyncBufRead`]: crate::io::AsyncBufRead
    pub trait AsyncBufReadExt: AsyncBufRead {
        /// Reads all bytes into `buf` until the delimiter `byte` or EOF is reached.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize>;
        /// ```
        ///
        /// This function will read bytes from the underlying stream until the
        /// delimiter or EOF is found. Once found, all bytes up to, and including,
        /// the delimiter (if found) will be appended to `buf`.
        ///
        /// If successful, this function will return the total number of bytes read.
        ///
        /// If this function returns `Ok(0)`, the stream has reached EOF.
        ///
        /// # Errors
        ///
        /// This function will ignore all instances of [`ErrorKind::Interrupted`] and
        /// will otherwise return any errors returned by [`fill_buf`].
        ///
        /// If an I/O error is encountered then all bytes read so far will be
        /// present in `buf` and its length will have been adjusted appropriately.
        ///
        /// [`fill_buf`]: AsyncBufRead::poll_fill_buf
        /// [`ErrorKind::Interrupted`]: std::io::ErrorKind::Interrupted
        ///
        /// # Cancel safety
        ///
        /// If the method is used as the event in a
        /// [`tokio::select!`](crate::select) statement and some other branch
        /// completes first, then some data may have been partially read. Any
        /// partially read bytes are appended to `buf`, and the method can be
        /// called again to continue reading until `byte`.
        ///
        /// This method returns the total number of bytes read. If you cancel
        /// the call to `read_until` and then call it again to continue reading,
        /// the counter is reset.
        ///
        /// # Examples
        ///
        /// [`std::io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
        /// this example, we use [`Cursor`] to read all the bytes in a byte slice
        /// in hyphen delimited segments:
        ///
        /// [`Cursor`]: std::io::Cursor
        ///
        /// ```
        /// use tokio::io::AsyncBufReadExt;
        ///
        /// use std::io::Cursor;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let mut cursor = Cursor::new(b"lorem-ipsum");
        ///     let mut buf = vec![];
        ///
        ///     // cursor is at 'l'
        ///     let num_bytes = cursor.read_until(b'-', &mut buf)
        ///         .await
        ///         .expect("reading from cursor won't fail");
        ///
        ///     assert_eq!(num_bytes, 6);
        ///     assert_eq!(buf, b"lorem-");
        ///     buf.clear();
        ///
        ///     // cursor is at 'i'
        ///     let num_bytes = cursor.read_until(b'-', &mut buf)
        ///         .await
        ///         .expect("reading from cursor won't fail");
        ///
        ///     assert_eq!(num_bytes, 5);
        ///     assert_eq!(buf, b"ipsum");
        ///     buf.clear();
        ///
        ///     // cursor is at EOF
        ///     let num_bytes = cursor.read_until(b'-', &mut buf)
        ///         .await
        ///         .expect("reading from cursor won't fail");
        ///     assert_eq!(num_bytes, 0);
        ///     assert_eq!(buf, b"");
        /// }
        /// ```
        fn read_until<'a>(&'a mut self, byte: u8, buf: &'a mut Vec<u8>) -> ReadUntil<'a, Self>
        where
            Self: Unpin,
        {
            read_until(self, byte, buf)
        }

        /// Reads all bytes until a newline (the 0xA byte) is reached, and append
        /// them to the provided buffer.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn read_line(&mut self, buf: &mut String) -> io::Result<usize>;
        /// ```
        ///
        /// This function will read bytes from the underlying stream until the
        /// newline delimiter (the 0xA byte) or EOF is found. Once found, all bytes
        /// up to, and including, the delimiter (if found) will be appended to
        /// `buf`.
        ///
        /// If successful, this function will return the total number of bytes read.
        ///
        /// If this function returns `Ok(0)`, the stream has reached EOF.
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
        /// # Cancel safety
        ///
        /// This method is not cancellation safe. If the method is used as the
        /// event in a [`tokio::select!`](crate::select) statement and some
        /// other branch completes first, then some data may have been partially
        /// read, and this data is lost. There are no guarantees regarding the
        /// contents of `buf` when the call is cancelled. The current
        /// implementation replaces `buf` with the empty string, but this may
        /// change in the future.
        ///
        /// This function does not behave like [`read_until`] because of the
        /// requirement that a string contains only valid utf-8. If you need a
        /// cancellation safe `read_line`, there are three options:
        ///
        ///  * Call [`read_until`] with a newline character and manually perform the utf-8 check.
        ///  * The stream returned by [`lines`] has a cancellation safe
        ///    [`next_line`] method.
        ///  * Use [`tokio_util::codec::LinesCodec`][LinesCodec].
        ///
        /// [LinesCodec]: https://docs.rs/tokio-util/latest/tokio_util/codec/struct.LinesCodec.html
        /// [`read_until`]: Self::read_until
        /// [`lines`]: Self::lines
        /// [`next_line`]: crate::io::Lines::next_line
        ///
        /// # Examples
        ///
        /// [`std::io::Cursor`][`Cursor`] is a type that implements
        /// `AsyncBufRead`. In this example, we use [`Cursor`] to read all the
        /// lines in a byte slice:
        ///
        /// [`Cursor`]: std::io::Cursor
        ///
        /// ```
        /// use tokio::io::AsyncBufReadExt;
        ///
        /// use std::io::Cursor;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let mut cursor = Cursor::new(b"foo\nbar");
        ///     let mut buf = String::new();
        ///
        ///     // cursor is at 'f'
        ///     let num_bytes = cursor.read_line(&mut buf)
        ///         .await
        ///         .expect("reading from cursor won't fail");
        ///
        ///     assert_eq!(num_bytes, 4);
        ///     assert_eq!(buf, "foo\n");
        ///     buf.clear();
        ///
        ///     // cursor is at 'b'
        ///     let num_bytes = cursor.read_line(&mut buf)
        ///         .await
        ///         .expect("reading from cursor won't fail");
        ///
        ///     assert_eq!(num_bytes, 3);
        ///     assert_eq!(buf, "bar");
        ///     buf.clear();
        ///
        ///     // cursor is at EOF
        ///     let num_bytes = cursor.read_line(&mut buf)
        ///         .await
        ///         .expect("reading from cursor won't fail");
        ///
        ///     assert_eq!(num_bytes, 0);
        ///     assert_eq!(buf, "");
        /// }
        /// ```
        fn read_line<'a>(&'a mut self, buf: &'a mut String) -> ReadLine<'a, Self>
        where
            Self: Unpin,
        {
            read_line(self, buf)
        }

        /// Returns a stream of the contents of this reader split on the byte
        /// `byte`.
        ///
        /// This method is the asynchronous equivalent to
        /// [`BufRead::split`](std::io::BufRead::split).
        ///
        /// The stream returned from this function will yield instances of
        /// [`io::Result`]`<`[`Option`]`<`[`Vec<u8>`]`>>`. Each vector returned will *not* have
        /// the delimiter byte at the end.
        ///
        /// [`io::Result`]: std::io::Result
        /// [`Option`]: core::option::Option
        /// [`Vec<u8>`]: std::vec::Vec
        ///
        /// # Errors
        ///
        /// Each item of the stream has the same error semantics as
        /// [`AsyncBufReadExt::read_until`](AsyncBufReadExt::read_until).
        ///
        /// # Examples
        ///
        /// ```
        /// # use tokio::io::AsyncBufRead;
        /// use tokio::io::AsyncBufReadExt;
        ///
        /// # async fn dox(my_buf_read: impl AsyncBufRead + Unpin) -> std::io::Result<()> {
        /// let mut segments = my_buf_read.split(b'f');
        ///
        /// while let Some(segment) = segments.next_segment().await? {
        ///     println!("length = {}", segment.len())
        /// }
        /// # Ok(())
        /// # }
        /// ```
        fn split(self, byte: u8) -> Split<Self>
        where
            Self: Sized + Unpin,
        {
            split(self, byte)
        }

        /// Returns the contents of the internal buffer, filling it with more
        /// data from the inner reader if it is empty.
        ///
        /// This function is a lower-level call. It needs to be paired with the
        /// [`consume`] method to function properly. When calling this method,
        /// none of the contents will be "read" in the sense that later calling
        /// `read` may return the same contents. As such, [`consume`] must be
        /// called with the number of bytes that are consumed from this buffer
        /// to ensure that the bytes are never returned twice.
        ///
        /// An empty buffer returned indicates that the stream has reached EOF.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn fill_buf(&mut self) -> io::Result<&[u8]>;
        /// ```
        ///
        /// # Errors
        ///
        /// This function will return an I/O error if the underlying reader was
        /// read, but returned an error.
        ///
        /// # Cancel safety
        ///
        /// This method is cancel safe. If you use it as the event in a
        /// [`tokio::select!`](crate::select) statement and some other branch
        /// completes first, then it is guaranteed that no data was read.
        ///
        /// [`consume`]: crate::io::AsyncBufReadExt::consume
        fn fill_buf(&mut self) -> FillBuf<'_, Self>
        where
            Self: Unpin,
        {
            fill_buf(self)
        }

        /// Tells this buffer that `amt` bytes have been consumed from the
        /// buffer, so they should no longer be returned in calls to [`read`].
        ///
        /// This function is a lower-level call. It needs to be paired with the
        /// [`fill_buf`] method to function properly. This function does not
        /// perform any I/O, it simply informs this object that some amount of
        /// its buffer, returned from [`fill_buf`], has been consumed and should
        /// no longer be returned. As such, this function may do odd things if
        /// [`fill_buf`] isn't called before calling it.
        ///
        /// The `amt` must be less than the number of bytes in the buffer
        /// returned by [`fill_buf`].
        ///
        /// [`read`]: crate::io::AsyncReadExt::read
        /// [`fill_buf`]: crate::io::AsyncBufReadExt::fill_buf
        fn consume(&mut self, amt: usize)
        where
            Self: Unpin,
        {
            std::pin::Pin::new(self).consume(amt);
        }

        /// Returns a stream over the lines of this reader.
        /// This method is the async equivalent to [`BufRead::lines`](std::io::BufRead::lines).
        ///
        /// The stream returned from this function will yield instances of
        /// [`io::Result`]`<`[`Option`]`<`[`String`]`>>`. Each string returned will *not* have a newline
        /// byte (the 0xA byte) or `CRLF` (0xD, 0xA bytes) at the end.
        ///
        /// [`io::Result`]: std::io::Result
        /// [`Option`]: core::option::Option
        /// [`String`]: String
        ///
        /// # Errors
        ///
        /// Each line of the stream has the same error semantics as [`AsyncBufReadExt::read_line`].
        ///
        /// # Examples
        ///
        /// [`std::io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
        /// this example, we use [`Cursor`] to iterate over all the lines in a byte
        /// slice.
        ///
        /// [`Cursor`]: std::io::Cursor
        ///
        /// ```
        /// use tokio::io::AsyncBufReadExt;
        ///
        /// use std::io::Cursor;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let cursor = Cursor::new(b"lorem\nipsum\r\ndolor");
        ///
        ///     let mut lines = cursor.lines();
        ///
        ///     assert_eq!(lines.next_line().await.unwrap(), Some(String::from("lorem")));
        ///     assert_eq!(lines.next_line().await.unwrap(), Some(String::from("ipsum")));
        ///     assert_eq!(lines.next_line().await.unwrap(), Some(String::from("dolor")));
        ///     assert_eq!(lines.next_line().await.unwrap(), None);
        /// }
        /// ```
        ///
        /// [`AsyncBufReadExt::read_line`]: AsyncBufReadExt::read_line
        fn lines(self) -> Lines<Self>
        where
            Self: Sized,
        {
            lines(self)
        }
    }
}

impl<R: AsyncBufRead + ?Sized> AsyncBufReadExt for R {}
