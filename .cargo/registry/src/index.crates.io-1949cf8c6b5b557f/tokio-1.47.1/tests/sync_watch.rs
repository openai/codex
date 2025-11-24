#![allow(clippy::cognitive_complexity)]
#![warn(rust_2018_idioms)]
#![cfg(feature = "sync")]

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use tokio::sync::watch;
use tokio_test::task::spawn;
use tokio_test::{
    assert_pending, assert_ready, assert_ready_eq, assert_ready_err, assert_ready_ok,
};

#[test]
fn single_rx_recv() {
    let (tx, mut rx) = watch::channel("one");

    {
        // Not initially notified
        let mut t = spawn(rx.changed());
        assert_pending!(t.poll());
    }
    assert_eq!(*rx.borrow(), "one");

    {
        let mut t = spawn(rx.changed());
        assert_pending!(t.poll());

        tx.send("two").unwrap();

        assert!(t.is_woken());

        assert_ready_ok!(t.poll());
    }
    assert_eq!(*rx.borrow(), "two");

    {
        let mut t = spawn(rx.changed());
        assert_pending!(t.poll());

        drop(tx);

        assert!(t.is_woken());
        assert_ready_err!(t.poll());
    }
    assert_eq!(*rx.borrow(), "two");
}

#[test]
fn rx_version_underflow() {
    let (_tx, mut rx) = watch::channel("one");

    // Version starts at 2, validate we do not underflow
    rx.mark_changed();
    rx.mark_changed();
}

#[test]
fn rx_mark_changed() {
    let (tx, mut rx) = watch::channel("one");

    let mut rx2 = rx.clone();
    let mut rx3 = rx.clone();
    let mut rx4 = rx.clone();
    {
        rx.mark_changed();
        assert!(rx.has_changed().unwrap());

        let mut t = spawn(rx.changed());
        assert_ready_ok!(t.poll());
    }

    {
        assert!(!rx2.has_changed().unwrap());

        let mut t = spawn(rx2.changed());
        assert_pending!(t.poll());
    }

    {
        rx3.mark_changed();
        assert_eq!(*rx3.borrow(), "one");

        assert!(rx3.has_changed().unwrap());

        assert_eq!(*rx3.borrow_and_update(), "one");

        assert!(!rx3.has_changed().unwrap());

        let mut t = spawn(rx3.changed());
        assert_pending!(t.poll());
    }

    {
        tx.send("two").unwrap();
        assert!(rx4.has_changed().unwrap());
        assert_eq!(*rx4.borrow_and_update(), "two");

        rx4.mark_changed();
        assert!(rx4.has_changed().unwrap());
        assert_eq!(*rx4.borrow_and_update(), "two")
    }

    assert_eq!(*rx.borrow(), "two");
}

#[test]
fn rx_mark_unchanged() {
    let (tx, mut rx) = watch::channel("one");

    let mut rx2 = rx.clone();

    {
        assert!(!rx.has_changed().unwrap());

        rx.mark_changed();
        assert!(rx.has_changed().unwrap());

        rx.mark_unchanged();
        assert!(!rx.has_changed().unwrap());

        let mut t = spawn(rx.changed());
        assert_pending!(t.poll());
    }

    {
        assert!(!rx2.has_changed().unwrap());

        tx.send("two").unwrap();
        assert!(rx2.has_changed().unwrap());

        rx2.mark_unchanged();
        assert!(!rx2.has_changed().unwrap());
        assert_eq!(*rx2.borrow_and_update(), "two");
    }

    assert_eq!(*rx.borrow(), "two");
}

#[test]
fn multi_rx() {
    let (tx, mut rx1) = watch::channel("one");
    let mut rx2 = rx1.clone();

    {
        let mut t1 = spawn(rx1.changed());
        let mut t2 = spawn(rx2.changed());

        assert_pending!(t1.poll());
        assert_pending!(t2.poll());
    }
    assert_eq!(*rx1.borrow(), "one");
    assert_eq!(*rx2.borrow(), "one");

    let mut t2 = spawn(rx2.changed());

    {
        let mut t1 = spawn(rx1.changed());

        assert_pending!(t1.poll());
        assert_pending!(t2.poll());

        tx.send("two").unwrap();

        assert!(t1.is_woken());
        assert!(t2.is_woken());

        assert_ready_ok!(t1.poll());
    }
    assert_eq!(*rx1.borrow(), "two");

    {
        let mut t1 = spawn(rx1.changed());

        assert_pending!(t1.poll());

        tx.send("three").unwrap();

        assert!(t1.is_woken());
        assert!(t2.is_woken());

        assert_ready_ok!(t1.poll());
        assert_ready_ok!(t2.poll());
    }
    assert_eq!(*rx1.borrow(), "three");

    drop(t2);

    assert_eq!(*rx2.borrow(), "three");

    {
        let mut t1 = spawn(rx1.changed());
        let mut t2 = spawn(rx2.changed());

        assert_pending!(t1.poll());
        assert_pending!(t2.poll());

        tx.send("four").unwrap();

        assert_ready_ok!(t1.poll());
        assert_ready_ok!(t2.poll());
    }
    assert_eq!(*rx1.borrow(), "four");
    assert_eq!(*rx2.borrow(), "four");
}

