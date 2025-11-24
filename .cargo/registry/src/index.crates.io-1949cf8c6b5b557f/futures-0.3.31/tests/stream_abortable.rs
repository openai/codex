use futures::channel::mpsc;
use futures::executor::block_on;
use futures::stream::{abortable, Stream, StreamExt};
use futures::task::{Context, Poll};
use futures::SinkExt;
use futures_test::task::new_count_waker;
use std::pin::Pin;

#[test]
fn abortable_works() {
    let (_tx, a_rx) = mpsc::channel::<()>(1);
    let (mut abortable_rx, abort_handle) = abortable(a_rx);

    abort_handle.abort();
    assert!(abortable_rx.is_aborted());
    assert_eq!(None, block_on(abortable_rx.next()));
}

#[test]
fn abortable_awakens() {
    let (_tx, a_rx) = mpsc::channel::<()>(1);
    let (mut abortable_rx, abort_handle) = abortable(a_rx);

    let (waker, counter) = new_count_waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(counter, 0);
    assert_eq!(Poll::Pending, Pin::new(&mut abortable_rx).poll_next(&mut cx));
    assert_eq!(counter, 0);

    abort_handle.abort();
    assert_eq!(counter, 1);
    assert!(abortable_rx.is_aborted());
    assert_eq!(Poll::Ready(None), Pin::new(&mut abortable_rx).poll_next(&mut cx));
}

#[test]
fn abortable_resolves() {
    let (mut tx, a_rx) = mpsc::channel::<()>(1);
    let (mut abortable_rx, _abort_handle) = abortable(a_rx);

    block_on(tx.send(())).unwrap();

    assert!(!abortable_rx.is_aborted());
    assert_eq!(Some(()), block_on(abortable_rx.next()));
}
