#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use tokio::sync::oneshot;
use tokio::time::{self, timeout, timeout_at, Instant};
use tokio_test::*;

use futures::future::pending;
use std::time::Duration;

#[tokio::test]
async fn simultaneous_deadline_future_completion() {
    // Create a future that is immediately ready
    let mut fut = task::spawn(timeout_at(Instant::now(), async {}));

    // Ready!
    assert_ready_ok!(fut.poll());
}

#[cfg_attr(target_os = "wasi", ignore = "FIXME: `fut.poll()` panics on Wasi")]
#[tokio::test]
async fn completed_future_past_deadline() {
    // Wrap it with a deadline
    let mut fut = task::spawn(timeout_at(Instant::now() - ms(1000), async {}));

    // Ready!
    assert_ready_ok!(fut.poll());
}

#[tokio::test]
async fn future_and_deadline_in_future() {
    time::pause();

    // Not yet complete
    let (tx, rx) = oneshot::channel();

    // Wrap it with a deadline
    let mut fut = task::spawn(timeout_at(Instant::now() + ms(100), rx));

    assert_pending!(fut.poll());

    // Turn the timer, it runs for the elapsed time
    time::advance(ms(90)).await;

    assert_pending!(fut.poll());

    // Complete the future
    tx.send(()).unwrap();
    assert!(fut.is_woken());

    assert_ready_ok!(fut.poll()).unwrap();
}

#[tokio::test]
async fn future_and_timeout_in_future() {
    time::pause();

    // Not yet complete
    let (tx, rx) = oneshot::channel();

    // Wrap it with a deadline
    let mut fut = task::spawn(timeout(ms(100), rx));

    // Ready!
    assert_pending!(fut.poll());

    // Turn the timer, it runs for the elapsed time
    time::advance(ms(90)).await;

    assert_pending!(fut.poll());

    // Complete the future
    tx.send(()).unwrap();

    assert_ready_ok!(fut.poll()).unwrap();
}

#[tokio::test]
async fn very_large_timeout() {
    time::pause();

    // Not yet complete
    let (tx, rx) = oneshot::channel();

    // copy-paste unstable `Duration::MAX`
    let duration_max = Duration::from_secs(u64::MAX) + Duration::from_nanos(999_999_999);

    // Wrap it with a deadline
    let mut fut = task::spawn(timeout(duration_max, rx));

    // Ready!
    assert_pending!(fut.poll());

    // Turn the timer, it runs for the elapsed time
    time::advance(Duration::from_secs(86400 * 365 * 10)).await;

    assert_pending!(fut.poll());

    // Complete the future
    tx.send(()).unwrap();

    assert_ready_ok!(fut.poll()).unwrap();
}

#[tokio::test]
async fn deadline_now_elapses() {
    use futures::future::pending;

    time::pause();

    // Wrap it with a deadline
    let mut fut = task::spawn(timeout_at(Instant::now(), pending::<()>()));

    // Factor in jitter
    // TODO: don't require this
    time::advance(ms(1)).await;

    assert_ready_err!(fut.poll());
}

#[tokio::test]
async fn deadline_future_elapses() {
    time::pause();

    // Wrap it with a deadline
    let mut fut = task::spawn(timeout_at(Instant::now() + ms(300), pending::<()>()));

    assert_pending!(fut.poll());

    time::advance(ms(301)).await;

    assert!(fut.is_woken());
    assert_ready_err!(fut.poll());
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[tokio::test]
async fn timeout_is_not_exhausted_by_future() {
    let fut = timeout(ms(1), async {
        let mut buffer = [0u8; 1];
        loop {
            use tokio::io::AsyncReadExt;
            let _ = tokio::io::empty().read(&mut buffer).await;
        }
    });

    assert!(fut.await.is_err());
}
