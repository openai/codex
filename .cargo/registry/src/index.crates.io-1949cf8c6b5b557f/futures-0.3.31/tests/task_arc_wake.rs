use futures::task::{self, ArcWake, Waker};
use std::panic;
use std::sync::{Arc, Mutex};

struct CountingWaker {
    nr_wake: Mutex<i32>,
}

impl CountingWaker {
    fn new() -> Self {
        Self { nr_wake: Mutex::new(0) }
    }

    fn wakes(&self) -> i32 {
        *self.nr_wake.lock().unwrap()
    }
}

impl ArcWake for CountingWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        let mut lock = arc_self.nr_wake.lock().unwrap();
        *lock += 1;
    }
}

#[test]
fn create_from_arc() {
    let some_w = Arc::new(CountingWaker::new());

    let w1: Waker = task::waker(some_w.clone());
    assert_eq!(2, Arc::strong_count(&some_w));
    w1.wake_by_ref();
    assert_eq!(1, some_w.wakes());

    let w2 = w1.clone();
    assert_eq!(3, Arc::strong_count(&some_w));

    w2.wake_by_ref();
    assert_eq!(2, some_w.wakes());

    drop(w2);
    assert_eq!(2, Arc::strong_count(&some_w));
    drop(w1);
    assert_eq!(1, Arc::strong_count(&some_w));
}

#[test]
fn ref_wake_same() {
    let some_w = Arc::new(CountingWaker::new());

    let w1: Waker = task::waker(some_w.clone());
    let w2 = task::waker_ref(&some_w);
    let w3 = w2.clone();

    assert!(w1.will_wake(&w2));
    assert!(w2.will_wake(&w3));
}

#[test]
fn proper_refcount_on_wake_panic() {
    struct PanicWaker;

    impl ArcWake for PanicWaker {
        fn wake_by_ref(_arc_self: &Arc<Self>) {
            panic!("WAKE UP");
        }
    }

    let some_w = Arc::new(PanicWaker);

    let w1: Waker = task::waker(some_w.clone());
    assert_eq!(
        "WAKE UP",
        *panic::catch_unwind(|| w1.wake_by_ref()).unwrap_err().downcast::<&str>().unwrap()
    );
    assert_eq!(2, Arc::strong_count(&some_w)); // some_w + w1
    drop(w1);
    assert_eq!(1, Arc::strong_count(&some_w)); // some_w
}
