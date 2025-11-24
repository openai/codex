use futures::channel::{mpsc, oneshot};
use futures::executor::{block_on, block_on_stream};
use futures::future::{poll_fn, FutureExt};
use futures::pin_mut;
use futures::sink::{Sink, SinkExt};
use futures::stream::{Stream, StreamExt};
use futures::task::{Context, Poll};
use futures_test::task::{new_count_waker, noop_context};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

#[allow(dead_code)]
trait AssertSend: Send {}
impl AssertSend for mpsc::Sender<i32> {}
impl AssertSend for mpsc::Receiver<i32> {}

#[test]
fn send_recv() {
    let (mut tx, rx) = mpsc::channel::<i32>(16);

    block_on(tx.send(1)).unwrap();
    drop(tx);
    let v: Vec<_> = block_on(rx.collect());
    assert_eq!(v, vec![1]);
}

#[test]
fn send_recv_no_buffer() {
    // Run on a task context
    block_on(poll_fn(move |cx| {
        let (tx, rx) = mpsc::channel::<i32>(0);
        pin_mut!(tx, rx);

        assert!(tx.as_mut().poll_flush(cx).is_ready());
        assert!(tx.as_mut().poll_ready(cx).is_ready());

        // Send first message
        assert!(tx.as_mut().start_send(1).is_ok());
        assert!(tx.as_mut().poll_ready(cx).is_pending());

        // poll_ready said Pending, so no room in buffer, therefore new sends
        // should get rejected with is_full.
        assert!(tx.as_mut().start_send(0).unwrap_err().is_full());
        assert!(tx.as_mut().poll_ready(cx).is_pending());

        // Take the value
        assert_eq!(rx.as_mut().poll_next(cx), Poll::Ready(Some(1)));
        assert!(tx.as_mut().poll_ready(cx).is_ready());

        // Send second message
        assert!(tx.as_mut().poll_ready(cx).is_ready());
        assert!(tx.as_mut().start_send(2).is_ok());
        assert!(tx.as_mut().poll_ready(cx).is_pending());

        // Take the value
        assert_eq!(rx.as_mut().poll_next(cx), Poll::Ready(Some(2)));
        assert!(tx.as_mut().poll_ready(cx).is_ready());

        Poll::Ready(())
    }));
}

#[test]
fn send_shared_recv() {
    let (mut tx1, rx) = mpsc::channel::<i32>(16);
    let mut rx = block_on_stream(rx);
    let mut tx2 = tx1.clone();

    block_on(tx1.send(1)).unwrap();
    assert_eq!(rx.next(), Some(1));

    block_on(tx2.send(2)).unwrap();
    assert_eq!(rx.next(), Some(2));
}

#[test]
fn send_recv_threads() {
    let (mut tx, rx) = mpsc::channel::<i32>(16);

    let t = thread::spawn(move || {
        block_on(tx.send(1)).unwrap();
    });

    let v: Vec<_> = block_on(rx.take(1).collect());
    assert_eq!(v, vec![1]);

    t.join().unwrap();
}

#[test]
fn send_recv_threads_no_capacity() {
    let (mut tx, rx) = mpsc::channel::<i32>(0);

    let t = thread::spawn(move || {
        block_on(tx.send(1)).unwrap();
        block_on(tx.send(2)).unwrap();
    });

    let v: Vec<_> = block_on(rx.collect());
    assert_eq!(v, vec![1, 2]);

    t.join().unwrap();
}

#[test]
fn recv_close_gets_none() {
    let (mut tx, mut rx) = mpsc::channel::<i32>(10);

    // Run on a task context
    block_on(poll_fn(move |cx| {
        rx.close();

        assert_eq!(rx.poll_next_unpin(cx), Poll::Ready(None));
        match tx.poll_ready(cx) {
            Poll::Pending | Poll::Ready(Ok(_)) => panic!(),
            Poll::Ready(Err(e)) => assert!(e.is_disconnected()),
        };

        Poll::Ready(())
    }));
}

