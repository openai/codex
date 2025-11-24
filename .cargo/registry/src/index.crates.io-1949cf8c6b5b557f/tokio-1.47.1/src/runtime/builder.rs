#![cfg_attr(loom, allow(unused_imports))]

use crate::runtime::handle::Handle;
use crate::runtime::{blocking, driver, Callback, HistogramBuilder, Runtime, TaskCallback};
#[cfg(tokio_unstable)]
use crate::runtime::{metrics::HistogramConfiguration, LocalOptions, LocalRuntime, TaskMeta};
use crate::util::rand::{RngSeed, RngSeedGenerator};

use crate::runtime::blocking::BlockingPool;
use crate::runtime::scheduler::CurrentThread;
use std::fmt;
use std::io;
use std::thread::ThreadId;
use std::time::Duration;

/// Builds Tokio Runtime with custom configuration values.
///
/// Methods can be chained in order to set the configuration values. The
/// Runtime is constructed by calling [`build`].
///
/// New instances of `Builder` are obtained via [`Builder::new_multi_thread`]
/// or [`Builder::new_current_thread`].
///
/// See function level documentation for details on the various configuration
/// settings.
///
/// [`build`]: method@Self::build
/// [`Builder::new_multi_thread`]: method@Self::new_multi_thread
/// [`Builder::new_current_thread`]: method@Self::new_current_thread
///
/// # Examples
///
/// ```
/// use tokio::runtime::Builder;
///
/// fn main() {
///     // build runtime
///     let runtime = Builder::new_multi_thread()
///         .worker_threads(4)
///         .thread_name("my-custom-name")
///         .thread_stack_size(3 * 1024 * 1024)
///         .build()
///         .unwrap();
///
///     // use runtime ...
/// }
/// ```
pub struct Builder {
    /// Runtime type
    kind: Kind,

    /// Whether or not to enable the I/O driver
    enable_io: bool,
    nevents: usize,

    /// Whether or not to enable the time driver
    enable_time: bool,

    /// Whether or not the clock should start paused.
    start_paused: bool,

    /// The number of worker threads, used by Runtime.
    ///
    /// Only used when not using the current-thread executor.
    worker_threads: Option<usize>,

    /// Cap on thread usage.
    max_blocking_threads: usize,

    /// Name fn used for threads spawned by the runtime.
    pub(super) thread_name: ThreadNameFn,

    /// Stack size used for threads spawned by the runtime.
    pub(super) thread_stack_size: Option<usize>,

    /// Callback to run after each thread starts.
    pub(super) after_start: Option<Callback>,

    /// To run before each worker thread stops
    pub(super) before_stop: Option<Callback>,

    /// To run before each worker thread is parked.
    pub(super) before_park: Option<Callback>,

    /// To run after each thread is unparked.
    pub(super) after_unpark: Option<Callback>,

    /// To run before each task is spawned.
    pub(super) before_spawn: Option<TaskCallback>,

    /// To run before each poll
    #[cfg(tokio_unstable)]
    pub(super) before_poll: Option<TaskCallback>,

    /// To run after each poll
    #[cfg(tokio_unstable)]
    pub(super) after_poll: Option<TaskCallback>,

    /// To run after each task is terminated.
    pub(super) after_termination: Option<TaskCallback>,

    /// Customizable keep alive timeout for `BlockingPool`
    pub(super) keep_alive: Option<Duration>,

    /// How many ticks before pulling a task from the global/remote queue?
    ///
    /// When `None`, the value is unspecified and behavior details are left to
    /// the scheduler. Each scheduler flavor could choose to either pick its own
    /// default value or use some other strategy to decide when to poll from the
    /// global queue. For example, the multi-threaded scheduler uses a
    /// self-tuning strategy based on mean task poll times.
    pub(super) global_queue_interval: Option<u32>,

    /// How many ticks before yielding to the driver for timer and I/O events?
    pub(super) event_interval: u32,

    /// When true, the multi-threade scheduler LIFO slot should not be used.
    ///
    /// This option should only be exposed as unstable.
    pub(super) disable_lifo_slot: bool,

    /// Specify a random number generator seed to provide deterministic results
    pub(super) seed_generator: RngSeedGenerator,

    /// When true, enables task poll count histogram instrumentation.
    pub(super) metrics_poll_count_histogram_enable: bool,

    /// Configures the task poll count histogram
    pub(super) metrics_poll_count_histogram: HistogramBuilder,

    #[cfg(tokio_unstable)]
    pub(super) unhandled_panic: UnhandledPanic,
}

cfg_unstable! {
    /// How the runtime should respond to unhandled panics.
    ///
    /// Instances of `UnhandledPanic` are passed to `Builder::unhandled_panic`
    /// to configure the runtime behavior when a spawned task panics.
    ///
    /// See [`Builder::unhandled_panic`] for more details.
    #[derive(Debug, Clone)]
    #[non_exhaustive]
    pub enum UnhandledPanic {
        /// The runtime should ignore panics on spawned tasks.
        ///
        /// The panic is forwarded to the task's [`JoinHandle`] and all spawned
        /// tasks continue running normally.
        ///
        /// This is the default behavior.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::{self, UnhandledPanic};
        ///
        /// # pub fn main() {
        /// let rt = runtime::Builder::new_current_thread()
        ///     .unhandled_panic(UnhandledPanic::Ignore)
        ///     .build()
        ///     .unwrap();
        ///
        /// let task1 = rt.spawn(async { panic!("boom"); });
        /// let task2 = rt.spawn(async {
        ///     // This task completes normally
        ///     "done"
        /// });
        ///
        /// rt.block_on(async {
        ///     // The panic on the first task is forwarded to the `JoinHandle`
        ///     assert!(task1.await.is_err());
        ///
        ///     // The second task completes normally
        ///     assert!(task2.await.is_ok());
        /// })
        /// # }
        /// ```
        ///
        /// [`JoinHandle`]: struct@crate::task::JoinHandle
        Ignore,

        /// The runtime should immediately shutdown if a spawned task panics.
        ///
        /// The runtime will immediately shutdown even if the panicked task's
        /// [`JoinHandle`] is still available. All further spawned tasks will be
        /// immediately dropped and call to [`Runtime::block_on`] will panic.
        ///
        /// # Examples
        ///
        /// ```should_panic
        /// use tokio::runtime::{self, UnhandledPanic};
        ///
        /// # pub fn main() {
        /// let rt = runtime::Builder::new_current_thread()
        ///     .unhandled_panic(UnhandledPanic::ShutdownRuntime)
        ///     .build()
        ///     .unwrap();
        ///
        /// rt.spawn(async { panic!("boom"); });
        /// rt.spawn(async {
        ///     // This task never completes.
        /// });
        ///
        /// rt.block_on(async {
        ///     // Do some work
        /// # loop { tokio::task::yield_now().await; }
        /// })
        /// # }
        /// ```
        ///
        /// [`JoinHandle`]: struct@crate::task::JoinHandle
        ShutdownRuntime,
    }
}

