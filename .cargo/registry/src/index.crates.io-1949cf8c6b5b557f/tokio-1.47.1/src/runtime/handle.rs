#[cfg(tokio_unstable)]
use crate::runtime;
use crate::runtime::{context, scheduler, RuntimeFlavor, RuntimeMetrics};

/// Handle to the runtime.
///
/// The handle is internally reference-counted and can be freely cloned. A handle can be
/// obtained using the [`Runtime::handle`] method.
///
/// [`Runtime::handle`]: crate::runtime::Runtime::handle()
#[derive(Debug, Clone)]
// When the `rt` feature is *not* enabled, this type is still defined, but not
// included in the public API.
pub struct Handle {
    pub(crate) inner: scheduler::Handle,
}

use crate::runtime::task::JoinHandle;
use crate::runtime::BOX_FUTURE_THRESHOLD;
use crate::util::error::{CONTEXT_MISSING_ERROR, THREAD_LOCAL_DESTROYED_ERROR};
use crate::util::trace::SpawnMeta;

use std::future::Future;
use std::marker::PhantomData;
use std::{error, fmt, mem};

/// Runtime context guard.
///
/// Returned by [`Runtime::enter`] and [`Handle::enter`], the context guard exits
/// the runtime context on drop.
///
/// [`Runtime::enter`]: fn@crate::runtime::Runtime::enter
#[derive(Debug)]
#[must_use = "Creating and dropping a guard does nothing"]
pub struct EnterGuard<'a> {
    _guard: context::SetCurrentGuard,
    _handle_lifetime: PhantomData<&'a Handle>,
}

