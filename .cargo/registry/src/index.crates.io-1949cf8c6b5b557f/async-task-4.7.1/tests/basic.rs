use std::future::Future;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use async_task::Runnable;
use smol::future;

// Creates a future with event counters.
//
// Usage: `future!(f, POLL, DROP)`
//
// The future `f` always returns `Poll::Ready`.
// When it gets polled, `POLL` is incremented.
// When it gets dropped, `DROP` is incremented.
macro_rules! future {
    ($name:pat, $poll:ident, $drop:ident) => {
        static $poll: AtomicUsize = AtomicUsize::new(0);
        static $drop: AtomicUsize = AtomicUsize::new(0);

        let $name = {
            struct Fut(#[allow(dead_code)] Box<i32>);

            impl Future for Fut {
                type Output = Box<i32>;

                fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                    $poll.fetch_add(1, Ordering::SeqCst);
                    Poll::Ready(Box::new(0))
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
            move |_runnable| {
                let _ = &guard;
                $sched.fetch_add(1, Ordering::SeqCst);
            }
        };
    };
}

fn try_await<T>(f: impl Future<Output = T>) -> Option<T> {
    future::block_on(future::poll_once(f))
}

#[test]
fn drop_and_detach() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    drop(runnable);
    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    task.detach();
    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
}

#[test]
fn detach_and_drop() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    task.detach();
    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    drop(runnable);
    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
}

#[test]
fn detach_and_run() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    task.detach();
    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    runnable.run();
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
}

#[test]
fn run_and_detach() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    runnable.run();
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    task.detach();
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
}

#[test]
fn cancel_and_run() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    drop(task);
    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    runnable.run();
    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
}

#[test]
fn run_and_cancel() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, task) = async_task::spawn(f, s);

    runnable.run();
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    drop(task);
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
}

#[test]
fn cancel_join() {
    future!(f, POLL, DROP_F);
    schedule!(s, SCHEDULE, DROP_S);
    let (runnable, mut task) = async_task::spawn(f, s);

    assert!(try_await(&mut task).is_none());
    assert_eq!(POLL.load(Ordering::SeqCst), 0);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    runnable.run();
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    assert!(try_await(&mut task).is_some());
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 0);

    drop(task);
    assert_eq!(POLL.load(Ordering::SeqCst), 1);
    assert_eq!(SCHEDULE.load(Ordering::SeqCst), 0);
    assert_eq!(DROP_F.load(Ordering::SeqCst), 1);
    assert_eq!(DROP_S.load(Ordering::SeqCst), 1);
}

#[test]
fn schedule() {
    let (s, r) = flume::unbounded();
    let schedule = move |runnable| s.send(runnable).unwrap();
    let (runnable, _task) = async_task::spawn(future::poll_fn(|_| Poll::<()>::Pending), schedule);

    assert!(r.is_empty());
    runnable.schedule();

    let runnable = r.recv().unwrap();
    assert!(r.is_empty());
    runnable.schedule();

    let runnable = r.recv().unwrap();
    assert!(r.is_empty());
    runnable.schedule();

    r.recv().unwrap();
}

#[test]
fn schedule_counter() {
    static COUNT: AtomicUsize = AtomicUsize::new(0);

    let (s, r) = flume::unbounded();
    let schedule = move |runnable: Runnable| {
        COUNT.fetch_add(1, Ordering::SeqCst);
        s.send(runnable).unwrap();
    };
    let (runnable, _task) = async_task::spawn(future::poll_fn(|_| Poll::<()>::Pending), schedule);
    runnable.schedule();

    r.recv().unwrap().schedule();
    r.recv().unwrap().schedule();
    assert_eq!(COUNT.load(Ordering::SeqCst), 3);
    r.recv().unwrap();
}

#[test]
fn drop_inside_schedule() {
    struct DropGuard(AtomicUsize);
    impl Drop for DropGuard {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let guard = DropGuard(AtomicUsize::new(0));

    let (runnable, _) = async_task::spawn(async {}, move |runnable| {
        assert_eq!(guard.0.load(Ordering::SeqCst), 0);
        drop(runnable);
        assert_eq!(guard.0.load(Ordering::SeqCst), 0);
    });
    runnable.schedule();
}

#[test]
fn waker() {
    let (s, r) = flume::unbounded();
    let schedule = move |runnable| s.send(runnable).unwrap();
    let (runnable, _task) = async_task::spawn(future::poll_fn(|_| Poll::<()>::Pending), schedule);

    assert!(r.is_empty());
    let waker = runnable.waker();
    runnable.run();
    waker.wake_by_ref();

    let runnable = r.recv().unwrap();
    runnable.run();
    waker.wake();
    r.recv().unwrap();
}

#[test]
fn raw() {
    // Dispatch schedules a function for execution at a later point. For tests, we execute it straight away.
    fn dispatch(trampoline: extern "C" fn(NonNull<()>), context: NonNull<()>) {
        trampoline(context)
    }
    extern "C" fn trampoline(runnable: NonNull<()>) {
        let task = unsafe { Runnable::<()>::from_raw(runnable) };
        task.run();
    }

    let task_got_executed = Arc::new(AtomicBool::new(false));
    let (runnable, _handle) = async_task::spawn(
        {
            let task_got_executed = task_got_executed.clone();
            async move { task_got_executed.store(true, Ordering::SeqCst) }
        },
        |runnable: Runnable<()>| dispatch(trampoline, runnable.into_raw()),
    );
    runnable.schedule();

    assert!(task_got_executed.load(Ordering::SeqCst));
}
