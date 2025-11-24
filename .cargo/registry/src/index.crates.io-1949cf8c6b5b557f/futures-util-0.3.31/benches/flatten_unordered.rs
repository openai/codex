#![feature(test)]

extern crate test;
use crate::test::Bencher;

use futures::channel::oneshot;
use futures::executor::block_on;
use futures::future;
use futures::stream::{self, StreamExt};
use futures::task::Poll;
use futures_util::FutureExt;
use std::collections::VecDeque;
use std::thread;

#[bench]
fn oneshot_streams(b: &mut Bencher) {
    const STREAM_COUNT: usize = 10_000;
    const STREAM_ITEM_COUNT: usize = 1;

    b.iter(|| {
        let mut txs = VecDeque::with_capacity(STREAM_COUNT);
        let mut rxs = Vec::new();

        for _ in 0..STREAM_COUNT {
            let (tx, rx) = oneshot::channel();
            txs.push_back(tx);
            rxs.push(rx);
        }

        thread::spawn(move || {
            let mut last = 1;
            while let Some(tx) = txs.pop_front() {
                let _ = tx.send(stream::iter(last..last + STREAM_ITEM_COUNT));
                last += STREAM_ITEM_COUNT;
            }
        });

        let mut flatten = stream::iter(rxs)
            .map(|recv| recv.into_stream().map(|val| val.unwrap()).flatten())
            .flatten_unordered(None);

        block_on(future::poll_fn(move |cx| {
            let mut count = 0;
            loop {
                match flatten.poll_next_unpin(cx) {
                    Poll::Ready(None) => break,
                    Poll::Ready(Some(_)) => {
                        count += 1;
                    }
                    _ => {}
                }
            }
            assert_eq!(count, STREAM_COUNT * STREAM_ITEM_COUNT);

            Poll::Ready(())
        }))
    });
}