impl Handle {
    /// Enters the runtime context. This allows you to construct types that must
    /// have an executor available on creation such as [`Sleep`] or
    /// [`TcpStream`]. It will also allow you to call methods such as
    /// [`tokio::spawn`] and [`Handle::current`] without panicking.
    ///
    /// # Panics
    ///
    /// When calling `Handle::enter` multiple times, the returned guards
    /// **must** be dropped in the reverse order that they were acquired.
    /// Failure to do so will result in a panic and possible memory leaks.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// let rt = Runtime::new().unwrap();
    ///
    /// let _guard = rt.enter();
    /// tokio::spawn(async {
    ///     println!("Hello world!");
    /// });
    /// ```
    ///
    /// Do **not** do the following, this shows a scenario that will result in a
    /// panic and possible memory leak.
    ///
    /// ```should_panic
    /// use tokio::runtime::Runtime;
    ///
    /// let rt1 = Runtime::new().unwrap();
    /// let rt2 = Runtime::new().unwrap();
    ///
    /// let enter1 = rt1.enter();
    /// let enter2 = rt2.enter();
    ///
    /// drop(enter1);
    /// drop(enter2);
    /// ```
    ///
    /// [`Sleep`]: struct@crate::time::Sleep
    /// [`TcpStream`]: struct@crate::net::TcpStream
    /// [`tokio::spawn`]: fn@crate::spawn
    pub fn enter(&self) -> EnterGuard<'_> {
        EnterGuard {
            _guard: match context::try_set_current(&self.inner) {
                Some(guard) => guard,
                None => panic!("{}", crate::util::error::THREAD_LOCAL_DESTROYED_ERROR),
            },
            _handle_lifetime: PhantomData,
        }
    }

    /// Returns a `Handle` view over the currently running `Runtime`.
    ///
    /// # Panics
    ///
    /// This will panic if called outside the context of a Tokio runtime. That means that you must
    /// call this on one of the threads **being run by the runtime**, or from a thread with an active
    /// `EnterGuard`. Calling this from within a thread created by `std::thread::spawn` (for example)
    /// will cause a panic unless that thread has an active `EnterGuard`.
    ///
    /// # Examples
    ///
    /// This can be used to obtain the handle of the surrounding runtime from an async
    /// block or function running on that runtime.
    ///
    /// ```
    /// # use std::thread;
    /// # use tokio::runtime::Runtime;
    /// # fn dox() {
    /// # let rt = Runtime::new().unwrap();
    /// # rt.spawn(async {
    /// use tokio::runtime::Handle;
    ///
    /// // Inside an async block or function.
    /// let handle = Handle::current();
    /// handle.spawn(async {
    ///     println!("now running in the existing Runtime");
    /// });
    ///
    /// # let handle =
    /// thread::spawn(move || {
    ///     // Notice that the handle is created outside of this thread and then moved in
    ///     handle.spawn(async { /* ... */ });
    ///     // This next line would cause a panic because we haven't entered the runtime
    ///     // and created an EnterGuard
    ///     // let handle2 = Handle::current(); // panic
    ///     // So we create a guard here with Handle::enter();
    ///     let _guard = handle.enter();
    ///     // Now we can call Handle::current();
    ///     let handle2 = Handle::current();
    /// });
    /// # handle.join().unwrap();
    /// # });
    /// # }
    /// ```
    #[track_caller]
    pub fn current() -> Self {
        Handle {
            inner: scheduler::Handle::current(),
        }
    }

    /// Returns a Handle view over the currently running Runtime
    ///
    /// Returns an error if no Runtime has been started
    ///
    /// Contrary to `current`, this never panics
    pub fn try_current() -> Result<Self, TryCurrentError> {
        context::with_current(|inner| Handle {
            inner: inner.clone(),
        })
    }

    /// Spawns a future onto the Tokio runtime.
    ///
    /// This spawns the given future onto the runtime's executor, usually a
    /// thread pool. The thread pool is then responsible for polling the future
    /// until it completes.
    ///
    /// The provided future will start running in the background immediately
    /// when `spawn` is called, even if you don't await the returned
    /// `JoinHandle`.
    ///
    /// See [module level][mod] documentation for more details.
    ///
    /// [mod]: index.html
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// # fn dox() {
    /// // Create the runtime
    /// let rt = Runtime::new().unwrap();
    /// // Get a handle from this runtime
    /// let handle = rt.handle();
    ///
    /// // Spawn a future onto the runtime using the handle
    /// handle.spawn(async {
    ///     println!("now running on a worker thread");
    /// });
    /// # }
    /// ```
    #[track_caller]
    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let fut_size = mem::size_of::<F>();
        if fut_size > BOX_FUTURE_THRESHOLD {
            self.spawn_named(Box::pin(future), SpawnMeta::new_unnamed(fut_size))
        } else {
            self.spawn_named(future, SpawnMeta::new_unnamed(fut_size))
        }
    }

    /// Runs the provided function on an executor dedicated to blocking
    /// operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// # fn dox() {
    /// // Create the runtime
    /// let rt = Runtime::new().unwrap();
    /// // Get a handle from this runtime
    /// let handle = rt.handle();
    ///
    /// // Spawn a blocking function onto the runtime using the handle
    /// handle.spawn_blocking(|| {
    ///     println!("now running on a worker thread");
    /// });
    /// # }
    #[track_caller]
    pub fn spawn_blocking<F, R>(&self, func: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.inner.blocking_spawner().spawn_blocking(self, func)
    }

    /// Runs a future to completion on this `Handle`'s associated `Runtime`.
    ///
    /// This runs the given future on the current thread, blocking until it is
    /// complete, and yielding its resolved result. Any tasks or timers which
    /// the future spawns internally will be executed on the runtime.
    ///
    /// When this is used on a `current_thread` runtime, only the
    /// [`Runtime::block_on`] method can drive the IO and timer drivers, but the
    /// `Handle::block_on` method cannot drive them. This means that, when using
    /// this method on a `current_thread` runtime, anything that relies on IO or
    /// timers will not work unless there is another thread currently calling
    /// [`Runtime::block_on`] on the same runtime.
    ///
    /// # If the runtime has been shut down
    ///
    /// If the `Handle`'s associated `Runtime` has been shut down (through
    /// [`Runtime::shutdown_background`], [`Runtime::shutdown_timeout`], or by
    /// dropping it) and `Handle::block_on` is used it might return an error or
    /// panic. Specifically IO resources will return an error and timers will
    /// panic. Runtime independent futures will run as normal.
    ///
    /// # Panics
    ///
    /// This function panics if the provided future panics, if called within an
    /// asynchronous execution context, or if a timer future is executed on a runtime that has been
    /// shut down.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// // Create the runtime
    /// let rt  = Runtime::new().unwrap();
    ///
    /// // Get a handle from this runtime
    /// let handle = rt.handle();
    ///
    /// // Execute the future, blocking the current thread until completion
    /// handle.block_on(async {
    ///     println!("hello");
    /// });
    /// ```
    ///
    /// Or using `Handle::current`:
    ///
    /// ```
    /// use tokio::runtime::Handle;
    ///
    /// #[tokio::main]
    /// async fn main () {
    ///     let handle = Handle::current();
    ///     std::thread::spawn(move || {
    ///         // Using Handle::block_on to run async code in the new thread.
    ///         handle.block_on(async {
    ///             println!("hello");
    ///         });
    ///     });
    /// }
    /// ```
    ///
    /// [`JoinError`]: struct@crate::task::JoinError
    /// [`JoinHandle`]: struct@crate::task::JoinHandle
    /// [`Runtime::block_on`]: fn@crate::runtime::Runtime::block_on
    /// [`Runtime::shutdown_background`]: fn@crate::runtime::Runtime::shutdown_background
    /// [`Runtime::shutdown_timeout`]: fn@crate::runtime::Runtime::shutdown_timeout
    /// [`spawn_blocking`]: crate::task::spawn_blocking
    /// [`tokio::fs`]: crate::fs
    /// [`tokio::net`]: crate::net
    /// [`tokio::time`]: crate::time
    #[track_caller]
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        let fut_size = mem::size_of::<F>();
        if fut_size > BOX_FUTURE_THRESHOLD {
            self.block_on_inner(Box::pin(future), SpawnMeta::new_unnamed(fut_size))
        } else {
            self.block_on_inner(future, SpawnMeta::new_unnamed(fut_size))
        }
    }

    #[track_caller]
    fn block_on_inner<F: Future>(&self, future: F, _meta: SpawnMeta<'_>) -> F::Output {
        #[cfg(all(
            tokio_unstable,
            tokio_taskdump,
            feature = "rt",
            target_os = "linux",
            any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")
        ))]
        let future = super::task::trace::Trace::root(future);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let future =
            crate::util::trace::task(future, "block_on", _meta, super::task::Id::next().as_u64());

        // Enter the runtime context. This sets the current driver handles and
        // prevents blocking an existing runtime.
        context::enter_runtime(&self.inner, true, |blocking| {
            blocking.block_on(future).expect("failed to park thread")
        })
    }

    #[track_caller]
    pub(crate) fn spawn_named<F>(&self, future: F, meta: SpawnMeta<'_>) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let id = crate::runtime::task::Id::next();
        #[cfg(all(
            tokio_unstable,
            tokio_taskdump,
            feature = "rt",
            target_os = "linux",
            any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")
        ))]
        let future = super::task::trace::Trace::root(future);
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let future = crate::util::trace::task(future, "task", meta, id.as_u64());
        self.inner.spawn(future, id, meta.spawned_at)
    }

    #[track_caller]
    #[allow(dead_code)]
    pub(crate) unsafe fn spawn_local_named<F>(
        &self,
        future: F,
        meta: SpawnMeta<'_>,
    ) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let id = crate::runtime::task::Id::next();
        #[cfg(all(
            tokio_unstable,
            tokio_taskdump,
            feature = "rt",
            target_os = "linux",
            any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")
        ))]
        let future = super::task::trace::Trace::root(future);
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let future = crate::util::trace::task(future, "task", meta, id.as_u64());
        self.inner.spawn_local(future, id, meta.spawned_at)
    }

    /// Returns the flavor of the current `Runtime`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::{Handle, RuntimeFlavor};
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() {
    ///   assert_eq!(RuntimeFlavor::CurrentThread, Handle::current().runtime_flavor());
    /// }
    /// ```
    ///
    /// ```
    /// use tokio::runtime::{Handle, RuntimeFlavor};
    ///
    /// #[tokio::main(flavor = "multi_thread", worker_threads = 4)]
    /// async fn main() {
    ///   assert_eq!(RuntimeFlavor::MultiThread, Handle::current().runtime_flavor());
    /// }
    /// ```
    pub fn runtime_flavor(&self) -> RuntimeFlavor {
        match self.inner {
            scheduler::Handle::CurrentThread(_) => RuntimeFlavor::CurrentThread,
            #[cfg(feature = "rt-multi-thread")]
            scheduler::Handle::MultiThread(_) => RuntimeFlavor::MultiThread,
        }
    }

    cfg_unstable! {
        /// Returns the [`Id`] of the current `Runtime`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main(flavor = "current_thread")]
        /// async fn main() {
        ///   println!("Current runtime id: {}", Handle::current().id());
        /// }
        /// ```
        ///
        /// **Note**: This is an [unstable API][unstable]. The public API of this type
        /// may break in 1.x releases. See [the documentation on unstable
        /// features][unstable] for details.
        ///
        /// [unstable]: crate#unstable-features
        /// [`Id`]: struct@crate::runtime::Id
        pub fn id(&self) -> runtime::Id {
            let owned_id = match &self.inner {
                scheduler::Handle::CurrentThread(handle) => handle.owned_id(),
                #[cfg(feature = "rt-multi-thread")]
                scheduler::Handle::MultiThread(handle) => handle.owned_id(),
            };
            owned_id.into()
        }
    }

    /// Returns a view that lets you get information about how the runtime
    /// is performing.
    pub fn metrics(&self) -> RuntimeMetrics {
        RuntimeMetrics::new(self.clone())
    }
}

