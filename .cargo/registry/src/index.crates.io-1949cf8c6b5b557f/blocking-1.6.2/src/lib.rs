//! A thread pool for isolating blocking I/O in async programs.
//!
//! Sometimes there's no way to avoid blocking I/O. Consider files or stdin, which have weak async
//! support on modern operating systems. While [IOCP], [AIO], and [io_uring] are possible
//! solutions, they're not always available or ideal.
//!
//! Since blocking is not allowed inside futures, we must move blocking I/O onto a special thread
//! pool provided by this crate. The pool dynamically spawns and stops threads depending on the
//! current number of running I/O jobs.
//!
//! Note that there is a limit on the number of active threads. Once that limit is hit, a running
//! job has to finish before others get a chance to run. When a thread is idle, it waits for the
//! next job or shuts down after a certain timeout.
//!
//! The default number of threads (set to 500) can be altered by setting BLOCKING_MAX_THREADS environment
//! variable with value between 1 and 10000.
//!
//! [IOCP]: https://en.wikipedia.org/wiki/Input/output_completion_port
//! [AIO]: http://man7.org/linux/man-pages/man2/io_submit.2.html
//! [io_uring]: https://lwn.net/Articles/776703
//!
//! # Examples
//!
//! Read the contents of a file:
//!
//! ```no_run
//! use blocking::unblock;
//! use std::fs;
//!
//! # futures_lite::future::block_on(async {
//! let contents = unblock(|| fs::read_to_string("file.txt")).await?;
//! println!("{}", contents);
//! # std::io::Result::Ok(()) });
//! ```
//!
//! Read a file and pipe its contents to stdout:
//!
//! ```no_run
//! use blocking::{unblock, Unblock};
//! use futures_lite::io;
//! use std::fs::File;
//!
//! # futures_lite::future::block_on(async {
//! let input = unblock(|| File::open("file.txt")).await?;
//! let input = Unblock::new(input);
//! let mut output = Unblock::new(std::io::stdout());
//!
//! io::copy(input, &mut output).await?;
//! # std::io::Result::Ok(()) });
//! ```
//!
//! Iterate over the contents of a directory:
//!
//! ```no_run
//! use blocking::Unblock;
//! use futures_lite::prelude::*;
//! use std::fs;
//!
//! # futures_lite::future::block_on(async {
//! let mut dir = Unblock::new(fs::read_dir(".")?);
//! while let Some(item) = dir.next().await {
//!     println!("{}", item?.file_name().to_string_lossy());
//! }
//! # std::io::Result::Ok(()) });
//! ```
//!
//! Spawn a process:
//!
//! ```no_run
//! use blocking::unblock;
//! use std::process::Command;
//!
//! # futures_lite::future::block_on(async {
//! let out = unblock(|| Command::new("dir").output()).await?;
//! # std::io::Result::Ok(()) });
//! ```

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![forbid(unsafe_code)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]

use std::any::Any;
use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;
use std::panic;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;

#[cfg(not(target_family = "wasm"))]
use std::env;

use async_channel::{bounded, Receiver};
use async_task::Runnable;
use futures_io::{AsyncRead, AsyncSeek, AsyncWrite};
use futures_lite::{
    future::{self, Future},
    ready,
    stream::Stream,
};
use piper::{pipe, Reader, Writer};

#[doc(no_inline)]
pub use async_task::Task;

/// Default value for max threads that Executor can grow to
#[cfg(not(target_family = "wasm"))]
const DEFAULT_MAX_THREADS: NonZeroUsize = {
    if let Some(size) = NonZeroUsize::new(500) {
        size
    } else {
        panic!("DEFAULT_MAX_THREADS is non-zero");
    }
};

/// Minimum value for max threads config
#[cfg(not(target_family = "wasm"))]
const MIN_MAX_THREADS: usize = 1;

/// Maximum value for max threads config
#[cfg(not(target_family = "wasm"))]
const MAX_MAX_THREADS: usize = 10000;

