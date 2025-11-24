use futures::channel::mpsc;
use futures::executor::block_on;
use futures::future::poll_fn;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

#[test]
fn sequence() {
    let (tx, rx) = mpsc::channel(1);

    let amt = 20;
    let t = thread::spawn(move || block_on(send_sequence(amt, tx)));
    let list: Vec<_> = block_on(rx.collect());
    let mut list = list.into_iter();
    for i in (1..=amt).rev() {
        assert_eq!(list.next(), Some(i));
    }
    assert_eq!(list.next(), None);

    t.join().unwrap();
}

async fn send_sequence(n: u32, mut sender: mpsc::Sender<u32>) {
    for x in 0..n {
        sender.send(n - x).await.unwrap();
    }
}

#[test]
fn drop_sender() {
    let (tx, mut rx) = mpsc::channel::<u32>(1);
    drop(tx);
    let f = poll_fn(|cx| rx.poll_next_unpin(cx));
    assert_eq!(block_on(f), None)
}

#[test]
fn drop_rx() {
    let (mut tx, rx) = mpsc::channel::<u32>(1);
    block_on(tx.send(1)).unwrap();
    drop(rx);
    assert!(block_on(tx.send(1)).is_err());
}

#[test]
fn drop_order() {
    static DROPS: AtomicUsize = AtomicUsize::new(0);
    let (mut tx, rx) = mpsc::channel(1);

    struct A;

    impl Drop for A {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }

    block_on(tx.send(A)).unwrap();
    assert_eq!(DROPS.load(Ordering::SeqCst), 0);
    drop(rx);
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);
    assert!(block_on(tx.send(A)).is_err());
    assert_eq!(DROPS.load(Ordering::SeqCst), 2);
}