impl std::panic::UnwindSafe for Handle {}

impl std::panic::RefUnwindSafe for Handle {}

cfg_taskdump! {
    impl Handle {
        /// Captures a snapshot of the runtime's state.
        ///
        /// If you only want to capture a snapshot of a single future's state, you can use
        /// [`Trace::capture`][crate::runtime::dump::Trace].
        ///
        /// This functionality is experimental, and comes with a number of
        /// requirements and limitations.
        ///
        /// # Examples
        ///
        /// This can be used to get call traces of each task in the runtime.
        /// Calls to `Handle::dump` should usually be enclosed in a
        /// [timeout][crate::time::timeout], so that dumping does not escalate a
        /// single blocked runtime thread into an entirely blocked runtime.
        ///
        /// ```
        /// # use tokio::runtime::Runtime;
        /// # fn dox() {
        /// # let rt = Runtime::new().unwrap();
        /// # rt.spawn(async {
        /// use tokio::runtime::Handle;
        /// use tokio::time::{timeout, Duration};
        ///
        /// // Inside an async block or function.
        /// let handle = Handle::current();
        /// if let Ok(dump) = timeout(Duration::from_secs(2), handle.dump()).await {
        ///     for (i, task) in dump.tasks().iter().enumerate() {
        ///         let trace = task.trace();
        ///         println!("TASK {i}:");
        ///         println!("{trace}\n");
        ///     }
        /// }
        /// # });
        /// # }
        /// ```
        ///
        /// This produces highly detailed traces of tasks; e.g.:
        ///
        /// ```plain
        /// TASK 0:
        /// ╼ dump::main::{{closure}}::a::{{closure}} at /tokio/examples/dump.rs:18:20
        /// └╼ dump::main::{{closure}}::b::{{closure}} at /tokio/examples/dump.rs:23:20
        ///    └╼ dump::main::{{closure}}::c::{{closure}} at /tokio/examples/dump.rs:28:24
        ///       └╼ tokio::sync::barrier::Barrier::wait::{{closure}} at /tokio/tokio/src/sync/barrier.rs:129:10
        ///          └╼ <tokio::util::trace::InstrumentedAsyncOp<F> as core::future::future::Future>::poll at /tokio/tokio/src/util/trace.rs:77:46
        ///             └╼ tokio::sync::barrier::Barrier::wait_internal::{{closure}} at /tokio/tokio/src/sync/barrier.rs:183:36
        ///                └╼ tokio::sync::watch::Receiver<T>::changed::{{closure}} at /tokio/tokio/src/sync/watch.rs:604:55
        ///                   └╼ tokio::sync::watch::changed_impl::{{closure}} at /tokio/tokio/src/sync/watch.rs:755:18
        ///                      └╼ <tokio::sync::notify::Notified as core::future::future::Future>::poll at /tokio/tokio/src/sync/notify.rs:1103:9
        ///                         └╼ tokio::sync::notify::Notified::poll_notified at /tokio/tokio/src/sync/notify.rs:996:32
        /// ```
        ///
        /// # Requirements
        ///
        /// ## Debug Info Must Be Available
        ///
        /// To produce task traces, the application must **not** be compiled
        /// with `split debuginfo`. On Linux, including `debuginfo` within the
        /// application binary is the (correct) default. You can further ensure
        /// this behavior with the following directive in your `Cargo.toml`:
        ///
        /// ```toml
        /// [profile.*]
        /// split-debuginfo = "off"
        /// ```
        ///
        /// ## Unstable Features
        ///
        /// This functionality is **unstable**, and requires both the
        /// `tokio_unstable` and `tokio_taskdump` `cfg` flags to be set.
        ///
        /// You can do this by setting the `RUSTFLAGS` environment variable
        /// before invoking `cargo`; e.g.:
        /// ```bash
        /// RUSTFLAGS="--cfg tokio_unstable --cfg tokio_taskdump" cargo run --example dump
        /// ```
        ///
        /// Or by [configuring][cargo-config] `rustflags` in
        /// `.cargo/config.toml`:
        /// ```text
        /// [build]
        /// rustflags = ["--cfg", "tokio_unstable", "--cfg", "tokio_taskdump"]
        /// ```
        ///
        /// [cargo-config]:
        ///     https://doc.rust-lang.org/cargo/reference/config.html
        ///
        /// ## Platform Requirements
        ///
        /// Task dumps are supported on Linux atop `aarch64`, `x86` and `x86_64`.
        ///
        /// ## Current Thread Runtime Requirements
        ///
        /// On the `current_thread` runtime, task dumps may only be requested
        /// from *within* the context of the runtime being dumped. Do not, for
        /// example, await `Handle::dump()` on a different runtime.
        ///
        /// # Limitations
        ///
        /// ## Performance
        ///
        /// Although enabling the `tokio_taskdump` feature imposes virtually no
        /// additional runtime overhead, actually calling `Handle::dump` is
        /// expensive. The runtime must synchronize and pause its workers, then
        /// re-poll every task in a special tracing mode. Avoid requesting dumps
        /// often.
        ///
        /// ## Local Executors
        ///
        /// Tasks managed by local executors (e.g., `FuturesUnordered` and
        /// [`LocalSet`][crate::task::LocalSet]) may not appear in task dumps.
        ///
        /// ## Non-Termination When Workers Are Blocked
        ///
        /// The future produced by `Handle::dump` may never produce `Ready` if
        /// another runtime worker is blocked for more than 250ms. This may
        /// occur if a dump is requested during shutdown, or if another runtime
        /// worker is infinite looping or synchronously deadlocked. For these
        /// reasons, task dumping should usually be paired with an explicit
        /// [timeout][crate::time::timeout].
        pub async fn dump(&self) -> crate::runtime::Dump {
            match &self.inner {
                scheduler::Handle::CurrentThread(handle) => handle.dump(),
                #[cfg(all(feature = "rt-multi-thread", not(target_os = "wasi")))]
                scheduler::Handle::MultiThread(handle) => {
                    // perform the trace in a separate thread so that the
                    // trace itself does not appear in the taskdump.
                    let handle = handle.clone();
                    spawn_thread(async {
                        let handle = handle;
                        handle.dump().await
                    }).await
                },
            }
        }

        /// Produces `true` if the current task is being traced for a dump;
        /// otherwise false. This function is only public for integration
        /// testing purposes. Do not rely on it.
        #[doc(hidden)]
        pub fn is_tracing() -> bool {
            super::task::trace::Context::is_tracing()
        }
    }

    cfg_rt_multi_thread! {
        /// Spawn a new thread and asynchronously await on its result.
        async fn spawn_thread<F>(f: F) -> <F as Future>::Output
        where
            F: Future + Send + 'static,
            <F as Future>::Output: Send + 'static
        {
            let (tx, rx) = crate::sync::oneshot::channel();
            crate::loom::thread::spawn(|| {
                let rt = crate::runtime::Builder::new_current_thread().build().unwrap();
                rt.block_on(async {
                    let _ = tx.send(f.await);
                });
            });
            rx.await.unwrap()
        }
    }
}