/// Env variable that allows to override default value for max threads.
#[cfg(not(target_family = "wasm"))]
const MAX_THREADS_ENV: &str = "BLOCKING_MAX_THREADS";

/// The blocking executor.
struct Executor {
    /// Inner state of the executor.
    inner: Mutex<Inner>,

    /// Used to put idle threads to sleep and wake them up when new work comes in.
    cvar: Condvar,
}

/// Inner state of the blocking executor.
struct Inner {
    /// Number of idle threads in the pool.
    ///
    /// Idle threads are sleeping, waiting to get a task to run.
    idle_count: usize,

    /// Total number of threads in the pool.
    ///
    /// This is the number of idle threads + the number of active threads.
    thread_count: usize,

    // TODO: The option is only used for const-initialization. This can be replaced with
    // a normal VecDeque when the MSRV can be bumped passed
    /// The queue of blocking tasks.
    queue: Option<VecDeque<Runnable>>,

    /// Maximum number of threads in the pool
    thread_limit: Option<NonZeroUsize>,
}

impl Inner {
    #[inline]
    fn queue(&mut self) -> &mut VecDeque<Runnable> {
        self.queue.get_or_insert_with(VecDeque::new)
    }
}

impl Executor {
    #[cfg(not(target_family = "wasm"))]
    fn max_threads() -> NonZeroUsize {
        match env::var(MAX_THREADS_ENV) {
            Ok(v) => v
                .parse::<usize>()
                .ok()
                .and_then(|v| NonZeroUsize::new(v.clamp(MIN_MAX_THREADS, MAX_MAX_THREADS)))
                .unwrap_or(DEFAULT_MAX_THREADS),
            Err(_) => DEFAULT_MAX_THREADS,
        }
    }

    #[cfg(target_family = "wasm")]
    fn max_threads() -> NonZeroUsize {
        NonZeroUsize::new(1).unwrap()
    }

    /// Get a reference to the global executor.
    #[inline]
    fn get() -> &'static Self {
        #[cfg(not(target_family = "wasm"))]
        {
            static EXECUTOR: Executor = Executor {
                inner: Mutex::new(Inner {
                    idle_count: 0,
                    thread_count: 0,
                    queue: None,
                    thread_limit: None,
                }),
                cvar: Condvar::new(),
            };

            &EXECUTOR
        }