#[test]
fn tx_close_gets_none() {
    let (_, mut rx) = mpsc::channel::<i32>(10);

    // Run on a task context
    block_on(poll_fn(move |cx| {
        assert_eq!(rx.poll_next_unpin(cx), Poll::Ready(None));
        Poll::Ready(())
    }));
}

// #[test]
// fn spawn_sends_items() {
//     let core = local_executor::Core::new();
//     let stream = unfold(0, |i| Some(ok::<_,u8>((i, i + 1))));
//     let rx = mpsc::spawn(stream, &core, 1);
//     assert_eq!(core.run(rx.take(4).collect()).unwrap(),
//                [0, 1, 2, 3]);
// }

// #[test]
// fn spawn_kill_dead_stream() {
//     use std::thread;
//     use std::time::Duration;
//     use futures::future::Either;
//     use futures::sync::oneshot;
//
//     // a stream which never returns anything (maybe a remote end isn't
//     // responding), but dropping it leads to observable side effects
//     // (like closing connections, releasing limited resources, ...)
//     #[derive(Debug)]
//     struct Dead {
//         // when dropped you should get Err(oneshot::Canceled) on the
//         // receiving end
//         done: oneshot::Sender<()>,
//     }
//     impl Stream for Dead {
//         type Item = ();
//         type Error = ();
//
//         fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
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
//     let stream = Dead{done: done_tx};
//     let rx = mpsc::spawn(stream, &core, 1);
//     let res = core.run(
//         Ok::<_, ()>(())
//         .into_future()
//         .then(move |_| {
//             // now drop the spawned stream: maybe some timeout exceeded,
//             // or some connection on this end was closed by the remote
//             // end.
//             drop(rx);
//             // and wait for the spawned stream to release its resources
//             done_rx
//         })
//         .select2(timeout_rx)
//     );
//     match res {
//         Err(Either::A((oneshot::Canceled, _))) => (),
//         _ => {
//             panic!("dead stream wasn't canceled");
//         },
//     }
// }

#[test]
fn stress_shared_unbounded() {
    const AMT: u32 = if cfg!(miri) { 100 } else { 10000 };
    const NTHREADS: u32 = 8;
    let (tx, rx) = mpsc::unbounded::<i32>();

    let t = thread::spawn(move || {
        let result: Vec<_> = block_on(rx.collect());
        assert_eq!(result.len(), (AMT * NTHREADS) as usize);
        for item in result {
            assert_eq!(item, 1);
        }
    });

    for _ in 0..NTHREADS {
        let tx = tx.clone();

        thread::spawn(move || {
            for _ in 0..AMT {
                tx.unbounded_send(1).unwrap();
            }
        });
    }

    drop(tx);

    t.join().ok().unwrap();
}

#[test]
fn stress_shared_bounded_hard() {
    const AMT: u32 = if cfg!(miri) { 100 } else { 10000 };
    const NTHREADS: u32 = 8;
    let (tx, rx) = mpsc::channel::<i32>(0);

    let t = thread::spawn(move || {
        let result: Vec<_> = block_on(rx.collect());
        assert_eq!(result.len(), (AMT * NTHREADS) as usize);
        for item in result {
            assert_eq!(item, 1);
        }
    });

    for _ in 0..NTHREADS {
        let mut tx = tx.clone();

        thread::spawn(move || {
            for _ in 0..AMT {
                block_on(tx.send(1)).unwrap();
            }
        });
    }

    drop(tx);

    t.join().unwrap();
}

