use super::BOX_FUTURE_THRESHOLD;
use crate::runtime::blocking::BlockingPool;
use crate::runtime::scheduler::CurrentThread;
use crate::runtime::{context, EnterGuard, Handle};
use crate::task::JoinHandle;
use crate::util::trace::SpawnMeta;

use std::future::Future;
use std::mem;
use std::time::Duration;

cfg_rt_multi_thread! {
    use crate::runtime::Builder;
    use crate::runtime::scheduler::MultiThread;
}

/// The Tokio runtime.
///
/// The runtime provides an I/O driver, task scheduler, [timer], and
/// blocking pool, necessary for running asynchronous tasks.
///
/// Instances of `Runtime` can be created using [`new`], or [`Builder`].
/// However, most users will use the [`#[tokio::main]`][main] annotation on
/// their entry point instead.
///
/// See [module level][mod] documentation for more details.
///
/// # Shutdown
///
/// Shutting down the runtime is done by dropping the value, or calling
/// [`shutdown_background`] or [`shutdown_timeout`].
///
/// Tasks spawned through [`Runtime::spawn`] keep running until they yield.
/// Then they are dropped. They are not *guaranteed* to run to completion, but
/// *might* do so if they do not yield until completion.
///
/// Blocking functions spawned through [`Runtime::spawn_blocking`] keep running
/// until they return.
///
/// The thread initiating the shutdown blocks until all spawned work has been
/// stopped. This can take an indefinite amount of time. The `Drop`
/// implementation waits forever for this.
///
/// The [`shutdown_background`] and [`shutdown_timeout`] methods can be used if
/// waiting forever is undesired. When the timeout is reached, spawned work that
/// did not stop in time and threads running it are leaked. The work continues
/// to run until one of the stopping conditions is fulfilled, but the thread
/// initiating the shutdown is unblocked.
///
/// Once the runtime has been dropped, any outstanding I/O resources bound to
/// it will no longer function. Calling any method on them will result in an
/// error.
///
/// # Sharing
///
/// There are several ways to establish shared access to a Tokio runtime:
///
///  * Using an <code>[Arc]\<Runtime></code>.
///  * Using a [`Handle`].
///  * Entering the runtime context.
///
/// Using an <code>[Arc]\<Runtime></code> or [`Handle`] allows you to do various
/// things with the runtime such as spawning new tasks or entering the runtime
/// context. Both types can be cloned to create a new handle that allows access
/// to the same runtime. By passing clones into different tasks or threads, you
/// will be able to access the runtime from those tasks or threads.
///
/// The difference between <code>[Arc]\<Runtime></code> and [`Handle`] is that
/// an <code>[Arc]\<Runtime></code> will prevent the runtime from shutting down,
/// whereas a [`Handle`] does not prevent that. This is because shutdown of the
/// runtime happens when the destructor of the `Runtime` object runs.
///
/// Calls to [`shutdown_background`] and [`shutdown_timeout`] require exclusive
/// ownership of the `Runtime` type. When using an <code>[Arc]\<Runtime></code>,
/// this can be achieved via [`Arc::try_unwrap`] when only one strong count
/// reference is left over.
///
/// The runtime context is entered using the [`Runtime::enter`] or
/// [`Handle::enter`] methods, which use a thread-local variable to store the
/// current runtime. Whenever you are inside the runtime context, methods such
/// as [`tokio::spawn`] will use the runtime whose context you are inside.
///
/// [timer]: crate::time
/// [mod]: index.html
/// [`new`]: method@Self::new
/// [`Builder`]: struct@Builder
/// [`Handle`]: struct@Handle
/// [main]: macro@crate::main
/// [`tokio::spawn`]: crate::spawn
/// [`Arc::try_unwrap`]: std::sync::Arc::try_unwrap
/// [Arc]: std::sync::Arc
/// [`shutdown_background`]: method@Runtime::shutdown_background
/// [`shutdown_timeout`]: method@Runtime::shutdown_timeout
#[derive(Debug)]
pub struct Runtime {
    /// Task scheduler
    scheduler: Scheduler,

    /// Handle to runtime, also contains driver handles
    handle: Handle,

    /// Blocking pool handle, used to signal shutdown
    blocking_pool: BlockingPool,
}

/// The flavor of a `Runtime`.
///
/// This is the return type for [`Handle::runtime_flavor`](crate::runtime::Handle::runtime_flavor()).
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeFlavor {
    /// The flavor that executes all tasks on the current thread.
    CurrentThread,
    /// The flavor that executes tasks across multiple threads.
    MultiThread,
}

/// The runtime scheduler is either a multi-thread or a current-thread executor.
#[derive(Debug)]
pub(super) enum Scheduler {
    /// Execute all tasks on the current-thread.
    CurrentThread(CurrentThread),

    /// Execute tasks across multiple threads.
    #[cfg(feature = "rt-multi-thread")]
    MultiThread(MultiThread),
}

impl Runtime {
    pub(super) fn from_parts(
        scheduler: Scheduler,
        handle: Handle,
        blocking_pool: BlockingPool,
    ) -> Runtime {
        Runtime {
            scheduler,
            handle,
            blocking_pool,
        }
    }

    /// Creates a new runtime instance with default configuration values.
    ///
    /// This results in the multi threaded scheduler, I/O driver, and time driver being
    /// initialized.
    ///
    /// Most applications will not need to call this function directly. Instead,
    /// they will use the  [`#[tokio::main]` attribute][main]. When a more complex
    /// configuration is necessary, the [runtime builder] may be used.
    ///
    /// See [module level][mod] documentation for more details.
    ///
    /// # Examples
    ///
    /// Creating a new `Runtime` with default configuration values.
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// let rt = Runtime::new()
    ///     .unwrap();
    ///
    /// // Use the runtime...
    /// ```
    ///
    /// [mod]: index.html
    /// [main]: ../attr.main.html
    /// [threaded scheduler]: index.html#threaded-scheduler
    /// [runtime builder]: crate::runtime::Builder
    #[cfg(feature = "rt-multi-thread")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rt-multi-thread")))]
    pub fn new() -> std::io::Result<Runtime> {
        Builder::new_multi_thread().enable_all().build()
    }

