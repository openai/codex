use futures::io::AsyncRead;
use futures_test::task::panic_context;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

struct MockReader {
    fun: Box<dyn FnMut(&mut [u8]) -> Poll<io::Result<usize>>>,
}

impl MockReader {
    fn new(fun: impl FnMut(&mut [u8]) -> Poll<io::Result<usize>> + 'static) -> Self {
        Self { fun: Box::new(fun) }
    }
}

impl AsyncRead for MockReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        (self.get_mut().fun)(buf)
    }
}

/// Verifies that the default implementation of `poll_read_vectored`
/// calls `poll_read` with an empty slice if no buffers are provided.
#[test]
fn read_vectored_no_buffers() {
    let mut reader = MockReader::new(|buf| {
        assert_eq!(buf, b"");
        Err(io::ErrorKind::BrokenPipe.into()).into()
    });
    let cx = &mut panic_context();
    let bufs = &mut [];

    let res = Pin::new(&mut reader).poll_read_vectored(cx, bufs);
    let res = res.map_err(|e| e.kind());
    assert_eq!(res, Poll::Ready(Err(io::ErrorKind::BrokenPipe)))
}

/// Verifies that the default implementation of `poll_read_vectored`
/// calls `poll_read` with the first non-empty buffer.
#[test]
fn read_vectored_first_non_empty() {
    let mut reader = MockReader::new(|buf| {
        assert_eq!(buf.len(), 4);
        buf.copy_from_slice(b"four");
        Poll::Ready(Ok(4))
    });
    let cx = &mut panic_context();
    let mut buf = [0; 4];
    let bufs = &mut [
        io::IoSliceMut::new(&mut []),
        io::IoSliceMut::new(&mut []),
        io::IoSliceMut::new(&mut buf),
    ];

    let res = Pin::new(&mut reader).poll_read_vectored(cx, bufs);
    let res = res.map_err(|e| e.kind());
    assert_eq!(res, Poll::Ready(Ok(4)));
    assert_eq!(buf, b"four"[..]);
}
