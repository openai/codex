use crate::sync::Notify;

use loom::future::block_on;
use loom::sync::Arc;
use loom::thread;

use tokio_test::{assert_pending, assert_ready};

/// `util::wake_list::NUM_WAKERS`
const WAKE_LIST_SIZE: usize = 32;

#[test]
fn notify_one() {
    loom::model(|| {
        let tx = Arc::new(Notify::new());
        let rx = tx.clone();

        let th = thread::spawn(move || {
            block_on(async {
                rx.notified().await;
            });
        });

        tx.notify_one();
        th.join().unwrap();
    });
}

#[test]
fn notify_waiters() {
    loom::model(|| {
        let notify = Arc::new(Notify::new());
        let tx = notify.clone();
        let notified1 = notify.notified();
        let notified2 = notify.notified();

        let th = thread::spawn(move || {
            tx.notify_waiters();
        });

        block_on(async {
            notified1.await;
            notified2.await;
        });

        th.join().unwrap();
    });
}

#[test]
fn notify_waiters_and_one() {
    loom::model(|| {
        let notify = Arc::new(Notify::new());
        let tx1 = notify.clone();
        let tx2 = notify.clone();

        let th1 = thread::spawn(move || {
            tx1.notify_waiters();
        });

        let th2 = thread::spawn(move || {
            tx2.notify_one();
        });

        let th3 = thread::spawn(move || {
            let notified = notify.notified();

            block_on(async {
                notified.await;
            });
        });

        th1.join().unwrap();
        th2.join().unwrap();
        th3.join().unwrap();
    });
}

#[test]
fn notify_multi() {
    loom::model(|| {
        let notify = Arc::new(Notify::new());

        let mut ths = vec![];

        for _ in 0..2 {
            let notify = notify.clone();

            ths.push(thread::spawn(move || {
                block_on(async {
                    notify.notified().await;
                    notify.notify_one();
                })
            }));
        }

        notify.notify_one();

        for th in ths.drain(..) {
            th.join().unwrap();
        }

        block_on(async {
            notify.notified().await;
        });
    });
}

#[test]
fn notify_drop() {
    use std::future::{poll_fn, Future};
    use std::task::Poll;

    loom::model(|| {
        let notify = Arc::new(Notify::new());
        let rx1 = notify.clone();
        let rx2 = notify.clone();

        let th1 = thread::spawn(move || {
            let mut recv = Box::pin(rx1.notified());

            block_on(poll_fn(|cx| {
                if recv.as_mut().poll(cx).is_ready() {
                    rx1.notify_one();
                }
                Poll::Ready(())
            }));
        });

        let th2 = thread::spawn(move || {
            block_on(async {
                rx2.notified().await;
                // Trigger second notification
                rx2.notify_one();
                rx2.notified().await;
            });
        });

        notify.notify_one();

        th1.join().unwrap();
        th2.join().unwrap();
    });
}

/// Polls two `Notified` futures and checks if poll results are consistent
/// with each other. If the first future is notified by a `notify_waiters`
/// call, then the second one must be notified as well.
#[test]
fn notify_waiters_poll_consistency() {
    fn notify_waiters_poll_consistency_variant(poll_setting: [bool; 2]) {
        let notify = Arc::new(Notify::new());
        let mut notified = [
            tokio_test::task::spawn(notify.notified()),
            tokio_test::task::spawn(notify.notified()),
        ];
        for i in 0..2 {
            if poll_setting[i] {
                assert_pending!(notified[i].poll());
            }
        }

        let tx = notify.clone();
        let th = thread::spawn(move || {
            tx.notify_waiters();
        });

        let res1 = notified[0].poll();
        let res2 = notified[1].poll();

        // If res1 is ready, then res2 must also be ready.
        assert!(res1.is_pending() || res2.is_ready());

        th.join().unwrap();
    }

    // We test different scenarios in which pending futures had or had not
    // been polled before the call to `notify_waiters`.
    loom::model(|| notify_waiters_poll_consistency_variant([false, false]));
    loom::model(|| notify_waiters_poll_consistency_variant([true, false]));
    loom::model(|| notify_waiters_poll_consistency_variant([false, true]));
    loom::model(|| notify_waiters_poll_consistency_variant([true, true]));
}

