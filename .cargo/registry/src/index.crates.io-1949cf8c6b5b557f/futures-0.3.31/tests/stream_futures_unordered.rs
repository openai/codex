use futures::channel::oneshot;
use futures::executor::{block_on, block_on_stream};
use futures::future::{self, join, Future, FutureExt};
use futures::stream::{FusedStream, FuturesUnordered, StreamExt};
use futures::task::{Context, Poll};
use futures_test::future::FutureTestExt;
use futures_test::task::noop_context;
use futures_test::{assert_stream_done, assert_stream_next, assert_stream_pending};
use std::iter::FromIterator;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn is_terminated() {
    let mut cx = noop_context();
    let mut tasks = FuturesUnordered::new();

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(&mut cx), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);

    // Test that the sentinel value doesn't leak
    assert_eq!(tasks.is_empty(), true);
    assert_eq!(tasks.len(), 0);
    assert_eq!(tasks.iter_mut().len(), 0);

    tasks.push(future::ready(1));

    assert_eq!(tasks.is_empty(), false);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks.iter_mut().len(), 1);

    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(&mut cx), Poll::Ready(Some(1)));
    assert_eq!(tasks.is_terminated(), false);
    assert_eq!(tasks.poll_next_unpin(&mut cx), Poll::Ready(None));
    assert_eq!(tasks.is_terminated(), true);
}

#[test]
fn works_1() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut iter =
        block_on_stream(vec![a_rx, b_rx, c_rx].into_iter().collect::<FuturesUnordered<_>>());

    b_tx.send(99).unwrap();
    assert_eq!(Some(Ok(99)), iter.next());

    a_tx.send(33).unwrap();
    c_tx.send(33).unwrap();
    assert_eq!(Some(Ok(33)), iter.next());
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
        .collect::<FuturesUnordered<_>>();

    a_tx.send(9).unwrap();
    b_tx.send(10).unwrap();

    let mut cx = noop_context();
    assert_eq!(stream.poll_next_unpin(&mut cx), Poll::Ready(Some(Ok(9))));
    c_tx.send(20).unwrap();
    assert_eq!(stream.poll_next_unpin(&mut cx), Poll::Ready(Some(Ok(30))));
    assert_eq!(stream.poll_next_unpin(&mut cx), Poll::Ready(None));
}

#[test]
fn from_iterator() {
    let stream = vec![future::ready::<i32>(1), future::ready::<i32>(2), future::ready::<i32>(3)]
        .into_iter()
        .collect::<FuturesUnordered<_>>();
    assert_eq!(stream.len(), 3);
    assert_eq!(block_on(stream.collect::<Vec<_>>()), vec![1, 2, 3]);
}

#[test]
fn finished_future() {
    let (_a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut stream = vec![
        Box::new(a_rx) as Box<dyn Future<Output = Result<_, _>> + Unpin>,
        Box::new(future::select(b_rx, c_rx).map(|e| e.factor_first().0)) as _,
    ]
    .into_iter()
    .collect::<FuturesUnordered<_>>();

    let cx = &mut noop_context();
    for _ in 0..10 {
        assert!(stream.poll_next_unpin(cx).is_pending());
    }

    b_tx.send(12).unwrap();
    c_tx.send(3).unwrap();
    assert!(stream.poll_next_unpin(cx).is_ready());
    assert!(stream.poll_next_unpin(cx).is_pending());
    assert!(stream.poll_next_unpin(cx).is_pending());
}

#[test]
fn iter_mut_cancel() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut stream = vec![a_rx, b_rx, c_rx].into_iter().collect::<FuturesUnordered<_>>();

    for rx in stream.iter_mut() {
        rx.close();
    }

    let mut iter = block_on_stream(stream);

    assert!(a_tx.is_canceled());
    assert!(b_tx.is_canceled());
    assert!(c_tx.is_canceled());

    assert_eq!(iter.next(), Some(Err(futures::channel::oneshot::Canceled)));
    assert_eq!(iter.next(), Some(Err(futures::channel::oneshot::Canceled)));
    assert_eq!(iter.next(), Some(Err(futures::channel::oneshot::Canceled)));
    assert_eq!(iter.next(), None);
}

