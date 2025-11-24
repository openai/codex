#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Stream, StreamExt};
use tokio::time::{self, Duration, Instant, Interval, MissedTickBehavior};
use tokio_test::{assert_pending, assert_ready, assert_ready_eq, task};

// Takes the `Interval` task, `start` variable, and optional time deltas
// For each time delta, it polls the `Interval` and asserts that the result is
// equal to `start` + the specific time delta. Then it asserts that the
// `Interval` is pending.
macro_rules! check_interval_poll {
    ($i:ident, $start:ident, $($delta:expr),*$(,)?) => {
        $(
            assert_ready_eq!(poll_next(&mut $i), $start + ms($delta));
        )*
        assert_pending!(poll_next(&mut $i));
    };
    ($i:ident, $start:ident) => {
        check_interval_poll!($i, $start,);
    };
}

#[tokio::test]
#[should_panic]
async fn interval_zero_duration() {
    let _ = time::interval_at(Instant::now(), ms(0));
}

// Expected ticks: |     1     |     2     |     3     |     4     |     5     |     6     |
// Actual ticks:   | work -----|          delay          | work | work | work -| work -----|
// Poll behavior:  |   |       |                         |      |      |       |           |
//                 |   |       |                         |      |      |       |           |
//          Ready(s)   |       |             Ready(s + 2p)      |      |       |           |
//               Pending       |                    Ready(s + 3p)      |       |           |
//                  Ready(s + p)                           Ready(s + 4p)       |           |
//                                                                 Ready(s + 5p)           |
//                                                                             Ready(s + 6p)
#[tokio::test(start_paused = true)]
async fn burst() {
    let start = Instant::now();

    // This is necessary because the timer is only so granular, and in order for
    // all our ticks to resolve, the time needs to be 1ms ahead of what we
    // expect, so that the runtime will see that it is time to resolve the timer
    time::advance(ms(1)).await;

    let mut i = task::spawn(time::interval_at(start, ms(300)));

    check_interval_poll!(i, start, 0);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 300);

    time::advance(ms(650)).await;
    check_interval_poll!(i, start, 600, 900);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start, 1200);

    time::advance(ms(250)).await;
    check_interval_poll!(i, start, 1500);

    time::advance(ms(300)).await;
    check_interval_poll!(i, start, 1800);
}

// Expected ticks: |     1     |     2     |     3     |     4     |     5     |     6     |
// Actual ticks:   | work -----|          delay          | work -----| work -----| work -----|
// Poll behavior:  |   |       |                         |   |       |           |           |
//                 |   |       |                         |   |       |           |           |
//          Ready(s)   |       |             Ready(s + 2p)   |       |           |           |
//               Pending       |                       Pending       |           |           |
//                  Ready(s + p)                     Ready(s + 2p + d)           |           |
//                                                               Ready(s + 3p + d)           |
//                                                                           Ready(s + 4p + d)
#[tokio::test(start_paused = true)]
async fn delay() {
    let start = Instant::now();

    // This is necessary because the timer is only so granular, and in order for
    // all our ticks to resolve, the time needs to be 1ms ahead of what we
    // expect, so that the runtime will see that it is time to resolve the timer
    time::advance(ms(1)).await;

    let mut i = task::spawn(time::interval_at(start, ms(300)));
    i.set_missed_tick_behavior(MissedTickBehavior::Delay);

    check_interval_poll!(i, start, 0);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 300);

    time::advance(ms(650)).await;
    check_interval_poll!(i, start, 600);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    // We have to add one here for the same reason as is above.
    // Because `Interval` has reset its timer according to `Instant::now()`,
    // we have to go forward 1 more millisecond than is expected so that the
    // runtime realizes that it's time to resolve the timer.
    time::advance(ms(201)).await;
    // We add one because when using the `Delay` behavior, `Interval`
    // adds the `period` from `Instant::now()`, which will always be off by one
    // because we have to advance time by 1 (see above).
    check_interval_poll!(i, start, 1251);

    time::advance(ms(300)).await;
    // Again, we add one.
    check_interval_poll!(i, start, 1551);

    time::advance(ms(300)).await;
    check_interval_poll!(i, start, 1851);
}

