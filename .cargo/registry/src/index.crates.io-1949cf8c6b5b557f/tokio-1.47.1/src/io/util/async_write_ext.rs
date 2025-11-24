use crate::io::util::flush::{flush, Flush};
use crate::io::util::shutdown::{shutdown, Shutdown};
use crate::io::util::write::{write, Write};
use crate::io::util::write_all::{write_all, WriteAll};
use crate::io::util::write_all_buf::{write_all_buf, WriteAllBuf};
use crate::io::util::write_buf::{write_buf, WriteBuf};
use crate::io::util::write_int::{WriteF32, WriteF32Le, WriteF64, WriteF64Le};
use crate::io::util::write_int::{
    WriteI128, WriteI128Le, WriteI16, WriteI16Le, WriteI32, WriteI32Le, WriteI64, WriteI64Le,
    WriteI8,
};
use crate::io::util::write_int::{
    WriteU128, WriteU128Le, WriteU16, WriteU16Le, WriteU32, WriteU32Le, WriteU64, WriteU64Le,
    WriteU8,
};
use crate::io::util::write_vectored::{write_vectored, WriteVectored};
use crate::io::AsyncWrite;
use std::io::IoSlice;

use bytes::Buf;

cfg_io_util! {
    /// Defines numeric writer.
    macro_rules! write_impl {
        (
            $(
                $(#[$outer:meta])*
                fn $name:ident(&mut self, n: $ty:ty) -> $($fut:ident)*;
            )*
        ) => {
            $(
                $(#[$outer])*
                fn $name(&mut self, n: $ty) -> $($fut)*<&mut Self> where Self: Unpin {
                    $($fut)*::new(self, n)
                }
            )*
        }
    }

    /// Writes bytes to a sink.
    ///
    /// Implemented as an extension trait, adding utility methods to all
    /// [`AsyncWrite`] types. Callers will tend to import this trait instead of
    /// [`AsyncWrite`].
    ///
    /// ```no_run
    /// use tokio::io::{self, AsyncWriteExt};
    /// use tokio::fs::File;
    ///
    /// #[tokio::main]
    /// async fn main() -> io::Result<()> {
    ///     let data = b"some bytes";
    ///
    ///     let mut pos = 0;
    ///     let mut buffer = File::create("foo.txt").await?;
    ///
    ///     while pos < data.len() {
    ///         let bytes_written = buffer.write(&data[pos..]).await?;
    ///         pos += bytes_written;
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// See [module][crate::io] documentation for more details.
    ///
    /// [`AsyncWrite`]: AsyncWrite
    pub trait AsyncWriteExt: AsyncWrite {
        /// Writes a buffer into this writer, returning how many bytes were
        /// written.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
        /// ```
        ///
        /// This function will attempt to write the entire contents of `buf`, but
        /// the entire write may not succeed, or the write may also generate an
        /// error. A call to `write` represents *at most one* attempt to write to
        /// any wrapped object.
        ///
        /// # Return
        ///
        /// If the return value is `Ok(n)` then it must be guaranteed that `n <=
        /// buf.len()`. A return value of `0` typically means that the
        /// underlying object is no longer able to accept bytes and will likely
        /// not be able to in the future as well, or that the buffer provided is
        /// empty.
        ///
        /// # Errors
        ///
        /// Each call to `write` may generate an I/O error indicating that the
        /// operation could not be completed. If an error is returned then no bytes
        /// in the buffer were written to this writer.
        ///
        /// It is **not** considered an error if the entire buffer could not be
        /// written to this writer.
        ///
        /// # Cancel safety
        ///
        /// This method is cancellation safe in the sense that if it is used as
        /// the event in a [`tokio::select!`](crate::select) statement and some
        /// other branch completes first, then it is guaranteed that no data was
        /// written to this `AsyncWrite`.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::io::{self, AsyncWriteExt};
        /// use tokio::fs::File;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     let mut file = File::create("foo.txt").await?;
        ///
        ///     // Writes some prefix of the byte string, not necessarily all of it.
        ///     file.write(b"some bytes").await?;
        ///     file.flush().await?;
        ///     Ok(())
        /// }
        /// ```
        fn write<'a>(&'a mut self, src: &'a [u8]) -> Write<'a, Self>
        where
            Self: Unpin,
        {
            write(self, src)
        }

        /// Like [`write`], except that it writes from a slice of buffers.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize>;
        /// ```
        ///
        /// See [`AsyncWrite::poll_write_vectored`] for more details.
        ///
        /// # Cancel safety
        ///
        /// This method is cancellation safe in the sense that if it is used as
        /// the event in a [`tokio::select!`](crate::select) statement and some
        /// other branch completes first, then it is guaranteed that no data was
        /// written to this `AsyncWrite`.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::io::{self, AsyncWriteExt};
        /// use tokio::fs::File;
        /// use std::io::IoSlice;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     let mut file = File::create("foo.txt").await?;
        ///
        ///     let bufs: &[_] = &[
        ///         IoSlice::new(b"hello"),
        ///         IoSlice::new(b" "),
        ///         IoSlice::new(b"world"),
        ///     ];
        ///
        ///     file.write_vectored(&bufs).await?;
        ///     file.flush().await?;
        ///
        ///     Ok(())
        /// }
        /// ```
        ///
        /// [`write`]: AsyncWriteExt::write
        fn write_vectored<'a, 'b>(&'a mut self, bufs: &'a [IoSlice<'b>]) -> WriteVectored<'a, 'b, Self>
        where
            Self: Unpin,
        {
            write_vectored(self, bufs)
        }

        /// Writes a buffer into this writer, advancing the buffer's internal
        /// cursor.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn write_buf<B: Buf>(&mut self, buf: &mut B) -> io::Result<usize>;
        /// ```
        ///
        /// This function will attempt to write the entire contents of `buf`, but
        /// the entire write may not succeed, or the write may also generate an
        /// error. After the operation completes, the buffer's
        /// internal cursor is advanced by the number of bytes written. A
        /// subsequent call to `write_buf` using the **same** `buf` value will
        /// resume from the point that the first call to `write_buf` completed.
        /// A call to `write_buf` represents *at most one* attempt to write to any
        /// wrapped object.
        ///
        /// # Return
        ///
        /// If the return value is `Ok(n)` then it must be guaranteed that `n <=
        /// buf.len()`. A return value of `0` typically means that the
        /// underlying object is no longer able to accept bytes and will likely
        /// not be able to in the future as well, or that the buffer provided is
        /// empty.
        ///
        /// # Errors
        ///
        /// Each call to `write` may generate an I/O error indicating that the
        /// operation could not be completed. If an error is returned then no bytes
        /// in the buffer were written to this writer.
        ///
        /// It is **not** considered an error if the entire buffer could not be
        /// written to this writer.
        ///
        /// # Cancel safety
        ///
        /// This method is cancellation safe in the sense that if it is used as
        /// the event in a [`tokio::select!`](crate::select) statement and some
        /// other branch completes first, then it is guaranteed that no data was
        /// written to this `AsyncWrite`.
        ///
        /// # Examples
        ///
        /// [`File`] implements [`AsyncWrite`] and [`Cursor`]`<&[u8]>` implements [`Buf`]:
        ///
        /// [`File`]: crate::fs::File
        /// [`Buf`]: bytes::Buf
        /// [`Cursor`]: std::io::Cursor
        ///
        /// ```no_run
        /// use tokio::io::{self, AsyncWriteExt};
        /// use tokio::fs::File;
        ///
        /// use bytes::Buf;
        /// use std::io::Cursor;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     let mut file = File::create("foo.txt").await?;
        ///     let mut buffer = Cursor::new(b"data to write");
        ///
        ///     // Loop until the entire contents of the buffer are written to
        ///     // the file.
        ///     while buffer.has_remaining() {
        ///         // Writes some prefix of the byte string, not necessarily
        ///         // all of it.
        ///         file.write_buf(&mut buffer).await?;
        ///     }
        ///     file.flush().await?;
        ///
        ///     Ok(())
        /// }
        /// ```
        fn write_buf<'a, B>(&'a mut self, src: &'a mut B) -> WriteBuf<'a, Self, B>
        where
            Self: Sized + Unpin,
            B: Buf,
        {
            write_buf(self, src)
        }

        /// Attempts to write an entire buffer into this writer.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn write_all_buf(&mut self, buf: impl Buf) -> Result<(), io::Error> {
        ///     while buf.has_remaining() {
        ///         self.write_buf(&mut buf).await?;
        ///     }
        ///     Ok(())
        /// }
        /// ```
        ///
        /// This method will continuously call [`write`] until
        /// [`buf.has_remaining()`](bytes::Buf::has_remaining) returns false. This method will not
        /// return until the entire buffer has been successfully written or an error occurs. The
        /// first error generated will be returned.
        ///
        /// The buffer is advanced after each chunk is successfully written. After failure,
        /// `src.chunk()` will return the chunk that failed to write.
        ///
        /// # Cancel safety
        ///
        /// If `write_all_buf` is used as the event in a
        /// [`tokio::select!`](crate::select) statement and some other branch
        /// completes first, then the data in the provided buffer may have been
        /// partially written. However, it is guaranteed that the provided
        /// buffer has been [advanced] by the amount of bytes that have been
        /// partially written.
        ///
        /// # Examples
        ///
        /// [`File`] implements [`AsyncWrite`] and [`Cursor`]`<&[u8]>` implements [`Buf`]:
        ///
        /// [`File`]: crate::fs::File
        /// [`Buf`]: bytes::Buf
        /// [`Cursor`]: std::io::Cursor
        /// [advanced]: bytes::Buf::advance
        ///
        /// ```no_run
        /// use tokio::io::{self, AsyncWriteExt};
        /// use tokio::fs::File;
        ///
        /// use std::io::Cursor;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     let mut file = File::create("foo.txt").await?;
        ///     let mut buffer = Cursor::new(b"data to write");
        ///
        ///     file.write_all_buf(&mut buffer).await?;
        ///     file.flush().await?;
        ///     Ok(())
        /// }
        /// ```
        ///
        /// [`write`]: AsyncWriteExt::write
        fn write_all_buf<'a, B>(&'a mut self, src: &'a mut B) -> WriteAllBuf<'a, Self, B>
        where
            Self: Sized + Unpin,
            B: Buf,
        {
            write_all_buf(self, src)
        }

        /// Attempts to write an entire buffer into this writer.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;
        /// ```
        ///
        /// This method will continuously call [`write`] until there is no more data
        /// to be written. This method will not return until the entire buffer
        /// has been successfully written or such an error occurs. The first
        /// error generated from this method will be returned.
        ///
        /// # Cancel safety
        ///
        /// This method is not cancellation safe. If it is used as the event
        /// in a [`tokio::select!`](crate::select) statement and some other
        /// branch completes first, then the provided buffer may have been
        /// partially written, but future calls to `write_all` will start over
        /// from the beginning of the buffer.
        ///
        /// # Errors
        ///
        /// This function will return the first error that [`write`] returns.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::io::{self, AsyncWriteExt};
        /// use tokio::fs::File;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     let mut file = File::create("foo.txt").await?;
        ///
        ///     file.write_all(b"some bytes").await?;
        ///     file.flush().await?;
        ///     Ok(())
        /// }
        /// ```
        ///
        /// [`write`]: AsyncWriteExt::write
        fn write_all<'a>(&'a mut self, src: &'a [u8]) -> WriteAll<'a, Self>
        where
            Self: Unpin,
        {
            write_all(self, src)
        }

        write_impl! {
            /// Writes an unsigned 8-bit integer to the underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_u8(&mut self, n: u8) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write unsigned 8 bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_u8(2).await?;
            ///     writer.write_u8(5).await?;
            ///
            ///     assert_eq!(writer, b"\x02\x05");
            ///     Ok(())
            /// }
            /// ```
            fn write_u8(&mut self, n: u8) -> WriteU8;

            /// Writes a signed 8-bit integer to the underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_i8(&mut self, n: i8) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write signed 8 bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_i8(-2).await?;
            ///     writer.write_i8(126).await?;
            ///
            ///     assert_eq!(writer, b"\xFE\x7E");
            ///     Ok(())
            /// }
            /// ```
            fn write_i8(&mut self, n: i8) -> WriteI8;

            /// Writes an unsigned 16-bit integer in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_u16(&mut self, n: u16) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write unsigned 16-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_u16(517).await?;
            ///     writer.write_u16(768).await?;
            ///
            ///     assert_eq!(writer, b"\x02\x05\x03\x00");
            ///     Ok(())
            /// }
            /// ```
            fn write_u16(&mut self, n: u16) -> WriteU16;

            /// Writes a signed 16-bit integer in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_i16(&mut self, n: i16) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write signed 16-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_i16(193).await?;
            ///     writer.write_i16(-132).await?;
            ///
            ///     assert_eq!(writer, b"\x00\xc1\xff\x7c");
            ///     Ok(())
            /// }
            /// ```
            fn write_i16(&mut self, n: i16) -> WriteI16;

            /// Writes an unsigned 32-bit integer in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_u32(&mut self, n: u32) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write unsigned 32-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_u32(267).await?;
            ///     writer.write_u32(1205419366).await?;
            ///
            ///     assert_eq!(writer, b"\x00\x00\x01\x0b\x47\xd9\x3d\x66");
            ///     Ok(())
            /// }
            /// ```
            fn write_u32(&mut self, n: u32) -> WriteU32;

            /// Writes a signed 32-bit integer in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_i32(&mut self, n: i32) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write signed 32-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_i32(267).await?;
            ///     writer.write_i32(1205419366).await?;
            ///
            ///     assert_eq!(writer, b"\x00\x00\x01\x0b\x47\xd9\x3d\x66");
            ///     Ok(())
            /// }
            /// ```
            fn write_i32(&mut self, n: i32) -> WriteI32;

            /// Writes an unsigned 64-bit integer in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_u64(&mut self, n: u64) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write unsigned 64-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_u64(918733457491587).await?;
            ///     writer.write_u64(143).await?;
            ///
            ///     assert_eq!(writer, b"\x00\x03\x43\x95\x4d\x60\x86\x83\x00\x00\x00\x00\x00\x00\x00\x8f");
            ///     Ok(())
            /// }
            /// ```
            fn write_u64(&mut self, n: u64) -> WriteU64;

            /// Writes an signed 64-bit integer in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_i64(&mut self, n: i64) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write signed 64-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_i64(i64::MIN).await?;
            ///     writer.write_i64(i64::MAX).await?;
            ///
            ///     assert_eq!(writer, b"\x80\x00\x00\x00\x00\x00\x00\x00\x7f\xff\xff\xff\xff\xff\xff\xff");
            ///     Ok(())
            /// }
            /// ```
            fn write_i64(&mut self, n: i64) -> WriteI64;

            /// Writes an unsigned 128-bit integer in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_u128(&mut self, n: u128) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write unsigned 128-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_u128(16947640962301618749969007319746179).await?;
            ///
            ///     assert_eq!(writer, vec![
            ///         0x00, 0x03, 0x43, 0x95, 0x4d, 0x60, 0x86, 0x83,
            ///         0x00, 0x03, 0x43, 0x95, 0x4d, 0x60, 0x86, 0x83
            ///     ]);
            ///     Ok(())
            /// }
            /// ```
            fn write_u128(&mut self, n: u128) -> WriteU128;

            /// Writes an signed 128-bit integer in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_i128(&mut self, n: i128) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write signed 128-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_i128(i128::MIN).await?;
            ///
            ///     assert_eq!(writer, vec![
            ///         0x80, 0, 0, 0, 0, 0, 0, 0,
            ///         0, 0, 0, 0, 0, 0, 0, 0
            ///     ]);
            ///     Ok(())
            /// }
            /// ```
            fn write_i128(&mut self, n: i128) -> WriteI128;

            /// Writes an 32-bit floating point type in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_f32(&mut self, n: f32) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write 32-bit floating point type to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_f32(f32::MIN).await?;
            ///
            ///     assert_eq!(writer, vec![0xff, 0x7f, 0xff, 0xff]);
            ///     Ok(())
            /// }
            /// ```
            fn write_f32(&mut self, n: f32) -> WriteF32;

            /// Writes an 64-bit floating point type in big-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_f64(&mut self, n: f64) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write 64-bit floating point type to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_f64(f64::MIN).await?;
            ///
            ///     assert_eq!(writer, vec![
            ///         0xff, 0xef, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
            ///     ]);
            ///     Ok(())
            /// }
            /// ```
            fn write_f64(&mut self, n: f64) -> WriteF64;

            /// Writes an unsigned 16-bit integer in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_u16_le(&mut self, n: u16) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write unsigned 16-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_u16_le(517).await?;
            ///     writer.write_u16_le(768).await?;
            ///
            ///     assert_eq!(writer, b"\x05\x02\x00\x03");
            ///     Ok(())
            /// }
            /// ```
            fn write_u16_le(&mut self, n: u16) -> WriteU16Le;

            /// Writes a signed 16-bit integer in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_i16_le(&mut self, n: i16) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write signed 16-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_i16_le(193).await?;
            ///     writer.write_i16_le(-132).await?;
            ///
            ///     assert_eq!(writer, b"\xc1\x00\x7c\xff");
            ///     Ok(())
            /// }
            /// ```
            fn write_i16_le(&mut self, n: i16) -> WriteI16Le;

            /// Writes an unsigned 32-bit integer in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_u32_le(&mut self, n: u32) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write unsigned 32-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_u32_le(267).await?;
            ///     writer.write_u32_le(1205419366).await?;
            ///
            ///     assert_eq!(writer, b"\x0b\x01\x00\x00\x66\x3d\xd9\x47");
            ///     Ok(())
            /// }
            /// ```
            fn write_u32_le(&mut self, n: u32) -> WriteU32Le;

            /// Writes a signed 32-bit integer in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_i32_le(&mut self, n: i32) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write signed 32-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_i32_le(267).await?;
            ///     writer.write_i32_le(1205419366).await?;
            ///
            ///     assert_eq!(writer, b"\x0b\x01\x00\x00\x66\x3d\xd9\x47");
            ///     Ok(())
            /// }
            /// ```
            fn write_i32_le(&mut self, n: i32) -> WriteI32Le;

            /// Writes an unsigned 64-bit integer in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_u64_le(&mut self, n: u64) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write unsigned 64-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_u64_le(918733457491587).await?;
            ///     writer.write_u64_le(143).await?;
            ///
            ///     assert_eq!(writer, b"\x83\x86\x60\x4d\x95\x43\x03\x00\x8f\x00\x00\x00\x00\x00\x00\x00");
            ///     Ok(())
            /// }
            /// ```
            fn write_u64_le(&mut self, n: u64) -> WriteU64Le;

            /// Writes an signed 64-bit integer in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_i64_le(&mut self, n: i64) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write signed 64-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_i64_le(i64::MIN).await?;
            ///     writer.write_i64_le(i64::MAX).await?;
            ///
            ///     assert_eq!(writer, b"\x00\x00\x00\x00\x00\x00\x00\x80\xff\xff\xff\xff\xff\xff\xff\x7f");
            ///     Ok(())
            /// }
            /// ```
            fn write_i64_le(&mut self, n: i64) -> WriteI64Le;

            /// Writes an unsigned 128-bit integer in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_u128_le(&mut self, n: u128) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write unsigned 128-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_u128_le(16947640962301618749969007319746179).await?;
            ///
            ///     assert_eq!(writer, vec![
            ///         0x83, 0x86, 0x60, 0x4d, 0x95, 0x43, 0x03, 0x00,
            ///         0x83, 0x86, 0x60, 0x4d, 0x95, 0x43, 0x03, 0x00,
            ///     ]);
            ///     Ok(())
            /// }
            /// ```
            fn write_u128_le(&mut self, n: u128) -> WriteU128Le;

            /// Writes an signed 128-bit integer in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_i128_le(&mut self, n: i128) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write signed 128-bit integers to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_i128_le(i128::MIN).await?;
            ///
            ///     assert_eq!(writer, vec![
            ///          0, 0, 0, 0, 0, 0, 0,
            ///         0, 0, 0, 0, 0, 0, 0, 0, 0x80
            ///     ]);
            ///     Ok(())
            /// }
            /// ```
            fn write_i128_le(&mut self, n: i128) -> WriteI128Le;

            /// Writes an 32-bit floating point type in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_f32_le(&mut self, n: f32) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write 32-bit floating point type to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_f32_le(f32::MIN).await?;
            ///
            ///     assert_eq!(writer, vec![0xff, 0xff, 0x7f, 0xff]);
            ///     Ok(())
            /// }
            /// ```
            fn write_f32_le(&mut self, n: f32) -> WriteF32Le;

            /// Writes an 64-bit floating point type in little-endian order to the
            /// underlying writer.
            ///
            /// Equivalent to:
            ///
            /// ```ignore
            /// async fn write_f64_le(&mut self, n: f64) -> io::Result<()>;
            /// ```
            ///
            /// It is recommended to use a buffered writer to avoid excessive
            /// syscalls.
            ///
            /// # Errors
            ///
            /// This method returns the same errors as [`AsyncWriteExt::write_all`].
            ///
            /// [`AsyncWriteExt::write_all`]: AsyncWriteExt::write_all
            ///
            /// # Examples
            ///
            /// Write 64-bit floating point type to a `AsyncWrite`:
            ///
            /// ```rust
            /// use tokio::io::{self, AsyncWriteExt};
            ///
            /// #[tokio::main]
            /// async fn main() -> io::Result<()> {
            ///     let mut writer = Vec::new();
            ///
            ///     writer.write_f64_le(f64::MIN).await?;
            ///
            ///     assert_eq!(writer, vec![
            ///         0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xef, 0xff
            ///     ]);
            ///     Ok(())
            /// }
            /// ```
            fn write_f64_le(&mut self, n: f64) -> WriteF64Le;
        }

        /// Flushes this output stream, ensuring that all intermediately buffered
        /// contents reach their destination.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn flush(&mut self) -> io::Result<()>;
        /// ```
        ///
        /// # Errors
        ///
        /// It is considered an error if not all bytes could be written due to
        /// I/O errors or EOF being reached.
        ///
        /// # Cancel safety
        ///
        /// This method is cancel safe.
        ///
        /// If `flush` is used as the event in a [`tokio::select!`](crate::select)
        /// statement and some other branch completes first, then the data in the
        /// buffered data in this `AsyncWrite` may have been partially flushed.
        /// However, it is guaranteed that the buffer is advanced by the amount of
        /// bytes that have been partially flushed.
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::io::{self, BufWriter, AsyncWriteExt};
        /// use tokio::fs::File;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     let f = File::create("foo.txt").await?;
        ///     let mut buffer = BufWriter::new(f);
        ///
        ///     buffer.write_all(b"some bytes").await?;
        ///     buffer.flush().await?;
        ///     Ok(())
        /// }
        /// ```
        fn flush(&mut self) -> Flush<'_, Self>
        where
            Self: Unpin,
        {
            flush(self)
        }

        /// Shuts down the output stream, ensuring that the value can be dropped
        /// cleanly.
        ///
        /// Equivalent to:
        ///
        /// ```ignore
        /// async fn shutdown(&mut self) -> io::Result<()>;
        /// ```
        ///
        /// Similar to [`flush`], all intermediately buffered is written to the
        /// underlying stream. Once the operation completes, the caller should
        /// no longer attempt to write to the stream. For example, the
        /// `TcpStream` implementation will issue a `shutdown(Write)` sys call.
        ///
        /// [`flush`]: fn@crate::io::AsyncWriteExt::flush
        ///
        /// # Examples
        ///
        /// ```no_run
        /// use tokio::io::{self, BufWriter, AsyncWriteExt};
        /// use tokio::fs::File;
        ///
        /// #[tokio::main]
        /// async fn main() -> io::Result<()> {
        ///     let f = File::create("foo.txt").await?;
        ///     let mut buffer = BufWriter::new(f);
        ///
        ///     buffer.write_all(b"some bytes").await?;
        ///     buffer.shutdown().await?;
        ///     Ok(())
        /// }
        /// ```
        fn shutdown(&mut self) -> Shutdown<'_, Self>
        where
            Self: Unpin,
        {
            shutdown(self)
        }
    }
}

impl<W: AsyncWrite + ?Sized> AsyncWriteExt for W {}
