#![cfg(all(feature = "time", feature = "sync", feature = "io-util"))]

use tokio::time::{self, sleep, Duration};
use tokio_stream::StreamExt;
use tokio_test::*;

use futures::stream;

async fn maybe_sleep(idx: i32) -> i32 {
    if idx % 2 == 0 {
        sleep(ms(200)).await;
    }
    idx
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[tokio::test]
async fn basic_usage() {
    time::pause();

    // Items 2 and 4 time out. If we run the stream until it completes,
    // we end up with the following items:
    //
    // [Ok(1), Err(Elapsed), Ok(2), Ok(3), Err(Elapsed), Ok(4)]

    let stream = stream::iter(1..=4).then(maybe_sleep).timeout(ms(100));
    let mut stream = task::spawn(stream);

    // First item completes immediately
    assert_ready_eq!(stream.poll_next(), Some(Ok(1)));

    // Second item is delayed 200ms, times out after 100ms
    assert_pending!(stream.poll_next());

    time::advance(ms(150)).await;
    let v = assert_ready!(stream.poll_next());
    assert!(v.unwrap().is_err());

    assert_pending!(stream.poll_next());

    time::advance(ms(100)).await;
    assert_ready_eq!(stream.poll_next(), Some(Ok(2)));

    // Third item is ready immediately
    assert_ready_eq!(stream.poll_next(), Some(Ok(3)));

    // Fourth item is delayed 200ms, times out after 100ms
    assert_pending!(stream.poll_next());

    time::advance(ms(60)).await;
    assert_pending!(stream.poll_next()); // nothing ready yet

    time::advance(ms(60)).await;
    let v = assert_ready!(stream.poll_next());
    assert!(v.unwrap().is_err()); // timeout!

    time::advance(ms(120)).await;
    assert_ready_eq!(stream.poll_next(), Some(Ok(4)));

    // Done.
    assert_ready_eq!(stream.poll_next(), None);
}

#[tokio::test]
async fn return_elapsed_errors_only_once() {
    time::pause();

    let stream = stream::iter(1..=3).then(maybe_sleep).timeout(ms(50));
    let mut stream = task::spawn(stream);

    // First item completes immediately
    assert_ready_eq!(stream.poll_next(), Some(Ok(1)));

    // Second item is delayed 200ms, times out after 50ms. Only one `Elapsed`
    // error is returned.
    assert_pending!(stream.poll_next());
    //
    time::advance(ms(51)).await;
    let v = assert_ready!(stream.poll_next());
    assert!(v.unwrap().is_err()); // timeout!

    // deadline elapses again, but no error is returned
    time::advance(ms(50)).await;
    assert_pending!(stream.poll_next());

    time::advance(ms(100)).await;
    assert_ready_eq!(stream.poll_next(), Some(Ok(2)));
    assert_ready_eq!(stream.poll_next(), Some(Ok(3)));

    // Done
    assert_ready_eq!(stream.poll_next(), None);
}

#[tokio::test]
async fn no_timeouts() {
    let stream = stream::iter(vec![1, 3, 5])
        .then(maybe_sleep)
        .timeout(ms(100));

    let mut stream = task::spawn(stream);

    assert_ready_eq!(stream.poll_next(), Some(Ok(1)));
    assert_ready_eq!(stream.poll_next(), Some(Ok(3)));
    assert_ready_eq!(stream.poll_next(), Some(Ok(5)));
    assert_ready_eq!(stream.poll_next(), None);
}
