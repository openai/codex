#![allow(missing_docs)]

use bencher::{benchmark_group, benchmark_main, Bencher};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use http_body::{Body, Frame, SizeHint};
use std::{
    fmt::{Error, Formatter},
    pin::Pin,
    task::{Context, Poll},
};
use tonic::{codec::DecodeBuf, codec::Decoder, Status, Streaming};

macro_rules! bench {
    ($name:ident, $message_size:expr, $chunk_size:expr, $message_count:expr) => {
        fn $name(b: &mut Bencher) {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .build()
                .expect("runtime");

            let payload = make_payload($message_size, $message_count);
            let body = MockBody::new(payload, $chunk_size);
            b.bytes = body.len() as u64;

            b.iter(|| {
                rt.block_on(async {
                    let decoder = MockDecoder::new($message_size);
                    let mut stream = Streaming::new_request(decoder, body.clone(), None, None);

                    let mut count = 0;
                    while let Some(msg) = stream.message().await.unwrap() {
                        assert_eq!($message_size, msg.len());
                        count += 1;
                    }

                    assert_eq!(count, $message_count);
                    assert!(stream.trailers().await.unwrap().is_none());
                })
            })
        }
    };
}

#[derive(Clone)]
struct MockBody {
    data: Bytes,
    chunk_size: usize,
}

impl MockBody {
    fn new(data: Bytes, chunk_size: usize) -> Self {
        MockBody { data, chunk_size }
    }

    fn len(&self) -> usize {
        self.data.len()
    }
}

impl Body for MockBody {
    type Data = Bytes;
    type Error = Status;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.data.has_remaining() {
            let split = std::cmp::min(self.chunk_size, self.data.remaining());
            Poll::Ready(Some(Ok(Frame::data(self.data.split_to(split)))))
        } else {
            Poll::Ready(None)
        }
    }

    fn is_end_stream(&self) -> bool {
        self.data.is_empty()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.data.len() as u64)
    }
}

impl std::fmt::Debug for MockBody {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let sample = self.data.iter().take(10).collect::<Vec<_>>();
        write!(f, "{:?}...({})", sample, self.data.len())
    }
}

#[derive(Debug, Clone)]
struct MockDecoder {
    message_size: usize,
}

impl MockDecoder {
    fn new(message_size: usize) -> Self {
        MockDecoder { message_size }
    }
}

impl Decoder for MockDecoder {
    type Item = Vec<u8>;
    type Error = Status;

    fn decode(&mut self, buf: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        let out = Vec::from(buf.chunk());
        buf.advance(self.message_size);
        Ok(Some(out))
    }
}

fn make_payload(message_length: usize, message_count: usize) -> Bytes {
    let mut buf = BytesMut::new();

    for _ in 0..message_count {
        let msg = vec![97u8; message_length];
        buf.reserve(msg.len() + 5);
        buf.put_u8(0);
        buf.put_u32(msg.len() as u32);
        buf.put(&msg[..]);
    }

    buf.freeze()
}

// change body chunk size only
bench!(chunk_size_100, 1_000, 100, 1);
bench!(chunk_size_500, 1_000, 500, 1);
bench!(chunk_size_1005, 1_000, 1_005, 1);

// change message size only
bench!(message_size_1k, 1_000, 1_005, 2);
bench!(message_size_5k, 5_000, 1_005, 2);
bench!(message_size_10k, 10_000, 1_005, 2);

// change message count only
bench!(message_count_1, 500, 505, 1);
bench!(message_count_10, 500, 505, 10);
bench!(message_count_20, 500, 505, 20);

benchmark_group!(chunk_size, chunk_size_100, chunk_size_500, chunk_size_1005);

benchmark_group!(
    message_size,
    message_size_1k,
    message_size_5k,
    message_size_10k
);

benchmark_group!(
    message_count,
    message_count_1,
    message_count_10,
    message_count_20
);

benchmark_main!(chunk_size, message_size, message_count);
