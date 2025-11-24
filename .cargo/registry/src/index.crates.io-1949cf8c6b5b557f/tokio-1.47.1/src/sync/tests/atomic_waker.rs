use crate::sync::AtomicWaker;
use tokio_test::task;

use std::task::Waker;

#[allow(unused)]
trait AssertSend: Send {}

#[allow(unused)]
trait AssertSync: Sync {}

impl AssertSend for AtomicWaker {}
impl AssertSync for AtomicWaker {}

impl AssertSend for Waker {}
impl AssertSync for Waker {}

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn basic_usage() {
    let mut waker = task::spawn(AtomicWaker::new());

    waker.enter(|cx, waker| waker.register_by_ref(cx.waker()));
    waker.wake();

    assert!(waker.is_woken());
}

#[test]
fn wake_without_register() {
    let mut waker = task::spawn(AtomicWaker::new());
    waker.wake();

    // Registering should not result in a notification
    waker.enter(|cx, waker| waker.register_by_ref(cx.waker()));

    assert!(!waker.is_woken());
}

#[test]
#[cfg_attr(target_family = "wasm", ignore)] // threads not supported
fn failed_wake_synchronizes() {
    for _ in 0..1000 {
        failed_wake_synchronizes_inner();
    }
}

fn failed_wake_synchronizes_inner() {
    use futures::task::noop_waker_ref;
    use std::sync::atomic::{AtomicBool, Ordering};
    static DID_SYNCHRONIZE: AtomicBool = AtomicBool::new(false);
    DID_SYNCHRONIZE.store(false, Ordering::Relaxed);

    let waker = AtomicWaker::new();
    waker.register_by_ref(noop_waker_ref());

    std::thread::scope(|s| {
        let jh = s.spawn(|| {
            DID_SYNCHRONIZE.store(true, Ordering::Relaxed);
            waker.take_waker()
        });

        waker.take_waker();
        waker.register_by_ref(noop_waker_ref());

        let did_synchronize = DID_SYNCHRONIZE.load(Ordering::Relaxed);
        let did_take = jh.join().unwrap().is_some();
        assert!(did_synchronize || did_take);
    });
}

#[cfg(panic = "unwind")]
#[test]
#[cfg(not(target_family = "wasm"))] // wasm currently doesn't support unwinding
fn atomic_waker_panic_safe() {
    use std::panic;
    use std::ptr;
    use std::task::{RawWaker, RawWakerVTable, Waker};

    static PANICKING_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| panic!("clone"),
        |_| unimplemented!("wake"),
        |_| unimplemented!("wake_by_ref"),
        |_| (),
    );

    static NONPANICKING_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RawWaker::new(ptr::null(), &NONPANICKING_VTABLE),
        |_| unimplemented!("wake"),
        |_| unimplemented!("wake_by_ref"),
        |_| (),
    );

    let panicking = unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &PANICKING_VTABLE)) };
    let nonpanicking = unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &NONPANICKING_VTABLE)) };

    let atomic_waker = AtomicWaker::new();

    let panicking = panic::AssertUnwindSafe(&panicking);

    let result = panic::catch_unwind(|| {
        let panic::AssertUnwindSafe(panicking) = panicking;
        atomic_waker.register_by_ref(panicking);
    });

    assert!(result.is_err());
    assert!(atomic_waker.take_waker().is_none());

    atomic_waker.register_by_ref(&nonpanicking);
    assert!(atomic_waker.take_waker().is_some());
}
