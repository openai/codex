use crate::runtime::task::{
    self, unowned, Id, JoinHandle, OwnedTasks, Schedule, SpawnLocation, Task,
    TaskHarnessScheduleHooks,
};
use crate::runtime::tests::NoopSchedule;

use std::collections::VecDeque;
use std::future::Future;
#[cfg(tokio_unstable)]
use std::panic::Location;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct AssertDropHandle {
    is_dropped: Arc<AtomicBool>,
}
impl AssertDropHandle {
    #[track_caller]
    fn assert_dropped(&self) {
        assert!(self.is_dropped.load(Ordering::SeqCst));
    }

    #[track_caller]
    fn assert_not_dropped(&self) {
        assert!(!self.is_dropped.load(Ordering::SeqCst));
    }
}

struct AssertDrop {
    is_dropped: Arc<AtomicBool>,
}
impl AssertDrop {
    fn new() -> (Self, AssertDropHandle) {
        let shared = Arc::new(AtomicBool::new(false));
        (
            AssertDrop {
                is_dropped: shared.clone(),
            },
            AssertDropHandle {
                is_dropped: shared.clone(),
            },
        )
    }
}
impl Drop for AssertDrop {
    fn drop(&mut self) {
        self.is_dropped.store(true, Ordering::SeqCst);
    }
}

// A Notified does not shut down on drop, but it is dropped once the ref-count
// hits zero.
#[test]
fn create_drop1() {
    let (ad, handle) = AssertDrop::new();
    let (notified, join) = unowned(
        async {
            drop(ad);
            unreachable!()
        },
        NoopSchedule,
        Id::next(),
        SpawnLocation::capture(),
    );
    drop(notified);
    handle.assert_not_dropped();
    drop(join);
    handle.assert_dropped();
}

#[test]
fn create_drop2() {
    let (ad, handle) = AssertDrop::new();
    let (notified, join) = unowned(
        async {
            drop(ad);
            unreachable!()
        },
        NoopSchedule,
        Id::next(),
        SpawnLocation::capture(),
    );
    drop(join);
    handle.assert_not_dropped();
    drop(notified);
    handle.assert_dropped();
}

#[test]
fn drop_abort_handle1() {
    let (ad, handle) = AssertDrop::new();
    let (notified, join) = unowned(
        async {
            drop(ad);
            unreachable!()
        },
        NoopSchedule,
        Id::next(),
        SpawnLocation::capture(),
    );
    let abort = join.abort_handle();
    drop(join);
    handle.assert_not_dropped();
    drop(notified);
    handle.assert_not_dropped();
    drop(abort);
    handle.assert_dropped();
}

#[test]
fn drop_abort_handle2() {
    let (ad, handle) = AssertDrop::new();
    let (notified, join) = unowned(
        async {
            drop(ad);
            unreachable!()
        },
        NoopSchedule,
        Id::next(),
        SpawnLocation::capture(),
    );
    let abort = join.abort_handle();
    drop(notified);
    handle.assert_not_dropped();
    drop(abort);
    handle.assert_not_dropped();
    drop(join);
    handle.assert_dropped();
}

#[test]
fn drop_abort_handle_clone() {
    let (ad, handle) = AssertDrop::new();
    let (notified, join) = unowned(
        async {
            drop(ad);
            unreachable!()
        },
        NoopSchedule,
        Id::next(),
        SpawnLocation::capture(),
    );
    let abort = join.abort_handle();
    let abort_clone = abort.clone();
    drop(join);
    handle.assert_not_dropped();
    drop(notified);
    handle.assert_not_dropped();
    drop(abort);
    handle.assert_not_dropped();
    drop(abort_clone);
    handle.assert_dropped();
}

// Shutting down through Notified works
#[test]
fn create_shutdown1() {
    let (ad, handle) = AssertDrop::new();
    let (notified, join) = unowned(
        async {
            drop(ad);
            unreachable!()
        },
        NoopSchedule,
        Id::next(),
        SpawnLocation::capture(),
    );
    drop(join);
    handle.assert_not_dropped();
    notified.shutdown();
    handle.assert_dropped();
}

#[test]
fn create_shutdown2() {
    let (ad, handle) = AssertDrop::new();
    let (notified, join) = unowned(
        async {
            drop(ad);
            unreachable!()
        },
        NoopSchedule,
        Id::next(),
        SpawnLocation::capture(),
    );
    handle.assert_not_dropped();
    notified.shutdown();
    handle.assert_dropped();
    drop(join);
}

#[test]
fn unowned_poll() {
    let (task, _) = unowned(async {}, NoopSchedule, Id::next(), SpawnLocation::capture());
    task.run();
}

#[test]
fn schedule() {
    with(|rt| {
        rt.spawn(async {
            crate::task::yield_now().await;
        });

        assert_eq!(2, rt.tick());
        rt.shutdown();
    })
}

#[test]
fn shutdown() {
    with(|rt| {
        rt.spawn(async {
            loop {
                crate::task::yield_now().await;
            }
        });

        rt.tick_max(1);

        rt.shutdown();
    })
}

#[test]
fn shutdown_immediately() {
    with(|rt| {
        rt.spawn(async {
            loop {
                crate::task::yield_now().await;
            }
        });

        rt.shutdown();
    })
}

