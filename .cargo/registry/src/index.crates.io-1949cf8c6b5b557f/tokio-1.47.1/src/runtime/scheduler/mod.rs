cfg_rt! {
    pub(crate) mod current_thread;
    pub(crate) use current_thread::CurrentThread;

    mod defer;
    use defer::Defer;

    pub(crate) mod inject;
    pub(crate) use inject::Inject;

    use crate::runtime::TaskHooks;

    use crate::runtime::WorkerMetrics;
}

cfg_rt_multi_thread! {
    mod block_in_place;
    pub(crate) use block_in_place::block_in_place;

    mod lock;
    use lock::Lock;

    pub(crate) mod multi_thread;
    pub(crate) use multi_thread::MultiThread;
}

use crate::runtime::driver;

#[derive(Debug, Clone)]
pub(crate) enum Handle {
    #[cfg(feature = "rt")]
    CurrentThread(Arc<current_thread::Handle>),

    #[cfg(feature = "rt-multi-thread")]
    MultiThread(Arc<multi_thread::Handle>),

    // TODO: This is to avoid triggering "dead code" warnings many other places
    // in the codebase. Remove this during a later cleanup
    #[cfg(not(feature = "rt"))]
    #[allow(dead_code)]
    Disabled,
}

#[cfg(feature = "rt")]
pub(super) enum Context {
    CurrentThread(current_thread::Context),

    #[cfg(feature = "rt-multi-thread")]
    MultiThread(multi_thread::Context),
}

impl Handle {
    #[cfg_attr(not(feature = "full"), allow(dead_code))]
    pub(crate) fn driver(&self) -> &driver::Handle {
        match *self {
            #[cfg(feature = "rt")]
            Handle::CurrentThread(ref h) => &h.driver,

            #[cfg(feature = "rt-multi-thread")]
            Handle::MultiThread(ref h) => &h.driver,

            #[cfg(not(feature = "rt"))]
            Handle::Disabled => unreachable!(),
        }
    }
}