#[allow(clippy::same_item_push)]
#[test]
fn stress_receiver_multi_task_bounded_hard() {
    const AMT: usize = if cfg!(miri) { 100 } else { 10_000 };
    const NTHREADS: u32 = 2;

    let (mut tx, rx) = mpsc::channel::<usize>(0);
    let rx = Arc::new(Mutex::new(Some(rx)));
    let n = Arc::new(AtomicUsize::new(0));

    let mut th = vec![];

    for _ in 0..NTHREADS {
        let rx = rx.clone();
        let n = n.clone();

        let t = thread::spawn(move || {
            let mut i = 0;

            loop {
                i += 1;
                let mut rx_opt = rx.lock().unwrap();
                if let Some(rx) = &mut *rx_opt {
                    if i % 5 == 0 {
                        let item = block_on(rx.next());

                        if item.is_none() {
                            *rx_opt = None;
                            break;
                        }

                        n.fetch_add(1, Ordering::Relaxed);
                    } else {
                        // Just poll
                        let n = n.clone();
                        match rx.poll_next_unpin(&mut noop_context()) {
                            Poll::Ready(Some(_)) => {
                                n.fetch_add(1, Ordering::Relaxed);
                            }
                            Poll::Ready(None) => {
                                *rx_opt = None;
                                break;
                            }
                            Poll::Pending => {}
                        }
                    }
                } else {
                    break;
                }
            }
        });

        th.push(t);
    }

    for i in 0..AMT {
        block_on(tx.send(i)).unwrap();
    }
    drop(tx);

    for t in th {
        t.join().unwrap();
    }

    assert_eq!(AMT, n.load(Ordering::Relaxed));
}

/// Stress test that receiver properly receives all the messages
/// after sender dropped.
#[test]
fn stress_drop_sender() {
    const ITER: usize = if cfg!(miri) { 100 } else { 10000 };

    fn list() -> impl Stream<Item = i32> {
        let (tx, rx) = mpsc::channel(1);
        thread::spawn(move || {
            block_on(send_one_two_three(tx));
        });
        rx
    }

    for _ in 0..ITER {
        let v: Vec<_> = block_on(list().collect());
        assert_eq!(v, vec![1, 2, 3]);
    }
}

async fn send_one_two_three(mut tx: mpsc::Sender<i32>) {
    for i in 1..=3 {
        tx.send(i).await.unwrap();
    }
}

/// Stress test that after receiver dropped,
/// no messages are lost.
fn stress_close_receiver_iter() {
    let (tx, rx) = mpsc::unbounded();
    let mut rx = block_on_stream(rx);
    let (unwritten_tx, unwritten_rx) = std::sync::mpsc::channel();
    let th = thread::spawn(move || {
        for i in 1.. {
            if tx.unbounded_send(i).is_err() {
                unwritten_tx.send(i).expect("unwritten_tx");
                return;
            }
        }
    });

    // Read one message to make sure thread effectively started
    assert_eq!(Some(1), rx.next());

    rx.close();

    for i in 2.. {
        match rx.next() {
            Some(r) => assert!(i == r),
            None => {
                let unwritten = unwritten_rx.recv().expect("unwritten_rx");
                assert_eq!(unwritten, i);
                th.join().unwrap();
                return;
            }
        }
    }
}

#[test]
fn stress_close_receiver() {
    const ITER: usize = if cfg!(miri) { 50 } else { 10000 };

    for _ in 0..ITER {
        stress_close_receiver_iter();
    }
}

async fn stress_poll_ready_sender(mut sender: mpsc::Sender<u32>, count: u32) {
    for i in (1..=count).rev() {
        sender.send(i).await.unwrap();
    }
}

/// Tests that after `poll_ready` indicates capacity a channel can always send without waiting.
#[allow(clippy::same_item_push)]
#[test]
fn stress_poll_ready() {
    const AMT: u32 = if cfg!(miri) { 100 } else { 1000 };
    const NTHREADS: u32 = 8;

    /// Run a stress test using the specified channel capacity.
    fn stress(capacity: usize) {
        let (tx, rx) = mpsc::channel(capacity);
        let mut threads = Vec::new();
        for _ in 0..NTHREADS {
            let sender = tx.clone();
            threads.push(thread::spawn(move || block_on(stress_poll_ready_sender(sender, AMT))));
        }
        drop(tx);

        let result: Vec<_> = block_on(rx.collect());
        assert_eq!(result.len() as u32, AMT * NTHREADS);

        for thread in threads {
            thread.join().unwrap();
        }
    }

    stress(0);
    stress(1);
    stress(8);
    stress(16);
}

