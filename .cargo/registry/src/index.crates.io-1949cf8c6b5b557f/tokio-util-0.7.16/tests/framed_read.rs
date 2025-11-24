#![warn(rust_2018_idioms)]

use tokio::io::{AsyncRead, ReadBuf};
use tokio_test::assert_ready;
use tokio_test::task;
use tokio_util::codec::{Decoder, FramedRead};

use bytes::{Buf, BytesMut};
use futures::Stream;
use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::task::Poll::{Pending, Ready};
use std::task::{Context, Poll};

macro_rules! mock {
    ($($x:expr,)*) => {{
        let mut v = VecDeque::new();
        v.extend(vec![$($x),*]);
        Mock { calls: v }
    }};
}

macro_rules! assert_read {
    ($e:expr, $n:expr) => {{
        let val = assert_ready!($e);
        assert_eq!(val.unwrap().unwrap(), $n);
    }};
}

macro_rules! pin {
    ($id:ident) => {
        Pin::new(&mut $id)
    };
}

struct U32Decoder;

impl Decoder for U32Decoder {
    type Item = u32;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<u32>> {
        if buf.len() < 4 {
            return Ok(None);
        }

        let n = buf.split_to(4).get_u32();
        Ok(Some(n))
    }
}

struct U64Decoder;

impl Decoder for U64Decoder {
    type Item = u64;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<u64>> {
        if buf.len() < 8 {
            return Ok(None);
        }

        let n = buf.split_to(8).get_u64();
        Ok(Some(n))
    }
}

#[test]
fn read_multi_frame_in_packet() {
    let mut task = task::spawn(());
    let mock = mock! {
        Ok(b"\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x02".to_vec()),
    };
    let mut framed = FramedRead::new(mock, U32Decoder);

    task.enter(|cx, _| {
        assert_read!(pin!(framed).poll_next(cx), 0);
        assert_read!(pin!(framed).poll_next(cx), 1);
        assert_read!(pin!(framed).poll_next(cx), 2);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
    });
}

#[test]
fn read_multi_frame_across_packets() {
    let mut task = task::spawn(());
    let mock = mock! {
        Ok(b"\x00\x00\x00\x00".to_vec()),
        Ok(b"\x00\x00\x00\x01".to_vec()),
        Ok(b"\x00\x00\x00\x02".to_vec()),
    };
    let mut framed = FramedRead::new(mock, U32Decoder);

    task.enter(|cx, _| {
        assert_read!(pin!(framed).poll_next(cx), 0);
        assert_read!(pin!(framed).poll_next(cx), 1);
        assert_read!(pin!(framed).poll_next(cx), 2);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
    });
}

#[test]
fn read_multi_frame_in_packet_after_codec_changed() {
    let mut task = task::spawn(());
    let mock = mock! {
        Ok(b"\x00\x00\x00\x04\x00\x00\x00\x00\x00\x00\x00\x08".to_vec()),
    };
    let mut framed = FramedRead::new(mock, U32Decoder);

    task.enter(|cx, _| {
        assert_read!(pin!(framed).poll_next(cx), 0x04);

        let mut framed = framed.map_decoder(|_| U64Decoder);
        assert_read!(pin!(framed).poll_next(cx), 0x08);

        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
    });
}

#[test]
fn read_not_ready() {
    let mut task = task::spawn(());
    let mock = mock! {
        Err(io::Error::new(io::ErrorKind::WouldBlock, "")),
        Ok(b"\x00\x00\x00\x00".to_vec()),
        Ok(b"\x00\x00\x00\x01".to_vec()),
    };
    let mut framed = FramedRead::new(mock, U32Decoder);

    task.enter(|cx, _| {
        assert!(pin!(framed).poll_next(cx).is_pending());
        assert_read!(pin!(framed).poll_next(cx), 0);
        assert_read!(pin!(framed).poll_next(cx), 1);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
    });
}

#[test]
fn read_partial_then_not_ready() {
    let mut task = task::spawn(());
    let mock = mock! {
        Ok(b"\x00\x00".to_vec()),
        Err(io::Error::new(io::ErrorKind::WouldBlock, "")),
        Ok(b"\x00\x00\x00\x00\x00\x01\x00\x00\x00\x02".to_vec()),
    };
    let mut framed = FramedRead::new(mock, U32Decoder);

    task.enter(|cx, _| {
        assert!(pin!(framed).poll_next(cx).is_pending());
        assert_read!(pin!(framed).poll_next(cx), 0);
        assert_read!(pin!(framed).poll_next(cx), 1);
        assert_read!(pin!(framed).poll_next(cx), 2);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
    });
}