        #[cfg(target_family = "wasm")]
        panic!("cannot spawn a blocking task on WASM")
    }

    /// Spawns a future onto this executor.
    ///
    /// Returns a [`Task`] handle for the spawned task.
    fn spawn<T: Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> Task<T> {
        let (runnable, task) = async_task::Builder::new().propagate_panic(true).spawn(
            move |()| future,
            |r| {
                // Initialize the executor if we haven't already.
                let executor = Self::get();

                // Schedule the task on our executor.
                executor.schedule(r)
            },
        );
        runnable.schedule();
        task
    }

    /// Runs the main loop on the current thread.
    ///
    /// This function runs blocking tasks until it becomes idle and times out.
    fn main_loop(&'static self) {
        #[cfg(feature = "tracing")]
        let _span = tracing::trace_span!("blocking::main_loop").entered();

        let mut inner = self.inner.lock().unwrap();
        loop {
            // This thread is not idle anymore because it's going to run tasks.
            inner.idle_count -= 1;

            // Run tasks in the queue.
            while let Some(runnable) = inner.queue().pop_front() {
                // We have found a task - grow the pool if needed.
                self.grow_pool(inner);

                // Run the task.
                panic::catch_unwind(|| runnable.run()).ok();

                // Re-lock the inner state and continue.
                inner = self.inner.lock().unwrap();
            }

            // This thread is now becoming idle.
            inner.idle_count += 1;

            // Put the thread to sleep until another task is scheduled.
            let timeout = Duration::from_millis(500);
            #[cfg(feature = "tracing")]
            tracing::trace!(?timeout, "going to sleep");
            let (lock, res) = self.cvar.wait_timeout(inner, timeout).unwrap();
            inner = lock;

            // If there are no tasks after a while, stop this thread.
            if res.timed_out() && inner.queue().is_empty() {
                inner.idle_count -= 1;
                inner.thread_count -= 1;
                break;
            }

            #[cfg(feature = "tracing")]
            tracing::trace!("notified");
        }

        #[cfg(feature = "tracing")]
        tracing::trace!("shutting down due to lack of tasks");
    }

    /// Schedules a runnable task for execution.
    fn schedule(&'static self, runnable: Runnable) {
        let mut inner = self.inner.lock().unwrap();
        inner.queue().push_back(runnable);

        // Notify a sleeping thread and spawn more threads if needed.
        self.cvar.notify_one();
        self.grow_pool(inner);
    }

    /// Spawns more blocking threads if the pool is overloaded with work.
    fn grow_pool(&'static self, mut inner: MutexGuard<'static, Inner>) {
        #[cfg(feature = "tracing")]
        let _span = tracing::trace_span!(
            "grow_pool",
            queue_len = inner.queue().len(),
            idle_count = inner.idle_count,
            thread_count = inner.thread_count,
        )
        .entered();

        let thread_limit = inner
            .thread_limit
            .get_or_insert_with(Self::max_threads)
            .get();

        // If runnable tasks greatly outnumber idle threads and there aren't too many threads
        // already, then be aggressive: wake all idle threads and spawn one more thread.
        while inner.queue().len() > inner.idle_count * 5 && inner.thread_count < thread_limit {
            #[cfg(feature = "tracing")]
            tracing::trace!("spawning a new thread to handle blocking tasks");

            // The new thread starts in idle state.
            inner.idle_count += 1;
            inner.thread_count += 1;

            // Notify all existing idle threads because we need to hurry up.
            self.cvar.notify_all();

            // Generate a new thread ID.
            static ID: AtomicUsize = AtomicUsize::new(1);
            let id = ID.fetch_add(1, Ordering::Relaxed);

            // Spawn the new thread.
            if let Err(_e) = thread::Builder::new()
                .name(format!("blocking-{id}"))
                .spawn(move || self.main_loop())
            {
                // We were unable to spawn the thread, so we need to undo the state changes.
                #[cfg(feature = "tracing")]
                tracing::error!("failed to spawn a blocking thread: {}", _e);
                inner.idle_count -= 1;
                inner.thread_count -= 1;

                // The current number of threads is likely to be the system's upper limit, so update
                // thread_limit accordingly.
                inner.thread_limit = {
                    let new_limit = inner.thread_count;

                    // If the limit is about to be set to zero, set it to one instead so that if,
                    // in the future, we are able to spawn more threads, we will be able to do so.
                    Some(NonZeroUsize::new(new_limit).unwrap_or_else(|| {
                        #[cfg(feature = "tracing")]
                        tracing::warn!(
                            "attempted to lower thread_limit to zero; setting to one instead"
                        );
                        NonZeroUsize::new(1).unwrap()
                    }))
                };
            }
        }
    }
}

/// Runs blocking code on a thread pool.
///
/// # Examples
///
/// Read the contents of a file:
///
/// ```no_run
/// use blocking::unblock;
/// use std::fs;
///
/// # futures_lite::future::block_on(async {
/// let contents = unblock(|| fs::read_to_string("file.txt")).await?;
/// # std::io::Result::Ok(()) });
/// ```
///
/// Spawn a process:
///
/// ```no_run
/// use blocking::unblock;
/// use std::process::Command;
///
/// # futures_lite::future::block_on(async {
/// let out = unblock(|| Command::new("dir").output()).await?;
/// # std::io::Result::Ok(()) });
/// ```
pub fn unblock<T, F>(f: F) -> Task<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    Executor::spawn(async move { f() })
}

