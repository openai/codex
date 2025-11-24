#![feature(test)]

extern crate test;
use crate::test::Bencher;

use {
    futures::{
        channel::mpsc::{self, Sender, UnboundedSender},
        ready,
        sink::Sink,
        stream::{Stream, StreamExt},
        task::{Context, Poll},
    },
    futures_test::task::noop_context,
    std::pin::Pin,
};

/// Single producer, single consumer
#[bench]
fn unbounded_1_tx(b: &mut Bencher) {
    let mut cx = noop_context();
    b.iter(|| {
        let (tx, mut rx) = mpsc::unbounded();

        // 1000 iterations to avoid measuring overhead of initialization
        // Result should be divided by 1000
        for i in 0..1000 {
            // Poll, not ready, park
            assert_eq!(Poll::Pending, rx.poll_next_unpin(&mut cx));

            UnboundedSender::unbounded_send(&tx, i).unwrap();

            // Now poll ready
            assert_eq!(Poll::Ready(Some(i)), rx.poll_next_unpin(&mut cx));
        }
    })
}

/// 100 producers, single consumer
#[bench]
fn unbounded_100_tx(b: &mut Bencher) {
    let mut cx = noop_context();
    b.iter(|| {
        let (tx, mut rx) = mpsc::unbounded();

        let tx: Vec<_> = (0..100).map(|_| tx.clone()).collect();

        // 1000 send/recv operations total, result should be divided by 1000
        for _ in 0..10 {
            for (i, x) in tx.iter().enumerate() {
                assert_eq!(Poll::Pending, rx.poll_next_unpin(&mut cx));

                UnboundedSender::unbounded_send(x, i).unwrap();

                assert_eq!(Poll::Ready(Some(i)), rx.poll_next_unpin(&mut cx));
            }
        }
    })
}

#[bench]
fn unbounded_uncontended(b: &mut Bencher) {
    let mut cx = noop_context();
    b.iter(|| {
        let (tx, mut rx) = mpsc::unbounded();

        for i in 0..1000 {
            UnboundedSender::unbounded_send(&tx, i).expect("send");
            // No need to create a task, because poll is not going to park.
            assert_eq!(Poll::Ready(Some(i)), rx.poll_next_unpin(&mut cx));
        }
    })
}

/// A Stream that continuously sends incrementing number of the queue
struct TestSender {
    tx: Sender<u32>,
    last: u32, // Last number sent
}

// Could be a Future, it doesn't matter
impl Stream for TestSender {
    type Item = u32;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        let mut tx = Pin::new(&mut this.tx);

        ready!(tx.as_mut().poll_ready(cx)).unwrap();
        tx.as_mut().start_send(this.last + 1).unwrap();
        this.last += 1;
        assert_eq!(Poll::Pending, tx.as_mut().poll_flush(cx));
        Poll::Ready(Some(this.last))
    }
}

/// Single producers, single consumer
#[bench]
fn bounded_1_tx(b: &mut Bencher) {
    let mut cx = noop_context();
    b.iter(|| {
        let (tx, mut rx) = mpsc::channel(0);

        let mut tx = TestSender { tx, last: 0 };

        for i in 0..1000 {
            assert_eq!(Poll::Ready(Some(i + 1)), tx.poll_next_unpin(&mut cx));
            assert_eq!(Poll::Pending, tx.poll_next_unpin(&mut cx));
            assert_eq!(Poll::Ready(Some(i + 1)), rx.poll_next_unpin(&mut cx));
        }
    })
}

/// 100 producers, single consumer
#[bench]
fn bounded_100_tx(b: &mut Bencher) {
    let mut cx = noop_context();
    b.iter(|| {
        // Each sender can send one item after specified capacity
        let (tx, mut rx) = mpsc::channel(0);

        let mut tx: Vec<_> = (0..100).map(|_| TestSender { tx: tx.clone(), last: 0 }).collect();

        for i in 0..10 {
            for x in &mut tx {
                // Send an item
                assert_eq!(Poll::Ready(Some(i + 1)), x.poll_next_unpin(&mut cx));
                // Then block
                assert_eq!(Poll::Pending, x.poll_next_unpin(&mut cx));
                // Recv the item
                assert_eq!(Poll::Ready(Some(i + 1)), rx.poll_next_unpin(&mut cx));
            }
        }
    })
}