/// Polls two `Notified` futures and checks if poll results are consistent
/// with each other. If the first future is notified by a `notify_waiters`
/// call, then the second one must be notified as well.
///
/// Here we also add other `Notified` futures in between to force the two
/// tested futures to end up in different chunks.
#[test]
fn notify_waiters_poll_consistency_many() {
    fn notify_waiters_poll_consistency_many_variant(order: [usize; 2]) {
        let notify = Arc::new(Notify::new());

        let mut futs = (0..WAKE_LIST_SIZE + 1)
            .map(|_| tokio_test::task::spawn(notify.notified()))
            .collect::<Vec<_>>();

        assert_pending!(futs[order[0]].poll());
        for i in 2..futs.len() {
            assert_pending!(futs[i].poll());
        }
        assert_pending!(futs[order[1]].poll());

        let tx = notify.clone();
        let th = thread::spawn(move || {
            tx.notify_waiters();
        });

        let res1 = futs[0].poll();
        let res2 = futs[1].poll();

        // If res1 is ready, then res2 must also be ready.
        assert!(res1.is_pending() || res2.is_ready());

        th.join().unwrap();
    }

    // We test different scenarios in which futures are polled in different order.
    loom::model(|| notify_waiters_poll_consistency_many_variant([0, 1]));
    loom::model(|| notify_waiters_poll_consistency_many_variant([1, 0]));
}

/// Checks if a call to `notify_waiters` is observed as atomic when combined
/// with a concurrent call to `notify_one`.
#[test]
fn notify_waiters_is_atomic() {
    fn notify_waiters_is_atomic_variant(tested_fut_index: usize) {
        let notify = Arc::new(Notify::new());

        let mut futs = (0..WAKE_LIST_SIZE + 1)
            .map(|_| tokio_test::task::spawn(notify.notified()))
            .collect::<Vec<_>>();

        for fut in &mut futs {
            assert_pending!(fut.poll());
        }

        let tx = notify.clone();
        let th = thread::spawn(move || {
            tx.notify_waiters();
        });

        block_on(async {
            // If awaiting one of the futures completes, then we should be
            // able to assume that all pending futures are notified. Therefore
            // a notification from a subsequent `notify_one` call should not
            // be consumed by an old future.
            futs.remove(tested_fut_index).await;

            let mut new_fut = tokio_test::task::spawn(notify.notified());
            assert_pending!(new_fut.poll());

            notify.notify_one();

            // `new_fut` must consume the notification from `notify_one`.
            assert_ready!(new_fut.poll());
        });

        th.join().unwrap();
    }

    // We test different scenarios in which the tested future is at the beginning
    // or at the end of the waiters queue used by `Notify`.
    loom::model(|| notify_waiters_is_atomic_variant(0));
    loom::model(|| notify_waiters_is_atomic_variant(32));
}

/// Checks if a single call to `notify_waiters` does not get through two `Notified`
/// futures created and awaited sequentially like this:
/// ```ignore
/// notify.notified().await;
/// notify.notified().await;
/// ```
#[test]
fn notify_waiters_sequential_notified_await() {
    use crate::sync::oneshot;

    loom::model(|| {
        let notify = Arc::new(Notify::new());

        let (tx_fst, rx_fst) = oneshot::channel();
        let (tx_snd, rx_snd) = oneshot::channel();

        let receiver = thread::spawn({
            let notify = notify.clone();
            move || {
                block_on(async {
                    // Poll the first `Notified` to put it as the first waiter
                    // in the queue.
                    let mut first_notified = tokio_test::task::spawn(notify.notified());
                    assert_pending!(first_notified.poll());

                    // Create additional waiters to force `notify_waiters` to
                    // release the lock at least once.
                    let _task_pile = (0..WAKE_LIST_SIZE + 1)
                        .map(|_| {
                            let mut fut = tokio_test::task::spawn(notify.notified());
                            assert_pending!(fut.poll());
                            fut
                        })
                        .collect::<Vec<_>>();

                    // We are ready for the notify_waiters call.
                    tx_fst.send(()).unwrap();

                    first_notified.await;

                    // Poll the second `Notified` future to try to insert
                    // it to the waiters queue.
                    let mut second_notified = tokio_test::task::spawn(notify.notified());
                    assert_pending!(second_notified.poll());

                    // Wait for the `notify_waiters` to end and check if we
                    // are woken up.
                    rx_snd.await.unwrap();
                    assert_pending!(second_notified.poll());
                });
            }
        });

        // Wait for the signal and call `notify_waiters`.
        block_on(rx_fst).unwrap();
        notify.notify_waiters();
        tx_snd.send(()).unwrap();

        receiver.join().unwrap();
    });
}