    /// Returns a handle to the runtime's spawner.
    ///
    /// The returned handle can be used to spawn tasks that run on this runtime, and can
    /// be cloned to allow moving the `Handle` to other threads.
    ///
    /// Calling [`Handle::block_on`] on a handle to a `current_thread` runtime is error-prone.
    /// Refer to the documentation of [`Handle::block_on`] for more.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// let rt = Runtime::new()
    ///     .unwrap();
    ///
    /// let handle = rt.handle();
    ///
    /// // Use the handle...
    /// ```
    pub fn handle(&self) -> &Handle {
        &self.handle
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
    ///
    /// // Spawn a future onto the runtime
    /// rt.spawn(async {
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
            self.handle
                .spawn_named(Box::pin(future), SpawnMeta::new_unnamed(fut_size))
        } else {
            self.handle
                .spawn_named(future, SpawnMeta::new_unnamed(fut_size))
        }
    }

    /// Runs the provided function on an executor dedicated to blocking operations.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// # fn dox() {
    /// // Create the runtime
    /// let rt = Runtime::new().unwrap();
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
    /// This runs the given future on the current thread, blocking until it is
    /// complete, and yielding its resolved result. Any tasks or timers
    /// which the future spawns internally will be executed on the runtime.
    ///
    /// # Non-worker future
    ///
    /// Note that the future required by this function does not run as a
    /// worker. The expectation is that other tasks are spawned by the future here.
    /// Awaiting on other futures from the future provided here will not
    /// perform as fast as those spawned as workers.
    ///
    /// # Multi thread scheduler
    ///
    /// When the multi thread scheduler is used this will allow futures
    /// to run within the io driver and timer context of the overall runtime.
    ///
    /// Any spawned tasks will continue running after `block_on` returns.
    ///
    /// # Current thread scheduler
    ///
    /// When the current thread scheduler is enabled `block_on`
    /// can be called concurrently from multiple threads. The first call
    /// will take ownership of the io and timer drivers. This means
    /// other threads which do not own the drivers will hook into that one.
    /// When the first `block_on` completes, other threads will be able to
    /// "steal" the driver to allow continued execution of their futures.
    ///
    /// Any spawned tasks will be suspended after `block_on` returns. Calling
    /// `block_on` again will resume previously spawned tasks.
    ///
    /// # Panics
    ///
    /// This function panics if the provided future panics, or if called within an
    /// asynchronous execution context.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio::runtime::Runtime;
    ///
    /// // Create the runtime
    /// let rt  = Runtime::new().unwrap();
    ///
    /// // Execute the future, blocking the current thread until completion
    /// rt.block_on(async {
    ///     println!("hello");
    /// });
    /// ```
    ///
    /// [handle]: fn@Handle::block_on
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
        let future = crate::util::trace::task(
            future,
            "block_on",
            _meta,
            crate::runtime::task::Id::next().as_u64(),
        );

        let _enter = self.enter();

        match &self.scheduler {
            Scheduler::CurrentThread(exec) => exec.block_on(&self.handle.inner, future),
            #[cfg(feature = "rt-multi-thread")]
            Scheduler::MultiThread(exec) => exec.block_on(&self.handle.inner, future),
        }
    }

    /// Enters the runtime context.
    ///
    /// This allows you to construct types that must have an executor
    /// available on creation such as [`Sleep`] or [`TcpStream`]. It will
    /// also allow you to call methods such as [`tokio::spawn`].
    ///
    /// [`Sleep`]: struct@crate::time::Sleep
    /// [`TcpStream`]: struct@crate::net::TcpStream
    /// [`tokio::spawn`]: fn@crate::spawn
    ///
    /// # Example
    ///
    /// ```
    /// use tokio::runtime::Runtime;
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
    ///     let rt = Runtime::new().unwrap();
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
    /// See the [struct level documentation](Runtime#shutdown) for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    /// use tokio::task;
    ///
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// fn main() {
    /// #  if cfg!(miri) { return } // Miri reports error when main thread terminated without waiting all remaining threads.
    ///    let runtime = Runtime::new().unwrap();
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
    /// return.
    ///
    /// See the [struct level documentation](Runtime#shutdown) for more details.
    ///
    /// This function is equivalent to calling `shutdown_timeout(Duration::from_nanos(0))`.
    ///
    /// ```
    /// use tokio::runtime::Runtime;
    ///
    /// fn main() {
    ///    let runtime = Runtime::new().unwrap();
    ///
    ///    runtime.block_on(async move {
    ///        let inner_runtime = Runtime::new().unwrap();
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

impl Drop for Runtime {
    fn drop(&mut self) {
        match &mut self.scheduler {
            Scheduler::CurrentThread(current_thread) => {
                // This ensures that tasks spawned on the current-thread
                // runtime are dropped inside the runtime's context.
                let _guard = context::try_set_current(&self.handle.inner);
                current_thread.shutdown(&self.handle.inner);
            }
            #[cfg(feature = "rt-multi-thread")]
            Scheduler::MultiThread(multi_thread) => {
                // The threaded scheduler drops its tasks on its worker threads, which is
                // already in the runtime's context.
                multi_thread.shutdown(&self.handle.inner);
            }
        }
    }
}

impl std::panic::UnwindSafe for Runtime {}

impl std::panic::RefUnwindSafe for Runtime {}
