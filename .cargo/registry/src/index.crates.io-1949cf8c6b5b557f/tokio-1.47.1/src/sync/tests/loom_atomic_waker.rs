use crate::sync::task::AtomicWaker;

use loom::future::block_on;
use loom::sync::atomic::AtomicUsize;
use loom::thread;
use std::future::poll_fn;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::task::Poll::{Pending, Ready};

struct Chan {
    num: AtomicUsize,
    task: AtomicWaker,
}

#[test]
fn basic_notification() {
    const NUM_NOTIFY: usize = 2;

    loom::model(|| {
        let chan = Arc::new(Chan {
            num: AtomicUsize::new(0),
            task: AtomicWaker::new(),
        });

        for _ in 0..NUM_NOTIFY {
            let chan = chan.clone();

            thread::spawn(move || {
                chan.num.fetch_add(1, Relaxed);
                chan.task.wake();
            });
        }

        block_on(poll_fn(move |cx| {
            chan.task.register_by_ref(cx.waker());

            if NUM_NOTIFY == chan.num.load(Relaxed) {
                return Ready(());
            }

            Pending
        }));
    });
}

#[test]
fn test_panicky_waker() {
    use std::panic;
    use std::ptr;
    use std::task::{RawWaker, RawWakerVTable, Waker};

    static PANICKING_VTABLE: RawWakerVTable =
        RawWakerVTable::new(|_| panic!("clone"), |_| (), |_| (), |_| ());

    let panicking = unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &PANICKING_VTABLE)) };

    // If you're working with this test (and I sure hope you never have to!),
    // uncomment the following section because there will be a lot of panics
    // which would otherwise log.
    //
    // We can't however leaved it uncommented, because it's global.
    // panic::set_hook(Box::new(|_| ()));

    const NUM_NOTIFY: usize = 2;

    loom::model(move || {
        let chan = Arc::new(Chan {
            num: AtomicUsize::new(0),
            task: AtomicWaker::new(),
        });

        for _ in 0..NUM_NOTIFY {
            let chan = chan.clone();

            thread::spawn(move || {
                chan.num.fetch_add(1, Relaxed);
                chan.task.wake();
            });
        }

        // Note: this panic should have no effect on the overall state of the
        // waker and it should proceed as normal.
        //
        // A thread above might race to flag a wakeup, and a WAKING state will
        // be preserved if this expected panic races with that so the below
        // procedure should be allowed to continue uninterrupted.
        let _ = panic::catch_unwind(|| chan.task.register_by_ref(&panicking));

        block_on(poll_fn(move |cx| {
            chan.task.register_by_ref(cx.waker());

            if NUM_NOTIFY == chan.num.load(Relaxed) {
                return Ready(());
            }

            Pending
        }));
    });
}