#[test]
fn iter_mut_len() {
    let mut stream =
        vec![future::pending::<()>(), future::pending::<()>(), future::pending::<()>()]
            .into_iter()
            .collect::<FuturesUnordered<_>>();

    let mut iter_mut = stream.iter_mut();
    assert_eq!(iter_mut.len(), 3);
    assert!(iter_mut.next().is_some());
    assert_eq!(iter_mut.len(), 2);
    assert!(iter_mut.next().is_some());
    assert_eq!(iter_mut.len(), 1);
    assert!(iter_mut.next().is_some());
    assert_eq!(iter_mut.len(), 0);
    assert!(iter_mut.next().is_none());
}

#[test]
fn iter_cancel() {
    struct AtomicCancel<F> {
        future: F,
        cancel: AtomicBool,
    }

    impl<F: Future + Unpin> Future for AtomicCancel<F> {
        type Output = Option<<F as Future>::Output>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.cancel.load(Ordering::Relaxed) {
                Poll::Ready(None)
            } else {
                self.future.poll_unpin(cx).map(Some)
            }
        }
    }

    impl<F: Future + Unpin> AtomicCancel<F> {
        fn new(future: F) -> Self {
            Self { future, cancel: AtomicBool::new(false) }
        }
    }

    let stream = vec![
        AtomicCancel::new(future::pending::<()>()),
        AtomicCancel::new(future::pending::<()>()),
        AtomicCancel::new(future::pending::<()>()),
    ]
    .into_iter()
    .collect::<FuturesUnordered<_>>();

    for f in stream.iter() {
        f.cancel.store(true, Ordering::Relaxed);
    }

    let mut iter = block_on_stream(stream);

    assert_eq!(iter.next(), Some(None));
    assert_eq!(iter.next(), Some(None));
    assert_eq!(iter.next(), Some(None));
    assert_eq!(iter.next(), None);
}

#[test]
fn iter_len() {
    let stream = vec![future::pending::<()>(), future::pending::<()>(), future::pending::<()>()]
        .into_iter()
        .collect::<FuturesUnordered<_>>();

    let mut iter = stream.iter();
    assert_eq!(iter.len(), 3);
    assert!(iter.next().is_some());
    assert_eq!(iter.len(), 2);
    assert!(iter.next().is_some());
    assert_eq!(iter.len(), 1);
    assert!(iter.next().is_some());
    assert_eq!(iter.len(), 0);
    assert!(iter.next().is_none());
}

#[test]
fn into_iter_cancel() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let stream = vec![a_rx, b_rx, c_rx].into_iter().collect::<FuturesUnordered<_>>();

    let stream = stream
        .into_iter()
        .map(|mut rx| {
            rx.close();
            rx
        })
        .collect::<FuturesUnordered<_>>();

    let mut iter = block_on_stream(stream);

    assert!(a_tx.is_canceled());
    assert!(b_tx.is_canceled());
    assert!(c_tx.is_canceled());

    assert_eq!(iter.next(), Some(Err(futures::channel::oneshot::Canceled)));
    assert_eq!(iter.next(), Some(Err(futures::channel::oneshot::Canceled)));
    assert_eq!(iter.next(), Some(Err(futures::channel::oneshot::Canceled)));
    assert_eq!(iter.next(), None);
}

#[test]
fn into_iter_len() {
    let stream = vec![future::pending::<()>(), future::pending::<()>(), future::pending::<()>()]
        .into_iter()
        .collect::<FuturesUnordered<_>>();

    let mut into_iter = stream.into_iter();
    assert_eq!(into_iter.len(), 3);
    assert!(into_iter.next().is_some());
    assert_eq!(into_iter.len(), 2);
    assert!(into_iter.next().is_some());
    assert_eq!(into_iter.len(), 1);
    assert!(into_iter.next().is_some());
    assert_eq!(into_iter.len(), 0);
    assert!(into_iter.next().is_none());
}

#[test]
fn into_iter_partial() {
    let stream = vec![future::ready(1), future::ready(2), future::ready(3), future::ready(4)]
        .into_iter()
        .collect::<FuturesUnordered<_>>();

    let mut into_iter = stream.into_iter();
    assert!(into_iter.next().is_some());
    assert!(into_iter.next().is_some());
    assert!(into_iter.next().is_some());
    assert_eq!(into_iter.len(), 1);
    // don't panic when iterator is dropped before completing
}

