use futures::channel::oneshot;
use futures::executor::block_on;
use futures::future::{abortable, Aborted, FutureExt};
use futures::task::{Context, Poll};
use futures_test::task::new_count_waker;

#[test]
fn abortable_works() {
    let (_tx, a_rx) = oneshot::channel::<()>();
    let (abortable_rx, abort_handle) = abortable(a_rx);

    abort_handle.abort();
    assert!(abortable_rx.is_aborted());
    assert_eq!(Err(Aborted), block_on(abortable_rx));
}

#[test]
fn abortable_awakens() {
    let (_tx, a_rx) = oneshot::channel::<()>();
    let (mut abortable_rx, abort_handle) = abortable(a_rx);

    let (waker, counter) = new_count_waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(counter, 0);
    assert_eq!(Poll::Pending, abortable_rx.poll_unpin(&mut cx));
    assert_eq!(counter, 0);

    abort_handle.abort();
    assert_eq!(counter, 1);
    assert!(abortable_rx.is_aborted());
    assert_eq!(Poll::Ready(Err(Aborted)), abortable_rx.poll_unpin(&mut cx));
}

#[test]
fn abortable_resolves() {
    let (tx, a_rx) = oneshot::channel::<()>();
    let (abortable_rx, _abort_handle) = abortable(a_rx);

    tx.send(()).unwrap();

    assert!(!abortable_rx.is_aborted());
    assert_eq!(Ok(Ok(())), block_on(abortable_rx));
}
