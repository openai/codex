mod queue;
mod shutdown;
mod yield_now;

/// Full runtime loom tests. These are heavy tests and take significant time to
/// run on CI.
///
/// Use `LOOM_MAX_PREEMPTIONS=1` to do a "quick" run as a smoke test.
///
/// In order to speed up the C
use crate::runtime::tests::loom_oneshot as oneshot;
use crate::runtime::{self, Runtime};
use crate::{spawn, task};
use tokio_test::assert_ok;

use loom::sync::atomic::{AtomicBool, AtomicUsize};
use loom::sync::Arc;

use pin_project_lite::pin_project;
use std::future::{poll_fn, Future};
use std::pin::Pin;
use std::sync::atomic::Ordering::{Relaxed, SeqCst};
use std::task::{ready, Context, Poll};

mod atomic_take {
    use loom::sync::atomic::AtomicBool;
    use std::mem::MaybeUninit;
    use std::sync::atomic::Ordering::SeqCst;

    pub(super) struct AtomicTake<T> {
        inner: MaybeUninit<T>,
        taken: AtomicBool,
    }

    impl<T> AtomicTake<T> {
        pub(super) fn new(value: T) -> Self {
            Self {
                inner: MaybeUninit::new(value),
                taken: AtomicBool::new(false),
            }
        }

        pub(super) fn take(&self) -> Option<T> {
            // safety: Only one thread will see the boolean change from false
            // to true, so that thread is able to take the value.
            match self.taken.fetch_or(true, SeqCst) {
                false => unsafe { Some(std::ptr::read(self.inner.as_ptr())) },
                true => None,
            }
        }
    }

    impl<T> Drop for AtomicTake<T> {
        fn drop(&mut self) {
            drop(self.take());
        }
    }
}

#[derive(Clone)]
struct AtomicOneshot<T> {
    value: std::sync::Arc<atomic_take::AtomicTake<oneshot::Sender<T>>>,
}
impl<T> AtomicOneshot<T> {
    fn new(sender: oneshot::Sender<T>) -> Self {
        Self {
            value: std::sync::Arc::new(atomic_take::AtomicTake::new(sender)),
        }
    }

    fn assert_send(&self, value: T) {
        self.value.take().unwrap().send(value);
    }
}

/// Tests are divided into groups to make the runs faster on CI.
mod group_a {
    use super::*;

    #[test]
    fn racy_shutdown() {
        loom::model(|| {
            let pool = mk_pool(1);

            // here's the case we want to exercise:
            //
            // a worker that still has tasks in its local queue gets sent to the blocking pool (due to
            // block_in_place). the blocking pool is shut down, so drops the worker. the worker's
            // shutdown method never gets run.
            //
            // we do this by spawning two tasks on one worker, the first of which does block_in_place,
            // and then immediately drop the pool.

            pool.spawn(track(async {
                crate::task::block_in_place(|| {});
            }));
            pool.spawn(track(async {}));
            drop(pool);
        });
    }

    #[test]
    fn pool_multi_spawn() {
        loom::model(|| {
            let pool = mk_pool(2);
            let c1 = Arc::new(AtomicUsize::new(0));

            let (tx, rx) = oneshot::channel();
            let tx1 = AtomicOneshot::new(tx);

            // Spawn a task
            let c2 = c1.clone();
            let tx2 = tx1.clone();
            pool.spawn(track(async move {
                spawn(track(async move {
                    if 1 == c1.fetch_add(1, Relaxed) {
                        tx1.assert_send(());
                    }
                }));
            }));

            // Spawn a second task
            pool.spawn(track(async move {
                spawn(track(async move {
                    if 1 == c2.fetch_add(1, Relaxed) {
                        tx2.assert_send(());
                    }
                }));
            }));

            rx.recv();
        });
    }

    fn only_blocking_inner(first_pending: bool) {
        loom::model(move || {
            let pool = mk_pool(1);
            let (block_tx, block_rx) = oneshot::channel();

            pool.spawn(track(async move {
                crate::task::block_in_place(move || {
                    block_tx.send(());
                });
                if first_pending {
                    task::yield_now().await
                }
            }));

            block_rx.recv();
            drop(pool);
        });
    }

    #[test]
    fn only_blocking_without_pending() {
        only_blocking_inner(false)
    }

    #[test]
    fn only_blocking_with_pending() {
        only_blocking_inner(true)
    }
}

mod group_b {
    use super::*;

    fn blocking_and_regular_inner(first_pending: bool) {
        const NUM: usize = 3;
        loom::model(move || {
            let pool = mk_pool(1);
            let cnt = Arc::new(AtomicUsize::new(0));

            let (block_tx, block_rx) = oneshot::channel();
            let (done_tx, done_rx) = oneshot::channel();
            let done_tx = AtomicOneshot::new(done_tx);

            pool.spawn(track(async move {
                crate::task::block_in_place(move || {
                    block_tx.send(());
                });
                if first_pending {
                    task::yield_now().await
                }
            }));

            for _ in 0..NUM {
                let cnt = cnt.clone();
                let done_tx = done_tx.clone();

                pool.spawn(track(async move {
                    if NUM == cnt.fetch_add(1, Relaxed) + 1 {
                        done_tx.assert_send(());
                    }
                }));
            }

            done_rx.recv();
            block_rx.recv();

            drop(pool);
        });
    }

