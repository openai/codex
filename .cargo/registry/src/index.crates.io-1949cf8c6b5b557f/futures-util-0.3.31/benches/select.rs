#![feature(test)]

extern crate test;
use crate::test::Bencher;

use futures::executor::block_on;
use futures::stream::{repeat, select, StreamExt};

#[bench]
fn select_streams(b: &mut Bencher) {
    const STREAM_COUNT: usize = 10_000;

    b.iter(|| {
        let stream1 = repeat(1).take(STREAM_COUNT);
        let stream2 = repeat(2).take(STREAM_COUNT);
        let stream3 = repeat(3).take(STREAM_COUNT);
        let stream4 = repeat(4).take(STREAM_COUNT);
        let stream5 = repeat(5).take(STREAM_COUNT);
        let stream6 = repeat(6).take(STREAM_COUNT);
        let stream7 = repeat(7).take(STREAM_COUNT);
        let count = block_on(async {
            let count = select(
                stream1,
                select(
                    stream2,
                    select(stream3, select(stream4, select(stream5, select(stream6, stream7)))),
                ),
            )
            .count()
            .await;
            count
        });
        assert_eq!(count, STREAM_COUNT * 7);
    });
}