// Expected ticks: |     1     |     2     |     3     |     4     |     5     |     6     |
// Actual ticks:   | work -----|          delay          | work ---| work -----| work -----|
// Poll behavior:  |   |       |                         |         |           |           |
//                 |   |       |                         |         |           |           |
//          Ready(s)   |       |             Ready(s + 2p)         |           |           |
//               Pending       |                       Ready(s + 4p)           |           |
//                  Ready(s + p)                                   Ready(s + 5p)           |
//                                                                             Ready(s + 6p)
#[tokio::test(start_paused = true)]
async fn skip() {
    let start = Instant::now();

    // This is necessary because the timer is only so granular, and in order for
    // all our ticks to resolve, the time needs to be 1ms ahead of what we
    // expect, so that the runtime will see that it is time to resolve the timer
    time::advance(ms(1)).await;

    let mut i = task::spawn(time::interval_at(start, ms(300)));
    i.set_missed_tick_behavior(MissedTickBehavior::Skip);

    check_interval_poll!(i, start, 0);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 300);

    time::advance(ms(650)).await;
    check_interval_poll!(i, start, 600);

    time::advance(ms(250)).await;
    check_interval_poll!(i, start, 1200);

    time::advance(ms(300)).await;
    check_interval_poll!(i, start, 1500);

    time::advance(ms(300)).await;
    check_interval_poll!(i, start, 1800);
}

#[tokio::test(start_paused = true)]
async fn reset() {
    let start = Instant::now();

    // This is necessary because the timer is only so granular, and in order for
    // all our ticks to resolve, the time needs to be 1ms ahead of what we
    // expect, so that the runtime will see that it is time to resolve the timer
    time::advance(ms(1)).await;

    let mut i = task::spawn(time::interval_at(start, ms(300)));

    check_interval_poll!(i, start, 0);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 300);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    i.reset();

    time::advance(ms(250)).await;
    check_interval_poll!(i, start);

    time::advance(ms(50)).await;
    // We add one because when using `reset` method, `Interval` adds the
    // `period` from `Instant::now()`, which will always be off by one
    check_interval_poll!(i, start, 701);

    time::advance(ms(300)).await;
    check_interval_poll!(i, start, 1001);
}

#[tokio::test(start_paused = true)]
async fn reset_immediately() {
    let start = Instant::now();

    // This is necessary because the timer is only so granular, and in order for
    // all our ticks to resolve, the time needs to be 1ms ahead of what we
    // expect, so that the runtime will see that it is time to resolve the timer
    time::advance(ms(1)).await;

    let mut i = task::spawn(time::interval_at(start, ms(300)));

    check_interval_poll!(i, start, 0);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 300);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    i.reset_immediately();

    // We add one because when using `reset` method, `Interval` adds the
    // `period` from `Instant::now()`, which will always be off by one
    check_interval_poll!(i, start, 401);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 701);
}

#[tokio::test(start_paused = true)]
async fn reset_after() {
    let start = Instant::now();

    // This is necessary because the timer is only so granular, and in order for
    // all our ticks to resolve, the time needs to be 1ms ahead of what we
    // expect, so that the runtime will see that it is time to resolve the timer
    time::advance(ms(1)).await;

    let mut i = task::spawn(time::interval_at(start, ms(300)));

    check_interval_poll!(i, start, 0);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 300);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    i.reset_after(Duration::from_millis(20));

    // We add one because when using `reset` method, `Interval` adds the
    // `period` from `Instant::now()`, which will always be off by one
    time::advance(ms(20)).await;
    check_interval_poll!(i, start, 421);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 721);
}

#[tokio::test(start_paused = true)]
async fn reset_at() {
    let start = Instant::now();

    // This is necessary because the timer is only so granular, and in order for
    // all our ticks to resolve, the time needs to be 1ms ahead of what we
    // expect, so that the runtime will see that it is time to resolve the timer
    time::advance(ms(1)).await;

    let mut i = task::spawn(time::interval_at(start, ms(300)));

    check_interval_poll!(i, start, 0);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 300);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    i.reset_at(Instant::now() + Duration::from_millis(40));

    // We add one because when using `reset` method, `Interval` adds the
    // `period` from `Instant::now()`, which will always be off by one
    time::advance(ms(40)).await;
    check_interval_poll!(i, start, 441);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 741);
}

