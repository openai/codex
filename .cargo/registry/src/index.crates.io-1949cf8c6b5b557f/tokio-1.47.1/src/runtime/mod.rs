//! The Tokio runtime.
//!
//! Unlike other Rust programs, asynchronous applications require runtime
//! support. In particular, the following runtime services are necessary:
//!
//! * An **I/O event loop**, called the driver, which drives I/O resources and
//!   dispatches I/O events to tasks that depend on them.
//! * A **scheduler** to execute [tasks] that use these I/O resources.
//! * A **timer** for scheduling work to run after a set period of time.
//!
//! Tokio's [`Runtime`] bundles all of these services as a single type, allowing
//! them to be started, shut down, and configured together. However, often it is
//! not required to configure a [`Runtime`] manually, and a user may just use the
//! [`tokio::main`] attribute macro, which creates a [`Runtime`] under the hood.
//!
//! # Usage
//!
//! When no fine tuning is required, the [`tokio::main`] attribute macro can be
//! used.
//!
//! ```no_run
//! use tokio::net::TcpListener;
//! use tokio::io::{AsyncReadExt, AsyncWriteExt};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let listener = TcpListener::bind("127.0.0.1:8080").await?;
//!
//!     loop {
//!         let (mut socket, _) = listener.accept().await?;
//!
//!         tokio::spawn(async move {
//!             let mut buf = [0; 1024];
//!
//!             // In a loop, read data from the socket and write the data back.
//!             loop {
//!                 let n = match socket.read(&mut buf).await {
//!                     // socket closed
//!                     Ok(0) => return,
//!                     Ok(n) => n,
//!                     Err(e) => {
//!                         println!("failed to read from socket; err = {:?}", e);
//!                         return;
//!                     }
//!                 };
//!
//!                 // Write the data back
//!                 if let Err(e) = socket.write_all(&buf[0..n]).await {
//!                     println!("failed to write to socket; err = {:?}", e);
//!                     return;
//!                 }
//!             }
//!         });
//!     }
//! }
//! ```
//!
//! From within the context of the runtime, additional tasks are spawned using
//! the [`tokio::spawn`] function. Futures spawned using this function will be
//! executed on the same thread pool used by the [`Runtime`].
//!
//! A [`Runtime`] instance can also be used directly.
//!
//! ```no_run
//! use tokio::net::TcpListener;
//! use tokio::io::{AsyncReadExt, AsyncWriteExt};
//! use tokio::runtime::Runtime;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create the runtime
//!     let rt  = Runtime::new()?;
//!
//!     // Spawn the root task
//!     rt.block_on(async {
//!         let listener = TcpListener::bind("127.0.0.1:8080").await?;
//!
//!         loop {
//!             let (mut socket, _) = listener.accept().await?;
//!
//!             tokio::spawn(async move {
//!                 let mut buf = [0; 1024];
//!
//!                 // In a loop, read data from the socket and write the data back.
//!                 loop {
//!                     let n = match socket.read(&mut buf).await {
//!                         // socket closed
//!                         Ok(0) => return,
//!                         Ok(n) => n,
//!                         Err(e) => {
//!                             println!("failed to read from socket; err = {:?}", e);
//!                             return;
//!                         }
//!                     };
//!
//!                     // Write the data back
//!                     if let Err(e) = socket.write_all(&buf[0..n]).await {
//!                         println!("failed to write to socket; err = {:?}", e);
//!                         return;
//!                     }
//!                 }
//!             });
//!         }
//!     })
//! }
//! ```
//!
//! ## Runtime Configurations
//!
//! Tokio provides multiple task scheduling strategies, suitable for different
//! applications. The [runtime builder] or `#[tokio::main]` attribute may be
//! used to select which scheduler to use.
//!
//! #### Multi-Thread Scheduler
//!
//! The multi-thread scheduler executes futures on a _thread pool_, using a
//! work-stealing strategy. By default, it will start a worker thread for each
//! CPU core available on the system. This tends to be the ideal configuration
//! for most applications. The multi-thread scheduler requires the `rt-multi-thread`
//! feature flag, and is selected by default:
//! ```
//! use tokio::runtime;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let threaded_rt = runtime::Runtime::new()?;
//! # Ok(()) }
//! ```
//!
//! Most applications should use the multi-thread scheduler, except in some
//! niche use-cases, such as when running only a single thread is required.
//!
//! #### Current-Thread Scheduler
//!
//! The current-thread scheduler provides a _single-threaded_ future executor.
//! All tasks will be created and executed on the current thread. This requires
//! the `rt` feature flag.
//! ```
//! use tokio::runtime;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let rt = runtime::Builder::new_current_thread()
//!     .build()?;
//! # Ok(()) }
//! ```
//!
//! #### Resource drivers
//!
//! When configuring a runtime by hand, no resource drivers are enabled by
//! default. In this case, attempting to use networking types or time types will
//! fail. In order to enable these types, the resource drivers must be enabled.
//! This is done with [`Builder::enable_io`] and [`Builder::enable_time`]. As a
//! shorthand, [`Builder::enable_all`] enables both resource drivers.
//!
//! ## Lifetime of spawned threads
//!
//! The runtime may spawn threads depending on its configuration and usage. The
//! multi-thread scheduler spawns threads to schedule tasks and for `spawn_blocking`
//! calls.
//!
//! While the `Runtime` is active, threads may shut down after periods of being
//! idle. Once `Runtime` is dropped, all runtime threads have usually been
//! terminated, but in the presence of unstoppable spawned work are not
//! guaranteed to have been terminated. See the
//! [struct level documentation](Runtime#shutdown) for more details.
//!
//! [tasks]: crate::task
//! [`Runtime`]: Runtime
//! [`tokio::spawn`]: crate::spawn
//! [`tokio::main`]: ../attr.main.html
//! [runtime builder]: crate::runtime::Builder
//! [`Runtime::new`]: crate::runtime::Runtime::new
//! [`Builder::threaded_scheduler`]: crate::runtime::Builder::threaded_scheduler
//! [`Builder::enable_io`]: crate::runtime::Builder::enable_io
//! [`Builder::enable_time`]: crate::runtime::Builder::enable_time
//! [`Builder::enable_all`]: crate::runtime::Builder::enable_all
//!
//! # Detailed runtime behavior
//!
//! This section gives more details into how the Tokio runtime will schedule
//! tasks for execution.
//!
//! At its most basic level, a runtime has a collection of tasks that need to be
//! scheduled. It will repeatedly remove a task from that collection and
//! schedule it (by calling [`poll`]). When the collection is empty, the thread
//! will go to sleep until a task is added to the collection.
//!
//! However, the above is not sufficient to guarantee a well-behaved runtime.
//! For example, the runtime might have a single task that is always ready to be
//! scheduled, and schedule that task every time. This is a problem because it
//! starves other tasks by not scheduling them. To solve this, Tokio provides
//! the following fairness guarantee:
//!
//! > If the total number of tasks does not grow without bound, and no task is
//! > [blocking the thread], then it is guaranteed that tasks are scheduled
//! > fairly.
//!
//! Or, more formally:
//!
//! > Under the following two assumptions:
//! >
//! > * There is some number `MAX_TASKS` such that the total number of tasks on
//! >   the runtime at any specific point in time never exceeds `MAX_TASKS`.
//! > * There is some number `MAX_SCHEDULE` such that calling [`poll`] on any
//! >   task spawned on the runtime returns within `MAX_SCHEDULE` time units.
//! >
//! > Then, there is some number `MAX_DELAY` such that when a task is woken, it
//! > will be scheduled by the runtime within `MAX_DELAY` time units.
//!
//! (Here, `MAX_TASKS` and `MAX_SCHEDULE` can be any number and the user of
//! the runtime may choose them. The `MAX_DELAY` number is controlled by the
//! runtime, and depends on the value of `MAX_TASKS` and `MAX_SCHEDULE`.)
//!
//! Other than the above fairness guarantee, there is no guarantee about the
//! order in which tasks are scheduled. There is also no guarantee that the
//! runtime is equally fair to all tasks. For example, if the runtime has two
//! tasks A and B that are both ready, then the runtime may schedule A five
//! times before it schedules B. This is the case even if A yields using
//! [`yield_now`]. All that is guaranteed is that it will schedule B eventually.
//!
//! Normally, tasks are scheduled only if they have been woken by calling
//! [`wake`] on their waker. However, this is not guaranteed, and Tokio may
//! schedule tasks that have not been woken under some circumstances. This is
//! called a spurious wakeup.
//!
//! ## IO and timers
//!
//! Beyond just scheduling tasks, the runtime must also manage IO resources and
//! timers. It does this by periodically checking whether there are any IO
//! resources or timers that are ready, and waking the relevant task so that
//! it will be scheduled.
//!
//! These checks are performed periodically between scheduling tasks. Under the
//! same assumptions as the previous fairness guarantee, Tokio guarantees that
//! it will wake tasks with an IO or timer event within some maximum number of
//! time units.
//!
//! ## Current thread runtime (behavior at the time of writing)
//!
//! This section describes how the [current thread runtime] behaves today. This
//! behavior may change in future versions of Tokio.
//!
//! The current thread runtime maintains two FIFO queues of tasks that are ready
//! to be scheduled: the global queue and the local queue. The runtime will prefer
//! to choose the next task to schedule from the local queue, and will only pick a
//! task from the global queue if the local queue is empty, or if it has picked
//! a task from the local queue 31 times in a row. The number 31 can be
//! changed using the [`global_queue_interval`] setting.
//!
//! The runtime will check for new IO or timer events whenever there are no
//! tasks ready to be scheduled, or when it has scheduled 61 tasks in a row. The
//! number 61 may be changed using the [`event_interval`] setting.
//!
//! When a task is woken from within a task running on the runtime, then the
//! woken task is added directly to the local queue. Otherwise, the task is
//! added to the global queue. The current thread runtime does not use [the lifo
//! slot optimization].
//!
//! ## Multi threaded runtime (behavior at the time of writing)
//!
//! This section describes how the [multi thread runtime] behaves today. This
//! behavior may change in future versions of Tokio.
//!
//! A multi thread runtime has a fixed number of worker threads, which are all
//! created on startup. The multi thread runtime maintains one global queue, and
//! a local queue for each worker thread. The local queue of a worker thread can
//! fit at most 256 tasks. If more than 256 tasks are added to the local queue,
//! then half of them are moved to the global queue to make space.
//!
//! The runtime will prefer to choose the next task to schedule from the local
//! queue, and will only pick a task from the global queue if the local queue is
//! empty, or if it has picked a task from the local queue
//! [`global_queue_interval`] times in a row. If the value of
//! [`global_queue_interval`] is not explicitly set using the runtime builder,
//! then the runtime will dynamically compute it using a heuristic that targets
//! 10ms intervals between each check of the global queue (based on the
//! [`worker_mean_poll_time`] metric).
//!
//! If both the local queue and global queue is empty, then the worker thread
//! will attempt to steal tasks from the local queue of another worker thread.
//! Stealing is done by moving half of the tasks in one local queue to another
//! local queue.
//!
//! The runtime will check for new IO or timer events whenever there are no
//! tasks ready to be scheduled, or when it has scheduled 61 tasks in a row. The
//! number 61 may be changed using the [`event_interval`] setting.
//!
//! The multi thread runtime uses [the lifo slot optimization]: Whenever a task
//! wakes up another task, the other task is added to the worker thread's lifo
//! slot instead of being added to a queue. If there was already a task in the
//! lifo slot when this happened, then the lifo slot is replaced, and the task
//! that used to be in the lifo slot is placed in the thread's local queue.
//! When the runtime finishes scheduling a task, it will schedule the task in
//! the lifo slot immediately, if any. When the lifo slot is used, the [coop
//! budget] is not reset. Furthermore, if a worker thread uses the lifo slot
//! three times in a row, it is temporarily disabled until the worker thread has
//! scheduled a task that didn't come from the lifo slot. The lifo slot can be
//! disabled using the [`disable_lifo_slot`] setting. The lifo slot is separate
//! from the local queue, so other worker threads cannot steal the task in the
//! lifo slot.
//!
//! When a task is woken from a thread that is not a worker thread, then the
//! task is placed in the global queue.
//!
//! [`poll`]: std::future::Future::poll
//! [`wake`]: std::task::Waker::wake
//! [`yield_now`]: crate::task::yield_now
//! [blocking the thread]: https://ryhl.io/blog/async-what-is-blocking/
//! [current thread runtime]: crate::runtime::Builder::new_current_thread
//! [multi thread runtime]: crate::runtime::Builder::new_multi_thread
//! [`global_queue_interval`]: crate::runtime::Builder::global_queue_interval
//! [`event_interval`]: crate::runtime::Builder::event_interval
//! [`disable_lifo_slot`]: crate::runtime::Builder::disable_lifo_slot
//! [the lifo slot optimization]: crate::runtime::Builder::disable_lifo_slot
//! [coop budget]: crate::task::coop#cooperative-scheduling
//! [`worker_mean_poll_time`]: crate::runtime::RuntimeMetrics::worker_mean_poll_time

