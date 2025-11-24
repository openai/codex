use futures::executor::block_on;
use futures::future::poll_fn;
use futures::task::{AtomicWaker, Poll};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;

#[test]
fn basic() {
    let atomic_waker = Arc::new(AtomicWaker::new());
    let atomic_waker_copy = atomic_waker.clone();

    let returned_pending = Arc::new(AtomicUsize::new(0));
    let returned_pending_copy = returned_pending.clone();

    let woken = Arc::new(AtomicUsize::new(0));
    let woken_copy = woken.clone();

    let t = thread::spawn(move || {
        let mut pending_count = 0;

        block_on(poll_fn(move |cx| {
            if woken_copy.load(Ordering::Relaxed) == 1 {
                Poll::Ready(())
            } else {
                // Assert we return pending exactly once
                assert_eq!(0, pending_count);
                pending_count += 1;
                atomic_waker_copy.register(cx.waker());

                returned_pending_copy.store(1, Ordering::Relaxed);

                Poll::Pending
            }
        }))
    });

    while returned_pending.load(Ordering::Relaxed) == 0 {}

    // give spawned thread some time to sleep in `block_on`
    thread::yield_now();

    woken.store(1, Ordering::Relaxed);
    atomic_waker.wake();

    t.join().unwrap();
}