#[tokio::test(start_paused = true)]
async fn reset_at_bigger_than_interval() {
    let start = Instant::now();

    // This is necessary because the timer is only so granular, and in order for
    // all our ticks to resolve, the time needs to be 1ms ahead of what we
    // expect, so that the runtime will see that it is time to resolve the timer
    time::advance(ms(1)).await;

    let mut i = task::spawn(time::interval_at(start, ms(300)));

    check_interval_poll!(i, start, 0);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    time::advance(ms(200)).await;
    check_interval_poll!(i, start, 300);

    time::advance(ms(100)).await;
    check_interval_poll!(i, start);

    i.reset_at(Instant::now() + Duration::from_millis(1000));

    // Validate the interval does not tick until 1000ms have passed
    time::advance(ms(300)).await;
    check_interval_poll!(i, start);
    time::advance(ms(300)).await;
    check_interval_poll!(i, start);
    time::advance(ms(300)).await;
    check_interval_poll!(i, start);

    // We add one because when using `reset` method, `Interval` adds the
    // `period` from `Instant::now()`, which will always be off by one
    time::advance(ms(100)).await;
    check_interval_poll!(i, start, 1401);

    time::advance(ms(300)).await;
    check_interval_poll!(i, start, 1701);
}

fn poll_next(interval: &mut task::Spawn<time::Interval>) -> Poll<Instant> {
    interval.enter(|cx, mut interval| interval.poll_tick(cx))
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// Helper struct to test the [tokio::time::Interval::poll_tick()] method.
///
/// `poll_tick()` should register the waker in the context only if it returns
/// `Poll::Pending`, not when returning `Poll::Ready`. This struct contains an
/// interval timer and counts up on every tick when used as stream. When the
/// counter is a multiple of four, it yields the current counter value.
/// Depending on the value for `wake_on_pending`, it will reschedule itself when
/// it returns `Poll::Pending` or not. When used with `wake_on_pending=false`,
/// we expect that the stream stalls because the timer will **not** reschedule
/// the next wake-up itself once it returned `Poll::Ready`.
struct IntervalStreamer {
    counter: u32,
    timer: Interval,
    wake_on_pending: bool,
}

impl Stream for IntervalStreamer {
    type Item = u32;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = Pin::into_inner(self);

        if this.counter > 12 {
            return Poll::Ready(None);
        }

        match this.timer.poll_tick(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => {
                this.counter += 1;
                if this.counter % 4 == 0 {
                    Poll::Ready(Some(this.counter))
                } else {
                    if this.wake_on_pending {
                        // Schedule this task for wake-up
                        cx.waker().wake_by_ref();
                    }
                    Poll::Pending
                }
            }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn stream_with_interval_poll_tick_self_waking() {
    let stream = IntervalStreamer {
        counter: 0,
        timer: tokio::time::interval(tokio::time::Duration::from_millis(10)),
        wake_on_pending: true,
    };

    let (res_tx, mut res_rx) = tokio::sync::mpsc::channel(12);

    // Wrap task in timeout so that it will finish eventually even if the stream
    // stalls.
    tokio::spawn(tokio::time::timeout(
        tokio::time::Duration::from_millis(150),
        async move {
            tokio::pin!(stream);

            while let Some(item) = stream.next().await {
                res_tx.send(item).await.ok();
            }
        },
    ));

    let mut items = Vec::with_capacity(3);
    while let Some(result) = res_rx.recv().await {
        items.push(result);
    }

    // We expect the stream to yield normally and thus three items.
    assert_eq!(items, vec![4, 8, 12]);
}

#[tokio::test(start_paused = true)]
async fn stream_with_interval_poll_tick_no_waking() {
    let stream = IntervalStreamer {
        counter: 0,
        timer: tokio::time::interval(tokio::time::Duration::from_millis(10)),
        wake_on_pending: false,
    };

    let (res_tx, mut res_rx) = tokio::sync::mpsc::channel(12);

    // Wrap task in timeout so that it will finish eventually even if the stream
    // stalls.
    tokio::spawn(tokio::time::timeout(
        tokio::time::Duration::from_millis(150),
        async move {
            tokio::pin!(stream);

            while let Some(item) = stream.next().await {
                res_tx.send(item).await.ok();
            }
        },
    ));

    let mut items = Vec::with_capacity(0);
    while let Some(result) = res_rx.recv().await {
        items.push(result);
    }

    // We expect the stream to stall because it does not reschedule itself on
    // `Poll::Pending` and neither does [tokio::time::Interval] reschedule the
    // task when returning `Poll::Ready`.
    assert_eq!(items, vec![]);
}

#[tokio::test(start_paused = true)]
async fn interval_doesnt_panic_max_duration_when_polling() {
    let mut timer = task::spawn(time::interval(Duration::MAX));
    assert_ready!(timer.enter(|cx, mut timer| timer.poll_tick(cx)));
}