/// Runs blocking I/O on a thread pool.
///
/// Blocking I/O must be isolated from async code. This type moves blocking I/O operations onto a
/// special thread pool while exposing a familiar async interface.
///
/// This type implements traits [`Stream`], [`AsyncRead`], [`AsyncWrite`], or [`AsyncSeek`] if the
/// inner type implements [`Iterator`], [`Read`], [`Write`], or [`Seek`], respectively.
///
/// # Caveats
///
/// [`Unblock`] is a low-level primitive, and as such it comes with some caveats.
///
/// For higher-level primitives built on top of [`Unblock`], look into [`async-fs`] or
/// [`async-process`] (on Windows).
///
/// [`async-fs`]: https://github.com/smol-rs/async-fs
/// [`async-process`]: https://github.com/smol-rs/async-process
///
/// [`Unblock`] communicates with I/O operations on the thread pool through a pipe. That means an
/// async read/write operation simply receives/sends some bytes from/into the pipe. When in reading
/// mode, the thread pool reads bytes from the I/O handle and forwards them into the pipe until it
/// becomes full. When in writing mode, the thread pool reads bytes from the pipe and forwards them
/// into the I/O handle.
///
/// Use [`Unblock::with_capacity()`] to configure the capacity of the pipe.
///
/// ### Reading
///
/// If you create an [`Unblock`]`<`[`Stdin`][`std::io::Stdin`]`>`, read some bytes from it,
/// and then drop it, a blocked read operation may keep hanging on the thread pool. The next
/// attempt to read from stdin will lose bytes read by the hanging operation. This is a difficult
/// problem to solve, so make sure you only use a single stdin handle for the duration of the
/// entire program.
///
/// ### Writing
///
/// If writing data through the [`AsyncWrite`] trait, make sure to flush before dropping the
/// [`Unblock`] handle or some buffered data might get lost.
///
/// ### Seeking
///
/// Because of buffering in the pipe, if [`Unblock`] wraps a [`File`][`std::fs::File`], a single
/// read operation may move the file cursor farther than is the span of the operation. In fact,
/// reading just keeps going in the background until the pipe gets full. Keep this mind when
/// using [`AsyncSeek`] with [relative][`SeekFrom::Current`] offsets.
///
/// # Examples
///
/// ```
/// use blocking::Unblock;
/// use futures_lite::prelude::*;
///
/// # futures_lite::future::block_on(async {
/// let mut stdout = Unblock::new(std::io::stdout());
/// stdout.write_all(b"Hello world!").await?;
/// stdout.flush().await?;
/// # std::io::Result::Ok(()) });
/// ```
pub struct Unblock<T> {
    state: State<T>,
    cap: Option<usize>,
}

impl<T> Unblock<T> {
    /// Wraps a blocking I/O handle into the async [`Unblock`] interface.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blocking::Unblock;
    ///
    /// let stdin = Unblock::new(std::io::stdin());
    /// ```
    pub fn new(io: T) -> Unblock<T> {
        Unblock {
            state: State::Idle(Some(Box::new(io))),
            cap: None,
        }
    }

    /// Wraps a blocking I/O handle into the async [`Unblock`] interface with a custom buffer
    /// capacity.
    ///
    /// When communicating with the inner [`Stream`]/[`Read`]/[`Write`] type from async code, data
    /// transferred between blocking and async code goes through a buffer of limited capacity. This
    /// constructor configures that capacity.
    ///
    /// The default capacity is:
    ///
    /// * For [`Iterator`] types: 8192 items.
    /// * For [`Read`]/[`Write`] types: 8 MB.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blocking::Unblock;
    ///
    /// let stdout = Unblock::with_capacity(64 * 1024, std::io::stdout());
    /// ```
    pub fn with_capacity(cap: usize, io: T) -> Unblock<T> {
        Unblock {
            state: State::Idle(Some(Box::new(io))),
            cap: Some(cap),
        }
    }