// Test for https://github.com/tokio-rs/tokio/issues/6729
#[test]
fn spawn_niche_in_task() {
    use std::future::poll_fn;
    use std::task::{Context, Poll, Waker};

    with(|rt| {
        let state = Arc::new(Mutex::new(State::new()));

        let mut subscriber = Subscriber::new(Arc::clone(&state), 1);
        rt.spawn(async move {
            subscriber.wait().await;
            subscriber.wait().await;
        });

        rt.spawn(async move {
            state.lock().unwrap().set_version(2);
            state.lock().unwrap().set_version(0);
        });

        rt.tick_max(10);
        assert!(rt.is_empty());
        rt.shutdown();
    });

    pub(crate) struct Subscriber {
        state: Arc<Mutex<State>>,
        observed_version: u64,
        waker_key: Option<usize>,
    }

    impl Subscriber {
        pub(crate) fn new(state: Arc<Mutex<State>>, version: u64) -> Self {
            Self {
                state,
                observed_version: version,
                waker_key: None,
            }
        }

        pub(crate) async fn wait(&mut self) {
            poll_fn(|cx| {
                self.state
                    .lock()
                    .unwrap()
                    .poll_update(&mut self.observed_version, &mut self.waker_key, cx)
                    .map(|_| ())
            })
            .await;
        }
    }

    struct State {
        version: u64,
        wakers: Vec<Waker>,
    }

    impl State {
        pub(crate) fn new() -> Self {
            Self {
                version: 1,
                wakers: Vec::new(),
            }
        }

        pub(crate) fn poll_update(
            &mut self,
            observed_version: &mut u64,
            waker_key: &mut Option<usize>,
            cx: &Context<'_>,
        ) -> Poll<Option<()>> {
            if self.version == 0 {
                *waker_key = None;
                Poll::Ready(None)
            } else if *observed_version < self.version {
                *waker_key = None;
                *observed_version = self.version;
                Poll::Ready(Some(()))
            } else {
                self.wakers.push(cx.waker().clone());
                *waker_key = Some(self.wakers.len());
                Poll::Pending
            }
        }

        pub(crate) fn set_version(&mut self, version: u64) {
            self.version = version;
            for waker in self.wakers.drain(..) {
                waker.wake();
            }
        }
    }
}

#[test]
fn spawn_during_shutdown() {
    static DID_SPAWN: AtomicBool = AtomicBool::new(false);

    struct SpawnOnDrop(Runtime);
    impl Drop for SpawnOnDrop {
        fn drop(&mut self) {
            DID_SPAWN.store(true, Ordering::SeqCst);
            self.0.spawn(async {});
        }
    }

    with(|rt| {
        let rt2 = rt.clone();
        rt.spawn(async move {
            let _spawn_on_drop = SpawnOnDrop(rt2);

            loop {
                crate::task::yield_now().await;
            }
        });

        rt.tick_max(1);
        rt.shutdown();
    });

    assert!(DID_SPAWN.load(Ordering::SeqCst));
}

fn with(f: impl FnOnce(Runtime)) {
    struct Reset;

    impl Drop for Reset {
        fn drop(&mut self) {
            let _rt = CURRENT.try_lock().unwrap().take();
        }
    }

    let _reset = Reset;

    let rt = Runtime(Arc::new(Inner {
        owned: OwnedTasks::new(16),
        core: Mutex::new(Core {
            queue: VecDeque::new(),
        }),
    }));

    *CURRENT.try_lock().unwrap() = Some(rt.clone());
    f(rt)
}

#[derive(Clone)]
struct Runtime(Arc<Inner>);

struct Inner {
    core: Mutex<Core>,
    owned: OwnedTasks<Runtime>,
}

struct Core {
    queue: VecDeque<task::Notified<Runtime>>,
}

static CURRENT: Mutex<Option<Runtime>> = Mutex::new(None);

impl Runtime {
    #[track_caller]
    fn spawn<T>(&self, future: T) -> JoinHandle<T::Output>
    where
        T: 'static + Send + Future,
        T::Output: 'static + Send,
    {
        let (handle, notified) =
            self.0
                .owned
                .bind(future, self.clone(), Id::next(), SpawnLocation::capture());

        if let Some(notified) = notified {
            self.schedule(notified);
        }

        handle
    }

    fn tick(&self) -> usize {
        self.tick_max(usize::MAX)
    }

    fn tick_max(&self, max: usize) -> usize {
        let mut n = 0;

        while !self.is_empty() && n < max {
            let task = self.next_task();
            n += 1;
            let task = self.0.owned.assert_owner(task);
            task.run();
        }

        n
    }

    fn is_empty(&self) -> bool {
        self.0.core.try_lock().unwrap().queue.is_empty()
    }

    fn next_task(&self) -> task::Notified<Runtime> {
        self.0.core.try_lock().unwrap().queue.pop_front().unwrap()
    }

    fn shutdown(&self) {
        let mut core = self.0.core.try_lock().unwrap();

        self.0.owned.close_and_shutdown_all(0);

        while let Some(task) = core.queue.pop_back() {
            drop(task);
        }

        drop(core);
        assert!(self.0.owned.is_empty());
    }
}

impl Schedule for Runtime {
    fn release(&self, task: &Task<Self>) -> Option<Task<Self>> {
        self.0.owned.remove(task)
    }

    fn schedule(&self, task: task::Notified<Self>) {
        self.0.core.try_lock().unwrap().queue.push_back(task);
    }

    fn hooks(&self) -> TaskHarnessScheduleHooks {
        TaskHarnessScheduleHooks {
            task_terminate_callback: None,
        }
    }
}
