use crate::sync::watch;

use loom::future::block_on;
use loom::thread;
use std::sync::Arc;

#[test]
fn smoke() {
    loom::model(|| {
        let (tx, mut rx1) = watch::channel(1);
        let mut rx2 = rx1.clone();
        let mut rx3 = rx1.clone();
        let mut rx4 = rx1.clone();
        let mut rx5 = rx1.clone();

        let th = thread::spawn(move || {
            tx.send(2).unwrap();
        });

        block_on(rx1.changed()).unwrap();
        assert_eq!(*rx1.borrow(), 2);

        block_on(rx2.changed()).unwrap();
        assert_eq!(*rx2.borrow(), 2);

        block_on(rx3.changed()).unwrap();
        assert_eq!(*rx3.borrow(), 2);

        block_on(rx4.changed()).unwrap();
        assert_eq!(*rx4.borrow(), 2);

        block_on(rx5.changed()).unwrap();
        assert_eq!(*rx5.borrow(), 2);

        th.join().unwrap();
    })
}

#[test]
fn wait_for_test() {
    loom::model(move || {
        let (tx, mut rx) = watch::channel(false);

        let tx_arc = Arc::new(tx);
        let tx1 = tx_arc.clone();
        let tx2 = tx_arc.clone();

        let th1 = thread::spawn(move || {
            for _ in 0..2 {
                tx1.send_modify(|_x| {});
            }
        });

        let th2 = thread::spawn(move || {
            tx2.send(true).unwrap();
        });

        assert_eq!(*block_on(rx.wait_for(|x| *x)).unwrap(), true);

        th1.join().unwrap();
        th2.join().unwrap();
    });
}

#[test]
fn wait_for_returns_correct_value() {
    loom::model(move || {
        let (tx, mut rx) = watch::channel(0);

        let jh = thread::spawn(move || {
            tx.send(1).unwrap();
            tx.send(2).unwrap();
            tx.send(3).unwrap();
        });

        // Stop at the first value we are called at.
        let mut stopped_at = usize::MAX;
        let returned = *block_on(rx.wait_for(|x| {
            stopped_at = *x;
            true
        }))
        .unwrap();

        // Check that it returned the same value as the one we returned
        // `true` for.
        assert_eq!(stopped_at, returned);

        jh.join().unwrap();
    });
}

#[test]
fn multiple_sender_drop_concurrently() {
    loom::model(move || {
        let (tx1, rx) = watch::channel(0);
        let tx2 = tx1.clone();

        let jh = thread::spawn(move || {
            drop(tx2);
        });
        assert!(rx.has_changed().is_ok());

        drop(tx1);

        jh.join().unwrap();

        // Check if all sender are dropped and closed flag is set.
        assert!(rx.has_changed().is_err());
    });
}