cfg_rt! {
    use crate::future::Future;
    use crate::loom::sync::Arc;
    use crate::runtime::{blocking, task::{Id, SpawnLocation}};
    use crate::runtime::context;
    use crate::task::JoinHandle;
    use crate::util::RngSeedGenerator;
    use std::task::Waker;

    macro_rules! match_flavor {
        ($self:expr, $ty:ident($h:ident) => $e:expr) => {
            match $self {
                $ty::CurrentThread($h) => $e,

                #[cfg(feature = "rt-multi-thread")]
                $ty::MultiThread($h) => $e,
            }
        }
    }

    impl Handle {
        #[track_caller]
        pub(crate) fn current() -> Handle {
            match context::with_current(Clone::clone) {
                Ok(handle) => handle,
                Err(e) => panic!("{}", e),
            }
        }

        pub(crate) fn blocking_spawner(&self) -> &blocking::Spawner {
            match_flavor!(self, Handle(h) => &h.blocking_spawner)
        }

        pub(crate) fn is_local(&self) -> bool {
            match self {
                Handle::CurrentThread(h) => h.local_tid.is_some(),

                #[cfg(feature = "rt-multi-thread")]
                Handle::MultiThread(_) => false,
            }
        }

        /// Returns true if this is a local runtime and the runtime is owned by the current thread.
        pub(crate) fn can_spawn_local_on_local_runtime(&self) -> bool {
            match self {
                Handle::CurrentThread(h) => h.local_tid.map(|x| std::thread::current().id() == x).unwrap_or(false),

                #[cfg(feature = "rt-multi-thread")]
                Handle::MultiThread(_) => false,
            }
        }

        pub(crate) fn spawn<F>(&self, future: F, id: Id, spawned_at: SpawnLocation) -> JoinHandle<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            match self {
                Handle::CurrentThread(h) => current_thread::Handle::spawn(h, future, id, spawned_at),

                #[cfg(feature = "rt-multi-thread")]
                Handle::MultiThread(h) => multi_thread::Handle::spawn(h, future, id, spawned_at),
            }
        }

        /// Spawn a local task
        ///
        /// # Safety
        /// This should only be called in `LocalRuntime` if the runtime has been verified to be owned
        /// by the current thread.
        #[allow(irrefutable_let_patterns)]
        #[track_caller]
        pub(crate) unsafe fn spawn_local<F>(&self, future: F, id: Id, spawned_at: SpawnLocation) -> JoinHandle<F::Output>
        where
            F: Future + 'static,
            F::Output: 'static,
        {
            if let Handle::CurrentThread(h) = self {
                current_thread::Handle::spawn_local(h, future, id, spawned_at)
            } else {
                panic!("Only current_thread and LocalSet have spawn_local internals implemented")
            }
        }

        pub(crate) fn shutdown(&self) {
            match *self {
                Handle::CurrentThread(_) => {},

                #[cfg(feature = "rt-multi-thread")]
                Handle::MultiThread(ref h) => h.shutdown(),
            }
        }

        pub(crate) fn seed_generator(&self) -> &RngSeedGenerator {
            match_flavor!(self, Handle(h) => &h.seed_generator)
        }

        pub(crate) fn as_current_thread(&self) -> &Arc<current_thread::Handle> {
            match self {
                Handle::CurrentThread(handle) => handle,
                #[cfg(feature = "rt-multi-thread")]
                _ => panic!("not a CurrentThread handle"),
            }
        }

        pub(crate) fn hooks(&self) -> &TaskHooks {
            match self {
                Handle::CurrentThread(h) => &h.task_hooks,
                #[cfg(feature = "rt-multi-thread")]
                Handle::MultiThread(h) => &h.task_hooks,
            }
        }
    }

    impl Handle {
        pub(crate) fn num_workers(&self) -> usize {
            match self {
                Handle::CurrentThread(_) => 1,
                #[cfg(feature = "rt-multi-thread")]
                Handle::MultiThread(handle) => handle.num_workers(),
            }
        }

        pub(crate) fn num_alive_tasks(&self) -> usize {
            match_flavor!(self, Handle(handle) => handle.num_alive_tasks())
        }

        pub(crate) fn injection_queue_depth(&self) -> usize {
            match_flavor!(self, Handle(handle) => handle.injection_queue_depth())
        }

        pub(crate) fn worker_metrics(&self, worker: usize) -> &WorkerMetrics {
            match_flavor!(self, Handle(handle) => handle.worker_metrics(worker))
        }
    }

    cfg_unstable_metrics! {
        use crate::runtime::SchedulerMetrics;

        impl Handle {
            cfg_64bit_metrics! {
                pub(crate) fn spawned_tasks_count(&self) -> u64 {
                    match_flavor!(self, Handle(handle) => handle.spawned_tasks_count())
                }
            }

            pub(crate) fn num_blocking_threads(&self) -> usize {
                match_flavor!(self, Handle(handle) => handle.num_blocking_threads())
            }

            pub(crate) fn num_idle_blocking_threads(&self) -> usize {
                match_flavor!(self, Handle(handle) => handle.num_idle_blocking_threads())
            }

            pub(crate) fn scheduler_metrics(&self) -> &SchedulerMetrics {
                match_flavor!(self, Handle(handle) => handle.scheduler_metrics())
            }

            pub(crate) fn worker_local_queue_depth(&self, worker: usize) -> usize {
                match_flavor!(self, Handle(handle) => handle.worker_local_queue_depth(worker))
            }

            pub(crate) fn blocking_queue_depth(&self) -> usize {
                match_flavor!(self, Handle(handle) => handle.blocking_queue_depth())
            }
        }
    }

    impl Context {
        #[track_caller]
        pub(crate) fn expect_current_thread(&self) -> &current_thread::Context {
            match self {
                Context::CurrentThread(context) => context,
                #[cfg(feature = "rt-multi-thread")]
                _ => panic!("expected `CurrentThread::Context`")
            }
        }

        pub(crate) fn defer(&self, waker: &Waker) {
            match_flavor!(self, Context(context) => context.defer(waker));
        }

        cfg_rt_multi_thread! {
            #[track_caller]
            pub(crate) fn expect_multi_thread(&self) -> &multi_thread::Context {
                match self {
                    Context::MultiThread(context) => context,
                    _ => panic!("expected `MultiThread::Context`")
                }
            }
        }
    }
}

cfg_not_rt! {
    #[cfg(any(
        feature = "net",
        all(unix, feature = "process"),
        all(unix, feature = "signal"),
        feature = "time",
    ))]
    impl Handle {
        #[track_caller]
        pub(crate) fn current() -> Handle {
            panic!("{}", crate::util::error::CONTEXT_MISSING_ERROR)
        }
    }
}
