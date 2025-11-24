use futures::channel::oneshot;
use futures::executor::{block_on, block_on_stream};
use futures::future::{self, join, Future, FutureExt, TryFutureExt};
use futures::stream::{FuturesOrdered, StreamExt};
use futures::task::Poll;
use futures_test::task::noop_context;
use std::any::Any;

#[test]
fn works_1() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut stream = vec![a_rx, b_rx, c_rx].into_iter().collect::<FuturesOrdered<_>>();

    b_tx.send(99).unwrap();
    assert!(stream.poll_next_unpin(&mut noop_context()).is_pending());

    a_tx.send(33).unwrap();
    c_tx.send(33).unwrap();

    let mut iter = block_on_stream(stream);
    assert_eq!(Some(Ok(33)), iter.next());
    assert_eq!(Some(Ok(99)), iter.next());
    assert_eq!(Some(Ok(33)), iter.next());
    assert_eq!(None, iter.next());
}

#[test]
fn works_2() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut stream = vec![a_rx.boxed(), join(b_rx, c_rx).map(|(a, b)| Ok(a? + b?)).boxed()]
        .into_iter()
        .collect::<FuturesOrdered<_>>();

    let mut cx = noop_context();
    a_tx.send(33).unwrap();
    b_tx.send(33).unwrap();
    assert!(stream.poll_next_unpin(&mut cx).is_ready());
    assert!(stream.poll_next_unpin(&mut cx).is_pending());
    c_tx.send(33).unwrap();
    assert!(stream.poll_next_unpin(&mut cx).is_ready());
}

#[test]
fn test_push_front() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();
    let (d_tx, d_rx) = oneshot::channel::<i32>();

    let mut stream = FuturesOrdered::new();

    let mut cx = noop_context();

    stream.push_back(a_rx);
    stream.push_back(b_rx);
    stream.push_back(c_rx);

    a_tx.send(1).unwrap();
    b_tx.send(2).unwrap();
    c_tx.send(3).unwrap();

    // 1 and 2 should be received in order
    assert_eq!(Poll::Ready(Some(Ok(1))), stream.poll_next_unpin(&mut cx));
    assert_eq!(Poll::Ready(Some(Ok(2))), stream.poll_next_unpin(&mut cx));

    stream.push_front(d_rx);
    d_tx.send(4).unwrap();

    // we pushed `d_rx` to the front and sent 4, so we should receive 4 next
    // and then 3 after it
    assert_eq!(Poll::Ready(Some(Ok(4))), stream.poll_next_unpin(&mut cx));
    assert_eq!(Poll::Ready(Some(Ok(3))), stream.poll_next_unpin(&mut cx));
}

#[test]
fn test_push_back() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();
    let (d_tx, d_rx) = oneshot::channel::<i32>();

    let mut stream = FuturesOrdered::new();

    let mut cx = noop_context();

    stream.push_back(a_rx);
    stream.push_back(b_rx);
    stream.push_back(c_rx);

    a_tx.send(1).unwrap();
    b_tx.send(2).unwrap();
    c_tx.send(3).unwrap();

    // All results should be received in order

    assert_eq!(Poll::Ready(Some(Ok(1))), stream.poll_next_unpin(&mut cx));
    assert_eq!(Poll::Ready(Some(Ok(2))), stream.poll_next_unpin(&mut cx));

    stream.push_back(d_rx);
    d_tx.send(4).unwrap();

    assert_eq!(Poll::Ready(Some(Ok(3))), stream.poll_next_unpin(&mut cx));
    assert_eq!(Poll::Ready(Some(Ok(4))), stream.poll_next_unpin(&mut cx));
}

#[test]
fn from_iterator() {
    let stream = vec![future::ready::<i32>(1), future::ready::<i32>(2), future::ready::<i32>(3)]
        .into_iter()
        .collect::<FuturesOrdered<_>>();
    assert_eq!(stream.len(), 3);
    assert_eq!(block_on(stream.collect::<Vec<_>>()), vec![1, 2, 3]);
}

#[test]
fn queue_never_unblocked() {
    let (_a_tx, a_rx) = oneshot::channel::<Box<dyn Any + Send>>();
    let (b_tx, b_rx) = oneshot::channel::<Box<dyn Any + Send>>();
    let (c_tx, c_rx) = oneshot::channel::<Box<dyn Any + Send>>();

    let mut stream = vec![
        Box::new(a_rx) as Box<dyn Future<Output = _> + Unpin>,
        Box::new(
            future::try_select(b_rx, c_rx)
                .map_err(|e| e.factor_first().0)
                .and_then(|e| future::ok(Box::new(e) as Box<dyn Any + Send>)),
        ) as _,
    ]
    .into_iter()
    .collect::<FuturesOrdered<_>>();

    let cx = &mut noop_context();
    for _ in 0..10 {
        assert!(stream.poll_next_unpin(cx).is_pending());
    }

    b_tx.send(Box::new(())).unwrap();
    assert!(stream.poll_next_unpin(cx).is_pending());
    c_tx.send(Box::new(())).unwrap();
    assert!(stream.poll_next_unpin(cx).is_pending());
    assert!(stream.poll_next_unpin(cx).is_pending());
}

#[test]
fn test_push_front_negative() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut stream = FuturesOrdered::new();

    let mut cx = noop_context();

    stream.push_front(a_rx);
    stream.push_front(b_rx);
    stream.push_front(c_rx);

    a_tx.send(1).unwrap();
    b_tx.send(2).unwrap();
    c_tx.send(3).unwrap();

    // These should all be received in reverse order
    assert_eq!(Poll::Ready(Some(Ok(3))), stream.poll_next_unpin(&mut cx));
    assert_eq!(Poll::Ready(Some(Ok(2))), stream.poll_next_unpin(&mut cx));
    assert_eq!(Poll::Ready(Some(Ok(1))), stream.poll_next_unpin(&mut cx));
}
