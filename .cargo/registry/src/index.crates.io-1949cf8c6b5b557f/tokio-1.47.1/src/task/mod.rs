//! Asynchronous green-threads.
//!
//! ## What are Tasks?
//!
//! A _task_ is a light weight, non-blocking unit of execution. A task is similar
//! to an OS thread, but rather than being managed by the OS scheduler, they are
//! managed by the [Tokio runtime][rt]. Another name for this general pattern is
//! [green threads]. If you are familiar with [Go's goroutines], [Kotlin's
//! coroutines], or [Erlang's processes], you can think of Tokio's tasks as
//! something similar.
//!
//! Key points about tasks include:
//!
//! * Tasks are **light weight**. Because tasks are scheduled by the Tokio
//!   runtime rather than the operating system, creating new tasks or switching
//!   between tasks does not require a context switch and has fairly low
//!   overhead. Creating, running, and destroying large numbers of tasks is
//!   quite cheap, especially compared to OS threads.
//!
//! * Tasks are scheduled **cooperatively**. Most operating systems implement
//!   _preemptive multitasking_. This is a scheduling technique where the
//!   operating system allows each thread to run for a period of time, and then
//!   _preempts_ it, temporarily pausing that thread and switching to another.
//!   Tasks, on the other hand, implement _cooperative multitasking_. In
//!   cooperative multitasking, a task is allowed to run until it _yields_,
//!   indicating to the Tokio runtime's scheduler that it cannot currently
//!   continue executing. When a task yields, the Tokio runtime switches to
//!   executing the next task.
//!
//! * Tasks are **non-blocking**. Typically, when an OS thread performs I/O or
//!   must synchronize with another thread, it _blocks_, allowing the OS to
//!   schedule another thread. When a task cannot continue executing, it must
//!   yield instead, allowing the Tokio runtime to schedule another task. Tasks
//!   should generally not perform system calls or other operations that could
//!   block a thread, as this would prevent other tasks running on the same
//!   thread from executing as well. Instead, this module provides APIs for
//!   running blocking operations in an asynchronous context.
//!
//! [rt]: crate::runtime
//! [green threads]: https://en.wikipedia.org/wiki/Green_threads
//! [Go's goroutines]: https://tour.golang.org/concurrency/1
//! [Kotlin's coroutines]: https://kotlinlang.org/docs/reference/coroutines-overview.html
//! [Erlang's processes]: http://erlang.org/doc/getting_started/conc_prog.html#processes
//!
//! ## Working with Tasks
//!
//! This module provides the following APIs for working with tasks:
//!
//! ### Spawning
//!
//! Perhaps the most important function in this module is [`task::spawn`]. This
//! function can be thought of as an async equivalent to the standard library's
//! [`thread::spawn`][`std::thread::spawn`]. It takes an `async` block or other
//! [future], and creates a new task to run that work concurrently:
//!
//! ```
//! use tokio::task;
//!
//! # async fn doc() {
//! task::spawn(async {
//!     // perform some work here...
//! });
//! # }
//! ```
//!
//! Like [`std::thread::spawn`], `task::spawn` returns a [`JoinHandle`] struct.
//! A `JoinHandle` is itself a future which may be used to await the output of
//! the spawned task. For example:
//!
//! ```
//! use tokio::task;
//!
//! # #[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let join = task::spawn(async {
//!     // ...
//!     "hello world!"
//! });
//!
//! // ...
//!
//! // Await the result of the spawned task.
//! let result = join.await?;
//! assert_eq!(result, "hello world!");
//! # Ok(())
//! # }
//! ```
//!
//! Again, like `std::thread`'s [`JoinHandle` type][thread_join], if the spawned
//! task panics, awaiting its `JoinHandle` will return a [`JoinError`]. For
//! example:
//!
//! ```
//! use tokio::task;
//!
//! # #[tokio::main] async fn main() {
//! let join = task::spawn(async {
//!     panic!("something bad happened!")
//! });
//!
//! // The returned result indicates that the task failed.
//! assert!(join.await.is_err());
//! # }
//! ```
//!
//! `spawn`, `JoinHandle`, and `JoinError` are present when the "rt"
//! feature flag is enabled.
//!
//! [`task::spawn`]: crate::task::spawn()
//! [future]: std::future::Future
//! [`std::thread::spawn`]: std::thread::spawn
//! [`JoinHandle`]: crate::task::JoinHandle
//! [thread_join]: std::thread::JoinHandle
//! [`JoinError`]: crate::task::JoinError
//!
//! #### Cancellation
//!
//! Spawned tasks may be cancelled using the [`JoinHandle::abort`] or
//! [`AbortHandle::abort`] methods. When one of these methods are called, the
//! task is signalled to shut down next time it yields at an `.await` point. If
//! the task is already idle, then it will be shut down as soon as possible
//! without running again before being shut down. Additionally, shutting down a
//! Tokio runtime (e.g. by returning from `#[tokio::main]`) immediately cancels
//! all tasks on it.
//!
//! When tasks are shut down, it will stop running at whichever `.await` it has
//! yielded at. All local variables are destroyed by running their destructor.
//! Once shutdown has completed, awaiting the [`JoinHandle`] will fail with a
//! [cancelled error](crate::task::JoinError::is_cancelled).
//!
//! Note that aborting a task does not guarantee that it fails with a cancelled
//! error, since it may complete normally first. For example, if the task does
//! not yield to the runtime at any point between the call to `abort` and the
//! end of the task, then the [`JoinHandle`] will instead report that the task
//! exited normally.
//!
//! Be aware that tasks spawned using [`spawn_blocking`] cannot be aborted
//! because they are not async. If you call `abort` on a `spawn_blocking`
//! task, then this *will not have any effect*, and the task will continue
//! running normally. The exception is if the task has not started running
//! yet; in that case, calling `abort` may prevent the task from starting.
//!
//! Be aware that calls to [`JoinHandle::abort`] just schedule the task for
//! cancellation, and will return before the cancellation has completed. To wait
//! for cancellation to complete, wait for the task to finish by awaiting the
//! [`JoinHandle`]. Similarly, the [`JoinHandle::is_finished`] method does not
//! return `true` until the cancellation has finished.
//!
//! Calling [`JoinHandle::abort`] multiple times has the same effect as calling
//! it once.
//!
//! Tokio also provides an [`AbortHandle`], which is like the [`JoinHandle`],
//! except that it does not provide a mechanism to wait for the task to finish.
//! Each task can only have one [`JoinHandle`], but it can have more than one
//! [`AbortHandle`].
//!
//! [`JoinHandle::abort`]: crate::task::JoinHandle::abort
//! [`AbortHandle::abort`]: crate::task::AbortHandle::abort
//! [`AbortHandle`]: crate::task::AbortHandle
//! [`JoinHandle::is_finished`]: crate::task::JoinHandle::is_finished
//!
//! ### Blocking and Yielding
//!
//! As we discussed above, code running in asynchronous tasks should not perform
//! operations that can block. A blocking operation performed in a task running
//! on a thread that is also running other tasks would block the entire thread,
//! preventing other tasks from running.
//!
//! Instead, Tokio provides two APIs for running blocking operations in an
//! asynchronous context: [`task::spawn_blocking`] and [`task::block_in_place`].
//!
//! Be aware that if you call a non-async method from async code, that non-async
//! method is still inside the asynchronous context, so you should also avoid
//! blocking operations there. This includes destructors of objects destroyed in
//! async code.
//!
//! #### `spawn_blocking`
//!
//! The `task::spawn_blocking` function is similar to the `task::spawn` function
//! discussed in the previous section, but rather than spawning an
//! _non-blocking_ future on the Tokio runtime, it instead spawns a
//! _blocking_ function on a dedicated thread pool for blocking tasks. For
//! example:
//!
//! ```
//! use tokio::task;
//!
//! # async fn docs() {
//! task::spawn_blocking(|| {
//!     // do some compute-heavy work or call synchronous code
//! });
//! # }
//! ```
//!
//! Just like `task::spawn`, `task::spawn_blocking` returns a `JoinHandle`
//! which we can use to await the result of the blocking operation:
//!
//! ```rust
//! # use tokio::task;
//! # async fn docs() -> Result<(), Box<dyn std::error::Error>>{
//! let join = task::spawn_blocking(|| {
//!     // do some compute-heavy work or call synchronous code
//!     "blocking completed"
//! });
//!
//! let result = join.await?;
//! assert_eq!(result, "blocking completed");
//! # Ok(())
//! # }
//! ```
//!
//! #### `block_in_place`
//!
//! When using the [multi-threaded runtime][rt-multi-thread], the [`task::block_in_place`]
//! function is also available. Like `task::spawn_blocking`, this function
//! allows running a blocking operation from an asynchronous context. Unlike
//! `spawn_blocking`, however, `block_in_place` works by transitioning the
//! _current_ worker thread to a blocking thread, moving other tasks running on
//! that thread to another worker thread. This can improve performance by avoiding
//! context switches.
//!
//! For example:
//!
//! ```
//! use tokio::task;
//!
//! # async fn docs() {
//! let result = task::block_in_place(|| {
//!     // do some compute-heavy work or call synchronous code
//!     "blocking completed"
//! });
//!
//! assert_eq!(result, "blocking completed");
//! # }
//! ```
//!
//! #### `yield_now`
//!
//! In addition, this module provides a [`task::yield_now`] async function
//! that is analogous to the standard library's [`thread::yield_now`]. Calling
//! and `await`ing this function will cause the current task to yield to the
//! Tokio runtime's scheduler, allowing other tasks to be
//! scheduled. Eventually, the yielding task will be polled again, allowing it
//! to execute. For example:
//!
//! ```rust
//! use tokio::task;
//!
//! # #[tokio::main] async fn main() {
//! async {
//!     task::spawn(async {
//!         // ...
//!         println!("spawned task done!")
//!     });
//!
//!     // Yield, allowing the newly-spawned task to execute first.
//!     task::yield_now().await;
//!     println!("main task done!");
//! }
//! # .await;
//! # }
//! ```
//!
//! [`task::spawn_blocking`]: crate::task::spawn_blocking
//! [`task::block_in_place`]: crate::task::block_in_place
//! [rt-multi-thread]: ../runtime/index.html#threaded-scheduler
//! [`task::yield_now`]: crate::task::yield_now()
//! [`thread::yield_now`]: std::thread::yield_now

