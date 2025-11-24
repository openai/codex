#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(not(miri))] // Too slow on Miri.

use std::future::Future;
use std::task::Context;

use futures::task::noop_waker_ref;

use tokio::time::{self, Duration, Instant};
use tokio_test::{assert_elapsed, assert_pending, assert_ready, task};

#[tokio::test]
async fn immediate_sleep() {
    time::pause();

    let now = Instant::now();

    // Ready!
    time::sleep_until(now).await;
    assert_elapsed!(now, ms(1));
}

#[tokio::test]
async fn is_elapsed() {
    time::pause();

    let sleep = time::sleep(Duration::from_millis(10));

    tokio::pin!(sleep);

    assert!(!sleep.is_elapsed());

    assert!(futures::poll!(sleep.as_mut()).is_pending());

    assert!(!sleep.is_elapsed());

    sleep.as_mut().await;

    assert!(sleep.is_elapsed());
}

#[tokio::test]
async fn delayed_sleep_level_0() {
    time::pause();

    for &i in &[1, 10, 60] {
        let now = Instant::now();
        let dur = ms(i);

        time::sleep_until(now + dur).await;

        assert_elapsed!(now, dur);
    }
}

#[tokio::test]
async fn sub_ms_delayed_sleep() {
    time::pause();

    for _ in 0..5 {
        let now = Instant::now();
        let deadline = now + ms(1) + Duration::new(0, 1);

        time::sleep_until(deadline).await;

        assert_elapsed!(now, ms(1));
    }
}

#[tokio::test]
async fn delayed_sleep_wrapping_level_0() {
    time::pause();

    time::sleep(ms(5)).await;

    let now = Instant::now();
    time::sleep_until(now + ms(60)).await;

    assert_elapsed!(now, ms(60));
}

#[tokio::test]
async fn reset_future_sleep_before_fire() {
    time::pause();

    let now = Instant::now();

    let mut sleep = task::spawn(Box::pin(time::sleep_until(now + ms(100))));
    assert_pending!(sleep.poll());

    let mut sleep = sleep.into_inner();

    sleep.as_mut().reset(Instant::now() + ms(200));
    sleep.await;

    assert_elapsed!(now, ms(200));
}

#[tokio::test]
async fn reset_past_sleep_before_turn() {
    time::pause();

    let now = Instant::now();

    let mut sleep = task::spawn(Box::pin(time::sleep_until(now + ms(100))));
    assert_pending!(sleep.poll());

    let mut sleep = sleep.into_inner();

    sleep.as_mut().reset(now + ms(80));
    sleep.await;

    assert_elapsed!(now, ms(80));
}

#[tokio::test]
async fn reset_past_sleep_before_fire() {
    time::pause();

    let now = Instant::now();

    let mut sleep = task::spawn(Box::pin(time::sleep_until(now + ms(100))));
    assert_pending!(sleep.poll());

    let mut sleep = sleep.into_inner();

    time::sleep(ms(10)).await;

    sleep.as_mut().reset(now + ms(80));
    sleep.await;

    assert_elapsed!(now, ms(80));
}

#[tokio::test]
async fn reset_future_sleep_after_fire() {
    time::pause();

    let now = Instant::now();
    let mut sleep = Box::pin(time::sleep_until(now + ms(100)));

    sleep.as_mut().await;
    assert_elapsed!(now, ms(100));

    sleep.as_mut().reset(now + ms(110));
    sleep.await;
    assert_elapsed!(now, ms(110));
}

#[tokio::test]
async fn reset_sleep_to_past() {
    time::pause();

    let now = Instant::now();

    let mut sleep = task::spawn(Box::pin(time::sleep_until(now + ms(100))));
    assert_pending!(sleep.poll());

    time::sleep(ms(50)).await;

    assert!(!sleep.is_woken());

    sleep.as_mut().reset(now + ms(40));

    // TODO: is this required?
    //assert!(sleep.is_woken());

    assert_ready!(sleep.poll());
}

#[cfg(not(target_os = "wasi"))] // Wasi doesn't support panic recovery
#[test]
#[should_panic]
fn creating_sleep_outside_of_context() {
    let now = Instant::now();

    // This creates a delay outside of the context of a mock timer. This tests
    // that it will panic.
    let _fut = time::sleep_until(now + ms(500));
}

#[tokio::test]
async fn greater_than_max() {
    const YR_5: u64 = 5 * 365 * 24 * 60 * 60 * 1000;

    time::pause();
    time::sleep_until(Instant::now() + ms(YR_5)).await;
}