    /// Gets a mutable reference to the blocking I/O handle.
    ///
    /// This is an async method because the I/O handle might be on the thread pool and needs to
    /// be moved onto the current thread before we can get a reference to it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blocking::{unblock, Unblock};
    /// use std::fs::File;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = unblock(|| File::create("file.txt")).await?;
    /// let mut file = Unblock::new(file);
    ///
    /// let metadata = file.get_mut().await.metadata()?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn get_mut(&mut self) -> &mut T {
        // Wait for the running task to stop and ignore I/O errors if there are any.
        future::poll_fn(|cx| self.poll_stop(cx)).await.ok();

        // Assume idle state and get a reference to the inner value.
        match &mut self.state {
            State::Idle(t) => t.as_mut().expect("inner value was taken out"),
            State::WithMut(..)
            | State::Streaming(..)
            | State::Reading(..)
            | State::Writing(..)
            | State::Seeking(..) => {
                unreachable!("when stopped, the state machine must be in idle state");
            }
        }
    }

    /// Performs a blocking operation on the I/O handle.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blocking::{unblock, Unblock};
    /// use std::fs::File;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = unblock(|| File::create("file.txt")).await?;
    /// let mut file = Unblock::new(file);
    ///
    /// let metadata = file.with_mut(|f| f.metadata()).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn with_mut<R, F>(&mut self, op: F) -> R
    where
        F: FnOnce(&mut T) -> R + Send + 'static,
        R: Send + 'static,
        T: Send + 'static,
    {
        // Wait for the running task to stop and ignore I/O errors if there are any.
        future::poll_fn(|cx| self.poll_stop(cx)).await.ok();

        // Assume idle state and take out the inner value.
        let mut t = match &mut self.state {
            State::Idle(t) => t.take().expect("inner value was taken out"),
            State::WithMut(..)
            | State::Streaming(..)
            | State::Reading(..)
            | State::Writing(..)
            | State::Seeking(..) => {
                unreachable!("when stopped, the state machine must be in idle state");
            }
        };

        let (sender, receiver) = bounded(1);
        let task = Executor::spawn(async move {
            sender.try_send(op(&mut t)).ok();
            t
        });
        self.state = State::WithMut(task);

        receiver
            .recv()
            .await
            .expect("`Unblock::with_mut()` operation has panicked")
    }

    /// Extracts the inner blocking I/O handle.
    ///
    /// This is an async method because the I/O handle might be on the thread pool and needs to
    /// be moved onto the current thread before we can extract it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use blocking::{unblock, Unblock};
    /// use futures_lite::prelude::*;
    /// use std::fs::File;
    ///
    /// # futures_lite::future::block_on(async {
    /// let file = unblock(|| File::create("file.txt")).await?;
    /// let file = Unblock::new(file);
    ///
    /// let file = file.into_inner().await;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn into_inner(self) -> T {
        // There's a bug in rustdoc causing it to render `mut self` as `__arg0: Self`, so we just
        // bind `self` to a local mutable variable.
        let mut this = self;

        // Wait for the running task to stop and ignore I/O errors if there are any.
        future::poll_fn(|cx| this.poll_stop(cx)).await.ok();

        // Assume idle state and extract the inner value.
        match &mut this.state {
            State::Idle(t) => *t.take().expect("inner value was taken out"),
            State::WithMut(..)
            | State::Streaming(..)
            | State::Reading(..)
            | State::Writing(..)
            | State::Seeking(..) => {
                unreachable!("when stopped, the state machine must be in idle state");
            }
        }
    }

    /// Waits for the running task to stop.
    ///
    /// On success, the state machine is moved into the idle state.
    fn poll_stop(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match &mut self.state {
                State::Idle(_) => return Poll::Ready(Ok(())),

                State::WithMut(task) => {
                    // Poll the task to wait for it to finish.
                    let io = ready!(Pin::new(task).poll(cx));
                    self.state = State::Idle(Some(io));
                }

                State::Streaming(any, task) => {
                    // Drop the receiver to close the channel. This stops the `send()` operation in
                    // the task, after which the task returns the iterator back.
                    any.take();

                    // Poll the task to retrieve the iterator.
                    let iter = ready!(Pin::new(task).poll(cx));
                    self.state = State::Idle(Some(iter));
                }

                State::Reading(reader, task) => {
                    // Drop the reader to close the pipe. This stops copying inside the task, after
                    // which the task returns the I/O handle back.
                    reader.take();

                    // Poll the task to retrieve the I/O handle.
                    let (res, io) = ready!(Pin::new(task).poll(cx));
                    // Make sure to move into the idle state before reporting errors.
                    self.state = State::Idle(Some(io));
                    res?;
                }

                State::Writing(writer, task) => {
                    // Drop the writer to close the pipe. This stops copying inside the task, after
                    // which the task flushes the I/O handle and
                    writer.take();

                    // Poll the task to retrieve the I/O handle.
                    let (res, io) = ready!(Pin::new(task).poll(cx));
                    // Make sure to move into the idle state before reporting errors.
                    self.state = State::Idle(Some(io));
                    res?;
                }

                State::Seeking(task) => {
                    // Poll the task to wait for it to finish.
                    let (_, res, io) = ready!(Pin::new(task).poll(cx));
                    // Make sure to move into the idle state before reporting errors.
                    self.state = State::Idle(Some(io));
                    res?;
                }
            }
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Unblock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Closed;
        impl fmt::Debug for Closed {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("<closed>")
            }
        }

        struct Blocked;
        impl fmt::Debug for Blocked {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("<blocked>")
            }
        }

        match &self.state {
            State::Idle(None) => f.debug_struct("Unblock").field("io", &Closed).finish(),
            State::Idle(Some(io)) => {
                let io: &T = io;
                f.debug_struct("Unblock").field("io", io).finish()
            }
            State::WithMut(..)
            | State::Streaming(..)
            | State::Reading(..)
            | State::Writing(..)
            | State::Seeking(..) => f.debug_struct("Unblock").field("io", &Blocked).finish(),
        }
    }
}

