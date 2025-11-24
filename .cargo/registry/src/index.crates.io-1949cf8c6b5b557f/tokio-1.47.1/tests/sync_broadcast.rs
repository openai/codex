#![allow(clippy::cognitive_complexity)]
#![warn(rust_2018_idioms)]
#![cfg(feature = "sync")]

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as test;

use tokio::sync::broadcast;
use tokio_test::task;
use tokio_test::{
    assert_err, assert_ok, assert_pending, assert_ready, assert_ready_err, assert_ready_ok,
};

use std::sync::Arc;

macro_rules! assert_recv {
    ($e:expr) => {
        match $e.try_recv() {
            Ok(value) => value,
            Err(e) => panic!("expected recv; got = {:?}", e),
        }
    };
}

macro_rules! assert_empty {
    ($e:expr) => {
        match $e.try_recv() {
            Ok(value) => panic!("expected empty; got = {:?}", value),
            Err(broadcast::error::TryRecvError::Empty) => {}
            Err(e) => panic!("expected empty; got = {:?}", e),
        }
    };
}

macro_rules! assert_lagged {
    ($e:expr, $n:expr) => {
        match assert_err!($e) {
            broadcast::error::TryRecvError::Lagged(n) => {
                assert_eq!(n, $n);
            }
            _ => panic!("did not lag"),
        }
    };
}

macro_rules! assert_closed {
    ($e:expr) => {
        match assert_err!($e) {
            broadcast::error::TryRecvError::Closed => {}
            _ => panic!("is not closed"),
        }
    };
}

#[allow(unused)]
trait AssertSend: Send + Sync {}
impl AssertSend for broadcast::Sender<i32> {}
impl AssertSend for broadcast::Receiver<i32> {}
impl AssertSend for broadcast::WeakSender<i32> {}

#[test]
fn send_try_recv_bounded() {
    let (tx, mut rx) = broadcast::channel(16);

    assert_empty!(rx);

    let n = assert_ok!(tx.send("hello"));
    assert_eq!(n, 1);

    let val = assert_recv!(rx);
    assert_eq!(val, "hello");

    assert_empty!(rx);
}

#[test]
fn send_two_recv() {
    let (tx, mut rx1) = broadcast::channel(16);
    let mut rx2 = tx.subscribe();

    assert_empty!(rx1);
    assert_empty!(rx2);

    let n = assert_ok!(tx.send("hello"));
    assert_eq!(n, 2);

    let val = assert_recv!(rx1);
    assert_eq!(val, "hello");

    let val = assert_recv!(rx2);
    assert_eq!(val, "hello");

    assert_empty!(rx1);
    assert_empty!(rx2);
}

#[test]
fn send_recv_bounded() {
    let (tx, mut rx) = broadcast::channel(16);

    let mut recv = task::spawn(rx.recv());

    assert_pending!(recv.poll());

    assert_ok!(tx.send("hello"));

    assert!(recv.is_woken());
    let val = assert_ready_ok!(recv.poll());
    assert_eq!(val, "hello");
}

#[test]
fn send_two_recv_bounded() {
    let (tx, mut rx1) = broadcast::channel(16);
    let mut rx2 = tx.subscribe();

    let mut recv1 = task::spawn(rx1.recv());
    let mut recv2 = task::spawn(rx2.recv());

    assert_pending!(recv1.poll());
    assert_pending!(recv2.poll());

    assert_ok!(tx.send("hello"));

    assert!(recv1.is_woken());
    assert!(recv2.is_woken());

    let val1 = assert_ready_ok!(recv1.poll());
    let val2 = assert_ready_ok!(recv2.poll());
    assert_eq!(val1, "hello");
    assert_eq!(val2, "hello");

    drop((recv1, recv2));

    let mut recv1 = task::spawn(rx1.recv());
    let mut recv2 = task::spawn(rx2.recv());

    assert_pending!(recv1.poll());

    assert_ok!(tx.send("world"));

    assert!(recv1.is_woken());
    assert!(!recv2.is_woken());

    let val1 = assert_ready_ok!(recv1.poll());
    let val2 = assert_ready_ok!(recv2.poll());
    assert_eq!(val1, "world");
    assert_eq!(val2, "world");
}

#[test]
fn change_tasks() {
    let (tx, mut rx) = broadcast::channel(1);

    let mut recv = Box::pin(rx.recv());

    let mut task1 = task::spawn(&mut recv);
    assert_pending!(task1.poll());

    let mut task2 = task::spawn(&mut recv);
    assert_pending!(task2.poll());

    tx.send("hello").unwrap();

    assert!(task2.is_woken());
}