pub(crate) type ThreadNameFn = std::sync::Arc<dyn Fn() -> String + Send + Sync + 'static>;

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    CurrentThread,
    #[cfg(feature = "rt-multi-thread")]
    MultiThread,
}

impl Builder {
    /// Returns a new builder with the current thread scheduler selected.
    ///
    /// Configuration methods can be chained on the return value.
    ///
    /// To spawn non-`Send` tasks on the resulting runtime, combine it with a
    /// [`LocalSet`].
    ///
    /// [`LocalSet`]: crate::task::LocalSet
    pub fn new_current_thread() -> Builder {
        #[cfg(loom)]
        const EVENT_INTERVAL: u32 = 4;
        // The number `61` is fairly arbitrary. I believe this value was copied from golang.
        #[cfg(not(loom))]
        const EVENT_INTERVAL: u32 = 61;

        Builder::new(Kind::CurrentThread, EVENT_INTERVAL)
    }

    /// Returns a new builder with the multi thread scheduler selected.
    ///
    /// Configuration methods can be chained on the return value.
    #[cfg(feature = "rt-multi-thread")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt-multi-thread")))]
    pub fn new_multi_thread() -> Builder {
        // The number `61` is fairly arbitrary. I believe this value was copied from golang.
        Builder::new(Kind::MultiThread, 61)
    }

    /// Returns a new runtime builder initialized with default configuration
    /// values.
    ///
    /// Configuration methods can be chained on the return value.
    pub(crate) fn new(kind: Kind, event_interval: u32) -> Builder {
        Builder {
            kind,

            // I/O defaults to "off"
            enable_io: false,
            nevents: 1024,

            // Time defaults to "off"
            enable_time: false,

            // The clock starts not-paused
            start_paused: false,

            // Read from environment variable first in multi-threaded mode.
            // Default to lazy auto-detection (one thread per CPU core)
            worker_threads: None,

            max_blocking_threads: 512,

            // Default thread name
            thread_name: std::sync::Arc::new(|| "tokio-runtime-worker".into()),

            // Do not set a stack size by default
            thread_stack_size: None,

            // No worker thread callbacks
            after_start: None,
            before_stop: None,
            before_park: None,
            after_unpark: None,

            before_spawn: None,
            after_termination: None,

            #[cfg(tokio_unstable)]
            before_poll: None,
            #[cfg(tokio_unstable)]
            after_poll: None,

            keep_alive: None,

            // Defaults for these values depend on the scheduler kind, so we get them
            // as parameters.
            global_queue_interval: None,
            event_interval,

            seed_generator: RngSeedGenerator::new(RngSeed::new()),

            #[cfg(tokio_unstable)]
            unhandled_panic: UnhandledPanic::Ignore,

            metrics_poll_count_histogram_enable: false,

            metrics_poll_count_histogram: HistogramBuilder::default(),

            disable_lifo_slot: false,
        }
    }

