use std::future::Future;
use std::task::{Context, Poll, Waker};
use std::thread::sleep;
use std::time::{Duration, Instant};

use easy_parallel::Parallel;
use parking::Parker;

#[test]
fn park_timeout_unpark_before() {
    let (p, u) = parking::pair();
    for _ in 0..10 {
        u.unpark();
        p.park_timeout(Duration::from_millis(u32::MAX as u64));
    }
}

#[test]
fn park_timeout_unpark_not_called() {
    let p = Parker::new();
    for _ in 0..10 {
        p.park_timeout(Duration::from_millis(10));
    }
}

#[test]
fn park_timeout_unpark_called_other_thread() {
    for _ in 0..10 {
        let p = Parker::new();
        let u = p.unparker();

        Parallel::new()
            .add(move || {
                sleep(Duration::from_millis(50));
                u.unpark();
            })
            .add(move || {
                p.park_timeout(Duration::from_millis(u32::MAX as u64));
            })
            .run();
    }
}

#[test]
fn park_deadline_unpark_called_other_thread() {
    for _ in 0..10 {
        let p = Parker::new();
        let u = p.unparker();

        Parallel::new()
            .add(move || {
                sleep(Duration::from_millis(50));
                u.unpark();
            })
            .add(move || {
                let deadline = Instant::now() + Duration::from_micros(u32::MAX as u64);
                p.park_deadline(deadline);
            })
            .run();
    }
}

#[test]
fn park_then_wake_from_other_thread() {
    for _ in 0..10 {
        let (p, u) = parking::pair();

        Parallel::new()
            .add(move || {
                sleep(Duration::from_millis(50));
                u.unpark();
            })
            .add(move || {
                let start = Instant::now();
                p.park();
                assert!(Instant::now().duration_since(start) >= Duration::from_millis(50));
            })
            .run();
    }
}

#[test]
fn unpark() {
    let p = Parker::default();

    assert!(p.unpark());
    assert!(!p.unpark());
}

#[test]
fn same_parker() {
    let (p1, u1) = parking::pair();
    let (p2, u2) = parking::pair();

    assert!(u1.will_unpark(&p1));
    assert!(!u1.will_unpark(&p2));
    assert!(u1.same_parker(&u1.clone()));
    assert!(!u1.same_parker(&u2));
}

#[test]
fn waker() {
    let (p, u) = parking::pair();
    let waker: Waker = u.into();

    waker.wake();
    assert!(p.park_timeout(Duration::from_secs(2)));
}

#[test]
fn future() {
    struct Yield(bool);

    impl Future for Yield {
        type Output = ();

        fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.0 {
                Poll::Ready(())
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }

    let (p, u) = parking::pair();
    let waker = u.into();
    let mut context = Context::from_waker(&waker);

    let mut future = Box::pin(Yield(false));

    assert!(!p.park_timeout(Duration::from_millis(0)));
    assert_eq!(future.as_mut().poll(&mut context), Poll::Pending);
    assert!(p.park_timeout(Duration::from_millis(0)));
    assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(()));
    assert!(!p.park_timeout(Duration::from_millis(0)));
}

#[test]
fn debug_for_coverage() {
    let (p, u) = parking::pair();
    let _ = format!("{:?} {:?}", &p, &u);
}