#[test]
fn send_slow_rx() {
    let (tx, mut rx1) = broadcast::channel(16);
    let mut rx2 = tx.subscribe();

    {
        let mut recv2 = task::spawn(rx2.recv());

        {
            let mut recv1 = task::spawn(rx1.recv());

            assert_pending!(recv1.poll());
            assert_pending!(recv2.poll());

            assert_ok!(tx.send("one"));

            assert!(recv1.is_woken());
            assert!(recv2.is_woken());

            assert_ok!(tx.send("two"));

            let val = assert_ready_ok!(recv1.poll());
            assert_eq!(val, "one");
        }

        let val = assert_ready_ok!(task::spawn(rx1.recv()).poll());
        assert_eq!(val, "two");

        let mut recv1 = task::spawn(rx1.recv());

        assert_pending!(recv1.poll());

        assert_ok!(tx.send("three"));

        assert!(recv1.is_woken());

        let val = assert_ready_ok!(recv1.poll());
        assert_eq!(val, "three");

        let val = assert_ready_ok!(recv2.poll());
        assert_eq!(val, "one");
    }

    let val = assert_recv!(rx2);
    assert_eq!(val, "two");

    let val = assert_recv!(rx2);
    assert_eq!(val, "three");
}

#[test]
fn drop_rx_while_values_remain() {
    let (tx, mut rx1) = broadcast::channel(16);
    let mut rx2 = tx.subscribe();

    assert_ok!(tx.send("one"));
    assert_ok!(tx.send("two"));

    assert_recv!(rx1);
    assert_recv!(rx2);

    drop(rx2);
    drop(rx1);
}

#[test]
fn lagging_rx() {
    let (tx, mut rx1) = broadcast::channel(2);
    let mut rx2 = tx.subscribe();

    assert_ok!(tx.send("one"));
    assert_ok!(tx.send("two"));

    assert_eq!("one", assert_recv!(rx1));

    assert_ok!(tx.send("three"));

    // Lagged too far
    let x = dbg!(rx2.try_recv());
    assert_lagged!(x, 1);

    // Calling again gets the next value
    assert_eq!("two", assert_recv!(rx2));

    assert_eq!("two", assert_recv!(rx1));
    assert_eq!("three", assert_recv!(rx1));

    assert_ok!(tx.send("four"));
    assert_ok!(tx.send("five"));

    assert_lagged!(rx2.try_recv(), 1);

    assert_ok!(tx.send("six"));

    assert_lagged!(rx2.try_recv(), 1);
}

#[test]
fn send_no_rx() {
    let (tx, _) = broadcast::channel(16);

    assert_err!(tx.send("hello"));

    let mut rx = tx.subscribe();

    assert_ok!(tx.send("world"));

    let val = assert_recv!(rx);
    assert_eq!("world", val);
}

#[test]
#[should_panic]
#[cfg(not(target_family = "wasm"))] // wasm currently doesn't support unwinding
fn zero_capacity() {
    broadcast::channel::<()>(0);
}

#[test]
#[should_panic]
#[cfg(not(target_family = "wasm"))] // wasm currently doesn't support unwinding
fn capacity_too_big() {
    broadcast::channel::<()>(1 + (usize::MAX >> 1));
}

#[test]
#[cfg(panic = "unwind")]
#[cfg(not(target_family = "wasm"))] // wasm currently doesn't support unwinding
fn panic_in_clone() {
    use std::panic::{self, AssertUnwindSafe};

    #[derive(Eq, PartialEq, Debug)]
    struct MyVal(usize);

    impl Clone for MyVal {
        fn clone(&self) -> MyVal {
            assert_ne!(0, self.0);
            MyVal(self.0)
        }
    }

    let (tx, mut rx) = broadcast::channel(16);

    assert_ok!(tx.send(MyVal(0)));
    assert_ok!(tx.send(MyVal(1)));

    let res = panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = rx.try_recv();
    }));

    assert_err!(res);

    let val = assert_recv!(rx);
    assert_eq!(val, MyVal(1));
}

#[test]
fn dropping_tx_notifies_rx() {
    let (tx, mut rx1) = broadcast::channel::<()>(16);
    let mut rx2 = tx.subscribe();

    let tx2 = tx.clone();

    let mut recv1 = task::spawn(rx1.recv());
    let mut recv2 = task::spawn(rx2.recv());

    assert_pending!(recv1.poll());
    assert_pending!(recv2.poll());

    drop(tx);

    assert_pending!(recv1.poll());
    assert_pending!(recv2.poll());

    drop(tx2);

    assert!(recv1.is_woken());
    assert!(recv2.is_woken());

    let err = assert_ready_err!(recv1.poll());
    assert!(is_closed(err));

    let err = assert_ready_err!(recv2.poll());
    assert!(is_closed(err));
}

#[test]
fn unconsumed_messages_are_dropped() {
    let (tx, rx) = broadcast::channel(16);

    let msg = Arc::new(());

    assert_ok!(tx.send(msg.clone()));

    assert_eq!(2, Arc::strong_count(&msg));

    drop(rx);

    assert_eq!(1, Arc::strong_count(&msg));
}

