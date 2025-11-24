#![allow(clippy::redundant_clone)]
#![warn(rust_2018_idioms)]
#![cfg(feature = "sync")]

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use tokio::sync::broadcast::{self, channel};

#[tokio::test]
async fn weak_sender() {
    let (tx, mut rx) = channel(11);

    let tx_weak = tokio::spawn(async move {
        let tx_weak = tx.clone().downgrade();

        for i in 0..10 {
            if tx.send(i).is_err() {
                return None;
            }
        }

        let tx2 = tx_weak
            .upgrade()
            .expect("expected to be able to upgrade tx_weak");
        let _ = tx2.send(20);
        let tx_weak = tx2.downgrade();

        Some(tx_weak)
    })
    .await
    .unwrap();

    for i in 0..12 {
        let recvd = rx.recv().await;

        match recvd {
            Ok(msg) => {
                if i == 10 {
                    assert_eq!(msg, 20);
                }
            }
            Err(_) => {
                assert_eq!(i, 11);
                break;
            }
        }
    }

    let tx_weak = tx_weak.unwrap();
    let upgraded = tx_weak.upgrade();
    assert!(upgraded.is_none());
}

// Tests that a `WeakSender` fails to upgrade when no other `Sender` exists.
#[test]
fn downgrade_upgrade_sender_failure() {
    let (tx, _rx) = broadcast::channel::<i32>(1);
    let weak_tx = tx.downgrade();
    drop(tx);
    assert!(weak_tx.upgrade().is_none());
}

// Tests that a `WeakSender` cannot be upgraded after a `Sender` was dropped,
// which existed at the time of the `downgrade` call.
#[test]
fn downgrade_drop_upgrade() {
    let (tx, _rx) = broadcast::channel::<i32>(1);

    // the cloned `Tx` is dropped right away
    let weak_tx = tx.clone().downgrade();
    drop(tx);
    assert!(weak_tx.upgrade().is_none());
}

// Tests that `downgrade` does not change the `strong_count` of the channel.
#[test]
fn test_tx_count_weak_sender() {
    let (tx, _rx) = broadcast::channel::<i32>(1);
    let tx_weak = tx.downgrade();
    let tx_weak2 = tx.downgrade();
    assert_eq!(tx.strong_count(), 1);
    assert_eq!(tx.weak_count(), 2);

    drop(tx);

    assert!(tx_weak.upgrade().is_none());
    assert!(tx_weak2.upgrade().is_none());
    assert_eq!(tx_weak.strong_count(), 0);
    assert_eq!(tx_weak.weak_count(), 2);
}

#[tokio::test]
async fn test_rx_is_closed_when_dropping_all_senders_except_weak_senders() {
    let (tx, rx) = broadcast::channel::<()>(10);
    let weak_sender = tx.clone().downgrade();
    drop(tx);
    // is_closed should return true after dropping all senders except for a weak sender.
    // The strong count should be 0 while the weak count should remain at 1.
    assert_eq!(weak_sender.strong_count(), 0);
    assert_eq!(weak_sender.weak_count(), 1);
    assert!(rx.is_closed());
}

#[tokio::test]
async fn sender_strong_count_when_cloned() {
    let (tx, rx) = broadcast::channel::<()>(1);

    let tx2 = tx.clone();

    assert_eq!(tx.strong_count(), 2);
    assert_eq!(tx2.strong_count(), 2);
    assert_eq!(rx.sender_strong_count(), 2);
}

#[tokio::test]
async fn sender_weak_count_when_downgraded() {
    let (tx, _rx) = broadcast::channel::<()>(1);

    let weak = tx.downgrade();

    assert_eq!(tx.weak_count(), 1);
    assert_eq!(weak.weak_count(), 1);
}

#[tokio::test]
async fn sender_strong_count_when_dropped() {
    let (tx, rx) = broadcast::channel::<()>(1);

    let tx2 = tx.clone();

    drop(tx2);

    assert_eq!(tx.strong_count(), 1);
    assert_eq!(rx.sender_strong_count(), 1);
}

#[tokio::test]
async fn sender_weak_count_when_dropped() {
    let (tx, rx) = broadcast::channel::<()>(1);

    let weak = tx.downgrade();

    drop(weak);

    assert_eq!(tx.weak_count(), 0);
    assert_eq!(rx.sender_weak_count(), 0);
}

#[tokio::test]
async fn sender_strong_and_weak_conut() {
    let (tx, rx) = broadcast::channel::<()>(1);

    let tx2 = tx.clone();

    let weak = tx.downgrade();
    let weak2 = tx2.downgrade();

    assert_eq!(tx.strong_count(), 2);
    assert_eq!(tx2.strong_count(), 2);
    assert_eq!(weak.strong_count(), 2);
    assert_eq!(weak2.strong_count(), 2);
    assert_eq!(rx.sender_strong_count(), 2);

    assert_eq!(tx.weak_count(), 2);
    assert_eq!(tx2.weak_count(), 2);
    assert_eq!(weak.weak_count(), 2);
    assert_eq!(weak2.weak_count(), 2);
    assert_eq!(rx.sender_weak_count(), 2);

    drop(tx2);
    drop(weak2);

    assert_eq!(tx.strong_count(), 1);
    assert_eq!(weak.strong_count(), 1);
    assert_eq!(rx.sender_strong_count(), 1);

    assert_eq!(tx.weak_count(), 1);
    assert_eq!(weak.weak_count(), 1);
    assert_eq!(rx.sender_weak_count(), 1);
}
