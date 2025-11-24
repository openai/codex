#![warn(rust_2018_idioms)]

use tokio_stream::StreamExt;
use tokio_test::assert_ok;
use tokio_util::codec::{Decoder, Encoder, Framed, FramedParts};

use bytes::{Buf, BufMut, BytesMut};
use std::io::{self, Read};
use std::pin::Pin;
use std::task::{Context, Poll};

const INITIAL_CAPACITY: usize = 8 * 1024;

/// Encode and decode u32 values.
#[derive(Default)]
struct U32Codec {
    read_bytes: usize,
}

impl Decoder for U32Codec {
    type Item = u32;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<u32>> {
        if buf.len() < 4 {
            return Ok(None);
        }

        let n = buf.split_to(4).get_u32();
        self.read_bytes += 4;
        Ok(Some(n))
    }
}

impl Encoder<u32> for U32Codec {
    type Error = io::Error;

    fn encode(&mut self, item: u32, dst: &mut BytesMut) -> io::Result<()> {
        // Reserve space
        dst.reserve(4);
        dst.put_u32(item);
        Ok(())
    }
}

/// Encode and decode u64 values.
#[derive(Default)]
struct U64Codec {
    read_bytes: usize,
}

impl Decoder for U64Codec {
    type Item = u64;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<u64>> {
        if buf.len() < 8 {
            return Ok(None);
        }

        let n = buf.split_to(8).get_u64();
        self.read_bytes += 8;
        Ok(Some(n))
    }
}

impl Encoder<u64> for U64Codec {
    type Error = io::Error;

    fn encode(&mut self, item: u64, dst: &mut BytesMut) -> io::Result<()> {
        // Reserve space
        dst.reserve(8);
        dst.put_u64(item);
        Ok(())
    }
}

/// This value should never be used
struct DontReadIntoThis;

impl Read for DontReadIntoThis {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Read into something you weren't supposed to.",
        ))
    }
}

impl tokio::io::AsyncRead for DontReadIntoThis {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        unreachable!()
    }
}

#[tokio::test]
async fn can_read_from_existing_buf() {
    let mut parts = FramedParts::new(DontReadIntoThis, U32Codec::default());
    parts.read_buf = BytesMut::from(&[0, 0, 0, 42][..]);

    let mut framed = Framed::from_parts(parts);
    let num = assert_ok!(framed.next().await.unwrap());

    assert_eq!(num, 42);
    assert_eq!(framed.codec().read_bytes, 4);
}

#[tokio::test]
async fn can_read_from_existing_buf_after_codec_changed() {
    let mut parts = FramedParts::new(DontReadIntoThis, U32Codec::default());
    parts.read_buf = BytesMut::from(&[0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 84][..]);

    let mut framed = Framed::from_parts(parts);
    let num = assert_ok!(framed.next().await.unwrap());

    assert_eq!(num, 42);
    assert_eq!(framed.codec().read_bytes, 4);

    let mut framed = framed.map_codec(|codec| U64Codec {
        read_bytes: codec.read_bytes,
    });
    let num = assert_ok!(framed.next().await.unwrap());

    assert_eq!(num, 84);
    assert_eq!(framed.codec().read_bytes, 12);
}

#[test]
fn external_buf_grows_to_init() {
    let mut parts = FramedParts::new(DontReadIntoThis, U32Codec::default());
    parts.read_buf = BytesMut::from(&[0, 0, 0, 42][..]);

    let framed = Framed::from_parts(parts);
    let FramedParts { read_buf, .. } = framed.into_parts();

    assert_eq!(read_buf.capacity(), INITIAL_CAPACITY);
}

#[test]
fn external_buf_does_not_shrink() {
    let mut parts = FramedParts::new(DontReadIntoThis, U32Codec::default());
    parts.read_buf = BytesMut::from(&vec![0; INITIAL_CAPACITY * 2][..]);

    let framed = Framed::from_parts(parts);
    let FramedParts { read_buf, .. } = framed.into_parts();

    assert_eq!(read_buf.capacity(), INITIAL_CAPACITY * 2);
}