#[test]
fn single_capacity_recvs() {
    let (tx, mut rx) = broadcast::channel(1);

    assert_ok!(tx.send(1));

    assert_eq!(assert_recv!(rx), 1);
    assert_empty!(rx);
}

#[test]
fn single_capacity_recvs_after_drop_1() {
    let (tx, mut rx) = broadcast::channel(1);

    assert_ok!(tx.send(1));
    drop(tx);

    assert_eq!(assert_recv!(rx), 1);
    assert_closed!(rx.try_recv());
}

#[test]
fn single_capacity_recvs_after_drop_2() {
    let (tx, mut rx) = broadcast::channel(1);

    assert_ok!(tx.send(1));
    assert_ok!(tx.send(2));
    drop(tx);

    assert_lagged!(rx.try_recv(), 1);
    assert_eq!(assert_recv!(rx), 2);
    assert_closed!(rx.try_recv());
}

#[test]
fn dropping_sender_does_not_overwrite() {
    let (tx, mut rx) = broadcast::channel(2);

    assert_ok!(tx.send(1));
    assert_ok!(tx.send(2));
    drop(tx);

    assert_eq!(assert_recv!(rx), 1);
    assert_eq!(assert_recv!(rx), 2);
    assert_closed!(rx.try_recv());
}

#[test]
fn lagging_receiver_recovers_after_wrap_closed_1() {
    let (tx, mut rx) = broadcast::channel(2);

    assert_ok!(tx.send(1));
    assert_ok!(tx.send(2));
    assert_ok!(tx.send(3));
    drop(tx);

    assert_lagged!(rx.try_recv(), 1);
    assert_eq!(assert_recv!(rx), 2);
    assert_eq!(assert_recv!(rx), 3);
    assert_closed!(rx.try_recv());
}

#[test]
fn lagging_receiver_recovers_after_wrap_closed_2() {
    let (tx, mut rx) = broadcast::channel(2);

    assert_ok!(tx.send(1));
    assert_ok!(tx.send(2));
    assert_ok!(tx.send(3));
    assert_ok!(tx.send(4));
    drop(tx);

    assert_lagged!(rx.try_recv(), 2);
    assert_eq!(assert_recv!(rx), 3);
    assert_eq!(assert_recv!(rx), 4);
    assert_closed!(rx.try_recv());
}

#[test]
fn lagging_receiver_recovers_after_wrap_open() {
    let (tx, mut rx) = broadcast::channel(2);

    assert_ok!(tx.send(1));
    assert_ok!(tx.send(2));
    assert_ok!(tx.send(3));

    assert_lagged!(rx.try_recv(), 1);
    assert_eq!(assert_recv!(rx), 2);
    assert_eq!(assert_recv!(rx), 3);
    assert_empty!(rx);
}

#[test]
fn receiver_len_with_lagged() {
    let (tx, mut rx) = broadcast::channel(3);

    tx.send(10).unwrap();
    tx.send(20).unwrap();
    tx.send(30).unwrap();
    tx.send(40).unwrap();

    assert_eq!(rx.len(), 4);
    assert_eq!(assert_recv!(rx), 10);

    tx.send(50).unwrap();
    tx.send(60).unwrap();

    assert_eq!(rx.len(), 5);
    assert_lagged!(rx.try_recv(), 1);
}

fn is_closed(err: broadcast::error::RecvError) -> bool {
    matches!(err, broadcast::error::RecvError::Closed)
}

#[test]
fn resubscribe_points_to_tail() {
    let (tx, mut rx) = broadcast::channel(3);
    tx.send(1).unwrap();

    let mut rx_resub = rx.resubscribe();

    // verify we're one behind at the start
    assert_empty!(rx_resub);
    assert_eq!(assert_recv!(rx), 1);

    // verify we do not affect rx
    tx.send(2).unwrap();
    assert_eq!(assert_recv!(rx_resub), 2);
    tx.send(3).unwrap();
    assert_eq!(assert_recv!(rx), 2);
    assert_eq!(assert_recv!(rx), 3);
    assert_empty!(rx);

    assert_eq!(assert_recv!(rx_resub), 3);
    assert_empty!(rx_resub);
}

#[test]
fn resubscribe_lagged() {
    let (tx, mut rx) = broadcast::channel(1);
    tx.send(1).unwrap();
    tx.send(2).unwrap();

    let mut rx_resub = rx.resubscribe();
    assert_lagged!(rx.try_recv(), 1);
    assert_empty!(rx_resub);

    assert_eq!(assert_recv!(rx), 2);
    assert_empty!(rx);
    assert_empty!(rx_resub);
}