#[test]
fn try_send_1() {
    const N: usize = if cfg!(miri) { 100 } else { 3000 };
    let (mut tx, rx) = mpsc::channel(0);

    let t = thread::spawn(move || {
        for i in 0..N {
            loop {
                if tx.try_send(i).is_ok() {
                    break;
                }
            }
        }
    });

    let result: Vec<_> = block_on(rx.collect());
    for (i, j) in result.into_iter().enumerate() {
        assert_eq!(i, j);
    }

    t.join().unwrap();
}

#[test]
fn try_send_2() {
    let (mut tx, rx) = mpsc::channel(0);
    let mut rx = block_on_stream(rx);

    tx.try_send("hello").unwrap();

    let (readytx, readyrx) = oneshot::channel::<()>();

    let th = thread::spawn(move || {
        block_on(poll_fn(|cx| {
            assert!(tx.poll_ready(cx).is_pending());
            Poll::Ready(())
        }));

        drop(readytx);
        block_on(tx.send("goodbye")).unwrap();
    });

    let _ = block_on(readyrx);
    assert_eq!(rx.next(), Some("hello"));
    assert_eq!(rx.next(), Some("goodbye"));
    assert_eq!(rx.next(), None);

    th.join().unwrap();
}

#[test]
fn try_send_fail() {
    let (mut tx, rx) = mpsc::channel(0);
    let mut rx = block_on_stream(rx);

    tx.try_send("hello").unwrap();

    // This should fail
    assert!(tx.try_send("fail").is_err());

    assert_eq!(rx.next(), Some("hello"));

    tx.try_send("goodbye").unwrap();
    drop(tx);

    assert_eq!(rx.next(), Some("goodbye"));
    assert_eq!(rx.next(), None);
}

#[test]
fn try_send_recv() {
    let (mut tx, mut rx) = mpsc::channel(1);
    tx.try_send("hello").unwrap();
    tx.try_send("hello").unwrap();
    tx.try_send("hello").unwrap_err(); // should be full
    rx.try_next().unwrap();
    rx.try_next().unwrap();
    rx.try_next().unwrap_err(); // should be empty
    tx.try_send("hello").unwrap();
    rx.try_next().unwrap();
    rx.try_next().unwrap_err(); // should be empty
}

#[test]
fn same_receiver() {
    let (mut txa1, _) = mpsc::channel::<i32>(1);
    let txa2 = txa1.clone();

    let (mut txb1, _) = mpsc::channel::<i32>(1);
    let txb2 = txb1.clone();

    assert!(txa1.same_receiver(&txa2));
    assert!(txb1.same_receiver(&txb2));
    assert!(!txa1.same_receiver(&txb1));

    txa1.disconnect();
    txb1.close_channel();

    assert!(!txa1.same_receiver(&txa2));
    assert!(txb1.same_receiver(&txb2));
}

#[test]
fn is_connected_to() {
    let (txa, rxa) = mpsc::channel::<i32>(1);
    let (txb, rxb) = mpsc::channel::<i32>(1);

    assert!(txa.is_connected_to(&rxa));
    assert!(txb.is_connected_to(&rxb));
    assert!(!txa.is_connected_to(&rxb));
    assert!(!txb.is_connected_to(&rxa));
}