// At the top due to macros
#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
#[macro_use]
mod tests;

pub(crate) mod context;

pub(crate) mod park;

mod driver;

pub(crate) mod scheduler;

cfg_io_driver_impl! {
    pub(crate) mod io;
}

cfg_process_driver! {
    mod process;
}

cfg_time! {
    pub(crate) mod time;
}

cfg_signal_internal_and_unix! {
    pub(crate) mod signal;
}

cfg_rt! {
    pub(crate) mod task;

    mod config;
    use config::Config;

    mod blocking;
    #[cfg_attr(target_os = "wasi", allow(unused_imports))]
    pub(crate) use blocking::spawn_blocking;

    cfg_trace! {
        pub(crate) use blocking::Mandatory;
    }

    cfg_fs! {
        pub(crate) use blocking::spawn_mandatory_blocking;
    }

    mod builder;
    pub use self::builder::Builder;
    cfg_unstable! {
        mod id;
        #[cfg_attr(not(tokio_unstable), allow(unreachable_pub))]
        pub use id::Id;

        pub use self::builder::UnhandledPanic;
        pub use crate::util::rand::RngSeed;

        mod local_runtime;
        pub use local_runtime::{LocalRuntime, LocalOptions};
    }

    cfg_taskdump! {
        pub mod dump;
        pub use dump::Dump;
    }

    mod task_hooks;
    pub(crate) use task_hooks::{TaskHooks, TaskCallback};
    cfg_unstable! {
        pub use task_hooks::TaskMeta;
    }
    #[cfg(not(tokio_unstable))]
    pub(crate) use task_hooks::TaskMeta;

    mod handle;
    pub use handle::{EnterGuard, Handle, TryCurrentError};

    mod runtime;
    pub use runtime::{Runtime, RuntimeFlavor};

    /// Boundary value to prevent stack overflow caused by a large-sized
    /// Future being placed in the stack.
    pub(crate) const BOX_FUTURE_THRESHOLD: usize = if cfg!(debug_assertions)  {
        2048
    } else {
        16384
    };

    mod thread_id;
    pub(crate) use thread_id::ThreadId;

    pub(crate) mod metrics;
    pub use metrics::RuntimeMetrics;

    cfg_unstable_metrics! {
        pub use metrics::{HistogramScale, HistogramConfiguration, LogHistogram, LogHistogramBuilder, InvalidHistogramConfiguration} ;

        cfg_net! {
            pub(crate) use metrics::IoDriverMetrics;
        }
    }

    pub(crate) use metrics::{MetricsBatch, SchedulerMetrics, WorkerMetrics, HistogramBuilder};

    /// After thread starts / before thread stops
    type Callback = std::sync::Arc<dyn Fn() + Send + Sync>;
}