    /// Enables both I/O and time drivers.
    ///
    /// Doing this is a shorthand for calling `enable_io` and `enable_time`
    /// individually. If additional components are added to Tokio in the future,
    /// `enable_all` will include these future components.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime;
    ///
    /// let rt = runtime::Builder::new_multi_thread()
    ///     .enable_all()
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn enable_all(&mut self) -> &mut Self {
        #[cfg(any(
            feature = "net",
            all(unix, feature = "process"),
            all(unix, feature = "signal")
        ))]
        self.enable_io();
        #[cfg(feature = "time")]
        self.enable_time();

        self
    }

    /// Sets the number of worker threads the `Runtime` will use.
    ///
    /// This can be any number above 0 though it is advised to keep this value
    /// on the smaller side.
    ///
    /// This will override the value read from environment variable `TOKIO_WORKER_THREADS`.
    ///
    /// # Default
    ///
    /// The default value is the number of cores available to the system.
    ///
    /// When using the `current_thread` runtime this method has no effect.
    ///
    /// # Examples
    ///
    /// ## Multi threaded runtime with 4 threads
    ///
    /// ```
    /// use tokio::runtime;
    ///
    /// // This will spawn a work-stealing runtime with 4 worker threads.
    /// let rt = runtime::Builder::new_multi_thread()
    ///     .worker_threads(4)
    ///     .build()
    ///     .unwrap();
    ///
    /// rt.spawn(async move {});
    /// ```
    ///
    /// ## Current thread runtime (will only run on the current thread via `Runtime::block_on`)
    ///
    /// ```
    /// use tokio::runtime;
    ///
    /// // Create a runtime that _must_ be driven from a call
    /// // to `Runtime::block_on`.
    /// let rt = runtime::Builder::new_current_thread()
    ///     .build()
    ///     .unwrap();
    ///
    /// // This will run the runtime and future on the current thread
    /// rt.block_on(async move {});
    /// ```
    ///
    /// # Panics
    ///
    /// This will panic if `val` is not larger than `0`.
    #[track_caller]
    pub fn worker_threads(&mut self, val: usize) -> &mut Self {
        assert!(val > 0, "Worker threads cannot be set to 0");
        self.worker_threads = Some(val);
        self
    }

    /// Specifies the limit for additional threads spawned by the Runtime.
    ///
    /// These threads are used for blocking operations like tasks spawned
    /// through [`spawn_blocking`], this includes but is not limited to:
    /// - [`fs`] operations
    /// - dns resolution through [`ToSocketAddrs`]
    /// - writing to [`Stdout`] or [`Stderr`]
    /// - reading from [`Stdin`]
    ///
    /// Unlike the [`worker_threads`], they are not always active and will exit
    /// if left idle for too long. You can change this timeout duration with [`thread_keep_alive`].
    ///
    /// It's recommended to not set this limit too low in order to avoid hanging on operations
    /// requiring [`spawn_blocking`].
    ///
    /// The default value is 512.
    ///
    /// # Queue Behavior
    ///
    /// When a blocking task is submitted, it will be inserted into a queue. If available, one of
    /// the idle threads will be notified to run the task. Otherwise, if the threshold set by this
    /// method has not been reached, a new thread will be spawned. If no idle thread is available
    /// and no more threads are allowed to be spawned, the task will remain in the queue until one
    /// of the busy threads pick it up. Note that since the queue does not apply any backpressure,
    /// it could potentially grow unbounded.
    ///
    /// # Panics
    ///
    /// This will panic if `val` is not larger than `0`.
    ///
    /// # Upgrading from 0.x
    ///
    /// In old versions `max_threads` limited both blocking and worker threads, but the
    /// current `max_blocking_threads` does not include async worker threads in the count.
    ///
    /// [`spawn_blocking`]: fn@crate::task::spawn_blocking
    /// [`fs`]: mod@crate::fs
    /// [`ToSocketAddrs`]: trait@crate::net::ToSocketAddrs
    /// [`Stdout`]: struct@crate::io::Stdout
    /// [`Stdin`]: struct@crate::io::Stdin
    /// [`Stderr`]: struct@crate::io::Stderr
    /// [`worker_threads`]: Self::worker_threads
    /// [`thread_keep_alive`]: Self::thread_keep_alive
    #[track_caller]
    #[cfg_attr(docsrs, doc(alias = "max_threads"))]
    pub fn max_blocking_threads(&mut self, val: usize) -> &mut Self {
        assert!(val > 0, "Max blocking threads cannot be set to 0");
        self.max_blocking_threads = val;
        self
    }

    /// Sets name of threads spawned by the `Runtime`'s thread pool.
    ///
    /// The default name is "tokio-runtime-worker".
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    ///
    /// # pub fn main() {
    /// let rt = runtime::Builder::new_multi_thread()
    ///     .thread_name("my-pool")
    ///     .build();
    /// # }
    /// ```
    pub fn thread_name(&mut self, val: impl Into<String>) -> &mut Self {
        let val = val.into();
        self.thread_name = std::sync::Arc::new(move || val.clone());
        self
    }

    /// Sets a function used to generate the name of threads spawned by the `Runtime`'s thread pool.
    ///
    /// The default name fn is `|| "tokio-runtime-worker".into()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    /// # use std::sync::atomic::{AtomicUsize, Ordering};
    /// # pub fn main() {
    /// let rt = runtime::Builder::new_multi_thread()
    ///     .thread_name_fn(|| {
    ///        static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
    ///        let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
    ///        format!("my-pool-{}", id)
    ///     })
    ///     .build();
    /// # }
    /// ```
    pub fn thread_name_fn<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.thread_name = std::sync::Arc::new(f);
        self
    }

    /// Sets the stack size (in bytes) for worker threads.
    ///
    /// The actual stack size may be greater than this value if the platform
    /// specifies minimal stack size.
    ///
    /// The default stack size for spawned threads is 2 MiB, though this
    /// particular stack size is subject to change in the future.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    ///
    /// # pub fn main() {
    /// let rt = runtime::Builder::new_multi_thread()
    ///     .thread_stack_size(32 * 1024)
    ///     .build();
    /// # }
    /// ```
    pub fn thread_stack_size(&mut self, val: usize) -> &mut Self {
        self.thread_stack_size = Some(val);
        self
    }

    /// Executes function `f` after each thread is started but before it starts
    /// doing work.
    ///
    /// This is intended for bookkeeping and monitoring use cases.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    /// # pub fn main() {
    /// let runtime = runtime::Builder::new_multi_thread()
    ///     .on_thread_start(|| {
    ///         println!("thread started");
    ///     })
    ///     .build();
    /// # }
    /// ```
    #[cfg(not(loom))]
    pub fn on_thread_start<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.after_start = Some(std::sync::Arc::new(f));
        self
    }

    /// Executes function `f` before each thread stops.
    ///
    /// This is intended for bookkeeping and monitoring use cases.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    /// # pub fn main() {
    /// let runtime = runtime::Builder::new_multi_thread()
    ///     .on_thread_stop(|| {
    ///         println!("thread stopping");
    ///     })
    ///     .build();
    /// # }
    /// ```
    #[cfg(not(loom))]
    pub fn on_thread_stop<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.before_stop = Some(std::sync::Arc::new(f));
        self
    }

    /// Executes function `f` just before a thread is parked (goes idle).
    /// `f` is called within the Tokio context, so functions like [`tokio::spawn`](crate::spawn)
    /// can be called, and may result in this thread being unparked immediately.
    ///
    /// This can be used to start work only when the executor is idle, or for bookkeeping
    /// and monitoring purposes.
    ///
    /// Note: There can only be one park callback for a runtime; calling this function
    /// more than once replaces the last callback defined, rather than adding to it.
    ///
    /// # Examples
    ///
    /// ## Multithreaded executor
    /// ```
    /// # use std::sync::Arc;
    /// # use std::sync::atomic::{AtomicBool, Ordering};
    /// # use tokio::runtime;
    /// # use tokio::sync::Barrier;
    /// # pub fn main() {
    /// let once = AtomicBool::new(true);
    /// let barrier = Arc::new(Barrier::new(2));
    ///
    /// let runtime = runtime::Builder::new_multi_thread()
    ///     .worker_threads(1)
    ///     .on_thread_park({
    ///         let barrier = barrier.clone();
    ///         move || {
    ///             let barrier = barrier.clone();
    ///             if once.swap(false, Ordering::Relaxed) {
    ///                 tokio::spawn(async move { barrier.wait().await; });
    ///            }
    ///         }
    ///     })
    ///     .build()
    ///     .unwrap();
    ///
    /// runtime.block_on(async {
    ///    barrier.wait().await;
    /// })
    /// # }
    /// ```
    /// ## Current thread executor
    /// ```
    /// # use std::sync::Arc;
    /// # use std::sync::atomic::{AtomicBool, Ordering};
    /// # use tokio::runtime;
    /// # use tokio::sync::Barrier;
    /// # pub fn main() {
    /// let once = AtomicBool::new(true);
    /// let barrier = Arc::new(Barrier::new(2));
    ///
    /// let runtime = runtime::Builder::new_current_thread()
    ///     .on_thread_park({
    ///         let barrier = barrier.clone();
    ///         move || {
    ///             let barrier = barrier.clone();
    ///             if once.swap(false, Ordering::Relaxed) {
    ///                 tokio::spawn(async move { barrier.wait().await; });
    ///            }
    ///         }
    ///     })
    ///     .build()
    ///     .unwrap();
    ///
    /// runtime.block_on(async {
    ///    barrier.wait().await;
    /// })
    /// # }
    /// ```
    #[cfg(not(loom))]
    pub fn on_thread_park<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.before_park = Some(std::sync::Arc::new(f));
        self
    }

    /// Executes function `f` just after a thread unparks (starts executing tasks).
    ///
    /// This is intended for bookkeeping and monitoring use cases; note that work
    /// in this callback will increase latencies when the application has allowed one or
    /// more runtime threads to go idle.
    ///
    /// Note: There can only be one unpark callback for a runtime; calling this function
    /// more than once replaces the last callback defined, rather than adding to it.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    /// # pub fn main() {
    /// let runtime = runtime::Builder::new_multi_thread()
    ///     .on_thread_unpark(|| {
    ///         println!("thread unparking");
    ///     })
    ///     .build();
    ///
    /// runtime.unwrap().block_on(async {
    ///    tokio::task::yield_now().await;
    ///    println!("Hello from Tokio!");
    /// })
    /// # }
    /// ```
    #[cfg(not(loom))]
    pub fn on_thread_unpark<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.after_unpark = Some(std::sync::Arc::new(f));
        self
    }

    /// Executes function `f` just before a task is spawned.
    ///
    /// `f` is called within the Tokio context, so functions like
    /// [`tokio::spawn`](crate::spawn) can be called, and may result in this callback being
    /// invoked immediately.
    ///
    /// This can be used for bookkeeping or monitoring purposes.
    ///
    /// Note: There can only be one spawn callback for a runtime; calling this function more
    /// than once replaces the last callback defined, rather than adding to it.
    ///
    /// This *does not* support [`LocalSet`](crate::task::LocalSet) at this time.
    ///
    /// **Note**: This is an [unstable API][unstable]. The public API of this type
    /// may break in 1.x releases. See [the documentation on unstable
    /// features][unstable] for details.
    ///
    /// [unstable]: crate#unstable-features
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    /// # pub fn main() {
    /// let runtime = runtime::Builder::new_current_thread()
    ///     .on_task_spawn(|_| {
    ///         println!("spawning task");
    ///     })
    ///     .build()
    ///     .unwrap();
    ///
    /// runtime.block_on(async {
    ///     tokio::task::spawn(std::future::ready(()));
    ///
    ///     for _ in 0..64 {
    ///         tokio::task::yield_now().await;
    ///     }
    /// })
    /// # }
    /// ```
    #[cfg(all(not(loom), tokio_unstable))]
    #[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
    pub fn on_task_spawn<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&TaskMeta<'_>) + Send + Sync + 'static,
    {
        self.before_spawn = Some(std::sync::Arc::new(f));
        self
    }

    /// Executes function `f` just before a task is polled
    ///
    /// `f` is called within the Tokio context, so functions like
    /// [`tokio::spawn`](crate::spawn) can be called, and may result in this callback being
    /// invoked immediately.
    ///
    /// **Note**: This is an [unstable API][unstable]. The public API of this type
    /// may break in 1.x releases. See [the documentation on unstable
    /// features][unstable] for details.
    ///
    /// [unstable]: crate#unstable-features
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::{atomic::AtomicUsize, Arc};
    /// # use tokio::task::yield_now;
    /// # pub fn main() {
    /// let poll_start_counter = Arc::new(AtomicUsize::new(0));
    /// let poll_start = poll_start_counter.clone();
    /// let rt = tokio::runtime::Builder::new_multi_thread()
    ///     .enable_all()
    ///     .on_before_task_poll(move |meta| {
    ///         println!("task {} is about to be polled", meta.id())
    ///     })
    ///     .build()
    ///     .unwrap();
    /// let task = rt.spawn(async {
    ///     yield_now().await;
    /// });
    /// let _ = rt.block_on(task);
    ///
    /// # }
    /// ```
    #[cfg(tokio_unstable)]
    #[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
    pub fn on_before_task_poll<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&TaskMeta<'_>) + Send + Sync + 'static,
    {
        self.before_poll = Some(std::sync::Arc::new(f));
        self
    }

    /// Executes function `f` just after a task is polled
    ///
    /// `f` is called within the Tokio context, so functions like
    /// [`tokio::spawn`](crate::spawn) can be called, and may result in this callback being
    /// invoked immediately.
    ///
    /// **Note**: This is an [unstable API][unstable]. The public API of this type
    /// may break in 1.x releases. See [the documentation on unstable
    /// features][unstable] for details.
    ///
    /// [unstable]: crate#unstable-features
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::{atomic::AtomicUsize, Arc};
    /// # use tokio::task::yield_now;
    /// # pub fn main() {
    /// let poll_stop_counter = Arc::new(AtomicUsize::new(0));
    /// let poll_stop = poll_stop_counter.clone();
    /// let rt = tokio::runtime::Builder::new_multi_thread()
    ///     .enable_all()
    ///     .on_after_task_poll(move |meta| {
    ///         println!("task {} completed polling", meta.id());
    ///     })
    ///     .build()
    ///     .unwrap();
    /// let task = rt.spawn(async {
    ///     yield_now().await;
    /// });
    /// let _ = rt.block_on(task);
    ///
    /// # }
    /// ```
    #[cfg(tokio_unstable)]
    #[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
    pub fn on_after_task_poll<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&TaskMeta<'_>) + Send + Sync + 'static,
    {
        self.after_poll = Some(std::sync::Arc::new(f));
        self
    }

    /// Executes function `f` just after a task is terminated.
    ///
    /// `f` is called within the Tokio context, so functions like
    /// [`tokio::spawn`](crate::spawn) can be called.
    ///
    /// This can be used for bookkeeping or monitoring purposes.
    ///
    /// Note: There can only be one task termination callback for a runtime; calling this
    /// function more than once replaces the last callback defined, rather than adding to it.
    ///
    /// This *does not* support [`LocalSet`](crate::task::LocalSet) at this time.
    ///
    /// **Note**: This is an [unstable API][unstable]. The public API of this type
    /// may break in 1.x releases. See [the documentation on unstable
    /// features][unstable] for details.
    ///
    /// [unstable]: crate#unstable-features
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    /// # pub fn main() {
    /// let runtime = runtime::Builder::new_current_thread()
    ///     .on_task_terminate(|_| {
    ///         println!("killing task");
    ///     })
    ///     .build()
    ///     .unwrap();
    ///
    /// runtime.block_on(async {
    ///     tokio::task::spawn(std::future::ready(()));
    ///
    ///     for _ in 0..64 {
    ///         tokio::task::yield_now().await;
    ///     }
    /// })
    /// # }
    /// ```
    #[cfg(all(not(loom), tokio_unstable))]
    #[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
    pub fn on_task_terminate<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&TaskMeta<'_>) + Send + Sync + 'static,
    {
        self.after_termination = Some(std::sync::Arc::new(f));
        self
    }

    /// Creates the configured `Runtime`.
    ///
    /// The returned `Runtime` instance is ready to spawn tasks.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Builder;
    ///
    /// let rt  = Builder::new_multi_thread().build().unwrap();
    ///
    /// rt.block_on(async {
    ///     println!("Hello from the Tokio runtime");
    /// });
    /// ```
    pub fn build(&mut self) -> io::Result<Runtime> {
        match &self.kind {
            Kind::CurrentThread => self.build_current_thread_runtime(),
            #[cfg(feature = "rt-multi-thread")]
            Kind::MultiThread => self.build_threaded_runtime(),
        }
    }

    /// Creates the configured [`LocalRuntime`].
    ///
    /// The returned [`LocalRuntime`] instance is ready to spawn tasks.
    ///
    /// # Panics
    ///
    /// This will panic if the runtime is configured with [`new_multi_thread()`].
    ///
    /// [`new_multi_thread()`]: Builder::new_multi_thread
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::{Builder, LocalOptions};
    ///
    /// let rt = Builder::new_current_thread()
    ///     .build_local(LocalOptions::default())
    ///     .unwrap();
    ///
    /// rt.spawn_local(async {
    ///     println!("Hello from the Tokio runtime");
    /// });
    /// ```
    #[allow(unused_variables, unreachable_patterns)]
    #[cfg(tokio_unstable)]
    #[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
    pub fn build_local(&mut self, options: LocalOptions) -> io::Result<LocalRuntime> {
        match &self.kind {
            Kind::CurrentThread => self.build_current_thread_local_runtime(),
            #[cfg(feature = "rt-multi-thread")]
            Kind::MultiThread => panic!("multi_thread is not supported for LocalRuntime"),
        }
    }

    fn get_cfg(&self) -> driver::Cfg {
        driver::Cfg {
            enable_pause_time: match self.kind {
                Kind::CurrentThread => true,
                #[cfg(feature = "rt-multi-thread")]
                Kind::MultiThread => false,
            },
            enable_io: self.enable_io,
            enable_time: self.enable_time,
            start_paused: self.start_paused,
            nevents: self.nevents,
        }
    }

    /// Sets a custom timeout for a thread in the blocking pool.
    ///
    /// By default, the timeout for a thread is set to 10 seconds. This can
    /// be overridden using `.thread_keep_alive()`.
    ///
    /// # Example
    ///
    /// ```
    /// # use tokio::runtime;
    /// # use std::time::Duration;
    /// # pub fn main() {
    /// let rt = runtime::Builder::new_multi_thread()
    ///     .thread_keep_alive(Duration::from_millis(100))
    ///     .build();
    /// # }
    /// ```
    pub fn thread_keep_alive(&mut self, duration: Duration) -> &mut Self {
        self.keep_alive = Some(duration);
        self
    }

    /// Sets the number of scheduler ticks after which the scheduler will poll the global
    /// task queue.
    ///
    /// A scheduler "tick" roughly corresponds to one `poll` invocation on a task.
    ///
    /// By default the global queue interval is 31 for the current-thread scheduler. Please see
    /// [the module documentation] for the default behavior of the multi-thread scheduler.
    ///
    /// Schedulers have a local queue of already-claimed tasks, and a global queue of incoming
    /// tasks. Setting the interval to a smaller value increases the fairness of the scheduler,
    /// at the cost of more synchronization overhead. That can be beneficial for prioritizing
    /// getting started on new work, especially if tasks frequently yield rather than complete
    /// or await on further I/O. Conversely, a higher value prioritizes existing work, and
    /// is a good choice when most tasks quickly complete polling.
    ///
    /// [the module documentation]: crate::runtime#multi-threaded-runtime-behavior-at-the-time-of-writing
    ///
    /// # Panics
    ///
    /// This function will panic if 0 is passed as an argument.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    /// # pub fn main() {
    /// let rt = runtime::Builder::new_multi_thread()
    ///     .global_queue_interval(31)
    ///     .build();
    /// # }
    /// ```
    #[track_caller]
    pub fn global_queue_interval(&mut self, val: u32) -> &mut Self {
        assert!(val > 0, "global_queue_interval must be greater than 0");
        self.global_queue_interval = Some(val);
        self
    }

    /// Sets the number of scheduler ticks after which the scheduler will poll for
    /// external events (timers, I/O, and so on).
    ///
    /// A scheduler "tick" roughly corresponds to one `poll` invocation on a task.
    ///
    /// By default, the event interval is `61` for all scheduler types.
    ///
    /// Setting the event interval determines the effective "priority" of delivering
    /// these external events (which may wake up additional tasks), compared to
    /// executing tasks that are currently ready to run. A smaller value is useful
    /// when tasks frequently spend a long time in polling, or frequently yield,
    /// which can result in overly long delays picking up I/O events. Conversely,
    /// picking up new events requires extra synchronization and syscall overhead,
    /// so if tasks generally complete their polling quickly, a higher event interval
    /// will minimize that overhead while still keeping the scheduler responsive to
    /// events.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::runtime;
    /// # pub fn main() {
    /// let rt = runtime::Builder::new_multi_thread()
    ///     .event_interval(31)
    ///     .build();
    /// # }
    /// ```
    pub fn event_interval(&mut self, val: u32) -> &mut Self {
        self.event_interval = val;
        self
    }

    cfg_unstable! {
        /// Configure how the runtime responds to an unhandled panic on a
        /// spawned task.
        ///
        /// By default, an unhandled panic (i.e. a panic not caught by
        /// [`std::panic::catch_unwind`]) has no impact on the runtime's
        /// execution. The panic's error value is forwarded to the task's
        /// [`JoinHandle`] and all other spawned tasks continue running.
        ///
        /// The `unhandled_panic` option enables configuring this behavior.
        ///
        /// * `UnhandledPanic::Ignore` is the default behavior. Panics on
        ///   spawned tasks have no impact on the runtime's execution.
        /// * `UnhandledPanic::ShutdownRuntime` will force the runtime to
        ///   shutdown immediately when a spawned task panics even if that
        ///   task's `JoinHandle` has not been dropped. All other spawned tasks
        ///   will immediately terminate and further calls to
        ///   [`Runtime::block_on`] will panic.
        ///
        /// # Panics
        /// This method panics if called with [`UnhandledPanic::ShutdownRuntime`]
        /// on a runtime other than the current thread runtime.
        ///
        /// # Unstable
        ///
        /// This option is currently unstable and its implementation is
        /// incomplete. The API may change or be removed in the future. See
        /// issue [tokio-rs/tokio#4516] for more details.
        ///
        /// # Examples
        ///
        /// The following demonstrates a runtime configured to shutdown on
        /// panic. The first spawned task panics and results in the runtime
        /// shutting down. The second spawned task never has a chance to
        /// execute. The call to `block_on` will panic due to the runtime being
        /// forcibly shutdown.
        ///
        /// ```should_panic
        /// use tokio::runtime::{self, UnhandledPanic};
        ///
        /// # pub fn main() {
        /// let rt = runtime::Builder::new_current_thread()
        ///     .unhandled_panic(UnhandledPanic::ShutdownRuntime)
        ///     .build()
        ///     .unwrap();
        ///
        /// rt.spawn(async { panic!("boom"); });
        /// rt.spawn(async {
        ///     // This task never completes.
        /// });
        ///
        /// rt.block_on(async {
        ///     // Do some work
        /// # loop { tokio::task::yield_now().await; }
        /// })
        /// # }
        /// ```
        ///
        /// [`JoinHandle`]: struct@crate::task::JoinHandle
        /// [tokio-rs/tokio#4516]: https://github.com/tokio-rs/tokio/issues/4516
        pub fn unhandled_panic(&mut self, behavior: UnhandledPanic) -> &mut Self {
            if !matches!(self.kind, Kind::CurrentThread) && matches!(behavior, UnhandledPanic::ShutdownRuntime) {
                panic!("UnhandledPanic::ShutdownRuntime is only supported in current thread runtime");
            }

            self.unhandled_panic = behavior;
            self
        }

        /// Disables the LIFO task scheduler heuristic.
        ///
        /// The multi-threaded scheduler includes a heuristic for optimizing
        /// message-passing patterns. This heuristic results in the **last**
        /// scheduled task being polled first.
        ///
        /// To implement this heuristic, each worker thread has a slot which
        /// holds the task that should be polled next. However, this slot cannot
        /// be stolen by other worker threads, which can result in lower total
        /// throughput when tasks tend to have longer poll times.
        ///
        /// This configuration option will disable this heuristic resulting in
        /// all scheduled tasks being pushed into the worker-local queue, which
        /// is stealable.
        ///
        /// Consider trying this option when the task "scheduled" time is high
        /// but the runtime is underutilized. Use [tokio-rs/tokio-metrics] to
        /// collect this data.
        ///
        /// # Unstable
        ///
        /// This configuration option is considered a workaround for the LIFO
        /// slot not being stealable. When the slot becomes stealable, we will
        /// revisit whether or not this option is necessary. See
        /// issue [tokio-rs/tokio#4941].
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime;
        ///
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .disable_lifo_slot()
        ///     .build()
        ///     .unwrap();
        /// ```
        ///
        /// [tokio-rs/tokio-metrics]: https://github.com/tokio-rs/tokio-metrics
        /// [tokio-rs/tokio#4941]: https://github.com/tokio-rs/tokio/issues/4941
        pub fn disable_lifo_slot(&mut self) -> &mut Self {
            self.disable_lifo_slot = true;
            self
        }

        /// Specifies the random number generation seed to use within all
        /// threads associated with the runtime being built.
        ///
        /// This option is intended to make certain parts of the runtime
        /// deterministic (e.g. the [`tokio::select!`] macro). In the case of
        /// [`tokio::select!`] it will ensure that the order that branches are
        /// polled is deterministic.
        ///
        /// In addition to the code specifying `rng_seed` and interacting with
        /// the runtime, the internals of Tokio and the Rust compiler may affect
        /// the sequences of random numbers. In order to ensure repeatable
        /// results, the version of Tokio, the versions of all other
        /// dependencies that interact with Tokio, and the Rust compiler version
        /// should also all remain constant.
        ///
        /// # Examples
        ///
        /// ```
        /// # use tokio::runtime::{self, RngSeed};
        /// # pub fn main() {
        /// let seed = RngSeed::from_bytes(b"place your seed here");
        /// let rt = runtime::Builder::new_current_thread()
        ///     .rng_seed(seed)
        ///     .build();
        /// # }
        /// ```
        ///
        /// [`tokio::select!`]: crate::select
        pub fn rng_seed(&mut self, seed: RngSeed) -> &mut Self {
            self.seed_generator = RngSeedGenerator::new(seed);
            self
        }
    }

    cfg_unstable_metrics! {
        /// Enables tracking the distribution of task poll times.
        ///
        /// Task poll times are not instrumented by default as doing so requires
        /// calling [`Instant::now()`] twice per task poll, which could add
        /// measurable overhead. Use the [`Handle::metrics()`] to access the
        /// metrics data.
        ///
        /// The histogram uses fixed bucket sizes. In other words, the histogram
        /// buckets are not dynamic based on input values. Use the
        /// `metrics_poll_time_histogram` builder methods to configure the
        /// histogram details.
        ///
        /// By default, a linear histogram with 10 buckets each 100 microseconds wide will be used.
        /// This has an extremely low memory footprint, but may not provide enough granularity. For
        /// better granularity with low memory usage, use [`metrics_poll_time_histogram_configuration()`]
        /// to select [`LogHistogram`] instead.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime;
        ///
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .enable_metrics_poll_time_histogram()
        ///     .build()
        ///     .unwrap();
        /// # // Test default values here
        /// # fn us(n: u64) -> std::time::Duration { std::time::Duration::from_micros(n) }
        /// # let m = rt.handle().metrics();
        /// # assert_eq!(m.poll_time_histogram_num_buckets(), 10);
        /// # assert_eq!(m.poll_time_histogram_bucket_range(0), us(0)..us(100));
        /// # assert_eq!(m.poll_time_histogram_bucket_range(1), us(100)..us(200));
        /// ```
        ///
        /// [`Handle::metrics()`]: crate::runtime::Handle::metrics
        /// [`Instant::now()`]: std::time::Instant::now
        /// [`LogHistogram`]: crate::runtime::LogHistogram
        /// [`metrics_poll_time_histogram_configuration()`]: Builder::metrics_poll_time_histogram_configuration
        pub fn enable_metrics_poll_time_histogram(&mut self) -> &mut Self {
            self.metrics_poll_count_histogram_enable = true;
            self
        }

        /// Deprecated. Use [`enable_metrics_poll_time_histogram()`] instead.
        ///
        /// [`enable_metrics_poll_time_histogram()`]: Builder::enable_metrics_poll_time_histogram
        #[deprecated(note = "`poll_count_histogram` related methods have been renamed `poll_time_histogram` to better reflect their functionality.")]
        #[doc(hidden)]
        pub fn enable_metrics_poll_count_histogram(&mut self) -> &mut Self {
            self.enable_metrics_poll_time_histogram()
        }

        /// Sets the histogram scale for tracking the distribution of task poll
        /// times.
        ///
        /// Tracking the distribution of task poll times can be done using a
        /// linear or log scale. When using linear scale, each histogram bucket
        /// will represent the same range of poll times. When using log scale,
        /// each histogram bucket will cover a range twice as big as the
        /// previous bucket.
        ///
        /// **Default:** linear scale.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::{self, HistogramScale};
        ///
        /// # #[allow(deprecated)]
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .enable_metrics_poll_time_histogram()
        ///     .metrics_poll_count_histogram_scale(HistogramScale::Log)
        ///     .build()
        ///     .unwrap();
        /// ```
        #[deprecated(note = "use `metrics_poll_time_histogram_configuration`")]
        pub fn metrics_poll_count_histogram_scale(&mut self, histogram_scale: crate::runtime::HistogramScale) -> &mut Self {
            self.metrics_poll_count_histogram.legacy_mut(|b|b.scale = histogram_scale);
            self
        }

        /// Configure the histogram for tracking poll times
        ///
        /// By default, a linear histogram with 10 buckets each 100 microseconds wide will be used.
        /// This has an extremely low memory footprint, but may not provide enough granularity. For
        /// better granularity with low memory usage, use [`LogHistogram`] instead.
        ///
        /// # Examples
        /// Configure a [`LogHistogram`] with [default configuration]:
        /// ```
        /// use tokio::runtime;
        /// use tokio::runtime::{HistogramConfiguration, LogHistogram};
        ///
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .enable_metrics_poll_time_histogram()
        ///     .metrics_poll_time_histogram_configuration(
        ///         HistogramConfiguration::log(LogHistogram::default())
        ///     )
        ///     .build()
        ///     .unwrap();
        /// ```
        ///
        /// Configure a linear histogram with 100 buckets, each 10μs wide
        /// ```
        /// use tokio::runtime;
        /// use std::time::Duration;
        /// use tokio::runtime::HistogramConfiguration;
        ///
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .enable_metrics_poll_time_histogram()
        ///     .metrics_poll_time_histogram_configuration(
        ///         HistogramConfiguration::linear(Duration::from_micros(10), 100)
        ///     )
        ///     .build()
        ///     .unwrap();
        /// ```
        ///
        /// Configure a [`LogHistogram`] with the following settings:
        /// - Measure times from 100ns to 120s
        /// - Max error of 0.1
        /// - No more than 1024 buckets
        /// ```
        /// use std::time::Duration;
        /// use tokio::runtime;
        /// use tokio::runtime::{HistogramConfiguration, LogHistogram};
        ///
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .enable_metrics_poll_time_histogram()
        ///     .metrics_poll_time_histogram_configuration(
        ///         HistogramConfiguration::log(LogHistogram::builder()
        ///             .max_value(Duration::from_secs(120))
        ///             .min_value(Duration::from_nanos(100))
        ///             .max_error(0.1)
        ///             .max_buckets(1024)
        ///             .expect("configuration uses 488 buckets")
        ///         )
        ///     )
        ///     .build()
        ///     .unwrap();
        /// ```
        ///
        /// When migrating from the legacy histogram ([`HistogramScale::Log`]) and wanting
        /// to match the previous behavior, use `precision_exact(0)`. This creates a histogram
        /// where each bucket is twice the size of the previous bucket.
        /// ```rust
        /// use std::time::Duration;
        /// use tokio::runtime::{HistogramConfiguration, LogHistogram};
        /// let rt = tokio::runtime::Builder::new_current_thread()
        ///     .enable_all()
        ///     .enable_metrics_poll_time_histogram()
        ///     .metrics_poll_time_histogram_configuration(HistogramConfiguration::log(
        ///         LogHistogram::builder()
        ///             .min_value(Duration::from_micros(20))
        ///             .max_value(Duration::from_millis(4))
        ///             // Set `precision_exact` to `0` to match `HistogramScale::Log`
        ///             .precision_exact(0)
        ///             .max_buckets(10)
        ///             .unwrap(),
        ///     ))
        ///     .build()
        ///     .unwrap();
        /// ```
        ///
        /// [`LogHistogram`]: crate::runtime::LogHistogram
        /// [default configuration]: crate::runtime::LogHistogramBuilder
        /// [`HistogramScale::Log`]: crate::runtime::HistogramScale::Log
        pub fn metrics_poll_time_histogram_configuration(&mut self, configuration: HistogramConfiguration) -> &mut Self {
            self.metrics_poll_count_histogram.histogram_type = configuration.inner;
            self
        }

        /// Sets the histogram resolution for tracking the distribution of task
        /// poll times.
        ///
        /// The resolution is the histogram's first bucket's range. When using a
        /// linear histogram scale, each bucket will cover the same range. When
        /// using a log scale, each bucket will cover a range twice as big as
        /// the previous bucket. In the log case, the resolution represents the
        /// smallest bucket range.
        ///
        /// Note that, when using log scale, the resolution is rounded up to the
        /// nearest power of 2 in nanoseconds.
        ///
        /// **Default:** 100 microseconds.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime;
        /// use std::time::Duration;
        ///
        /// # #[allow(deprecated)]
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .enable_metrics_poll_time_histogram()
        ///     .metrics_poll_count_histogram_resolution(Duration::from_micros(100))
        ///     .build()
        ///     .unwrap();
        /// ```
        #[deprecated(note = "use `metrics_poll_time_histogram_configuration`")]
        pub fn metrics_poll_count_histogram_resolution(&mut self, resolution: Duration) -> &mut Self {
            assert!(resolution > Duration::from_secs(0));
            // Sanity check the argument and also make the cast below safe.
            assert!(resolution <= Duration::from_secs(1));

            let resolution = resolution.as_nanos() as u64;

            self.metrics_poll_count_histogram.legacy_mut(|b|b.resolution = resolution);
            self
        }

        /// Sets the number of buckets for the histogram tracking the
        /// distribution of task poll times.
        ///
        /// The last bucket tracks all greater values that fall out of other
        /// ranges. So, configuring the histogram using a linear scale,
        /// resolution of 50ms, and 10 buckets, the 10th bucket will track task
        /// polls that take more than 450ms to complete.
        ///
        /// **Default:** 10
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime;
        ///
        /// # #[allow(deprecated)]
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .enable_metrics_poll_time_histogram()
        ///     .metrics_poll_count_histogram_buckets(15)
        ///     .build()
        ///     .unwrap();
        /// ```
        #[deprecated(note = "use `metrics_poll_time_histogram_configuration`")]
        pub fn metrics_poll_count_histogram_buckets(&mut self, buckets: usize) -> &mut Self {
            self.metrics_poll_count_histogram.legacy_mut(|b|b.num_buckets = buckets);
            self
        }
    }

    fn build_current_thread_runtime(&mut self) -> io::Result<Runtime> {
        use crate::runtime::runtime::Scheduler;

        let (scheduler, handle, blocking_pool) =
            self.build_current_thread_runtime_components(None)?;

        Ok(Runtime::from_parts(
            Scheduler::CurrentThread(scheduler),
            handle,
            blocking_pool,
        ))
    }

    #[cfg(tokio_unstable)]
    fn build_current_thread_local_runtime(&mut self) -> io::Result<LocalRuntime> {
        use crate::runtime::local_runtime::LocalRuntimeScheduler;

        let tid = std::thread::current().id();

        let (scheduler, handle, blocking_pool) =
            self.build_current_thread_runtime_components(Some(tid))?;

        Ok(LocalRuntime::from_parts(
            LocalRuntimeScheduler::CurrentThread(scheduler),
            handle,
            blocking_pool,
        ))
    }

    fn build_current_thread_runtime_components(
        &mut self,
        local_tid: Option<ThreadId>,
    ) -> io::Result<(CurrentThread, Handle, BlockingPool)> {
        use crate::runtime::scheduler;
        use crate::runtime::Config;

        let (driver, driver_handle) = driver::Driver::new(self.get_cfg())?;

        // Blocking pool
        let blocking_pool = blocking::create_blocking_pool(self, self.max_blocking_threads);
        let blocking_spawner = blocking_pool.spawner().clone();

        // Generate a rng seed for this runtime.
        let seed_generator_1 = self.seed_generator.next_generator();
        let seed_generator_2 = self.seed_generator.next_generator();

        // And now put a single-threaded scheduler on top of the timer. When
        // there are no futures ready to do something, it'll let the timer or
        // the reactor to generate some new stimuli for the futures to continue
        // in their life.
        let (scheduler, handle) = CurrentThread::new(
            driver,
            driver_handle,
            blocking_spawner,
            seed_generator_2,
            Config {
                before_park: self.before_park.clone(),
                after_unpark: self.after_unpark.clone(),
                before_spawn: self.before_spawn.clone(),
                #[cfg(tokio_unstable)]
                before_poll: self.before_poll.clone(),
                #[cfg(tokio_unstable)]
                after_poll: self.after_poll.clone(),
                after_termination: self.after_termination.clone(),
                global_queue_interval: self.global_queue_interval,
                event_interval: self.event_interval,
                #[cfg(tokio_unstable)]
                unhandled_panic: self.unhandled_panic.clone(),
                disable_lifo_slot: self.disable_lifo_slot,
                seed_generator: seed_generator_1,
                metrics_poll_count_histogram: self.metrics_poll_count_histogram_builder(),
            },
            local_tid,
        );

        let handle = Handle {
            inner: scheduler::Handle::CurrentThread(handle),
        };

        Ok((scheduler, handle, blocking_pool))
    }

    fn metrics_poll_count_histogram_builder(&self) -> Option<HistogramBuilder> {
        if self.metrics_poll_count_histogram_enable {
            Some(self.metrics_poll_count_histogram.clone())
        } else {
            None
        }
    }
}