/// Current state of a blocking task.
enum State<T> {
    /// There is no blocking task.
    ///
    /// The inner value is readily available, unless it has already been extracted. The value is
    /// extracted out by [`Unblock::into_inner()`], [`AsyncWrite::poll_close()`], or by awaiting
    /// [`Unblock`].
    Idle(Option<Box<T>>),

    /// A [`Unblock::with_mut()`] closure was spawned and is still running.
    WithMut(Task<Box<T>>),

    /// The inner value is an [`Iterator`] currently iterating in a task.
    ///
    /// The `dyn Any` value here is a `Pin<Box<Receiver<<T as Iterator>::Item>>>`.
    Streaming(Option<Box<dyn Any + Send + Sync>>, Task<Box<T>>),

    /// The inner value is a [`Read`] currently reading in a task.
    Reading(Option<Reader>, Task<(io::Result<()>, Box<T>)>),

    /// The inner value is a [`Write`] currently writing in a task.
    Writing(Option<Writer>, Task<(io::Result<()>, Box<T>)>),

    /// The inner value is a [`Seek`] currently seeking in a task.
    Seeking(Task<(SeekFrom, io::Result<u64>, Box<T>)>),
}

impl<T: Iterator + Send + 'static> Stream for Unblock<T>
where
    T::Item: Send + 'static,
{
    type Item = T::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T::Item>> {
        loop {
            match &mut self.state {
                // If not in idle or active streaming state, stop the running task.
                State::WithMut(..)
                | State::Streaming(None, _)
                | State::Reading(..)
                | State::Writing(..)
                | State::Seeking(..) => {
                    // Wait for the running task to stop.
                    ready!(self.poll_stop(cx)).ok();
                }

                // If idle, start a streaming task.
                State::Idle(iter) => {
                    // Take the iterator out to run it on a blocking task.
                    let mut iter = iter.take().expect("inner iterator was taken out");

                    // This channel capacity seems to work well in practice. If it's too low, there
                    // will be too much synchronization between tasks. If too high, memory
                    // consumption increases.
                    let (sender, receiver) = bounded(self.cap.unwrap_or(8 * 1024)); // 8192 items

                    // Spawn a blocking task that runs the iterator and returns it when done.
                    let task = Executor::spawn(async move {
                        for item in &mut iter {
                            if sender.send(item).await.is_err() {
                                break;
                            }
                        }
                        iter
                    });

                    // Move into the busy state and poll again.
                    self.state = State::Streaming(Some(Box::new(Box::pin(receiver))), task);
                }

                // If streaming, receive an item.
                State::Streaming(Some(any), task) => {
                    let receiver = any.downcast_mut::<Pin<Box<Receiver<T::Item>>>>().unwrap();

                    // Poll the channel.
                    let opt = ready!(receiver.as_mut().poll_next(cx));

                    // If the channel is closed, retrieve the iterator back from the blocking task.
                    // This is not really a required step, but it's cleaner to drop the iterator on
                    // the same thread that created it.
                    if opt.is_none() {
                        // Poll the task to retrieve the iterator.
                        let iter = ready!(Pin::new(task).poll(cx));
                        self.state = State::Idle(Some(iter));
                    }

                    return Poll::Ready(opt);
                }
            }
        }
    }
}

