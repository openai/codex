use crate::sync::mpsc;

use loom::future::block_on;
use loom::sync::Arc;
use loom::thread;
use std::future::poll_fn;
use tokio_test::assert_ok;

#[test]
fn closing_tx() {
    loom::model(|| {
        let (tx, mut rx) = mpsc::channel(16);

        thread::spawn(move || {
            tx.try_send(()).unwrap();
            drop(tx);
        });

        let v = block_on(rx.recv());
        assert!(v.is_some());

        let v = block_on(rx.recv());
        assert!(v.is_none());
    });
}

#[test]
fn closing_unbounded_tx() {
    loom::model(|| {
        let (tx, mut rx) = mpsc::unbounded_channel();

        thread::spawn(move || {
            tx.send(()).unwrap();
            drop(tx);
        });

        let v = block_on(rx.recv());
        assert!(v.is_some());

        let v = block_on(rx.recv());
        assert!(v.is_none());
    });
}

#[test]
fn closing_bounded_rx() {
    loom::model(|| {
        let (tx1, rx) = mpsc::channel::<()>(16);
        let tx2 = tx1.clone();
        thread::spawn(move || {
            drop(rx);
        });

        block_on(tx1.closed());
        block_on(tx2.closed());
    });
}

#[test]
fn closing_and_sending() {
    loom::model(|| {
        let (tx1, mut rx) = mpsc::channel::<()>(16);
        let tx1 = Arc::new(tx1);
        let tx2 = tx1.clone();

        let th1 = thread::spawn(move || {
            tx1.try_send(()).unwrap();
        });

        let th2 = thread::spawn(move || {
            block_on(tx2.closed());
        });

        let th3 = thread::spawn(move || {
            let v = block_on(rx.recv());
            assert!(v.is_some());
            drop(rx);
        });

        assert_ok!(th1.join());
        assert_ok!(th2.join());
        assert_ok!(th3.join());
    });
}

#[test]
fn closing_unbounded_rx() {
    loom::model(|| {
        let (tx1, rx) = mpsc::unbounded_channel::<()>();
        let tx2 = tx1.clone();
        thread::spawn(move || {
            drop(rx);
        });

        block_on(tx1.closed());
        block_on(tx2.closed());
    });
}

#[test]
fn dropping_tx() {
    loom::model(|| {
        let (tx, mut rx) = mpsc::channel::<()>(16);

        for _ in 0..2 {
            let tx = tx.clone();
            thread::spawn(move || {
                drop(tx);
            });
        }
        drop(tx);

        let v = block_on(rx.recv());
        assert!(v.is_none());
    });
}

#[test]
fn dropping_unbounded_tx() {
    loom::model(|| {
        let (tx, mut rx) = mpsc::unbounded_channel::<()>();

        for _ in 0..2 {
            let tx = tx.clone();
            thread::spawn(move || {
                drop(tx);
            });
        }
        drop(tx);

        let v = block_on(rx.recv());
        assert!(v.is_none());
    });
}

#[test]
fn try_recv() {
    loom::model(|| {
        use crate::sync::{mpsc, Semaphore};
        use loom::sync::{Arc, Mutex};

        const PERMITS: usize = 2;
        const TASKS: usize = 2;
        const CYCLES: usize = 1;

        struct Context {
            sem: Arc<Semaphore>,
            tx: mpsc::Sender<()>,
            rx: Mutex<mpsc::Receiver<()>>,
        }

        fn run(ctx: &Context) {
            block_on(async {
                let permit = ctx.sem.acquire().await;
                assert_ok!(ctx.rx.lock().unwrap().try_recv());
                crate::task::yield_now().await;
                assert_ok!(ctx.tx.clone().try_send(()));
                drop(permit);
            });
        }

        let (tx, rx) = mpsc::channel(PERMITS);
        let sem = Arc::new(Semaphore::new(PERMITS));
        let ctx = Arc::new(Context {
            sem,
            tx,
            rx: Mutex::new(rx),
        });

        for _ in 0..PERMITS {
            assert_ok!(ctx.tx.clone().try_send(()));
        }

        let mut ths = Vec::new();

        for _ in 0..TASKS {
            let ctx = ctx.clone();

            ths.push(thread::spawn(move || {
                run(&ctx);
            }));
        }

        run(&ctx);

        for th in ths {
            th.join().unwrap();
        }
    });
}

#[test]
fn len_nonzero_after_send() {
    loom::model(|| {
        let (send, recv) = mpsc::channel(10);
        let send2 = send.clone();

        let join = thread::spawn(move || {
            block_on(send2.send("message2")).unwrap();
        });

        block_on(send.send("message1")).unwrap();
        assert!(recv.len() != 0);

        join.join().unwrap();
    });
}

#[test]
fn nonempty_after_send() {
    loom::model(|| {
        let (send, recv) = mpsc::channel(10);
        let send2 = send.clone();

        let join = thread::spawn(move || {
            block_on(send2.send("message2")).unwrap();
        });

        block_on(send.send("message1")).unwrap();
        assert!(!recv.is_empty());

        join.join().unwrap();
    });
}
