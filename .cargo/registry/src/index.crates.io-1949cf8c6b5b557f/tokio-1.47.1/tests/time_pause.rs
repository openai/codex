#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]
#![cfg(not(miri))] // Too slow on miri.

use rand::SeedableRng;
use rand::{rngs::StdRng, Rng};
use tokio::time::{self, Duration, Instant, Sleep};
use tokio_test::{assert_elapsed, assert_pending, assert_ready, assert_ready_eq, task};

#[cfg(not(target_os = "wasi"))]
use tokio_test::assert_err;

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

#[tokio::test]
async fn pause_time_in_main() {
    tokio::time::pause();
}

#[tokio::test]
async fn pause_time_in_task() {
    let t = tokio::spawn(async {
        tokio::time::pause();
    });

    t.await.unwrap();
}

#[cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[should_panic]
async fn pause_time_in_main_threads() {
    tokio::time::pause();
}

#[cfg_attr(panic = "abort", ignore)]
#[cfg(all(feature = "full", not(target_os = "wasi")))] // Wasi doesn't support threads
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pause_time_in_spawn_threads() {
    let t = tokio::spawn(async {
        tokio::time::pause();
    });

    assert_err!(t.await);
}

#[test]
fn paused_time_is_deterministic() {
    let run_1 = paused_time_stress_run();
    let run_2 = paused_time_stress_run();

    assert_eq!(run_1, run_2);
}

#[tokio::main(flavor = "current_thread", start_paused = true)]
async fn paused_time_stress_run() -> Vec<Duration> {
    let mut rng = StdRng::seed_from_u64(1);

    let mut times = vec![];
    let start = Instant::now();
    for _ in 0..10_000 {
        let sleep = rng.random_range(Duration::from_secs(0)..Duration::from_secs(1));
        time::sleep(sleep).await;
        times.push(start.elapsed());
    }

    times
}

#[tokio::test(start_paused = true)]
async fn advance_after_poll() {
    time::sleep(ms(1)).await;

    let start = Instant::now();

    let mut sleep = task::spawn(time::sleep_until(start + ms(300)));

    assert_pending!(sleep.poll());

    let before = Instant::now();
    time::advance(ms(100)).await;
    assert_elapsed!(before, ms(100));

    assert_pending!(sleep.poll());
}

#[tokio::test(start_paused = true)]
async fn sleep_no_poll() {
    let start = Instant::now();

    // TODO: Skip this
    time::advance(ms(1)).await;

    let mut sleep = task::spawn(time::sleep_until(start + ms(300)));

    let before = Instant::now();
    time::advance(ms(100)).await;
    assert_elapsed!(before, ms(100));

    assert_pending!(sleep.poll());
}

enum State {
    Begin,
    AwaitingAdvance(Pin<Box<dyn Future<Output = ()>>>),
    AfterAdvance,
}

struct Tester {
    sleep: Pin<Box<Sleep>>,
    state: State,
    before: Option<Instant>,
    poll: bool,
}

