//! Adaptors from `AsyncRead`/`AsyncWrite` to Stream/Sink
//!
//! Raw I/O objects work with byte sequences, but higher-level code usually
//! wants to batch these into meaningful chunks, called "frames".
//!
//! This module contains adapters to go from streams of bytes, [`AsyncRead`] and
//! [`AsyncWrite`], to framed streams implementing [`Sink`] and [`Stream`].
//! Framed streams are also known as transports.
//!
//! # Example encoding using `LinesCodec`
//!
//! The following example demonstrates how to use a codec such as [`LinesCodec`] to
//! write framed data. [`FramedWrite`] can be used to achieve this. Data sent to
//! [`FramedWrite`] are first framed according to a specific codec, and then sent to
//! an implementor of [`AsyncWrite`].
//!
//! ```
//! use futures::sink::SinkExt;
//! use tokio_util::codec::LinesCodec;
//! use tokio_util::codec::FramedWrite;
//!
//! #[tokio::main]
//! async fn main() {
//!     let buffer = Vec::new();
//!     let messages = vec!["Hello", "World"];
//!     let encoder = LinesCodec::new();
//!
//!     // FramedWrite is a sink which means you can send values into it
//!     // asynchronously.
//!     let mut writer = FramedWrite::new(buffer, encoder);
//!
//!     // To be able to send values into a FramedWrite, you need to bring the
//!     // `SinkExt` trait into scope.
//!     writer.send(messages[0]).await.unwrap();
//!     writer.send(messages[1]).await.unwrap();
//!
//!     let buffer = writer.get_ref();
//!
//!     assert_eq!(buffer.as_slice(), "Hello\nWorld\n".as_bytes());
//! }
//!```
//!
//! # Example decoding using `LinesCodec`
//! The following example demonstrates how to use a codec such as [`LinesCodec`] to
//! read a stream of framed data. [`FramedRead`] can be used to achieve this. [`FramedRead`]
//! will keep reading from an [`AsyncRead`] implementor until a whole frame, according to a codec,
//! can be parsed.
//!
//!```
//! use tokio_stream::StreamExt;
//! use tokio_util::codec::LinesCodec;
//! use tokio_util::codec::FramedRead;
//!
//! #[tokio::main]
//! async fn main() {
//!     let message = "Hello\nWorld".as_bytes();
//!     let decoder = LinesCodec::new();
//!
//!     // FramedRead can be used to read a stream of values that are framed according to
//!     // a codec. FramedRead will read from its input (here `buffer`) until a whole frame
//!     // can be parsed.
//!     let mut reader = FramedRead::new(message, decoder);
//!
//!     // To read values from a FramedRead, you need to bring the
//!     // `StreamExt` trait into scope.
//!     let frame1 = reader.next().await.unwrap().unwrap();
//!     let frame2 = reader.next().await.unwrap().unwrap();
//!
//!     assert!(reader.next().await.is_none());
//!     assert_eq!(frame1, "Hello");
//!     assert_eq!(frame2, "World");
//! }
//! ```
//!
//! # The Decoder trait
//!
//! A [`Decoder`] is used together with [`FramedRead`] or [`Framed`] to turn an
//! [`AsyncRead`] into a [`Stream`]. The job of the decoder trait is to specify
//! how sequences of bytes are turned into a sequence of frames, and to
//! determine where the boundaries between frames are.  The job of the
//! `FramedRead` is to repeatedly switch between reading more data from the IO
//! resource, and asking the decoder whether we have received enough data to
//! decode another frame of data.
//!
//! The main method on the `Decoder` trait is the [`decode`] method. This method
//! takes as argument the data that has been read so far, and when it is called,
//! it will be in one of the following situations:
//!
//!  1. The buffer contains less than a full frame.
//!  2. The buffer contains exactly a full frame.
//!  3. The buffer contains more than a full frame.
//!
//! In the first situation, the decoder should return `Ok(None)`.
//!
//! In the second situation, the decoder should clear the provided buffer and
//! return `Ok(Some(the_decoded_frame))`.
//!
//! In the third situation, the decoder should use a method such as [`split_to`]
//! or [`advance`] to modify the buffer such that the frame is removed from the
//! buffer, but any data in the buffer after that frame should still remain in
//! the buffer. The decoder should also return `Ok(Some(the_decoded_frame))` in
//! this case.
//!
//! Finally the decoder may return an error if the data is invalid in some way.
//! The decoder should _not_ return an error just because it has yet to receive
//! a full frame.
//!
//! It is guaranteed that, from one call to `decode` to another, the provided
//! buffer will contain the exact same data as before, except that if more data
//! has arrived through the IO resource, that data will have been appended to
//! the buffer.  This means that reading frames from a `FramedRead` is
//! essentially equivalent to the following loop:
//!
//! ```no_run
//! use tokio::io::AsyncReadExt;
//! # // This uses async_stream to create an example that compiles.
//! # fn foo() -> impl futures_core::Stream<Item = std::io::Result<bytes::BytesMut>> { async_stream::try_stream! {
//! # use tokio_util::codec::Decoder;
//! # let mut decoder = tokio_util::codec::BytesCodec::new();
//! # let io_resource = &mut &[0u8, 1, 2, 3][..];
//!
//! let mut buf = bytes::BytesMut::new();
//! loop {
//!     // The read_buf call will append to buf rather than overwrite existing data.
//!     let len = io_resource.read_buf(&mut buf).await?;
//!
//!     if len == 0 {
//!         while let Some(frame) = decoder.decode_eof(&mut buf)? {
//!             yield frame;
//!         }
//!         break;
//!     }
//!
//!     while let Some(frame) = decoder.decode(&mut buf)? {
//!         yield frame;
//!     }
//! }
//! # }}
//! ```
//! The example above uses `yield` whenever the `Stream` produces an item.
//!
//! ## Example decoder
//!
//! As an example, consider a protocol that can be used to send strings where
//! each frame is a four byte integer that contains the length of the frame,
//! followed by that many bytes of string data. The decoder fails with an error
//! if the string data is not valid utf-8 or too long.
//!
//! Such a decoder can be written like this:
//! ```
//! use tokio_util::codec::Decoder;
//! use bytes::{BytesMut, Buf};
//!
//! struct MyStringDecoder {}
//!
//! const MAX: usize = 8 * 1024 * 1024;
//!
//! impl Decoder for MyStringDecoder {
//!     type Item = String;
//!     type Error = std::io::Error;
//!
//!     fn decode(
//!         &mut self,
//!         src: &mut BytesMut
//!     ) -> Result<Option<Self::Item>, Self::Error> {
//!         if src.len() < 4 {
//!             // Not enough data to read length marker.
//!             return Ok(None);
//!         }
//!
//!         // Read length marker.
//!         let mut length_bytes = [0u8; 4];
//!         length_bytes.copy_from_slice(&src[..4]);
//!         let length = u32::from_le_bytes(length_bytes) as usize;
//!
//!         // Check that the length is not too large to avoid a denial of
//!         // service attack where the server runs out of memory.
//!         if length > MAX {
//!             return Err(std::io::Error::new(
//!                 std::io::ErrorKind::InvalidData,
//!                 format!("Frame of length {} is too large.", length)
//!             ));
//!         }
//!
//!         if src.len() < 4 + length {
//!             // The full string has not yet arrived.
//!             //
//!             // We reserve more space in the buffer. This is not strictly
//!             // necessary, but is a good idea performance-wise.
//!             src.reserve(4 + length - src.len());
//!
//!             // We inform the Framed that we need more bytes to form the next
//!             // frame.
//!             return Ok(None);
//!         }
//!
//!         // Use advance to modify src such that it no longer contains
//!         // this frame.
//!         let data = src[4..4 + length].to_vec();
//!         src.advance(4 + length);
//!
//!         // Convert the data to a string, or fail if it is not valid utf-8.
//!         match String::from_utf8(data) {
//!             Ok(string) => Ok(Some(string)),
//!             Err(utf8_error) => {
//!                 Err(std::io::Error::new(
//!                     std::io::ErrorKind::InvalidData,
//!                     utf8_error.utf8_error(),
//!                 ))
//!             },
//!         }
//!     }
//! }
//! ```
//!
//! # The Encoder trait
//!
//! An [`Encoder`] is used together with [`FramedWrite`] or [`Framed`] to turn
//! an [`AsyncWrite`] into a [`Sink`]. The job of the encoder trait is to
//! specify how frames are turned into a sequences of bytes.  The job of the
//! `FramedWrite` is to take the resulting sequence of bytes and write it to the
//! IO resource.
//!
//! The main method on the `Encoder` trait is the [`encode`] method. This method
//! takes an item that is being written, and a buffer to write the item to. The
//! buffer may already contain data, and in this case, the encoder should append
//! the new frame to the buffer rather than overwrite the existing data.
//!
//! It is guaranteed that, from one call to `encode` to another, the provided
//! buffer will contain the exact same data as before, except that some of the
//! data may have been removed from the front of the buffer. Writing to a
//! `FramedWrite` is essentially equivalent to the following loop:
//!
//! ```no_run
//! use tokio::io::AsyncWriteExt;
//! use bytes::Buf; // for advance
//! # use tokio_util::codec::Encoder;
//! # async fn next_frame() -> bytes::Bytes { bytes::Bytes::new() }
//! # async fn no_more_frames() { }
//! # #[tokio::main] async fn main() -> std::io::Result<()> {
//! # let mut io_resource = tokio::io::sink();
//! # let mut encoder = tokio_util::codec::BytesCodec::new();
//!
//! const MAX: usize = 8192;
//!
//! let mut buf = bytes::BytesMut::new();
//! loop {
//!     tokio::select! {
//!         num_written = io_resource.write(&buf), if !buf.is_empty() => {
//!             buf.advance(num_written?);
//!         },
//!         frame = next_frame(), if buf.len() < MAX => {
//!             encoder.encode(frame, &mut buf)?;
//!         },
//!         _ = no_more_frames() => {
//!             io_resource.write_all(&buf).await?;
//!             io_resource.shutdown().await?;
//!             return Ok(());
//!         },
//!     }
//! }
//! # }
//! ```
//! Here the `next_frame` method corresponds to any frames you write to the
//! `FramedWrite`. The `no_more_frames` method corresponds to closing the
//! `FramedWrite` with [`SinkExt::close`].
//!
//! ## Example encoder
//!
//! As an example, consider a protocol that can be used to send strings where
//! each frame is a four byte integer that contains the length of the frame,
//! followed by that many bytes of string data. The encoder will fail if the
//! string is too long.
//!
//! Such an encoder can be written like this:
//! ```
//! use tokio_util::codec::Encoder;
//! use bytes::BytesMut;
//!
//! struct MyStringEncoder {}
//!
//! const MAX: usize = 8 * 1024 * 1024;
//!
//! impl Encoder<String> for MyStringEncoder {
//!     type Error = std::io::Error;
//!
//!     fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
//!         // Don't send a string if it is longer than the other end will
//!         // accept.
//!         if item.len() > MAX {
//!             return Err(std::io::Error::new(
//!                 std::io::ErrorKind::InvalidData,
//!                 format!("Frame of length {} is too large.", item.len())
//!             ));
//!         }
//!
//!         // Convert the length into a byte array.
//!         // The cast to u32 cannot overflow due to the length check above.
//!         let len_slice = u32::to_le_bytes(item.len() as u32);
//!
//!         // Reserve space in the buffer.
//!         dst.reserve(4 + item.len());
//!
//!         // Write the length and string to the buffer.
//!         dst.extend_from_slice(&len_slice);
//!         dst.extend_from_slice(item.as_bytes());
//!         Ok(())
//!     }
//! }
//! ```
//!
//! [`AsyncRead`]: tokio::io::AsyncRead
//! [`AsyncWrite`]: tokio::io::AsyncWrite
//! [`Stream`]: futures_core::Stream
//! [`Sink`]: futures_sink::Sink
//! [`SinkExt`]: futures::sink::SinkExt
//! [`SinkExt::close`]: https://docs.rs/futures/0.3/futures/sink/trait.SinkExt.html#method.close
//! [`FramedRead`]: struct@crate::codec::FramedRead
//! [`FramedWrite`]: struct@crate::codec::FramedWrite
//! [`Framed`]: struct@crate::codec::Framed
//! [`Decoder`]: trait@crate::codec::Decoder
//! [`decode`]: fn@crate::codec::Decoder::decode
//! [`encode`]: fn@crate::codec::Encoder::encode
//! [`split_to`]: fn@bytes::BytesMut::split_to
//! [`advance`]: fn@bytes::Buf::advance

mod bytes_codec;
pub use self::bytes_codec::BytesCodec;

mod decoder;
pub use self::decoder::Decoder;

mod encoder;
pub use self::encoder::Encoder;

mod framed_impl;
#[allow(unused_imports)]
pub(crate) use self::framed_impl::{FramedImpl, RWFrames, ReadFrame, WriteFrame};

mod framed;
pub use self::framed::{Framed, FramedParts};

mod framed_read;
pub use self::framed_read::FramedRead;

mod framed_write;
pub use self::framed_write::FramedWrite;

pub mod length_delimited;
pub use self::length_delimited::{LengthDelimitedCodec, LengthDelimitedCodecError};

mod lines_codec;
pub use self::lines_codec::{LinesCodec, LinesCodecError};

mod any_delimiter_codec;
pub use self::any_delimiter_codec::{AnyDelimiterCodec, AnyDelimiterCodecError};