#[test]
fn resubscribe_to_closed_channel() {
    let (tx, rx) = tokio::sync::broadcast::channel::<u32>(2);
    drop(tx);

    let mut rx_resub = rx.resubscribe();
    assert_closed!(rx_resub.try_recv());
}

#[test]
fn sender_len() {
    let (tx, mut rx1) = broadcast::channel(4);
    let mut rx2 = tx.subscribe();

    assert_eq!(tx.len(), 0);
    assert!(tx.is_empty());

    tx.send(1).unwrap();
    tx.send(2).unwrap();
    tx.send(3).unwrap();

    assert_eq!(tx.len(), 3);
    assert!(!tx.is_empty());

    assert_recv!(rx1);
    assert_recv!(rx1);

    assert_eq!(tx.len(), 3);
    assert!(!tx.is_empty());

    assert_recv!(rx2);

    assert_eq!(tx.len(), 2);
    assert!(!tx.is_empty());

    tx.send(4).unwrap();
    tx.send(5).unwrap();
    tx.send(6).unwrap();

    assert_eq!(tx.len(), 4);
    assert!(!tx.is_empty());
}

#[test]
#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
fn sender_len_random() {
    let (tx, mut rx1) = broadcast::channel(16);
    let mut rx2 = tx.subscribe();

    for _ in 0..1000 {
        match rand::random_range(0..4) {
            0 => {
                let _ = rx1.try_recv();
            }
            1 => {
                let _ = rx2.try_recv();
            }
            _ => {
                tx.send(0).unwrap();
            }
        }

        let expected_len = usize::min(usize::max(rx1.len(), rx2.len()), 16);
        assert_eq!(tx.len(), expected_len);
    }
}

#[test]
fn send_in_waker_drop() {
    use futures::task::ArcWake;
    use std::future::Future;
    use std::task::Context;

    struct SendOnDrop(broadcast::Sender<()>);

    impl Drop for SendOnDrop {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    impl ArcWake for SendOnDrop {
        fn wake_by_ref(_arc_self: &Arc<Self>) {}
    }

    // Test if there is no deadlock when replacing the old waker.

    let (tx, mut rx) = broadcast::channel(16);

    let mut fut = Box::pin(async {
        let _ = rx.recv().await;
    });

    // Store our special waker in the receiving future.
    let waker = futures::task::waker(Arc::new(SendOnDrop(tx)));
    let mut cx = Context::from_waker(&waker);
    assert!(fut.as_mut().poll(&mut cx).is_pending());
    drop(waker);

    // Second poll shouldn't deadlock.
    let mut cx = Context::from_waker(futures::task::noop_waker_ref());
    let _ = fut.as_mut().poll(&mut cx);

    // Test if there is no deadlock when calling waker.wake().

    let (tx, mut rx) = broadcast::channel(16);

    let mut fut = Box::pin(async {
        let _ = rx.recv().await;
    });

    // Store our special waker in the receiving future.
    let waker = futures::task::waker(Arc::new(SendOnDrop(tx.clone())));
    let mut cx = Context::from_waker(&waker);
    assert!(fut.as_mut().poll(&mut cx).is_pending());
    drop(waker);

    // Shouldn't deadlock.
    let _ = tx.send(());
}

#[tokio::test]
async fn receiver_recv_is_cooperative() {
    let (tx, mut rx) = broadcast::channel(8);

    tokio::select! {
        biased;
        _ = async {
            loop {
                assert!(tx.send(()).is_ok());
                assert!(rx.recv().await.is_ok());
            }
        } => {},
        _ = tokio::task::yield_now() => {},
    }
}

#[test]
fn broadcast_sender_closed() {
    let (tx, rx) = broadcast::channel::<()>(1);
    let rx2 = tx.subscribe();

    let mut task = task::spawn(tx.closed());
    assert_pending!(task.poll());

    drop(rx);
    assert!(!task.is_woken());
    assert_pending!(task.poll());

    drop(rx2);
    assert!(task.is_woken());
    assert_ready!(task.poll());
}

#[test]
fn broadcast_sender_closed_with_extra_subscribe() {
    let (tx, rx) = broadcast::channel::<()>(1);
    let rx2 = tx.subscribe();

    let mut task = task::spawn(tx.closed());
    assert_pending!(task.poll());

    drop(rx);
    assert!(!task.is_woken());
    assert_pending!(task.poll());

    drop(rx2);
    assert!(task.is_woken());

    let rx3 = tx.subscribe();
    assert_pending!(task.poll());

    drop(rx3);
    assert!(task.is_woken());
    assert_ready!(task.poll());

    let mut task2 = task::spawn(tx.closed());
    assert_ready!(task2.poll());

    let rx4 = tx.subscribe();
    let mut task3 = task::spawn(tx.closed());
    assert_pending!(task3.poll());

    drop(rx4);
    assert!(task3.is_woken());
    assert_ready!(task3.poll());
}
