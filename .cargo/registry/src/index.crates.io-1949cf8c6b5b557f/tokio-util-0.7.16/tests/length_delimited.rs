#![warn(rust_2018_idioms)]

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_test::task;
use tokio_test::{
    assert_err, assert_ok, assert_pending, assert_ready, assert_ready_err, assert_ready_ok,
};
use tokio_util::codec::*;

use bytes::{BufMut, Bytes, BytesMut};
use futures::{pin_mut, Sink, Stream};
use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

macro_rules! mock {
    ($($x:expr,)*) => {{
        let mut v = VecDeque::new();
        v.extend(vec![$($x),*]);
        Mock { calls: v }
    }};
}

macro_rules! assert_next_eq {
    ($io:ident, $expect:expr) => {{
        task::spawn(()).enter(|cx, _| {
            let res = assert_ready!($io.as_mut().poll_next(cx));
            match res {
                Some(Ok(v)) => assert_eq!(v, $expect.as_ref()),
                Some(Err(e)) => panic!("error = {:?}", e),
                None => panic!("none"),
            }
        });
    }};
}

macro_rules! assert_next_pending {
    ($io:ident) => {{
        task::spawn(()).enter(|cx, _| match $io.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(v))) => panic!("value = {:?}", v),
            Poll::Ready(Some(Err(e))) => panic!("error = {:?}", e),
            Poll::Ready(None) => panic!("done"),
            Poll::Pending => {}
        });
    }};
}

macro_rules! assert_next_err {
    ($io:ident) => {{
        task::spawn(()).enter(|cx, _| match $io.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(v))) => panic!("value = {:?}", v),
            Poll::Ready(Some(Err(_))) => {}
            Poll::Ready(None) => panic!("done"),
            Poll::Pending => panic!("pending"),
        });
    }};
}

macro_rules! assert_done {
    ($io:ident) => {{
        task::spawn(()).enter(|cx, _| {
            let res = assert_ready!($io.as_mut().poll_next(cx));
            match res {
                Some(Ok(v)) => panic!("value = {:?}", v),
                Some(Err(e)) => panic!("error = {:?}", e),
                None => {}
            }
        });
    }};
}

#[test]
fn read_empty_io_yields_nothing() {
    let io = Box::pin(FramedRead::new(mock!(), LengthDelimitedCodec::new()));
    pin_mut!(io);

    assert_done!(io);
}

