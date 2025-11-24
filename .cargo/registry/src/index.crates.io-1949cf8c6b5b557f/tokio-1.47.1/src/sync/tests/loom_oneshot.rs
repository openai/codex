use crate::sync::oneshot;

use loom::future::block_on;
use loom::thread;
use std::future::poll_fn;
use std::task::Poll::{Pending, Ready};

#[test]
fn smoke() {
    loom::model(|| {
        let (tx, rx) = oneshot::channel();

        thread::spawn(move || {
            tx.send(1).unwrap();
        });

        let value = block_on(rx).unwrap();
        assert_eq!(1, value);
    });
}

#[test]
fn changing_rx_task() {
    loom::model(|| {
        let (tx, mut rx) = oneshot::channel();

        thread::spawn(move || {
            tx.send(1).unwrap();
        });

        let rx = thread::spawn(move || {
            let ready = block_on(poll_fn(|cx| match Pin::new(&mut rx).poll(cx) {
                Ready(Ok(value)) => {
                    assert_eq!(1, value);
                    Ready(true)
                }
                Ready(Err(_)) => unimplemented!(),
                Pending => Ready(false),
            }));

            if ready {
                None
            } else {
                Some(rx)
            }
        })
        .join()
        .unwrap();

        if let Some(rx) = rx {
            // Previous task parked, use a new task...
            let value = block_on(rx).unwrap();
            assert_eq!(1, value);
        }
    });
}

#[test]
fn try_recv_close() {
    // reproduces https://github.com/tokio-rs/tokio/issues/4225
    loom::model(|| {
        let (tx, mut rx) = oneshot::channel();
        thread::spawn(move || {
            let _ = tx.send(());
        });

        rx.close();
        let _ = rx.try_recv();
    })
}

#[test]
fn recv_closed() {
    // reproduces https://github.com/tokio-rs/tokio/issues/4225
    loom::model(|| {
        let (tx, mut rx) = oneshot::channel();

        thread::spawn(move || {
            let _ = tx.send(1);
        });

        rx.close();
        let _ = block_on(rx);
    });
}

// TODO: Move this into `oneshot` proper.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct OnClose<'a> {
    tx: &'a mut oneshot::Sender<i32>,
}

impl<'a> OnClose<'a> {
    fn new(tx: &'a mut oneshot::Sender<i32>) -> Self {
        OnClose { tx }
    }
}

impl Future for OnClose<'_> {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<bool> {
        let fut = self.get_mut().tx.closed();
        crate::pin!(fut);

        Ready(fut.poll(cx).is_ready())
    }
}

#[test]
fn changing_tx_task() {
    loom::model(|| {
        let (mut tx, rx) = oneshot::channel::<i32>();

        thread::spawn(move || {
            drop(rx);
        });

        let tx = thread::spawn(move || {
            let t1 = block_on(OnClose::new(&mut tx));

            if t1 {
                None
            } else {
                Some(tx)
            }
        })
        .join()
        .unwrap();

        if let Some(mut tx) = tx {
            // Previous task parked, use a new task...
            block_on(OnClose::new(&mut tx));
        }
    });
}

#[test]
fn checking_tx_send_ok_not_drop() {
    use std::borrow::Borrow;
    use std::cell::Cell;

    loom::thread_local! {
        static IS_RX: Cell<bool> = Cell::new(true);
    }

    struct Msg;

    impl Drop for Msg {
        fn drop(&mut self) {
            IS_RX.with(|is_rx: &Cell<_>| {
                // On `tx.send(msg)` returning `Err(msg)`,
                // we call `std::mem::forget(msg)`, so that
                // `drop` is not expected to be called in the
                // tx thread.
                assert!(is_rx.get());
            });
        }
    }

    let mut builder = loom::model::Builder::new();
    builder.preemption_bound = Some(2);

    builder.check(|| {
        let (tx, rx) = oneshot::channel();

        // tx thread
        let tx_thread_join_handle = thread::spawn(move || {
            // Ensure that `Msg::drop` in this thread will see is_rx == false
            IS_RX.with(|is_rx: &Cell<_>| {
                is_rx.set(false);
            });
            if let Err(msg) = tx.send(Msg) {
                std::mem::forget(msg);
            }
        });

        // main thread is the rx thread
        drop(rx);

        tx_thread_join_handle.join().unwrap();
    });
}
