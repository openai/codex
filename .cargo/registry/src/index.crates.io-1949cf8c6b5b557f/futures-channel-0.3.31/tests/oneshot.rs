use futures::channel::oneshot::{self, Sender};
use futures::executor::block_on;
use futures::future::{poll_fn, FutureExt};
use futures::task::{Context, Poll};
use futures_test::task::panic_waker_ref;
use std::sync::mpsc;
use std::thread;

#[test]
fn smoke_poll() {
    let (mut tx, rx) = oneshot::channel::<u32>();
    let mut rx = Some(rx);
    let f = poll_fn(|cx| {
        assert!(tx.poll_canceled(cx).is_pending());
        assert!(tx.poll_canceled(cx).is_pending());
        drop(rx.take());
        assert!(tx.poll_canceled(cx).is_ready());
        assert!(tx.poll_canceled(cx).is_ready());
        Poll::Ready(())
    });

    block_on(f);
}

#[test]
fn cancel_notifies() {
    let (mut tx, rx) = oneshot::channel::<u32>();

    let t = thread::spawn(move || {
        block_on(tx.cancellation());
    });
    drop(rx);
    t.join().unwrap();
}

#[test]
fn cancel_lots() {
    const N: usize = if cfg!(miri) { 100 } else { 20000 };

    let (tx, rx) = mpsc::channel::<(Sender<_>, mpsc::Sender<_>)>();
    let t = thread::spawn(move || {
        for (mut tx, tx2) in rx {
            block_on(tx.cancellation());
            tx2.send(()).unwrap();
        }
    });

    for _ in 0..N {
        let (otx, orx) = oneshot::channel::<u32>();
        let (tx2, rx2) = mpsc::channel();
        tx.send((otx, tx2)).unwrap();
        drop(orx);
        rx2.recv().unwrap();
    }
    drop(tx);

    t.join().unwrap();
}

#[test]
fn cancel_after_sender_drop_doesnt_notify() {
    let (mut tx, rx) = oneshot::channel::<u32>();
    let mut cx = Context::from_waker(panic_waker_ref());
    assert_eq!(tx.poll_canceled(&mut cx), Poll::Pending);
    drop(tx);
    drop(rx);
}

#[test]
fn close() {
    let (mut tx, mut rx) = oneshot::channel::<u32>();
    rx.close();
    block_on(poll_fn(|cx| {
        match rx.poll_unpin(cx) {
            Poll::Ready(Err(_)) => {}
            _ => panic!(),
        };
        assert!(tx.poll_canceled(cx).is_ready());
        Poll::Ready(())
    }));
}

#[test]
fn close_wakes() {
    let (mut tx, mut rx) = oneshot::channel::<u32>();
    let (tx2, rx2) = mpsc::channel();
    let t = thread::spawn(move || {
        rx.close();
        rx2.recv().unwrap();
    });
    block_on(tx.cancellation());
    tx2.send(()).unwrap();
    t.join().unwrap();
}

#[test]
fn is_canceled() {
    let (tx, rx) = oneshot::channel::<u32>();
    assert!(!tx.is_canceled());
    drop(rx);
    assert!(tx.is_canceled());
}

#[test]
fn cancel_sends() {
    const N: usize = if cfg!(miri) { 100 } else { 20000 };

    let (tx, rx) = mpsc::channel::<Sender<_>>();
    let t = thread::spawn(move || {
        for otx in rx {
            let _ = otx.send(42);
        }
    });

    for _ in 0..N {
        let (otx, mut orx) = oneshot::channel::<u32>();
        tx.send(otx).unwrap();

        orx.close();
        let _ = block_on(orx);
    }

    drop(tx);
    t.join().unwrap();
}

// #[test]
// fn spawn_sends_items() {
//     let core = local_executor::Core::new();
//     let future = ok::<_, ()>(1);
//     let rx = spawn(future, &core);
//     assert_eq!(core.run(rx).unwrap(), 1);
// }
//
// #[test]
// fn spawn_kill_dead_stream() {
//     use std::thread;
//     use std::time::Duration;
//     use futures::future::Either;
//     use futures::sync::oneshot;
//
//     // a future which never returns anything (forever accepting incoming
//     // connections), but dropping it leads to observable side effects
//     // (like closing listening sockets, releasing limited resources,
//     // ...)
//     #[derive(Debug)]
//     struct Dead {
//         // when dropped you should get Err(oneshot::Canceled) on the
//         // receiving end
//         done: oneshot::Sender<()>,
//     }
//     impl Future for Dead {
//         type Item = ();
//         type Error = ();
//
//         fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
//             Ok(Poll::Pending)
//         }
//     }
//
//     // need to implement a timeout for the test, as it would hang
//     // forever right now
//     let (timeout_tx, timeout_rx) = oneshot::channel();
//     thread::spawn(move || {
//         thread::sleep(Duration::from_millis(1000));
//         let _ = timeout_tx.send(());
//     });
//
//     let core = local_executor::Core::new();
//     let (done_tx, done_rx) = oneshot::channel();
//     let future = Dead{done: done_tx};
//     let rx = spawn(future, &core);
//     let res = core.run(
//         Ok::<_, ()>(())
//         .into_future()
//         .then(move |_| {
//             // now drop the spawned future: maybe some timeout exceeded,
//             // or some connection on this end was closed by the remote
//             // end.
//             drop(rx);
//             // and wait for the spawned future to release its resources
//             done_rx
//         })
//         .select2(timeout_rx)
//     );
//     match res {
//         Err(Either::A((oneshot::Canceled, _))) => (),
//         Ok(Either::B(((), _))) => {
//             panic!("dead future wasn't canceled (timeout)");
//         },
//         _ => {
//             panic!("dead future wasn't canceled (unexpected result)");
//         },
//     }
// }
//
// #[test]
// fn spawn_dont_kill_forgot_dead_stream() {
//     use std::thread;
//     use std::time::Duration;
//     use futures::future::Either;
//     use futures::sync::oneshot;
//
//     // a future which never returns anything (forever accepting incoming
//     // connections), but dropping it leads to observable side effects
//     // (like closing listening sockets, releasing limited resources,
//     // ...)
//     #[derive(Debug)]
//     struct Dead {
//         // when dropped you should get Err(oneshot::Canceled) on the
//         // receiving end
//         done: oneshot::Sender<()>,
//     }
//     impl Future for Dead {
//         type Item = ();
//         type Error = ();
//
//         fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
//             Ok(Poll::Pending)
//         }
//     }
//
//     // need to implement a timeout for the test, as it would hang
//     // forever right now
//     let (timeout_tx, timeout_rx) = oneshot::channel();
//     thread::spawn(move || {
//         thread::sleep(Duration::from_millis(1000));
//         let _ = timeout_tx.send(());
//     });
//
//     let core = local_executor::Core::new();
//     let (done_tx, done_rx) = oneshot::channel();
//     let future = Dead{done: done_tx};
//     let rx = spawn(future, &core);
//     let res = core.run(
//         Ok::<_, ()>(())
//         .into_future()
//         .then(move |_| {
//             // forget the spawned future: should keep running, i.e. hit
//             // the timeout below.
//             rx.forget();
//             // and wait for the spawned future to release its resources
//             done_rx
//         })
//         .select2(timeout_rx)
//     );
//     match res {
//         Err(Either::A((oneshot::Canceled, _))) => {
//             panic!("forgotten dead future was canceled");
//         },
//         Ok(Either::B(((), _))) => (), // reached timeout
//         _ => {
//             panic!("forgotten dead future was canceled (unexpected result)");
//         },
//     }
// }
