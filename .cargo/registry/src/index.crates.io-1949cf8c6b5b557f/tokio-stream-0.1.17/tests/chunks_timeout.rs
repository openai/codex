#![warn(rust_2018_idioms)]
#![cfg(all(feature = "time", feature = "sync", feature = "io-util"))]

use tokio::time;
use tokio_stream::{self as stream, StreamExt};
use tokio_test::assert_pending;
use tokio_test::task;

use futures::FutureExt;
use std::time::Duration;

#[tokio::test(start_paused = true)]
async fn usage() {
    let iter = vec![1, 2, 3].into_iter();
    let stream0 = stream::iter(iter);

    let iter = vec![4].into_iter();
    let stream1 =
        stream::iter(iter).then(move |n| time::sleep(Duration::from_secs(3)).map(move |_| n));

    let chunk_stream = stream0
        .chain(stream1)
        .chunks_timeout(4, Duration::from_secs(2));

    let mut chunk_stream = task::spawn(chunk_stream);

    assert_pending!(chunk_stream.poll_next());
    time::advance(Duration::from_secs(2)).await;
    assert_eq!(chunk_stream.next().await, Some(vec![1, 2, 3]));

    assert_pending!(chunk_stream.poll_next());
    time::advance(Duration::from_secs(2)).await;
    assert_eq!(chunk_stream.next().await, Some(vec![4]));
}

#[tokio::test(start_paused = true)]
async fn full_chunk_with_timeout() {
    let iter = vec![1, 2].into_iter();
    let stream0 = stream::iter(iter);

    let iter = vec![3].into_iter();
    let stream1 =
        stream::iter(iter).then(move |n| time::sleep(Duration::from_secs(1)).map(move |_| n));

    let iter = vec![4].into_iter();
    let stream2 =
        stream::iter(iter).then(move |n| time::sleep(Duration::from_secs(3)).map(move |_| n));

    let chunk_stream = stream0
        .chain(stream1)
        .chain(stream2)
        .chunks_timeout(3, Duration::from_secs(2));

    let mut chunk_stream = task::spawn(chunk_stream);

    assert_pending!(chunk_stream.poll_next());
    time::advance(Duration::from_secs(2)).await;
    assert_eq!(chunk_stream.next().await, Some(vec![1, 2, 3]));

    assert_pending!(chunk_stream.poll_next());
    time::advance(Duration::from_secs(2)).await;
    assert_eq!(chunk_stream.next().await, Some(vec![4]));
}

#[tokio::test]
#[ignore]
async fn real_time() {
    let iter = vec![1, 2, 3, 4].into_iter();
    let stream0 = stream::iter(iter);

    let iter = vec![5].into_iter();
    let stream1 =
        stream::iter(iter).then(move |n| time::sleep(Duration::from_secs(5)).map(move |_| n));

    let chunk_stream = stream0
        .chain(stream1)
        .chunks_timeout(3, Duration::from_secs(2));

    let mut chunk_stream = task::spawn(chunk_stream);

    assert_eq!(chunk_stream.next().await, Some(vec![1, 2, 3]));
    assert_eq!(chunk_stream.next().await, Some(vec![4]));
    assert_eq!(chunk_stream.next().await, Some(vec![5]));
}