impl Future for Tester {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.state {
            State::Begin => {
                if self.poll {
                    assert_pending!(self.sleep.as_mut().poll(cx));
                }
                self.before = Some(Instant::now());
                let advance_fut = Box::pin(time::advance(ms(100)));
                self.state = State::AwaitingAdvance(advance_fut);
                self.poll(cx)
            }
            State::AwaitingAdvance(ref mut advance_fut) => match advance_fut.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => {
                    self.state = State::AfterAdvance;
                    self.poll(cx)
                }
            },
            State::AfterAdvance => {
                assert_elapsed!(self.before.unwrap(), ms(100));

                assert_pending!(self.sleep.as_mut().poll(cx));

                Poll::Ready(())
            }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn sleep_same_task() {
    let start = Instant::now();

    // TODO: Skip this
    time::advance(ms(1)).await;

    let sleep = Box::pin(time::sleep_until(start + ms(300)));

    Tester {
        sleep,
        state: State::Begin,
        before: None,
        poll: true,
    }
    .await;
}

#[tokio::test(start_paused = true)]
async fn sleep_same_task_no_poll() {
    let start = Instant::now();

    // TODO: Skip this
    time::advance(ms(1)).await;

    let sleep = Box::pin(time::sleep_until(start + ms(300)));

    Tester {
        sleep,
        state: State::Begin,
        before: None,
        poll: false,
    }
    .await;
}

#[tokio::test(start_paused = true)]
async fn interval() {
    let start = Instant::now();

    // TODO: Skip this
    time::advance(ms(1)).await;

    let mut i = task::spawn(time::interval_at(start, ms(300)));

    assert_ready_eq!(poll_next(&mut i), start);
    assert_pending!(poll_next(&mut i));

    let before = Instant::now();
    time::advance(ms(100)).await;
    assert_elapsed!(before, ms(100));
    assert_pending!(poll_next(&mut i));

    let before = Instant::now();
    time::advance(ms(200)).await;
    assert_elapsed!(before, ms(200));
    assert_ready_eq!(poll_next(&mut i), start + ms(300));
    assert_pending!(poll_next(&mut i));

    let before = Instant::now();
    time::advance(ms(400)).await;
    assert_elapsed!(before, ms(400));
    assert_ready_eq!(poll_next(&mut i), start + ms(600));
    assert_pending!(poll_next(&mut i));

    let before = Instant::now();
    time::advance(ms(500)).await;
    assert_elapsed!(before, ms(500));
    assert_ready_eq!(poll_next(&mut i), start + ms(900));
    assert_ready_eq!(poll_next(&mut i), start + ms(1200));
    assert_pending!(poll_next(&mut i));
}

#[tokio::test(start_paused = true)]
async fn test_time_advance_sub_ms() {
    let now = Instant::now();

    let dur = Duration::from_micros(51_592);
    time::advance(dur).await;

    assert_eq!(now.elapsed(), dur);

    let now = Instant::now();
    let dur = Duration::from_micros(1);
    time::advance(dur).await;

    assert_eq!(now.elapsed(), dur);
}

#[tokio::test(start_paused = true)]
async fn test_time_advance_3ms_and_change() {
    let now = Instant::now();

    let dur = Duration::from_micros(3_141_592);
    time::advance(dur).await;

    assert_eq!(now.elapsed(), dur);

    let now = Instant::now();
    let dur = Duration::from_micros(3_123_456);
    time::advance(dur).await;

    assert_eq!(now.elapsed(), dur);
}

#[tokio::test(start_paused = true)]
async fn regression_3710_with_submillis_advance() {
    let start = Instant::now();

    time::advance(Duration::from_millis(1)).await;

    let mut sleep = task::spawn(time::sleep_until(start + Duration::from_secs(60)));

    assert_pending!(sleep.poll());

    let before = Instant::now();
    let dur = Duration::from_micros(51_592);
    time::advance(dur).await;
    assert_eq!(before.elapsed(), dur);

    assert_pending!(sleep.poll());
}

#[tokio::test(start_paused = true)]
async fn exact_1ms_advance() {
    let now = Instant::now();

    let dur = Duration::from_millis(1);
    time::advance(dur).await;

    assert_eq!(now.elapsed(), dur);

    let now = Instant::now();
    let dur = Duration::from_millis(1);
    time::advance(dur).await;

    assert_eq!(now.elapsed(), dur);
}

#[tokio::test(start_paused = true)]
async fn advance_once_with_timer() {
    let mut sleep = task::spawn(time::sleep(Duration::from_millis(1)));
    assert_pending!(sleep.poll());

    time::advance(Duration::from_micros(250)).await;
    assert_pending!(sleep.poll());

    time::advance(Duration::from_micros(1500)).await;

    assert!(sleep.is_woken());
    assert_ready!(sleep.poll());
}

#[tokio::test(start_paused = true)]
async fn advance_multi_with_timer() {
    // Round to the nearest ms
    // time::sleep(Duration::from_millis(1)).await;

    let mut sleep = task::spawn(time::sleep(Duration::from_millis(1)));
    assert_pending!(sleep.poll());

    time::advance(Duration::from_micros(250)).await;
    assert_pending!(sleep.poll());

    time::advance(Duration::from_micros(250)).await;
    assert_pending!(sleep.poll());

    time::advance(Duration::from_micros(250)).await;
    assert_pending!(sleep.poll());

    time::advance(Duration::from_micros(250)).await;
    assert!(sleep.is_woken());
    assert_ready!(sleep.poll());
}

fn poll_next(interval: &mut task::Spawn<time::Interval>) -> Poll<Instant> {
    interval.enter(|cx, mut interval| interval.poll_tick(cx))
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