#[test]
fn futures_not_moved_after_poll() {
    // Future that will be ready after being polled twice,
    // asserting that it does not move.
    let fut = future::ready(()).pending_once().assert_unmoved();
    let mut stream = vec![fut; 3].into_iter().collect::<FuturesUnordered<_>>();
    assert_stream_pending!(stream);
    assert_stream_next!(stream, ());
    assert_stream_next!(stream, ());
    assert_stream_next!(stream, ());
    assert_stream_done!(stream);
}

#[test]
fn len_valid_during_out_of_order_completion() {
    // Complete futures out-of-order and add new futures afterwards to ensure
    // length values remain correct.
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();
    let (d_tx, d_rx) = oneshot::channel::<i32>();

    let mut cx = noop_context();
    let mut stream = FuturesUnordered::new();
    assert_eq!(stream.len(), 0);

    stream.push(a_rx);
    assert_eq!(stream.len(), 1);
    stream.push(b_rx);
    assert_eq!(stream.len(), 2);
    stream.push(c_rx);
    assert_eq!(stream.len(), 3);

    b_tx.send(4).unwrap();
    assert_eq!(stream.poll_next_unpin(&mut cx), Poll::Ready(Some(Ok(4))));
    assert_eq!(stream.len(), 2);

    stream.push(d_rx);
    assert_eq!(stream.len(), 3);

    c_tx.send(5).unwrap();
    assert_eq!(stream.poll_next_unpin(&mut cx), Poll::Ready(Some(Ok(5))));
    assert_eq!(stream.len(), 2);

    d_tx.send(6).unwrap();
    assert_eq!(stream.poll_next_unpin(&mut cx), Poll::Ready(Some(Ok(6))));
    assert_eq!(stream.len(), 1);

    a_tx.send(7).unwrap();
    assert_eq!(stream.poll_next_unpin(&mut cx), Poll::Ready(Some(Ok(7))));
    assert_eq!(stream.len(), 0);
}

#[test]
fn polled_only_once_at_most_per_iteration() {
    #[derive(Debug, Clone, Copy, Default)]
    struct F {
        polled: bool,
    }

    impl Future for F {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            if self.polled {
                panic!("polled twice")
            } else {
                self.polled = true;
                Poll::Pending
            }
        }
    }

    let cx = &mut noop_context();

    let mut tasks = FuturesUnordered::from_iter(vec![F::default(); 10]);
    assert!(tasks.poll_next_unpin(cx).is_pending());
    assert_eq!(10, tasks.iter().filter(|f| f.polled).count());

    let mut tasks = FuturesUnordered::from_iter(vec![F::default(); 33]);
    assert!(tasks.poll_next_unpin(cx).is_pending());
    assert_eq!(33, tasks.iter().filter(|f| f.polled).count());

    let mut tasks = FuturesUnordered::<F>::new();
    assert_eq!(Poll::Ready(None), tasks.poll_next_unpin(cx));
}

#[test]
fn clear() {
    let mut tasks = FuturesUnordered::from_iter(vec![future::ready(1), future::ready(2)]);

    assert_eq!(block_on(tasks.next()), Some(1));
    assert!(!tasks.is_empty());

    tasks.clear();
    assert!(tasks.is_empty());

    tasks.push(future::ready(3));
    assert!(!tasks.is_empty());

    tasks.clear();
    assert!(tasks.is_empty());

    assert_eq!(block_on(tasks.next()), None);
    assert!(tasks.is_terminated());
    tasks.clear();
    assert!(!tasks.is_terminated());
}

// https://github.com/rust-lang/futures-rs/issues/2529#issuecomment-997290279
#[test]
fn clear_in_loop() {
    const N: usize =
        if cfg!(miri) || option_env!("QEMU_LD_PREFIX").is_some() { 100 } else { 10_000 };
    futures::executor::block_on(async {
        async fn task() {
            let (s, r) = oneshot::channel();
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_micros(100));
                let _ = s.send(());
            });
            r.await.unwrap()
        }
        let mut futures = FuturesUnordered::new();
        for _ in 0..N {
            for _ in 0..24 {
                futures.push(task());
            }
            let _ = futures.next().await;
            futures.clear();
        }
    });
}

// https://github.com/rust-lang/futures-rs/issues/2863#issuecomment-2219441515
#[test]
#[should_panic]
fn panic_on_drop_fut() {
    struct BadFuture;

    impl Drop for BadFuture {
        fn drop(&mut self) {
            panic!()
        }
    }

    impl Future for BadFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    FuturesUnordered::default().push(BadFuture);
}
