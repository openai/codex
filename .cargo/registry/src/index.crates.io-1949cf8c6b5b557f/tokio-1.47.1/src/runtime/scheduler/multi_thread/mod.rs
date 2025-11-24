//! Multi-threaded runtime

mod counters;
use counters::Counters;

mod handle;
pub(crate) use handle::Handle;

mod overflow;
pub(crate) use overflow::Overflow;

mod idle;
use self::idle::Idle;

mod stats;
pub(crate) use stats::Stats;

mod park;
pub(crate) use park::{Parker, Unparker};

pub(crate) mod queue;

mod worker;
pub(crate) use worker::{Context, Launch, Shared};

cfg_taskdump! {
    mod trace;
    use trace::TraceStatus;

    pub(crate) use worker::Synced;
}

cfg_not_taskdump! {
    mod trace_mock;
    use trace_mock::TraceStatus;
}

pub(crate) use worker::block_in_place;

use crate::loom::sync::Arc;
use crate::runtime::{
    blocking,
    driver::{self, Driver},
    scheduler, Config,
};
use crate::util::RngSeedGenerator;

use std::fmt;
use std::future::Future;

/// Work-stealing based thread pool for executing futures.
pub(crate) struct MultiThread;

// ===== impl MultiThread =====

impl MultiThread {
    pub(crate) fn new(
        size: usize,
        driver: Driver,
        driver_handle: driver::Handle,
        blocking_spawner: blocking::Spawner,
        seed_generator: RngSeedGenerator,
        config: Config,
    ) -> (MultiThread, Arc<Handle>, Launch) {
        let parker = Parker::new(driver);
        let (handle, launch) = worker::create(
            size,
            parker,
            driver_handle,
            blocking_spawner,
            seed_generator,
            config,
        );

        (MultiThread, handle, launch)
    }

    /// Blocks the current thread waiting for the future to complete.
    ///
    /// The future will execute on the current thread, but all spawned tasks
    /// will be executed on the thread pool.
    pub(crate) fn block_on<F>(&self, handle: &scheduler::Handle, future: F) -> F::Output
    where
        F: Future,
    {
        crate::runtime::context::enter_runtime(handle, true, |blocking| {
            blocking.block_on(future).expect("failed to park thread")
        })
    }

    pub(crate) fn shutdown(&mut self, handle: &scheduler::Handle) {
        match handle {
            scheduler::Handle::MultiThread(handle) => handle.shutdown(),
            _ => panic!("expected MultiThread scheduler"),
        }
    }
}

impl fmt::Debug for MultiThread {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("MultiThread").finish()
    }
}
