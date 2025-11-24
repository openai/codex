#![allow(irrefutable_let_patterns)]

use crate::runtime::blocking::BlockingPool;
use crate::runtime::scheduler::CurrentThread;
use crate::runtime::{context, Builder, EnterGuard, Handle, BOX_FUTURE_THRESHOLD};
use crate::task::JoinHandle;

use crate::util::trace::SpawnMeta;
use std::future::Future;
use std::marker::PhantomData;
use std::mem;
use std::time::Duration;

/// A local Tokio runtime.
///
/// This runtime is capable of driving tasks which are not `Send + Sync` without the use of a
/// `LocalSet`, and thus supports `spawn_local` without the need for a `LocalSet` context.
///
/// This runtime cannot be moved between threads or driven from different threads.
///
/// This runtime is incompatible with `LocalSet`. You should not attempt to drive a `LocalSet` within a
/// `LocalRuntime`.
///
/// Currently, this runtime supports one flavor, which is internally identical to `current_thread`,
/// save for the aforementioned differences related to `spawn_local`.
///
/// For more general information on how to use runtimes, see the [module] docs.
///
/// [runtime]: crate::runtime::Runtime
/// [module]: crate::runtime
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(tokio_unstable)))]
pub struct LocalRuntime {
    /// Task scheduler
    scheduler: LocalRuntimeScheduler,

    /// Handle to runtime, also contains driver handles
    handle: Handle,

    /// Blocking pool handle, used to signal shutdown
    blocking_pool: BlockingPool,

    /// Marker used to make this !Send and !Sync.
    _phantom: PhantomData<*mut u8>,
}

/// The runtime scheduler is always a `current_thread` scheduler right now.
#[derive(Debug)]
pub(crate) enum LocalRuntimeScheduler {
    /// Execute all tasks on the current-thread.
    CurrentThread(CurrentThread),
}

impl LocalRuntime {
    pub(crate) fn from_parts(
        scheduler: LocalRuntimeScheduler,
        handle: Handle,
        blocking_pool: BlockingPool,
    ) -> LocalRuntime {
        LocalRuntime {
            scheduler,
            handle,
            blocking_pool,
            _phantom: Default::default(),
        }
    }

    /// Creates a new local runtime instance with default configuration values.
    ///
    /// This results in the scheduler, I/O driver, and time driver being
    /// initialized.
    ///
    /// When a more complex configuration is necessary, the [runtime builder] may be used.
    ///
    /// See [module level][mod] documentation for more details.
    ///
    /// # Examples
    ///
    /// Creating a new `LocalRuntime` with default configuration values.
    ///
    /// ```
    /// use tokio::runtime::LocalRuntime;
    ///
    /// let rt = LocalRuntime::new()
    ///     .unwrap();
    ///
    /// // Use the runtime...
    /// ```
    ///
    /// [mod]: crate::runtime
    /// [runtime builder]: crate::runtime::Builder
    pub fn new() -> std::io::Result<LocalRuntime> {
        Builder::new_current_thread()
            .enable_all()
            .build_local(Default::default())
    }

    /// Returns a handle to the runtime's spawner.
    ///
    /// The returned handle can be used to spawn tasks that run on this runtime, and can
    /// be cloned to allow moving the `Handle` to other threads.
    ///
    /// As the handle can be sent to other threads, it can only be used to spawn tasks that are `Send`.
    ///
    /// Calling [`Handle::block_on`] on a handle to a `LocalRuntime` is error-prone.
    /// Refer to the documentation of [`Handle::block_on`] for more.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::LocalRuntime;
    ///
    /// let rt = LocalRuntime::new()
    ///     .unwrap();
    ///
    /// let handle = rt.handle();
    ///
    /// // Use the handle...
    /// ```
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Spawns a task on the runtime.
    ///
    /// This is analogous to the [`spawn`] method on the standard [`Runtime`], but works even if the task is not thread-safe.
    ///
    /// [`spawn`]: crate::runtime::Runtime::spawn
    /// [`Runtime`]: crate::runtime::Runtime
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::LocalRuntime;
    ///
    /// # fn dox() {
    /// // Create the runtime
    /// let rt = LocalRuntime::new().unwrap();
    ///
    /// // Spawn a future onto the runtime
    /// rt.spawn_local(async {
    ///     println!("now running on a worker thread");
    /// });
    /// # }
    /// ```
    #[track_caller]
    pub fn spawn_local<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let fut_size = std::mem::size_of::<F>();
        let meta = SpawnMeta::new_unnamed(fut_size);

