#![warn(rust_2018_idioms)]

use tokio::io::AsyncWrite;
use tokio_test::{assert_ready, task};
use tokio_util::codec::{Encoder, FramedWrite};

use bytes::{BufMut, BytesMut};
use futures_sink::Sink;
use std::collections::VecDeque;
use std::io::{self, Write};
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

macro_rules! pin {
    ($id:ident) => {
        Pin::new(&mut $id)
    };
}

struct U32Encoder;

impl Encoder<u32> for U32Encoder {
    type Error = io::Error;

    fn encode(&mut self, item: u32, dst: &mut BytesMut) -> io::Result<()> {
        // Reserve space
        dst.reserve(4);
        dst.put_u32(item);
        Ok(())
    }
}

struct U64Encoder;

impl Encoder<u64> for U64Encoder {
    type Error = io::Error;

    fn encode(&mut self, item: u64, dst: &mut BytesMut) -> io::Result<()> {
        // Reserve space
        dst.reserve(8);
        dst.put_u64(item);
        Ok(())
    }
}

#[test]
fn write_multi_frame_in_packet() {
    let mut task = task::spawn(());
    let mock = mock! {
        Ok(b"\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\x02".to_vec()),
    };
    let mut framed = FramedWrite::new(mock, U32Encoder);

    task.enter(|cx, _| {
        assert!(assert_ready!(pin!(framed).poll_ready(cx)).is_ok());
        assert!(pin!(framed).start_send(0).is_ok());
        assert!(assert_ready!(pin!(framed).poll_ready(cx)).is_ok());
        assert!(pin!(framed).start_send(1).is_ok());
        assert!(assert_ready!(pin!(framed).poll_ready(cx)).is_ok());
        assert!(pin!(framed).start_send(2).is_ok());

        // Nothing written yet
        assert_eq!(1, framed.get_ref().calls.len());

        // Flush the writes
        assert!(assert_ready!(pin!(framed).poll_flush(cx)).is_ok());

        assert_eq!(0, framed.get_ref().calls.len());
    });
}

#[test]
fn write_multi_frame_after_codec_changed() {
    let mut task = task::spawn(());
    let mock = mock! {
        Ok(b"\x00\x00\x00\x04\x00\x00\x00\x00\x00\x00\x00\x08".to_vec()),
    };
    let mut framed = FramedWrite::new(mock, U32Encoder);

    task.enter(|cx, _| {
        assert!(assert_ready!(pin!(framed).poll_ready(cx)).is_ok());
        assert!(pin!(framed).start_send(0x04).is_ok());

        let mut framed = framed.map_encoder(|_| U64Encoder);
        assert!(assert_ready!(pin!(framed).poll_ready(cx)).is_ok());
        assert!(pin!(framed).start_send(0x08).is_ok());

        // Nothing written yet
        assert_eq!(1, framed.get_ref().calls.len());

        // Flush the writes
        assert!(assert_ready!(pin!(framed).poll_flush(cx)).is_ok());

        assert_eq!(0, framed.get_ref().calls.len());
    });
}

#[test]
fn write_hits_backpressure() {
    const ITER: usize = 2 * 1024;

    let mut mock = mock! {
        // Block the `ITER*2`th write
        Err(io::Error::new(io::ErrorKind::WouldBlock, "not ready")),
        Ok(b"".to_vec()),
    };

    for i in 0..=ITER * 2 {
        let mut b = BytesMut::with_capacity(4);
        b.put_u32(i as u32);

        // Append to the end
        match mock.calls.back_mut().unwrap() {
            Ok(ref mut data) => {
                // Write in 2kb chunks
                if data.len() < ITER {
                    data.extend_from_slice(&b[..]);
                    continue;
                } // else fall through and create a new buffer
            }
            _ => unreachable!(),
        }

        // Push a new chunk
        mock.calls.push_back(Ok(b[..].to_vec()));
    }
    // 1 'wouldblock', 8 * 2KB buffers, 1 b-byte buffer
    assert_eq!(mock.calls.len(), 10);

    let mut task = task::spawn(());
    let mut framed = FramedWrite::new(mock, U32Encoder);
    framed.set_backpressure_boundary(ITER * 8);
    task.enter(|cx, _| {
        // Send 16KB. This fills up FramedWrite buffer
        for i in 0..ITER * 2 {
            assert!(assert_ready!(pin!(framed).poll_ready(cx)).is_ok());
            assert!(pin!(framed).start_send(i as u32).is_ok());
        }

        // Now we poll_ready which forces a flush. The mock pops the front message
        // and decides to block.
        assert!(pin!(framed).poll_ready(cx).is_pending());

        // We poll again, forcing another flush, which this time succeeds
        // The whole 16KB buffer is flushed
        assert!(assert_ready!(pin!(framed).poll_ready(cx)).is_ok());

        // Send more data. This matches the final message expected by the mock
        assert!(pin!(framed).start_send((ITER * 2) as u32).is_ok());

        // Flush the rest of the buffer
        assert!(assert_ready!(pin!(framed).poll_flush(cx)).is_ok());

        // Ensure the mock is empty
        assert_eq!(0, framed.get_ref().calls.len());
    })
}

// // ===== Mock ======

struct Mock {
    calls: VecDeque<io::Result<Vec<u8>>>,
}

impl Write for Mock {
    fn write(&mut self, src: &[u8]) -> io::Result<usize> {
        match self.calls.pop_front() {
            Some(Ok(data)) => {
                assert!(src.len() >= data.len());
                assert_eq!(&data[..], &src[..data.len()]);
                Ok(data.len())
            }
            Some(Err(e)) => Err(e),
            None => panic!("unexpected write; {src:?}"),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsyncWrite for Mock {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match Pin::get_mut(self).write(buf) {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Pending,
            other => Ready(other),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match Pin::get_mut(self).flush() {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Pending,
            other => Ready(other),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        unimplemented!()
    }
}