impl<T: Read + Send + 'static> AsyncRead for Unblock<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match &mut self.state {
                // If not in idle or active reading state, stop the running task.
                State::WithMut(..)
                | State::Reading(None, _)
                | State::Streaming(..)
                | State::Writing(..)
                | State::Seeking(..) => {
                    // Wait for the running task to stop.
                    ready!(self.poll_stop(cx))?;
                }

                // If idle, start a reading task.
                State::Idle(io) => {
                    // Take the I/O handle out to read it on a blocking task.
                    let mut io = io.take().expect("inner value was taken out");

                    // This pipe capacity seems to work well in practice. If it's too low, there
                    // will be too much synchronization between tasks. If too high, memory
                    // consumption increases.
                    let (reader, mut writer) = pipe(self.cap.unwrap_or(8 * 1024 * 1024)); // 8 MB

                    // Spawn a blocking task that reads and returns the I/O handle when done.
                    let task = Executor::spawn(async move {
                        // Copy bytes from the I/O handle into the pipe until the pipe is closed or
                        // an error occurs.
                        loop {
                            match future::poll_fn(|cx| writer.poll_fill(cx, &mut io)).await {
                                Ok(0) => return (Ok(()), io),
                                Ok(_) => {}
                                Err(err) => return (Err(err), io),
                            }
                        }
                    });

                    // Move into the busy state and poll again.
                    self.state = State::Reading(Some(reader), task);
                }

                // If reading, read bytes from the pipe.
                State::Reading(Some(reader), task) => {
                    // Poll the pipe.
                    let n = ready!(reader.poll_drain(cx, buf))?;

                    // If the pipe is closed, retrieve the I/O handle back from the blocking task.
                    // This is not really a required step, but it's cleaner to drop the handle on
                    // the same thread that created it.
                    if n == 0 {
                        // Poll the task to retrieve the I/O handle.
                        let (res, io) = ready!(Pin::new(task).poll(cx));
                        // Make sure to move into the idle state before reporting errors.
                        self.state = State::Idle(Some(io));
                        res?;
                    }

                    return Poll::Ready(Ok(n));
                }
            }
        }
    }
}