/// Error returned by `try_current` when no Runtime has been started
#[derive(Debug)]
pub struct TryCurrentError {
    kind: TryCurrentErrorKind,
}

impl TryCurrentError {
    pub(crate) fn new_no_context() -> Self {
        Self {
            kind: TryCurrentErrorKind::NoContext,
        }
    }

    pub(crate) fn new_thread_local_destroyed() -> Self {
        Self {
            kind: TryCurrentErrorKind::ThreadLocalDestroyed,
        }
    }

    /// Returns true if the call failed because there is currently no runtime in
    /// the Tokio context.
    pub fn is_missing_context(&self) -> bool {
        matches!(self.kind, TryCurrentErrorKind::NoContext)
    }

    /// Returns true if the call failed because the Tokio context thread-local
    /// had been destroyed. This can usually only happen if in the destructor of
    /// other thread-locals.
    pub fn is_thread_local_destroyed(&self) -> bool {
        matches!(self.kind, TryCurrentErrorKind::ThreadLocalDestroyed)
    }
}

enum TryCurrentErrorKind {
    NoContext,
    ThreadLocalDestroyed,
}

impl fmt::Debug for TryCurrentErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TryCurrentErrorKind::NoContext => f.write_str("NoContext"),
            TryCurrentErrorKind::ThreadLocalDestroyed => f.write_str("ThreadLocalDestroyed"),
        }
    }
}

impl fmt::Display for TryCurrentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TryCurrentErrorKind as E;
        match self.kind {
            E::NoContext => f.write_str(CONTEXT_MISSING_ERROR),
            E::ThreadLocalDestroyed => f.write_str(THREAD_LOCAL_DESTROYED_ERROR),
        }
    }
}

impl error::Error for TryCurrentError {}