#[test]
fn hash_receiver() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    let mut hasher_a1 = DefaultHasher::new();
    let mut hasher_a2 = DefaultHasher::new();
    let mut hasher_b1 = DefaultHasher::new();
    let mut hasher_b2 = DefaultHasher::new();
    let (mut txa1, _) = mpsc::channel::<i32>(1);
    let txa2 = txa1.clone();

    let (mut txb1, _) = mpsc::channel::<i32>(1);
    let txb2 = txb1.clone();

    txa1.hash_receiver(&mut hasher_a1);
    let hash_a1 = hasher_a1.finish();
    txa2.hash_receiver(&mut hasher_a2);
    let hash_a2 = hasher_a2.finish();
    txb1.hash_receiver(&mut hasher_b1);
    let hash_b1 = hasher_b1.finish();
    txb2.hash_receiver(&mut hasher_b2);
    let hash_b2 = hasher_b2.finish();

    assert_eq!(hash_a1, hash_a2);
    assert_eq!(hash_b1, hash_b2);
    assert!(hash_a1 != hash_b1);

    txa1.disconnect();
    txb1.close_channel();

    let mut hasher_a1 = DefaultHasher::new();
    let mut hasher_a2 = DefaultHasher::new();
    let mut hasher_b1 = DefaultHasher::new();
    let mut hasher_b2 = DefaultHasher::new();

    txa1.hash_receiver(&mut hasher_a1);
    let hash_a1 = hasher_a1.finish();
    txa2.hash_receiver(&mut hasher_a2);
    let hash_a2 = hasher_a2.finish();
    txb1.hash_receiver(&mut hasher_b1);
    let hash_b1 = hasher_b1.finish();
    txb2.hash_receiver(&mut hasher_b2);
    let hash_b2 = hasher_b2.finish();

    assert!(hash_a1 != hash_a2);
    assert_eq!(hash_b1, hash_b2);
}

#[test]
fn send_backpressure() {
    let (waker, counter) = new_count_waker();
    let mut cx = Context::from_waker(&waker);

    let (mut tx, mut rx) = mpsc::channel(1);
    block_on(tx.send(1)).unwrap();

    let mut task = tx.send(2);
    assert_eq!(task.poll_unpin(&mut cx), Poll::Pending);
    assert_eq!(counter, 0);

    let item = block_on(rx.next()).unwrap();
    assert_eq!(item, 1);
    assert_eq!(counter, 1);
    assert_eq!(task.poll_unpin(&mut cx), Poll::Ready(Ok(())));

    let item = block_on(rx.next()).unwrap();
    assert_eq!(item, 2);
}

#[test]
fn send_backpressure_multi_senders() {
    let (waker, counter) = new_count_waker();
    let mut cx = Context::from_waker(&waker);

    let (mut tx1, mut rx) = mpsc::channel(1);
    let mut tx2 = tx1.clone();
    block_on(tx1.send(1)).unwrap();

    let mut task = tx2.send(2);
    assert_eq!(task.poll_unpin(&mut cx), Poll::Pending);
    assert_eq!(counter, 0);

    let item = block_on(rx.next()).unwrap();
    assert_eq!(item, 1);
    assert_eq!(counter, 1);
    assert_eq!(task.poll_unpin(&mut cx), Poll::Ready(Ok(())));

    let item = block_on(rx.next()).unwrap();
    assert_eq!(item, 2);
}

/// Test that empty channel has zero length and that non-empty channel has length equal to number
/// of enqueued items
#[test]
fn unbounded_len() {
    let (tx, mut rx) = mpsc::unbounded();
    assert_eq!(tx.len(), 0);
    assert!(tx.is_empty());
    tx.unbounded_send(1).unwrap();
    assert_eq!(tx.len(), 1);
    assert!(!tx.is_empty());
    tx.unbounded_send(2).unwrap();
    assert_eq!(tx.len(), 2);
    assert!(!tx.is_empty());
    let item = block_on(rx.next()).unwrap();
    assert_eq!(item, 1);
    assert_eq!(tx.len(), 1);
    assert!(!tx.is_empty());
    let item = block_on(rx.next()).unwrap();
    assert_eq!(item, 2);
    assert_eq!(tx.len(), 0);
    assert!(tx.is_empty());
}