cfg_rt! {
    pub use crate::runtime::task::{JoinError, JoinHandle};

    mod blocking;
    pub use blocking::spawn_blocking;

    mod spawn;
    pub use spawn::spawn;

    cfg_rt_multi_thread! {
        pub use blocking::block_in_place;
    }

    mod yield_now;
    pub use yield_now::yield_now;

    pub mod coop;
    #[doc(hidden)]
    #[deprecated = "Moved to tokio::task::coop::consume_budget"]
    pub use coop::consume_budget;
    #[doc(hidden)]
    #[deprecated = "Moved to tokio::task::coop::unconstrained"]
    pub use coop::unconstrained;
    #[doc(hidden)]
    #[deprecated = "Moved to tokio::task::coop::Unconstrained"]
    pub use coop::Unconstrained;

    mod local;
    pub use local::{spawn_local, LocalSet, LocalEnterGuard};

    mod task_local;
    pub use task_local::LocalKey;

    #[doc(inline)]
    pub use join_set::JoinSet;
    pub use crate::runtime::task::AbortHandle;

    // Uses #[cfg(...)] instead of macro since the macro adds docsrs annotations.
    #[cfg(not(tokio_unstable))]
    mod join_set;
    #[cfg(tokio_unstable)]
    pub mod join_set;

    pub use crate::runtime::task::{Id, id, try_id};

    cfg_trace! {
        mod builder;
        pub use builder::Builder;
    }

    /// Task-related futures.
    pub mod futures {
        pub use super::task_local::TaskLocalFuture;
    }
}

cfg_not_rt! {
    pub(crate) mod coop;
}
