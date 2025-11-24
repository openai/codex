use futures::channel::oneshot;
use futures::executor::{block_on, block_on_stream};
use futures::future;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::task::Poll;
use futures_test::task::noop_context;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Barrier};
use std::thread;

#[test]
fn basic_usage() {
    block_on(future::lazy(move |cx| {
        let mut queue = FuturesUnordered::new();
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        let (tx3, rx3) = oneshot::channel();

        queue.push(rx1);
        queue.push(rx2);
        queue.push(rx3);

        assert!(!queue.poll_next_unpin(cx).is_ready());

        tx2.send("hello").unwrap();

        assert_eq!(Poll::Ready(Some(Ok("hello"))), queue.poll_next_unpin(cx));
        assert!(!queue.poll_next_unpin(cx).is_ready());

        tx1.send("world").unwrap();
        tx3.send("world2").unwrap();

        assert_eq!(Poll::Ready(Some(Ok("world"))), queue.poll_next_unpin(cx));
        assert_eq!(Poll::Ready(Some(Ok("world2"))), queue.poll_next_unpin(cx));
        assert_eq!(Poll::Ready(None), queue.poll_next_unpin(cx));
    }));
}

#[test]
fn resolving_errors() {
    block_on(future::lazy(move |cx| {
        let mut queue = FuturesUnordered::new();
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        let (tx3, rx3) = oneshot::channel();

        queue.push(rx1);
        queue.push(rx2);
        queue.push(rx3);

        assert!(!queue.poll_next_unpin(cx).is_ready());

        drop(tx2);

        assert_eq!(Poll::Ready(Some(Err(oneshot::Canceled))), queue.poll_next_unpin(cx));
        assert!(!queue.poll_next_unpin(cx).is_ready());

        drop(tx1);
        tx3.send("world2").unwrap();

        assert_eq!(Poll::Ready(Some(Err(oneshot::Canceled))), queue.poll_next_unpin(cx));
        assert_eq!(Poll::Ready(Some(Ok("world2"))), queue.poll_next_unpin(cx));
        assert_eq!(Poll::Ready(None), queue.poll_next_unpin(cx));
    }));
}

#[test]
fn dropping_ready_queue() {
    block_on(future::lazy(move |_| {
        let queue = FuturesUnordered::new();
        let (mut tx1, rx1) = oneshot::channel::<()>();
        let (mut tx2, rx2) = oneshot::channel::<()>();
        let (mut tx3, rx3) = oneshot::channel::<()>();

        queue.push(rx1);
        queue.push(rx2);
        queue.push(rx3);

        {
            let cx = &mut noop_context();
            assert!(!tx1.poll_canceled(cx).is_ready());
            assert!(!tx2.poll_canceled(cx).is_ready());
            assert!(!tx3.poll_canceled(cx).is_ready());

            drop(queue);

            assert!(tx1.poll_canceled(cx).is_ready());
            assert!(tx2.poll_canceled(cx).is_ready());
            assert!(tx3.poll_canceled(cx).is_ready());
        }
    }));
}

#[test]
fn stress() {
    const ITER: usize = if cfg!(miri) { 30 } else { 300 };

    for i in 0..ITER {
        let n = (i % 10) + 1;

        let mut queue = FuturesUnordered::new();

        for _ in 0..5 {
            let barrier = Arc::new(Barrier::new(n + 1));

            for num in 0..n {
                let barrier = barrier.clone();
                let (tx, rx) = oneshot::channel();

                queue.push(rx);

                thread::spawn(move || {
                    barrier.wait();
                    tx.send(num).unwrap();
                });
            }

            barrier.wait();

            let mut sync = block_on_stream(queue);

            let mut rx: Vec<_> = (&mut sync).take(n).map(|res| res.unwrap()).collect();

            assert_eq!(rx.len(), n);

            rx.sort_unstable();

            for (i, x) in rx.into_iter().enumerate() {
                assert_eq!(i, x);
            }

            queue = sync.into_inner();
        }
    }
}

#[test]
fn panicking_future_dropped() {
    block_on(future::lazy(move |cx| {
        let mut queue = FuturesUnordered::new();
        queue.push(future::poll_fn(|_| -> Poll<Result<i32, i32>> { panic!() }));

        let r = panic::catch_unwind(AssertUnwindSafe(|| queue.poll_next_unpin(cx)));
        assert!(r.is_err());
        assert!(queue.is_empty());
        assert_eq!(Poll::Ready(None), queue.poll_next_unpin(cx));
    }));
}
