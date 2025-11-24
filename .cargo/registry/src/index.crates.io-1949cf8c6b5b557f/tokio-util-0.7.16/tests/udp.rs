#![warn(rust_2018_idioms)]
#![cfg(not(target_os = "wasi"))] // Wasi doesn't support UDP
#![cfg(not(miri))] // No `socket` in Miri.

use tokio::net::UdpSocket;
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, LinesCodec};
use tokio_util::udp::UdpFramed;

use bytes::{BufMut, BytesMut};
use futures::future::try_join;
use futures::future::FutureExt;
use futures::sink::SinkExt;
use std::io;
use std::sync::Arc;

#[cfg_attr(
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    ),
    allow(unused_assignments)
)]
#[tokio::test]
async fn send_framed_byte_codec() -> std::io::Result<()> {
    let mut a_soc = UdpSocket::bind("127.0.0.1:0").await?;
    let mut b_soc = UdpSocket::bind("127.0.0.1:0").await?;

    let a_addr = a_soc.local_addr()?;
    let b_addr = b_soc.local_addr()?;

    // test sending & receiving bytes
    {
        let mut a = UdpFramed::new(a_soc, ByteCodec);
        let mut b = UdpFramed::new(b_soc, ByteCodec);

        let msg = b"4567";

        let send = a.send((msg, b_addr));
        let recv = b.next().map(|e| e.unwrap());
        let (_, received) = try_join(send, recv).await.unwrap();

        let (data, addr) = received;
        assert_eq!(msg, &*data);
        assert_eq!(a_addr, addr);

        a_soc = a.into_inner();
        b_soc = b.into_inner();
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos",
        target_os = "visionos"
    )))]
    // test sending & receiving an empty message
    {
        let mut a = UdpFramed::new(a_soc, ByteCodec);
        let mut b = UdpFramed::new(b_soc, ByteCodec);

        let msg = b"";

        let send = a.send((msg, b_addr));
        let recv = b.next().map(|e| e.unwrap());
        let (_, received) = try_join(send, recv).await.unwrap();

        let (data, addr) = received;
        assert_eq!(msg, &*data);
        assert_eq!(a_addr, addr);
    }

    Ok(())
}

pub struct ByteCodec;

impl Decoder for ByteCodec {
    type Item = Vec<u8>;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Vec<u8>>, io::Error> {
        let len = buf.len();
        Ok(Some(buf.split_to(len).to_vec()))
    }
}

impl Encoder<&[u8]> for ByteCodec {
    type Error = io::Error;

    fn encode(&mut self, data: &[u8], buf: &mut BytesMut) -> Result<(), io::Error> {
        buf.reserve(data.len());
        buf.put_slice(data);
        Ok(())
    }
}

#[tokio::test]
async fn send_framed_lines_codec() -> std::io::Result<()> {
    let a_soc = UdpSocket::bind("127.0.0.1:0").await?;
    let b_soc = UdpSocket::bind("127.0.0.1:0").await?;

    let a_addr = a_soc.local_addr()?;
    let b_addr = b_soc.local_addr()?;

    let mut a = UdpFramed::new(a_soc, ByteCodec);
    let mut b = UdpFramed::new(b_soc, LinesCodec::new());

    let msg = b"1\r\n2\r\n3\r\n".to_vec();
    a.send((&msg, b_addr)).await?;

    assert_eq!(b.next().await.unwrap().unwrap(), ("1".to_string(), a_addr));
    assert_eq!(b.next().await.unwrap().unwrap(), ("2".to_string(), a_addr));
    assert_eq!(b.next().await.unwrap().unwrap(), ("3".to_string(), a_addr));

    Ok(())
}

#[tokio::test]
async fn framed_half() -> std::io::Result<()> {
    let a_soc = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let b_soc = a_soc.clone();

    let a_addr = a_soc.local_addr()?;
    let b_addr = b_soc.local_addr()?;

    let mut a = UdpFramed::new(a_soc, ByteCodec);
    let mut b = UdpFramed::new(b_soc, LinesCodec::new());

    let msg = b"1\r\n2\r\n3\r\n".to_vec();
    a.send((&msg, b_addr)).await?;

    let msg = b"4\r\n5\r\n6\r\n".to_vec();
    a.send((&msg, b_addr)).await?;

    assert_eq!(b.next().await.unwrap().unwrap(), ("1".to_string(), a_addr));
    assert_eq!(b.next().await.unwrap().unwrap(), ("2".to_string(), a_addr));
    assert_eq!(b.next().await.unwrap().unwrap(), ("3".to_string(), a_addr));

    assert_eq!(b.next().await.unwrap().unwrap(), ("4".to_string(), a_addr));
    assert_eq!(b.next().await.unwrap().unwrap(), ("5".to_string(), a_addr));
    assert_eq!(b.next().await.unwrap().unwrap(), ("6".to_string(), a_addr));

    Ok(())
}