#[tokio::test]
async fn short_sleeps() {
    for _ in 0..1000 {
        tokio::time::sleep(std::time::Duration::from_millis(0)).await;
    }
}

#[tokio::test]
async fn multi_long_sleeps() {
    tokio::time::pause();

    for _ in 0..5u32 {
        tokio::time::sleep(Duration::from_secs(
            // about a year
            365 * 24 * 3600,
        ))
        .await;
    }

    let deadline = tokio::time::Instant::now()
        + Duration::from_secs(
            // about 10 years
            10 * 365 * 24 * 3600,
        );

    tokio::time::sleep_until(deadline).await;

    assert!(tokio::time::Instant::now() >= deadline);
}

#[tokio::test]
async fn long_sleeps() {
    tokio::time::pause();

    let deadline = tokio::time::Instant::now()
        + Duration::from_secs(
            // about 10 years
            10 * 365 * 24 * 3600,
        );

    tokio::time::sleep_until(deadline).await;

    assert!(tokio::time::Instant::now() >= deadline);
    assert!(tokio::time::Instant::now() <= deadline + Duration::from_millis(1));
}

#[tokio::test]
async fn reset_after_firing() {
    let timer = tokio::time::sleep(std::time::Duration::from_millis(1));
    tokio::pin!(timer);

    let deadline = timer.deadline();

    timer.as_mut().await;
    assert_ready!(timer
        .as_mut()
        .poll(&mut Context::from_waker(noop_waker_ref())));
    timer
        .as_mut()
        .reset(tokio::time::Instant::now() + std::time::Duration::from_secs(600));

    assert_ne!(deadline, timer.deadline());

    assert_pending!(timer
        .as_mut()
        .poll(&mut Context::from_waker(noop_waker_ref())));
    assert_pending!(timer
        .as_mut()
        .poll(&mut Context::from_waker(noop_waker_ref())));
}

#[tokio::test]
async fn exactly_max() {
    time::pause();
    time::sleep(Duration::MAX).await;
}

#[tokio::test]
async fn issue_5183() {
    time::pause();

    let big = std::time::Duration::from_secs(u64::MAX / 10);
    // This is a workaround since awaiting sleep(big) will never finish.
    #[rustfmt::skip]
    tokio::select! {
	biased;
        _ = tokio::time::sleep(big) => {}
        _ = tokio::time::sleep(std::time::Duration::from_nanos(1)) => {}
    }
}

#[tokio::test]
async fn no_out_of_bounds_close_to_max() {
    time::pause();
    time::sleep(Duration::MAX - Duration::from_millis(1)).await;
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[tokio::test]
async fn drop_after_reschedule_at_new_scheduled_time() {
    use futures::poll;

    tokio::time::pause();

    let start = tokio::time::Instant::now();

    let mut a = Box::pin(tokio::time::sleep(Duration::from_millis(5)));
    let mut b = Box::pin(tokio::time::sleep(Duration::from_millis(5)));
    let mut c = Box::pin(tokio::time::sleep(Duration::from_millis(10)));

    let _ = poll!(&mut a);
    let _ = poll!(&mut b);
    let _ = poll!(&mut c);

    b.as_mut().reset(start + Duration::from_millis(10));
    a.await;

    drop(b);
}

#[tokio::test]
async fn drop_from_wake() {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::Context;

    let panicked = Arc::new(AtomicBool::new(false));
    let list: Arc<Mutex<Vec<Pin<Box<tokio::time::Sleep>>>>> = Arc::new(Mutex::new(Vec::new()));

    let arc_wake = Arc::new(DropWaker(panicked.clone(), list.clone()));
    let arc_wake = futures::task::waker(arc_wake);

    tokio::time::pause();

    {
        let mut lock = list.lock().unwrap();

        for _ in 0..100 {
            let mut timer = Box::pin(tokio::time::sleep(Duration::from_millis(10)));

            let _ = timer.as_mut().poll(&mut Context::from_waker(&arc_wake));

            lock.push(timer);
        }
    }

    tokio::time::sleep(Duration::from_millis(11)).await;

    assert!(
        !panicked.load(Ordering::SeqCst),
        "panicked when dropping timers"
    );

    #[derive(Clone)]
    struct DropWaker(
        Arc<AtomicBool>,
        Arc<Mutex<Vec<Pin<Box<tokio::time::Sleep>>>>>,
    );

    impl futures::task::ArcWake for DropWaker {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                *arc_self.1.lock().expect("panic in lock") = Vec::new()
            }));

            if result.is_err() {
                arc_self.0.store(true, Ordering::SeqCst);
            }
        }
    }
}