#[test]
fn rx_observes_final_value() {
    // Initial value

    let (tx, mut rx) = watch::channel("one");
    drop(tx);

    {
        let mut t1 = spawn(rx.changed());
        assert_ready_err!(t1.poll());
    }
    assert_eq!(*rx.borrow(), "one");

    // Sending a value

    let (tx, mut rx) = watch::channel("one");

    tx.send("two").unwrap();

    {
        let mut t1 = spawn(rx.changed());
        assert_ready_ok!(t1.poll());
    }
    assert_eq!(*rx.borrow(), "two");

    {
        let mut t1 = spawn(rx.changed());
        assert_pending!(t1.poll());

        tx.send("three").unwrap();
        drop(tx);

        assert!(t1.is_woken());

        assert_ready_ok!(t1.poll());
    }
    assert_eq!(*rx.borrow(), "three");

    {
        let mut t1 = spawn(rx.changed());
        assert_ready_err!(t1.poll());
    }
    assert_eq!(*rx.borrow(), "three");
}

#[test]
fn poll_close() {
    let (tx, rx) = watch::channel("one");

    {
        let mut t = spawn(tx.closed());
        assert_pending!(t.poll());

        drop(rx);

        assert!(t.is_woken());
        assert_ready!(t.poll());
    }

    assert!(tx.send("two").is_err());
}

#[test]
fn borrow_and_update() {
    let (tx, mut rx) = watch::channel("one");

    assert!(!rx.has_changed().unwrap());

    tx.send("two").unwrap();
    assert!(rx.has_changed().unwrap());
    assert_ready!(spawn(rx.changed()).poll()).unwrap();
    assert_pending!(spawn(rx.changed()).poll());
    assert!(!rx.has_changed().unwrap());

    tx.send("three").unwrap();
    assert!(rx.has_changed().unwrap());
    assert_eq!(*rx.borrow_and_update(), "three");
    assert_pending!(spawn(rx.changed()).poll());
    assert!(!rx.has_changed().unwrap());

    drop(tx);
    assert_eq!(*rx.borrow_and_update(), "three");
    assert_ready!(spawn(rx.changed()).poll()).unwrap_err();
    assert!(rx.has_changed().is_err());
}

#[test]
fn reopened_after_subscribe() {
    let (tx, rx) = watch::channel("one");
    assert!(!tx.is_closed());

    drop(rx);
    assert!(tx.is_closed());

    let rx = tx.subscribe();
    assert!(!tx.is_closed());

    drop(rx);
    assert!(tx.is_closed());
}

#[test]
#[cfg(panic = "unwind")]
#[cfg(not(target_family = "wasm"))] // wasm currently doesn't support unwinding
fn send_modify_panic() {
    let (tx, mut rx) = watch::channel("one");

    tx.send_modify(|old| *old = "two");
    assert_eq!(*rx.borrow_and_update(), "two");

    let mut rx2 = rx.clone();
    assert_eq!(*rx2.borrow_and_update(), "two");

    let mut task = spawn(rx2.changed());

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tx.send_modify(|old| {
            *old = "panicked";
            panic!();
        })
    }));
    assert!(result.is_err());

    assert_pending!(task.poll());
    assert_eq!(*rx.borrow(), "panicked");

    tx.send_modify(|old| *old = "three");
    assert_ready_ok!(task.poll());
    assert_eq!(*rx.borrow_and_update(), "three");
}

#[tokio::test]
async fn multiple_sender() {
    let (tx1, mut rx) = watch::channel(0);
    let tx2 = tx1.clone();

    let mut t = spawn(async {
        rx.changed().await.unwrap();
        let v1 = *rx.borrow_and_update();
        rx.changed().await.unwrap();
        let v2 = *rx.borrow_and_update();
        (v1, v2)
    });

    tx1.send(1).unwrap();
    assert_pending!(t.poll());
    tx2.send(2).unwrap();
    assert_ready_eq!(t.poll(), (1, 2));
}

#[tokio::test]
async fn receiver_is_notified_when_last_sender_is_dropped() {
    let (tx1, mut rx) = watch::channel(0);
    let tx2 = tx1.clone();

    let mut t = spawn(rx.changed());
    assert_pending!(t.poll());

    drop(tx1);
    assert!(!t.is_woken());
    drop(tx2);

    assert!(t.is_woken());
}

#[tokio::test]
async fn receiver_changed_is_cooperative() {
    let (tx, mut rx) = watch::channel(());

    drop(tx);

    tokio::select! {
        biased;
        _ = async {
            loop {
                assert!(rx.changed().await.is_err());
            }
        } => {},
        _ = tokio::task::yield_now() => {},
    }
}

#[tokio::test]
async fn receiver_changed_is_cooperative_ok() {
    let (tx, mut rx) = watch::channel(());

    tokio::select! {
        biased;
        _ = async {
            loop {
                assert!(tx.send(()).is_ok());
                assert!(rx.changed().await.is_ok());
            }
        } => {},
        _ = tokio::task::yield_now() => {},
    }
}

#[tokio::test]
async fn receiver_wait_for_is_cooperative() {
    let (tx, mut rx) = watch::channel(0);

    drop(tx);

    tokio::select! {
        biased;
        _ = async {
            loop {
                assert!(rx.wait_for(|val| *val == 1).await.is_err());
            }
        } => {},
        _ = tokio::task::yield_now() => {},
    }
}

#[tokio::test]
async fn receiver_wait_for_is_cooperative_ok() {
    let (tx, mut rx) = watch::channel(0);

    tokio::select! {
        biased;
        _ = async {
            loop {
                assert!(tx.send(1).is_ok());
                assert!(rx.wait_for(|val| *val == 1).await.is_ok());
            }
        } => {},
        _ = tokio::task::yield_now() => {},
    }
}

#[tokio::test]
async fn sender_closed_is_cooperative() {
    let (tx, rx) = watch::channel(());

    drop(rx);

    tokio::select! {
        _ = async {
            loop {
                tx.closed().await;
            }
        } => {},
        _ = tokio::task::yield_now() => {},
    }
}
