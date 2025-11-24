#[cfg(not(miri))]
use std::mem;
use std::panic::catch_unwind;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::task::{Poll, Waker};

use async_executor::{Executor, Task};
use futures_lite::future;
use once_cell::sync::Lazy;

#[test]
fn executor_cancels_everything() {
    static DROP: AtomicUsize = AtomicUsize::new(0);
    static WAKER: Lazy<Mutex<Option<Waker>>> = Lazy::new(Default::default);

    let ex = Executor::new();

    let task = ex.spawn(async {
        let _guard = CallOnDrop(|| {
            DROP.fetch_add(1, Ordering::SeqCst);
        });

        future::poll_fn(|cx| {
            *WAKER.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending::<()>
        })
        .await;
    });

    future::block_on(ex.tick());
    assert!(WAKER.lock().unwrap().is_some());
    assert_eq!(DROP.load(Ordering::SeqCst), 0);

    drop(ex);
    assert_eq!(DROP.load(Ordering::SeqCst), 1);

    assert!(catch_unwind(|| future::block_on(task)).is_err());
    assert_eq!(DROP.load(Ordering::SeqCst), 1);
}

#[cfg(not(miri))]
#[test]
fn leaked_executor_leaks_everything() {
    static DROP: AtomicUsize = AtomicUsize::new(0);
    static WAKER: Lazy<Mutex<Option<Waker>>> = Lazy::new(Default::default);

    let ex = Executor::new();

    let task = ex.spawn(async {
        let _guard = CallOnDrop(|| {
            DROP.fetch_add(1, Ordering::SeqCst);
        });

        future::poll_fn(|cx| {
            *WAKER.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending::<()>
        })
        .await;
    });

    future::block_on(ex.tick());
    assert!(WAKER.lock().unwrap().is_some());
    assert_eq!(DROP.load(Ordering::SeqCst), 0);

    mem::forget(ex);
    assert_eq!(DROP.load(Ordering::SeqCst), 0);

    assert!(future::block_on(future::poll_once(task)).is_none());
    assert_eq!(DROP.load(Ordering::SeqCst), 0);
}

#[test]
fn await_task_after_dropping_executor() {
    let s: String = "hello".into();

    let ex = Executor::new();
    let task: Task<&str> = ex.spawn(async { &*s });
    assert!(ex.try_tick());

    drop(ex);
    assert_eq!(future::block_on(task), "hello");
    drop(s);
}

#[test]
fn drop_executor_and_then_drop_finished_task() {
    static DROP: AtomicUsize = AtomicUsize::new(0);

    let ex = Executor::new();
    let task = ex.spawn(async {
        CallOnDrop(|| {
            DROP.fetch_add(1, Ordering::SeqCst);
        })
    });
    assert!(ex.try_tick());

    assert_eq!(DROP.load(Ordering::SeqCst), 0);
    drop(ex);
    assert_eq!(DROP.load(Ordering::SeqCst), 0);
    drop(task);
    assert_eq!(DROP.load(Ordering::SeqCst), 1);
}

#[test]
fn drop_finished_task_and_then_drop_executor() {
    static DROP: AtomicUsize = AtomicUsize::new(0);

    let ex = Executor::new();
    let task = ex.spawn(async {
        CallOnDrop(|| {
            DROP.fetch_add(1, Ordering::SeqCst);
        })
    });
    assert!(ex.try_tick());

    assert_eq!(DROP.load(Ordering::SeqCst), 0);
    drop(task);
    assert_eq!(DROP.load(Ordering::SeqCst), 1);
    drop(ex);
    assert_eq!(DROP.load(Ordering::SeqCst), 1);
}

#[test]
fn iterator_panics_mid_run() {
    let ex = Executor::new();

    let panic = std::panic::catch_unwind(|| {
        let mut handles = vec![];
        ex.spawn_many(
            (0..50).map(|i| if i == 25 { panic!() } else { future::ready(i) }),
            &mut handles,
        )
    });
    assert!(panic.is_err());

    let task = ex.spawn(future::ready(0));
    assert_eq!(future::block_on(ex.run(task)), 0);
}

struct CallOnDrop<F: Fn()>(F);

impl<F: Fn()> Drop for CallOnDrop<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}