cfg_io_driver! {
    impl Builder {
        /// Enables the I/O driver.
        ///
        /// Doing this enables using net, process, signal, and some I/O types on
        /// the runtime.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime;
        ///
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .enable_io()
        ///     .build()
        ///     .unwrap();
        /// ```
        pub fn enable_io(&mut self) -> &mut Self {
            self.enable_io = true;
            self
        }

        /// Enables the I/O driver and configures the max number of events to be
        /// processed per tick.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime;
        ///
        /// let rt = runtime::Builder::new_current_thread()
        ///     .enable_io()
        ///     .max_io_events_per_tick(1024)
        ///     .build()
        ///     .unwrap();
        /// ```
        pub fn max_io_events_per_tick(&mut self, capacity: usize) -> &mut Self {
            self.nevents = capacity;
            self
        }
    }
}

cfg_time! {
    impl Builder {
        /// Enables the time driver.
        ///
        /// Doing this enables using `tokio::time` on the runtime.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime;
        ///
        /// let rt = runtime::Builder::new_multi_thread()
        ///     .enable_time()
        ///     .build()
        ///     .unwrap();
        /// ```
        pub fn enable_time(&mut self) -> &mut Self {
            self.enable_time = true;
            self
        }
    }
}

cfg_test_util! {
    impl Builder {
        /// Controls if the runtime's clock starts paused or advancing.
        ///
        /// Pausing time requires the current-thread runtime; construction of
        /// the runtime will panic otherwise.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime;
        ///
        /// let rt = runtime::Builder::new_current_thread()
        ///     .enable_time()
        ///     .start_paused(true)
        ///     .build()
        ///     .unwrap();
        /// ```
        pub fn start_paused(&mut self, start_paused: bool) -> &mut Self {
            self.start_paused = start_paused;
            self
        }
    }
}

