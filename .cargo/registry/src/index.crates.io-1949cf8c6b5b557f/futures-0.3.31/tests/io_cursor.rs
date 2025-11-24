use assert_matches::assert_matches;
use futures::executor::block_on;
use futures::future::lazy;
use futures::io::{AsyncWrite, Cursor};
use futures::task::Poll;
use std::pin::Pin;

#[test]
fn cursor_asyncwrite_vec() {
    let mut cursor = Cursor::new(vec![0; 5]);
    block_on(lazy(|cx| {
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[1, 2]), Poll::Ready(Ok(2)));
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[3, 4]), Poll::Ready(Ok(2)));
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[5, 6]), Poll::Ready(Ok(2)));
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[6, 7]), Poll::Ready(Ok(2)));
    }));
    assert_eq!(cursor.into_inner(), [1, 2, 3, 4, 5, 6, 6, 7]);
}

#[test]
fn cursor_asyncwrite_box() {
    let mut cursor = Cursor::new(vec![0; 5].into_boxed_slice());
    block_on(lazy(|cx| {
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[1, 2]), Poll::Ready(Ok(2)));
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[3, 4]), Poll::Ready(Ok(2)));
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[5, 6]), Poll::Ready(Ok(1)));
        assert_matches!(Pin::new(&mut cursor).poll_write(cx, &[6, 7]), Poll::Ready(Ok(0)));
    }));
    assert_eq!(&*cursor.into_inner(), [1, 2, 3, 4, 5]);
}