    #[test]
    fn blocking_and_regular() {
        blocking_and_regular_inner(false);
    }

    #[test]
    fn blocking_and_regular_with_pending() {
        blocking_and_regular_inner(true);
    }

    #[test]
    fn join_output() {
        loom::model(|| {
            let rt = mk_pool(1);

            rt.block_on(async {
                let t = crate::spawn(track(async { "hello" }));

                let out = assert_ok!(t.await);
                assert_eq!("hello", out.into_inner());
            });
        });
    }

    #[test]
    fn poll_drop_handle_then_drop() {
        loom::model(|| {
            let rt = mk_pool(1);

            rt.block_on(async move {
                let mut t = crate::spawn(track(async { "hello" }));

                poll_fn(|cx| {
                    let _ = Pin::new(&mut t).poll(cx);
                    Poll::Ready(())
                })
                .await;
            });
        })
    }

    #[test]
    fn complete_block_on_under_load() {
        loom::model(|| {
            let pool = mk_pool(1);

            pool.block_on(async {
                // Trigger a re-schedule
                crate::spawn(track(async {
                    for _ in 0..2 {
                        task::yield_now().await;
                    }
                }));

                gated2(true).await
            });
        });
    }

    #[test]
    fn shutdown_with_notification() {
        use crate::sync::oneshot;

        loom::model(|| {
            let rt = mk_pool(2);
            let (done_tx, done_rx) = oneshot::channel::<()>();

            rt.spawn(track(async move {
                let (tx, rx) = oneshot::channel::<()>();

                crate::spawn(async move {
                    crate::task::spawn_blocking(move || {
                        let _ = tx.send(());
                    });

                    let _ = done_rx.await;
                });

                let _ = rx.await;

                let _ = done_tx.send(());
            }));
        });
    }
}

mod group_c {
    use super::*;

    #[test]
    fn pool_shutdown() {
        loom::model(|| {
            let pool = mk_pool(2);

            pool.spawn(track(async move {
                gated2(true).await;
            }));

            pool.spawn(track(async move {
                gated2(false).await;
            }));

            drop(pool);
        });
    }
}

mod group_d {
    use super::*;

    #[test]
    fn pool_multi_notify() {
        loom::model(|| {
            let pool = mk_pool(2);

            let c1 = Arc::new(AtomicUsize::new(0));

            let (done_tx, done_rx) = oneshot::channel();
            let done_tx1 = AtomicOneshot::new(done_tx);
            let done_tx2 = done_tx1.clone();

            // Spawn a task
            let c2 = c1.clone();
            pool.spawn(track(async move {
                multi_gated().await;

                if 1 == c1.fetch_add(1, Relaxed) {
                    done_tx1.assert_send(());
                }
            }));

            // Spawn a second task
            pool.spawn(track(async move {
                multi_gated().await;

                if 1 == c2.fetch_add(1, Relaxed) {
                    done_tx2.assert_send(());
                }
            }));

            done_rx.recv();
        });
    }
}

fn mk_pool(num_threads: usize) -> Runtime {
    runtime::Builder::new_multi_thread()
        .worker_threads(num_threads)
        // Set the intervals to avoid tuning logic
        .event_interval(2)
        .build()
        .unwrap()
}

fn gated2(thread: bool) -> impl Future<Output = &'static str> {
    use loom::thread;
    use std::sync::Arc;

    let gate = Arc::new(AtomicBool::new(false));
    let mut fired = false;

    poll_fn(move |cx| {
        if !fired {
            let gate = gate.clone();
            let waker = cx.waker().clone();

            if thread {
                thread::spawn(move || {
                    gate.store(true, SeqCst);
                    waker.wake_by_ref();
                });
            } else {
                spawn(track(async move {
                    gate.store(true, SeqCst);
                    waker.wake_by_ref();
                }));
            }

            fired = true;

            return Poll::Pending;
        }

        if gate.load(SeqCst) {
            Poll::Ready("hello world")
        } else {
            Poll::Pending
        }
    })
}

async fn multi_gated() {
    struct Gate {
        waker: loom::future::AtomicWaker,
        count: AtomicUsize,
    }

    let gate = Arc::new(Gate {
        waker: loom::future::AtomicWaker::new(),
        count: AtomicUsize::new(0),
    });

    {
        let gate = gate.clone();
        spawn(track(async move {
            for i in 1..3 {
                gate.count.store(i, SeqCst);
                gate.waker.wake();
            }
        }));
    }

    poll_fn(move |cx| {
        gate.waker.register_by_ref(cx.waker());
        if gate.count.load(SeqCst) < 2 {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await;
}

fn track<T: Future>(f: T) -> Track<T> {
    Track {
        inner: f,
        arc: Arc::new(()),
    }
}

pin_project! {
    struct Track<T> {
        #[pin]
        inner: T,
        // Arc is used to hook into loom's leak tracking.
        arc: Arc<()>,
    }
}

impl<T> Track<T> {
    fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: Future> Future for Track<T> {
    type Output = Track<T::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.project();

        Poll::Ready(Track {
            inner: ready!(me.inner.poll(cx)),
            arc: me.arc.clone(),
        })
    }
}