cfg_rt_multi_thread! {
    impl Builder {
        fn build_threaded_runtime(&mut self) -> io::Result<Runtime> {
            use crate::loom::sys::num_cpus;
            use crate::runtime::{Config, runtime::Scheduler};
            use crate::runtime::scheduler::{self, MultiThread};

            let worker_threads = self.worker_threads.unwrap_or_else(num_cpus);

            let (driver, driver_handle) = driver::Driver::new(self.get_cfg())?;

            // Create the blocking pool
            let blocking_pool =
                blocking::create_blocking_pool(self, self.max_blocking_threads + worker_threads);
            let blocking_spawner = blocking_pool.spawner().clone();

            // Generate a rng seed for this runtime.
            let seed_generator_1 = self.seed_generator.next_generator();
            let seed_generator_2 = self.seed_generator.next_generator();

            let (scheduler, handle, launch) = MultiThread::new(
                worker_threads,
                driver,
                driver_handle,
                blocking_spawner,
                seed_generator_2,
                Config {
                    before_park: self.before_park.clone(),
                    after_unpark: self.after_unpark.clone(),
                    before_spawn: self.before_spawn.clone(),
                    #[cfg(tokio_unstable)]
                    before_poll: self.before_poll.clone(),
                    #[cfg(tokio_unstable)]
                    after_poll: self.after_poll.clone(),
                    after_termination: self.after_termination.clone(),
                    global_queue_interval: self.global_queue_interval,
                    event_interval: self.event_interval,
                    #[cfg(tokio_unstable)]
                    unhandled_panic: self.unhandled_panic.clone(),
                    disable_lifo_slot: self.disable_lifo_slot,
                    seed_generator: seed_generator_1,
                    metrics_poll_count_histogram: self.metrics_poll_count_histogram_builder(),
                },
            );

            let handle = Handle { inner: scheduler::Handle::MultiThread(handle) };

            // Spawn the thread pool workers
            let _enter = handle.enter();
            launch.launch();

            Ok(Runtime::from_parts(Scheduler::MultiThread(scheduler), handle, blocking_pool))
        }
    }
}

impl fmt::Debug for Builder {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Builder")
            .field("worker_threads", &self.worker_threads)
            .field("max_blocking_threads", &self.max_blocking_threads)
            .field(
                "thread_name",
                &"<dyn Fn() -> String + Send + Sync + 'static>",
            )
            .field("thread_stack_size", &self.thread_stack_size)
            .field("after_start", &self.after_start.as_ref().map(|_| "..."))
            .field("before_stop", &self.before_stop.as_ref().map(|_| "..."))
            .field("before_park", &self.before_park.as_ref().map(|_| "..."))
            .field("after_unpark", &self.after_unpark.as_ref().map(|_| "..."))
            .finish()
    }
}