#[test]
fn read_err() {
    let mut task = task::spawn(());
    let mock = mock! {
        Err(io::Error::new(io::ErrorKind::Other, "")),
    };
    let mut framed = FramedRead::new(mock, U32Decoder);

    task.enter(|cx, _| {
        assert_eq!(
            io::ErrorKind::Other,
            assert_ready!(pin!(framed).poll_next(cx))
                .unwrap()
                .unwrap_err()
                .kind()
        )
    });
}

#[test]
fn read_partial_then_err() {
    let mut task = task::spawn(());
    let mock = mock! {
        Ok(b"\x00\x00".to_vec()),
        Err(io::Error::new(io::ErrorKind::Other, "")),
    };
    let mut framed = FramedRead::new(mock, U32Decoder);

    task.enter(|cx, _| {
        assert_eq!(
            io::ErrorKind::Other,
            assert_ready!(pin!(framed).poll_next(cx))
                .unwrap()
                .unwrap_err()
                .kind()
        )
    });
}

#[test]
fn read_partial_would_block_then_err() {
    let mut task = task::spawn(());
    let mock = mock! {
        Ok(b"\x00\x00".to_vec()),
        Err(io::Error::new(io::ErrorKind::WouldBlock, "")),
        Err(io::Error::new(io::ErrorKind::Other, "")),
    };
    let mut framed = FramedRead::new(mock, U32Decoder);

    task.enter(|cx, _| {
        assert!(pin!(framed).poll_next(cx).is_pending());
        assert_eq!(
            io::ErrorKind::Other,
            assert_ready!(pin!(framed).poll_next(cx))
                .unwrap()
                .unwrap_err()
                .kind()
        )
    });
}

#[test]
fn huge_size() {
    let mut task = task::spawn(());
    let data = &[0; 32 * 1024][..];
    let mut framed = FramedRead::new(data, BigDecoder);

    task.enter(|cx, _| {
        assert_read!(pin!(framed).poll_next(cx), 0);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
    });

    struct BigDecoder;

    impl Decoder for BigDecoder {
        type Item = u32;
        type Error = io::Error;

        fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<u32>> {
            if buf.len() < 32 * 1024 {
                return Ok(None);
            }
            buf.advance(32 * 1024);
            Ok(Some(0))
        }
    }
}

#[test]
fn data_remaining_is_error() {
    let mut task = task::spawn(());
    let slice = &[0; 5][..];
    let mut framed = FramedRead::new(slice, U32Decoder);

    task.enter(|cx, _| {
        assert_read!(pin!(framed).poll_next(cx), 0);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).unwrap().is_err());
    });
}

#[test]
fn multi_frames_on_eof() {
    let mut task = task::spawn(());
    struct MyDecoder(Vec<u32>);

    impl Decoder for MyDecoder {
        type Item = u32;
        type Error = io::Error;

        fn decode(&mut self, _buf: &mut BytesMut) -> io::Result<Option<u32>> {
            unreachable!();
        }

        fn decode_eof(&mut self, _buf: &mut BytesMut) -> io::Result<Option<u32>> {
            if self.0.is_empty() {
                return Ok(None);
            }

            Ok(Some(self.0.remove(0)))
        }
    }

    let mut framed = FramedRead::new(mock!(), MyDecoder(vec![0, 1, 2, 3]));

    task.enter(|cx, _| {
        assert_read!(pin!(framed).poll_next(cx), 0);
        assert_read!(pin!(framed).poll_next(cx), 1);
        assert_read!(pin!(framed).poll_next(cx), 2);
        assert_read!(pin!(framed).poll_next(cx), 3);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
    });
}

#[test]
fn read_eof_then_resume() {
    let mut task = task::spawn(());
    let mock = mock! {
        Ok(b"\x00\x00\x00\x01".to_vec()),
        Ok(b"".to_vec()),
        Ok(b"\x00\x00\x00\x02".to_vec()),
        Ok(b"".to_vec()),
        Ok(b"\x00\x00\x00\x03".to_vec()),
    };
    let mut framed = FramedRead::new(mock, U32Decoder);

    task.enter(|cx, _| {
        assert_read!(pin!(framed).poll_next(cx), 1);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
        assert_read!(pin!(framed).poll_next(cx), 2);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
        assert_read!(pin!(framed).poll_next(cx), 3);
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
        assert!(assert_ready!(pin!(framed).poll_next(cx)).is_none());
    });
}

// ===== Mock ======

struct Mock {
    calls: VecDeque<io::Result<Vec<u8>>>,
}

impl AsyncRead for Mock {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        use io::ErrorKind::WouldBlock;

        match self.calls.pop_front() {
            Some(Ok(data)) => {
                debug_assert!(buf.remaining() >= data.len());
                buf.put_slice(&data);
                Ready(Ok(()))
            }
            Some(Err(ref e)) if e.kind() == WouldBlock => Pending,
            Some(Err(e)) => Ready(Err(e)),
            None => Ready(Ok(())),
        }
    }
}