impl<T: Write + Send + 'static> AsyncWrite for Unblock<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            match &mut self.state {
                // If not in idle or active writing state, stop the running task.
                State::WithMut(..)
                | State::Writing(None, _)
                | State::Streaming(..)
                | State::Reading(..)
                | State::Seeking(..) => {
                    // Wait for the running task to stop.
                    ready!(self.poll_stop(cx))?;
                }

                // If idle, start the writing task.
                State::Idle(io) => {
                    // Take the I/O handle out to write on a blocking task.
                    let mut io = io.take().expect("inner value was taken out");

                    // This pipe capacity seems to work well in practice. If it's too low, there will
                    // be too much synchronization between tasks. If too high, memory consumption
                    // increases.
                    let (mut reader, writer) = pipe(self.cap.unwrap_or(8 * 1024 * 1024)); // 8 MB

                    // Spawn a blocking task that writes and returns the I/O handle when done.
                    let task = Executor::spawn(async move {
                        // Copy bytes from the pipe into the I/O handle until the pipe is closed or an
                        // error occurs. Flush the I/O handle at the end.
                        loop {
                            match future::poll_fn(|cx| reader.poll_drain(cx, &mut io)).await {
                                Ok(0) => return (io.flush(), io),
                                Ok(_) => {}
                                Err(err) => {
                                    io.flush().ok();
                                    return (Err(err), io);
                                }
                            }
                        }
                    });

                    // Move into the busy state and poll again.
                    self.state = State::Writing(Some(writer), task);
                }

                // If writing, write more bytes into the pipe.
                State::Writing(Some(writer), _) => return writer.poll_fill(cx, buf),
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match &mut self.state {
                // If not in idle state, stop the running task.
                State::WithMut(..)
                | State::Streaming(..)
                | State::Writing(..)
                | State::Reading(..)
                | State::Seeking(..) => {
                    // Wait for the running task to stop.
                    ready!(self.poll_stop(cx))?;
                }

                // Idle implies flushed.
                State::Idle(_) => return Poll::Ready(Ok(())),
            }
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // First, make sure the I/O handle is flushed.
        ready!(Pin::new(&mut self).poll_flush(cx))?;

        // Then move into the idle state with no I/O handle, thus dropping it.
        self.state = State::Idle(None);
        Poll::Ready(Ok(()))
    }
}

impl<T: Seek + Send + 'static> AsyncSeek for Unblock<T> {
    fn poll_seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        loop {
            match &mut self.state {
                // If not in idle state, stop the running task.
                State::WithMut(..)
                | State::Streaming(..)
                | State::Reading(..)
                | State::Writing(..) => {
                    // Wait for the running task to stop.
                    ready!(self.poll_stop(cx))?;
                }

                State::Idle(io) => {
                    // Take the I/O handle out to seek on a blocking task.
                    let mut io = io.take().expect("inner value was taken out");

                    let task = Executor::spawn(async move {
                        let res = io.seek(pos);
                        (pos, res, io)
                    });
                    self.state = State::Seeking(task);
                }

                State::Seeking(task) => {
                    // Poll the task to wait for it to finish.
                    let (original_pos, res, io) = ready!(Pin::new(task).poll(cx));
                    // Make sure to move into the idle state before reporting errors.
                    self.state = State::Idle(Some(io));
                    let current = res?;

                    // If the `pos` argument matches the original one, return the result.
                    if original_pos == pos {
                        return Poll::Ready(Ok(current));
                    }
                }
            }
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    #[test]
    fn test_max_threads() {
        // properly set env var
        env::set_var(MAX_THREADS_ENV, "100");
        assert_eq!(100, Executor::max_threads().get());

        // passed value below minimum, so we set it to minimum
        env::set_var(MAX_THREADS_ENV, "0");
        assert_eq!(1, Executor::max_threads().get());

        // passed value above maximum, so we set to allowed maximum
        env::set_var(MAX_THREADS_ENV, "50000");
        assert_eq!(10000, Executor::max_threads().get());

        // no env var, use default
        env::set_var(MAX_THREADS_ENV, "");
        assert_eq!(500, Executor::max_threads().get());

        // not a number, use default
        env::set_var(MAX_THREADS_ENV, "NOTINT");
        assert_eq!(500, Executor::max_threads().get());
    }
}
