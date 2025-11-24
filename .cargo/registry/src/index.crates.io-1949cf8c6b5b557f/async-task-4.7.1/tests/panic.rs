use std::future::Future;
use std::panic::catch_unwind;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;

use async_task::Runnable;
use easy_parallel::Parallel;
use smol::future;

// Creates a future with event counters.
//
// Usage: `future!(f, POLL, DROP)`
//
// The future `f` sleeps for 200 ms and then panics.
// When it gets polled, `POLL` is incremented.
// When it gets dropped, `DROP` is incremented.
macro_rules! future {
    ($name:pat, $poll:ident, $drop:ident) => {
        static $poll: AtomicUsize = AtomicUsize::new(0);
        static $drop: AtomicUsize = AtomicUsize::new(0);

        let $name = {
            struct Fut(#[allow(dead_code)] Box<i32>);

            impl Future for Fut {
                type Output = ();

                fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                    $poll.fetch_add(1, Ordering::SeqCst);
                    thread::sleep(ms(400));
                    panic!()
                }
            }

            impl Drop for Fut {
                fn drop(&mut self) {
                    $drop.fetch_add(1, Ordering::SeqCst);
                }
            }

            Fut(Box::new(0))
        };
    };
}

// Creates a schedule function with event counters.
//
// Usage: `schedule!(s, SCHED, DROP)`
//
// The schedule function `s` does nothing.
// When it gets invoked, `SCHED` is incremented.
// When it gets dropped, `DROP` is incremented.
macro_rules! schedule {
    ($name:pat, $sched:ident, $drop:ident) => {
        static $drop: AtomicUsize = AtomicUsize::new(0);
        static $sched: AtomicUsize = AtomicUsize::new(0);

        let $name = {
            struct Guard(#[allow(dead_code)] Box<i32>);

            impl Drop for Guard {
                fn drop(&mut self) {
                    $drop.fetch_add(1, Ordering::SeqCst);
                }
            }

            let guard = Guard(Box::new(0));
            move |_runnable: Runnable| {
                let _ = &guard;
                $sched.fetch_add(1, Ordering::SeqCst);
            }
        };
    };
}

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn cancel_during_run() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    Parallel::new()
        .add(|| {
            assert!(catch_unwind(|| runnable.run()).is_err());
            assert_eq!(POLL.load(Ordering::SeqCst), 1);
            assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
            assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
        })
        .add(|| {
            thread::sleep(ms(200));

            drop(task);
            assert_eq!(POLL.load(Ordering::SeqCst), 1);
            assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_F.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_S.load(Ordering::SeqCst), 0);
        })
        .run();
}

#[test]
fn run_and_join() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    assert!(catch_unwind(|| runnable.run()).is_err());
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    assert!(catch_unwind(|| future::block_on(task)).is_err());
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
}

#[test]
fn try_join_and_run_and_join() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, mut task) = async_task::spawn(f, s);

    future::block_on(future::or(&mut task, future::ready(())));
    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    assert!(catch_unwind(|| runnable.run()).is_err());
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    assert!(catch_unwind(|| future::block_on(task)).is_err());
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
}

#[test]
fn join_during_run() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    Parallel::new()
        .add(|| {
            assert!(catch_unwind(|| runnable.run()).is_err());
            assert_eq!(POLL.load(Ordering::SeqCst), 1);
            assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_F.load(Ordering::SeqCst), 1);

            thread::sleep(ms(200));
            assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
        })
        .add(|| {
            thread::sleep(ms(200));

            assert!(catch_unwind(|| future::block_on(task)).is_err());
            assert_eq!(POLL.load(Ordering::SeqCst), 1);
            assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_F.load(Ordering::SeqCst), 1);

            thread::sleep(ms(200));
            assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
        })
        .run();
}

#[test]
fn try_join_during_run() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, mut task) = async_task::spawn(f, s);

    Parallel::new()
        .add(|| {
            assert!(catch_unwind(|| runnable.run()).is_err());
            assert_eq!(POLL.load(Ordering::SeqCst), 1);
            assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
            assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
        })
        .add(|| {
            thread::sleep(ms(200));

            future::block_on(future::or(&mut task, future::ready(())));
            assert_eq!(POLL.load(Ordering::SeqCst), 1);
            assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_F.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_S.load(Ordering::SeqCst), 0);
            drop(task);
        })
        .run();
}

#[test]
fn detach_during_run() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    Parallel::new()
        .add(|| {
            assert!(catch_unwind(|| runnable.run()).is_err());
            assert_eq!(POLL.load(Ordering::SeqCst), 1);
            assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
            assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
        })
        .add(|| {
            thread::sleep(ms(200));

            task.detach();
            assert_eq!(POLL.load(Ordering::SeqCst), 1);
            assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_F.load(Ordering::SeqCst), 0);
            assert_eq!(DROP_S.load(Ordering::SeqCst), 0);
        })
        .run();
}