        // safety: spawn_local can only be called from `LocalRuntime`, which this is
        unsafe {
            if std::mem::size_of::<F>() > BOX_FUTURE_THRESHOLD {
                self.handle.spawn_local_named(Box::pin(future), meta)
            } else {
                self.handle.spawn_local_named(future, meta)
            }
        }
    }

    /// Runs the provided function on a thread from a dedicated blocking thread pool.
    ///
    /// This function _will_ be run on another thread.
    ///
    /// See the [documentation in the non-local runtime][Runtime] for more
    /// information.
    ///
    /// [Runtime]: crate::runtime::Runtime::spawn_blocking
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::LocalRuntime;
    ///
    /// # fn dox() {
    /// // Create the runtime
    /// let rt = LocalRuntime::new().unwrap();
    ///
    /// // Spawn a blocking function onto the runtime
    /// rt.spawn_blocking(|| {
    ///     println!("now running on a worker thread");
    /// });
    /// # }
    /// ```
    #[track_caller]
    pub fn spawn_blocking<F, R>(&self, func: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.handle.spawn_blocking(func)
    }

    /// Runs a future to completion on the Tokio runtime. This is the
    /// runtime's entry point.
    ///
    /// See the documentation for [the equivalent method on Runtime][Runtime]
    /// for more information.
    ///
    /// [Runtime]: crate::runtime::Runtime::block_on
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::runtime::LocalRuntime;
    ///
    /// // Create the runtime
    /// let rt  = LocalRuntime::new().unwrap();
    ///
    /// // Execute the future, blocking the current thread until completion
    /// rt.block_on(async {
    ///     println!("hello");
    /// });
    /// ```
    #[track_caller]
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        let fut_size = mem::size_of::<F>();
        let meta = SpawnMeta::new_unnamed(fut_size);

        if std::mem::size_of::<F>() > BOX_FUTURE_THRESHOLD {
            self.block_on_inner(Box::pin(future), meta)
        } else {
            self.block_on_inner(future, meta)
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
        let future = crate::runtime::task::trace::Trace::root(future);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let future = crate::util::trace::task(
            future,
            "block_on",
            _meta,
            crate::runtime::task::Id::next().as_u64(),
        );

        let _enter = self.enter();

        if let LocalRuntimeScheduler::CurrentThread(exec) = &self.scheduler {
            exec.block_on(&self.handle.inner, future)
        } else {
            unreachable!("LocalRuntime only supports current_thread")
        }
    }

    /// Enters the runtime context.
    ///
    /// This allows you to construct types that must have an executor
    /// available on creation such as [`Sleep`] or [`TcpStream`]. It will
    /// also allow you to call methods such as [`tokio::spawn`].
    ///
    /// If this is a handle to a [`LocalRuntime`], and this function is being invoked from the same
    /// thread that the runtime was created on, you will also be able to call
    /// [`tokio::task::spawn_local`].
    ///
    /// [`Sleep`]: struct@crate::time::Sleep
    /// [`TcpStream`]: struct@crate::net::TcpStream
    /// [`tokio::spawn`]: fn@crate::spawn
    /// [`LocalRuntime`]: struct@crate::runtime::LocalRuntime
    /// [`tokio::task::spawn_local`]: fn@crate::task::spawn_local
    ///
    /// # Example
    ///
    /// ```
    /// use tokio::runtime::LocalRuntime;
    /// use tokio::task::JoinHandle;
    ///
    /// fn function_that_spawns(msg: String) -> JoinHandle<()> {
    ///     // Had we not used `rt.enter` below, this would panic.
    ///     tokio::spawn(async move {
    ///         println!("{}", msg);
    ///     })
    /// }
    ///
    /// fn main() {
    ///     let rt = LocalRuntime::new().unwrap();
    ///
    ///     let s = "Hello World!".to_string();
    ///
    ///     // By entering the context, we tie `tokio::spawn` to this executor.
    ///     let _guard = rt.enter();
    ///     let handle = function_that_spawns(s);
    ///
    ///     // Wait for the task before we end the test.
    ///     rt.block_on(handle).unwrap();
    /// }
    /// ```
    pub fn enter(&self) -> EnterGuard<'_> {
        self.handle.enter()
    }

    /// Shuts down the runtime, waiting for at most `duration` for all spawned
    /// work to stop.
    ///
    /// Note that `spawn_blocking` tasks, and only `spawn_blocking` tasks, can get left behind if
    /// the timeout expires.
    ///
    /// See the [struct level documentation](LocalRuntime#shutdown) for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::LocalRuntime;
    /// use tokio::task;
    ///
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// fn main() {
    ///    let runtime = LocalRuntime::new().unwrap();
    ///
    ///    runtime.block_on(async move {
    ///        task::spawn_blocking(move || {
    ///            thread::sleep(Duration::from_secs(10_000));
    ///        });
    ///    });
    ///
    ///    runtime.shutdown_timeout(Duration::from_millis(100));
    /// }
    /// ```
    pub fn shutdown_timeout(mut self, duration: Duration) {
        // Wakeup and shutdown all the worker threads
        self.handle.inner.shutdown();
        self.blocking_pool.shutdown(Some(duration));
    }

    /// Shuts down the runtime, without waiting for any spawned work to stop.
    ///
    /// This can be useful if you want to drop a runtime from within another runtime.
    /// Normally, dropping a runtime will block indefinitely for spawned blocking tasks
    /// to complete, which would normally not be permitted within an asynchronous context.
    /// By calling `shutdown_background()`, you can drop the runtime from such a context.
    ///
    /// Note however, that because we do not wait for any blocking tasks to complete, this
    /// may result in a resource leak (in that any blocking tasks are still running until they
    /// return. No other tasks will leak.
    ///
    /// See the [struct level documentation](LocalRuntime#shutdown) for more details.
    ///
    /// This function is equivalent to calling `shutdown_timeout(Duration::from_nanos(0))`.
    ///
    /// ```
    /// use tokio::runtime::LocalRuntime;
    ///
    /// fn main() {
    ///    let runtime = LocalRuntime::new().unwrap();
    ///
    ///    runtime.block_on(async move {
    ///        let inner_runtime = LocalRuntime::new().unwrap();
    ///        // ...
    ///        inner_runtime.shutdown_background();
    ///    });
    /// }
    /// ```
    pub fn shutdown_background(self) {
        self.shutdown_timeout(Duration::from_nanos(0));
    }

    /// Returns a view that lets you get information about how the runtime
    /// is performing.
    pub fn metrics(&self) -> crate::runtime::RuntimeMetrics {
        self.handle.metrics()
    }
}

impl Drop for LocalRuntime {
    fn drop(&mut self) {
        if let LocalRuntimeScheduler::CurrentThread(current_thread) = &mut self.scheduler {
            // This ensures that tasks spawned on the current-thread
            // runtime are dropped inside the runtime's context.
            let _guard = context::try_set_current(&self.handle.inner);
            current_thread.shutdown(&self.handle.inner);
        } else {
            unreachable!("LocalRuntime only supports current-thread")
        }
    }
}

impl std::panic::UnwindSafe for LocalRuntime {}

impl std::panic::RefUnwindSafe for LocalRuntime {}