#[test]
fn read_single_frame_one_packet() {
    let io = FramedRead::new(
        mock! {
            data(b"\x00\x00\x00\x09abcdefghi"),
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    assert_done!(io);
}

#[test]
fn read_single_frame_one_packet_little_endian() {
    let io = length_delimited::Builder::new()
        .little_endian()
        .new_read(mock! {
            data(b"\x09\x00\x00\x00abcdefghi"),
        });
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    assert_done!(io);
}

#[test]
fn read_single_frame_one_packet_native_endian() {
    let d = if cfg!(target_endian = "big") {
        b"\x00\x00\x00\x09abcdefghi"
    } else {
        b"\x09\x00\x00\x00abcdefghi"
    };
    let io = length_delimited::Builder::new()
        .native_endian()
        .new_read(mock! {
            data(d),
        });
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    assert_done!(io);
}

#[test]
fn read_single_multi_frame_one_packet() {
    let mut d: Vec<u8> = vec![];
    d.extend_from_slice(b"\x00\x00\x00\x09abcdefghi");
    d.extend_from_slice(b"\x00\x00\x00\x03123");
    d.extend_from_slice(b"\x00\x00\x00\x0bhello world");

    let io = FramedRead::new(
        mock! {
            data(&d),
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    assert_next_eq!(io, b"123");
    assert_next_eq!(io, b"hello world");
    assert_done!(io);
}

#[test]
fn read_single_frame_multi_packet() {
    let io = FramedRead::new(
        mock! {
            data(b"\x00\x00"),
            data(b"\x00\x09abc"),
            data(b"defghi"),
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    assert_done!(io);
}

#[test]
fn read_multi_frame_multi_packet() {
    let io = FramedRead::new(
        mock! {
            data(b"\x00\x00"),
            data(b"\x00\x09abc"),
            data(b"defghi"),
            data(b"\x00\x00\x00\x0312"),
            data(b"3\x00\x00\x00\x0bhello world"),
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    assert_next_eq!(io, b"123");
    assert_next_eq!(io, b"hello world");
    assert_done!(io);
}

#[test]
fn read_single_frame_multi_packet_wait() {
    let io = FramedRead::new(
        mock! {
            data(b"\x00\x00"),
            Poll::Pending,
            data(b"\x00\x09abc"),
            Poll::Pending,
            data(b"defghi"),
            Poll::Pending,
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    assert_next_pending!(io);
    assert_next_pending!(io);
    assert_next_eq!(io, b"abcdefghi");
    assert_next_pending!(io);
    assert_done!(io);
}

#[test]
fn read_multi_frame_multi_packet_wait() {
    let io = FramedRead::new(
        mock! {
            data(b"\x00\x00"),
            Poll::Pending,
            data(b"\x00\x09abc"),
            Poll::Pending,
            data(b"defghi"),
            Poll::Pending,
            data(b"\x00\x00\x00\x0312"),
            Poll::Pending,
            data(b"3\x00\x00\x00\x0bhello world"),
            Poll::Pending,
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    assert_next_pending!(io);
    assert_next_pending!(io);
    assert_next_eq!(io, b"abcdefghi");
    assert_next_pending!(io);
    assert_next_pending!(io);
    assert_next_eq!(io, b"123");
    assert_next_eq!(io, b"hello world");
    assert_next_pending!(io);
    assert_done!(io);
}

#[test]
fn read_incomplete_head() {
    let io = FramedRead::new(
        mock! {
            data(b"\x00\x00"),
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    assert_next_err!(io);
}

#[test]
fn read_incomplete_head_multi() {
    let io = FramedRead::new(
        mock! {
            Poll::Pending,
            data(b"\x00"),
            Poll::Pending,
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    assert_next_pending!(io);
    assert_next_pending!(io);
    assert_next_err!(io);
}

#[test]
fn read_incomplete_payload() {
    let io = FramedRead::new(
        mock! {
            data(b"\x00\x00\x00\x09ab"),
            Poll::Pending,
            data(b"cd"),
            Poll::Pending,
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    assert_next_pending!(io);
    assert_next_pending!(io);
    assert_next_err!(io);
}

#[test]
fn read_max_frame_len() {
    let io = length_delimited::Builder::new()
        .max_frame_length(5)
        .new_read(mock! {
            data(b"\x00\x00\x00\x09abcdefghi"),
        });
    pin_mut!(io);

    assert_next_err!(io);
}

#[test]
fn read_update_max_frame_len_at_rest() {
    let io = length_delimited::Builder::new().new_read(mock! {
        data(b"\x00\x00\x00\x09abcdefghi"),
        data(b"\x00\x00\x00\x09abcdefghi"),
    });
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    io.decoder_mut().set_max_frame_length(5);
    assert_next_err!(io);
}

#[test]
fn read_update_max_frame_len_in_flight() {
    let io = length_delimited::Builder::new().new_read(mock! {
        data(b"\x00\x00\x00\x09abcd"),
        Poll::Pending,
        data(b"efghi"),
        data(b"\x00\x00\x00\x09abcdefghi"),
    });
    pin_mut!(io);

    assert_next_pending!(io);
    io.decoder_mut().set_max_frame_length(5);
    assert_next_eq!(io, b"abcdefghi");
    assert_next_err!(io);
}

#[test]
fn read_one_byte_length_field() {
    let io = length_delimited::Builder::new()
        .length_field_length(1)
        .new_read(mock! {
            data(b"\x09abcdefghi"),
        });
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    assert_done!(io);
}

#[test]
fn read_header_offset() {
    let io = length_delimited::Builder::new()
        .length_field_length(2)
        .length_field_offset(4)
        .new_read(mock! {
            data(b"zzzz\x00\x09abcdefghi"),
        });
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    assert_done!(io);
}

#[test]
fn read_single_multi_frame_one_packet_skip_none_adjusted() {
    let mut d: Vec<u8> = vec![];
    d.extend_from_slice(b"xx\x00\x09abcdefghi");
    d.extend_from_slice(b"yy\x00\x03123");
    d.extend_from_slice(b"zz\x00\x0bhello world");

    let io = length_delimited::Builder::new()
        .length_field_length(2)
        .length_field_offset(2)
        .num_skip(0)
        .length_adjustment(4)
        .new_read(mock! {
            data(&d),
        });
    pin_mut!(io);

    assert_next_eq!(io, b"xx\x00\x09abcdefghi");
    assert_next_eq!(io, b"yy\x00\x03123");
    assert_next_eq!(io, b"zz\x00\x0bhello world");
    assert_done!(io);
}

#[test]
fn read_single_frame_length_adjusted() {
    let mut d: Vec<u8> = vec![];
    d.extend_from_slice(b"\x00\x00\x0b\x0cHello world");

    let io = length_delimited::Builder::new()
        .length_field_offset(0)
        .length_field_length(3)
        .length_adjustment(0)
        .num_skip(4)
        .new_read(mock! {
            data(&d),
        });
    pin_mut!(io);

    assert_next_eq!(io, b"Hello world");
    assert_done!(io);
}

#[test]
fn read_single_multi_frame_one_packet_length_includes_head() {
    let mut d: Vec<u8> = vec![];
    d.extend_from_slice(b"\x00\x0babcdefghi");
    d.extend_from_slice(b"\x00\x05123");
    d.extend_from_slice(b"\x00\x0dhello world");

    let io = length_delimited::Builder::new()
        .length_field_length(2)
        .length_adjustment(-2)
        .new_read(mock! {
            data(&d),
        });
    pin_mut!(io);

    assert_next_eq!(io, b"abcdefghi");
    assert_next_eq!(io, b"123");
    assert_next_eq!(io, b"hello world");
    assert_done!(io);
}

#[test]
fn write_single_frame_length_adjusted() {
    let io = length_delimited::Builder::new()
        .length_adjustment(-2)
        .new_write(mock! {
            data(b"\x00\x00\x00\x0b"),
            data(b"abcdefghi"),
            flush(),
        });
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdefghi")));
        assert_ready_ok!(io.as_mut().poll_flush(cx));
        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_nothing_yields_nothing() {
    let io = FramedWrite::new(mock!(), LengthDelimitedCodec::new());
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.poll_flush(cx));
    });
}

#[test]
fn write_single_frame_one_packet() {
    let io = FramedWrite::new(
        mock! {
            data(b"\x00\x00\x00\x09"),
            data(b"abcdefghi"),
            flush(),
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdefghi")));
        assert_ready_ok!(io.as_mut().poll_flush(cx));
        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_single_multi_frame_one_packet() {
    let io = FramedWrite::new(
        mock! {
            data(b"\x00\x00\x00\x09"),
            data(b"abcdefghi"),
            data(b"\x00\x00\x00\x03"),
            data(b"123"),
            data(b"\x00\x00\x00\x0b"),
            data(b"hello world"),
            flush(),
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdefghi")));

        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("123")));

        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("hello world")));

        assert_ready_ok!(io.as_mut().poll_flush(cx));
        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_single_multi_frame_multi_packet() {
    let io = FramedWrite::new(
        mock! {
            data(b"\x00\x00\x00\x09"),
            data(b"abcdefghi"),
            flush(),
            data(b"\x00\x00\x00\x03"),
            data(b"123"),
            flush(),
            data(b"\x00\x00\x00\x0b"),
            data(b"hello world"),
            flush(),
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdefghi")));

        assert_ready_ok!(io.as_mut().poll_flush(cx));

        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("123")));

        assert_ready_ok!(io.as_mut().poll_flush(cx));

        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("hello world")));

        assert_ready_ok!(io.as_mut().poll_flush(cx));
        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_single_frame_would_block() {
    let io = FramedWrite::new(
        mock! {
            Poll::Pending,
            data(b"\x00\x00"),
            Poll::Pending,
            data(b"\x00\x09"),
            data(b"abcdefghi"),
            flush(),
        },
        LengthDelimitedCodec::new(),
    );
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdefghi")));

        assert_pending!(io.as_mut().poll_flush(cx));
        assert_pending!(io.as_mut().poll_flush(cx));
        assert_ready_ok!(io.as_mut().poll_flush(cx));

        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_single_frame_little_endian() {
    let io = length_delimited::Builder::new()
        .little_endian()
        .new_write(mock! {
            data(b"\x09\x00\x00\x00"),
            data(b"abcdefghi"),
            flush(),
        });
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdefghi")));

        assert_ready_ok!(io.as_mut().poll_flush(cx));
        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_single_frame_with_short_length_field() {
    let io = length_delimited::Builder::new()
        .length_field_length(1)
        .new_write(mock! {
            data(b"\x09"),
            data(b"abcdefghi"),
            flush(),
        });
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdefghi")));

        assert_ready_ok!(io.as_mut().poll_flush(cx));

        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_max_frame_len() {
    let io = length_delimited::Builder::new()
        .max_frame_length(5)
        .new_write(mock! {});
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_err!(io.as_mut().start_send(Bytes::from("abcdef")));

        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_update_max_frame_len_at_rest() {
    let io = length_delimited::Builder::new().new_write(mock! {
        data(b"\x00\x00\x00\x06"),
        data(b"abcdef"),
        flush(),
    });
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdef")));

        assert_ready_ok!(io.as_mut().poll_flush(cx));

        io.encoder_mut().set_max_frame_length(5);

        assert_err!(io.as_mut().start_send(Bytes::from("abcdef")));

        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_update_max_frame_len_in_flight() {
    let io = length_delimited::Builder::new().new_write(mock! {
        data(b"\x00\x00\x00\x06"),
        data(b"ab"),
        Poll::Pending,
        data(b"cdef"),
        flush(),
    });
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdef")));

        assert_pending!(io.as_mut().poll_flush(cx));

        io.encoder_mut().set_max_frame_length(5);

        assert_ready_ok!(io.as_mut().poll_flush(cx));

        assert_err!(io.as_mut().start_send(Bytes::from("abcdef")));
        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn write_zero() {
    let io = length_delimited::Builder::new().new_write(mock! {});
    pin_mut!(io);

    task::spawn(()).enter(|cx, _| {
        assert_ready_ok!(io.as_mut().poll_ready(cx));
        assert_ok!(io.as_mut().start_send(Bytes::from("abcdef")));

        assert_ready_err!(io.as_mut().poll_flush(cx));

        assert!(io.get_ref().calls.is_empty());
    });
}

#[test]
fn encode_overflow() {
    // Test reproducing tokio-rs/tokio#681.
    let mut codec = length_delimited::Builder::new().new_codec();
    let mut buf = BytesMut::with_capacity(1024);

    // Put some data into the buffer without resizing it to hold more.
    let some_as = std::iter::repeat(b'a').take(1024).collect::<Vec<_>>();
    buf.put_slice(&some_as[..]);

    // Trying to encode the length header should resize the buffer if it won't fit.
    codec.encode(Bytes::from("hello"), &mut buf).unwrap();
}

#[test]
fn frame_does_not_fit() {
    let codec = LengthDelimitedCodec::builder()
        .length_field_length(1)
        .max_frame_length(256)
        .new_codec();

    assert_eq!(codec.max_frame_length(), 255);
}

#[test]
fn neg_adjusted_frame_does_not_fit() {
    let codec = LengthDelimitedCodec::builder()
        .length_field_length(1)
        .length_adjustment(-1)
        .new_codec();

    assert_eq!(codec.max_frame_length(), 254);
}

#[test]
fn pos_adjusted_frame_does_not_fit() {
    let codec = LengthDelimitedCodec::builder()
        .length_field_length(1)
        .length_adjustment(1)
        .new_codec();

    assert_eq!(codec.max_frame_length(), 256);
}

#[test]
fn max_allowed_frame_fits() {
    let codec = LengthDelimitedCodec::builder()
        .length_field_length(std::mem::size_of::<usize>())
        .max_frame_length(usize::MAX)
        .new_codec();

    assert_eq!(codec.max_frame_length(), usize::MAX);
}

#[test]
fn smaller_frame_len_not_adjusted() {
    let codec = LengthDelimitedCodec::builder()
        .max_frame_length(10)
        .length_field_length(std::mem::size_of::<usize>())
        .new_codec();

    assert_eq!(codec.max_frame_length(), 10);
}

#[test]
fn max_allowed_length_field() {
    let codec = LengthDelimitedCodec::builder()
        .length_field_length(8)
        .max_frame_length(usize::MAX)
        .new_codec();

    assert_eq!(codec.max_frame_length(), usize::MAX);
}

// ===== Test utils =====

struct Mock {
    calls: VecDeque<Poll<io::Result<Op>>>,
}

enum Op {
    Data(Vec<u8>),
    Flush,
}

impl AsyncRead for Mock {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        dst: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.calls.pop_front() {
            Some(Poll::Ready(Ok(Op::Data(data)))) => {
                debug_assert!(dst.remaining() >= data.len());
                dst.put_slice(&data);
                Poll::Ready(Ok(()))
            }
            Some(Poll::Ready(Ok(_))) => panic!(),
            Some(Poll::Ready(Err(e))) => Poll::Ready(Err(e)),
            Some(Poll::Pending) => Poll::Pending,
            None => Poll::Ready(Ok(())),
        }
    }
}

impl AsyncWrite for Mock {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        src: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match self.calls.pop_front() {
            Some(Poll::Ready(Ok(Op::Data(data)))) => {
                let len = data.len();
                assert!(src.len() >= len, "expect={data:?}; actual={src:?}");
                assert_eq!(&data[..], &src[..len]);
                Poll::Ready(Ok(len))
            }
            Some(Poll::Ready(Ok(_))) => panic!(),
            Some(Poll::Ready(Err(e))) => Poll::Ready(Err(e)),
            Some(Poll::Pending) => Poll::Pending,
            None => Poll::Ready(Ok(0)),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match self.calls.pop_front() {
            Some(Poll::Ready(Ok(Op::Flush))) => Poll::Ready(Ok(())),
            Some(Poll::Ready(Ok(_))) => panic!(),
            Some(Poll::Ready(Err(e))) => Poll::Ready(Err(e)),
            Some(Poll::Pending) => Poll::Pending,
            None => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl<'a> From<&'a [u8]> for Op {
    fn from(src: &'a [u8]) -> Op {
        Op::Data(src.into())
    }
}

impl From<Vec<u8>> for Op {
    fn from(src: Vec<u8>) -> Op {
        Op::Data(src)
    }
}

fn data(bytes: &[u8]) -> Poll<io::Result<Op>> {
    Poll::Ready(Ok(bytes.into()))
}

fn flush() -> Poll<io::Result<Op>> {
    Poll::Ready(Ok(Op::Flush))
}
