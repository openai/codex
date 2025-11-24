use futures_util::future::{AbortHandle, Abortable};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::runtime::Builder;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::task::{spawn_local, JoinHandle, LocalSet};

/// A cloneable handle to a local pool, used for spawning `!Send` tasks.
///
/// Internally the local pool uses a [`tokio::task::LocalSet`] for each worker thread
/// in the pool. Consequently you can also use [`tokio::task::spawn_local`] (which will
/// execute on the same thread) inside the Future you supply to the various spawn methods
/// of `LocalPoolHandle`.
///
/// [`tokio::task::LocalSet`]: tokio::task::LocalSet
/// [`tokio::task::spawn_local`]: tokio::task::spawn_local
///
/// # Examples
///
/// ```
/// use std::rc::Rc;
/// use tokio::task;
/// use tokio_util::task::LocalPoolHandle;
///
/// #[tokio::main(flavor = "current_thread")]
/// async fn main() {
///     let pool = LocalPoolHandle::new(5);
///
///     let output = pool.spawn_pinned(|| {
///         // `data` is !Send + !Sync
///         let data = Rc::new("local data");
///         let data_clone = data.clone();
///
///         async move {
///             task::spawn_local(async move {
///                 println!("{}", data_clone);
///             });
///
///             data.to_string()
///         }
///     }).await.unwrap();
///     println!("output: {}", output);
/// }
/// ```
///
#[derive(Clone)]
pub struct LocalPoolHandle {
    pool: Arc<LocalPool>,
}

impl LocalPoolHandle {
    /// Create a new pool of threads to handle `!Send` tasks. Spawn tasks onto this
    /// pool via [`LocalPoolHandle::spawn_pinned`].
    ///
    /// # Panics
    ///
    /// Panics if the pool size is less than one.
    #[track_caller]
    pub fn new(pool_size: usize) -> LocalPoolHandle {
        assert!(pool_size > 0);

        let workers = (0..pool_size)
            .map(|_| LocalWorkerHandle::new_worker())
            .collect();

        let pool = Arc::new(LocalPool { workers });

        LocalPoolHandle { pool }
    }

    /// Returns the number of threads of the Pool.
    #[inline]
    pub fn num_threads(&self) -> usize {
        self.pool.workers.len()
    }

    /// Returns the number of tasks scheduled on each worker. The indices of the
    /// worker threads correspond to the indices of the returned `Vec`.
    pub fn get_task_loads_for_each_worker(&self) -> Vec<usize> {
        self.pool
            .workers
            .iter()
            .map(|worker| worker.task_count.load(Ordering::SeqCst))
            .collect::<Vec<_>>()
    }

    /// Spawn a task onto a worker thread and pin it there so it can't be moved
    /// off of the thread. Note that the future is not [`Send`], but the
    /// [`FnOnce`] which creates it is.
    ///
    /// # Examples
    /// ```
    /// use std::rc::Rc;
    /// use tokio_util::task::LocalPoolHandle;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Create the local pool
    ///     let pool = LocalPoolHandle::new(1);
    ///
    ///     // Spawn a !Send future onto the pool and await it
    ///     let output = pool
    ///         .spawn_pinned(|| {
    ///             // Rc is !Send + !Sync
    ///             let local_data = Rc::new("test");
    ///
    ///             // This future holds an Rc, so it is !Send
    ///             async move { local_data.to_string() }
    ///         })
    ///         .await
    ///         .unwrap();
    ///
    ///     assert_eq!(output, "test");
    /// }
    /// ```
    pub fn spawn_pinned<F, Fut>(&self, create_task: F) -> JoinHandle<Fut::Output>
    where
        F: FnOnce() -> Fut,
        F: Send + 'static,
        Fut: Future + 'static,
        Fut::Output: Send + 'static,
    {
        self.pool
            .spawn_pinned(create_task, WorkerChoice::LeastBurdened)
    }

    /// Differs from `spawn_pinned` only in that you can choose a specific worker thread
    /// of the pool, whereas `spawn_pinned` chooses the worker with the smallest
    /// number of tasks scheduled.
    ///
    /// A worker thread is chosen by index. Indices are 0 based and the largest index
    /// is given by `num_threads() - 1`
    ///
    /// # Panics
    ///
    /// This method panics if the index is out of bounds.
    ///
    /// # Examples
    ///
    /// This method can be used to spawn a task on all worker threads of the pool:
    ///
    /// ```
    /// use tokio_util::task::LocalPoolHandle;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     const NUM_WORKERS: usize = 3;
    ///     let pool = LocalPoolHandle::new(NUM_WORKERS);
    ///     let handles = (0..pool.num_threads())
    ///         .map(|worker_idx| {
    ///             pool.spawn_pinned_by_idx(
    ///                 || {
    ///                     async {
    ///                         "test"
    ///                     }
    ///                 },
    ///                 worker_idx,
    ///             )
    ///         })
    ///         .collect::<Vec<_>>();
    ///
    ///     for handle in handles {
    ///         handle.await.unwrap();
    ///     }
    /// }
    /// ```
    ///
    #[track_caller]
    pub fn spawn_pinned_by_idx<F, Fut>(&self, create_task: F, idx: usize) -> JoinHandle<Fut::Output>
    where
        F: FnOnce() -> Fut,
        F: Send + 'static,
        Fut: Future + 'static,
        Fut::Output: Send + 'static,
    {
        self.pool
            .spawn_pinned(create_task, WorkerChoice::ByIdx(idx))
    }
}

impl Debug for LocalPoolHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("LocalPoolHandle")
    }
}

enum WorkerChoice {
    LeastBurdened,
    ByIdx(usize),
}

struct LocalPool {
    workers: Box<[LocalWorkerHandle]>,
}

impl LocalPool {
    /// Spawn a `?Send` future onto a worker
    #[track_caller]
    fn spawn_pinned<F, Fut>(
        &self,
        create_task: F,
        worker_choice: WorkerChoice,
    ) -> JoinHandle<Fut::Output>
    where
        F: FnOnce() -> Fut,
        F: Send + 'static,
        Fut: Future + 'static,
        Fut::Output: Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let (worker, job_guard) = match worker_choice {
            WorkerChoice::LeastBurdened => self.find_and_incr_least_burdened_worker(),
            WorkerChoice::ByIdx(idx) => self.find_worker_by_idx(idx),
        };
        let worker_spawner = worker.spawner.clone();

        // Spawn a future onto the worker's runtime so we can immediately return
        // a join handle.
        worker.runtime_handle.spawn(async move {
            // Move the job guard into the task
            let _job_guard = job_guard;

            // Propagate aborts via Abortable/AbortHandle
            let (abort_handle, abort_registration) = AbortHandle::new_pair();
            let _abort_guard = AbortGuard(abort_handle);

            // Inside the future we can't run spawn_local yet because we're not
            // in the context of a LocalSet. We need to send create_task to the
            // LocalSet task for spawning.
            let spawn_task = Box::new(move || {
                // Once we're in the LocalSet context we can call spawn_local
                let join_handle =
                    spawn_local(
                        async move { Abortable::new(create_task(), abort_registration).await },
                    );

                // Send the join handle back to the spawner. If sending fails,
                // we assume the parent task was canceled, so cancel this task
                // as well.
                if let Err(join_handle) = sender.send(join_handle) {
                    join_handle.abort()
                }
            });

            // Send the callback to the LocalSet task
            if let Err(e) = worker_spawner.send(spawn_task) {
                // Propagate the error as a panic in the join handle.
                panic!("Failed to send job to worker: {e}");
            }

            // Wait for the task's join handle
            let join_handle = match receiver.await {
                Ok(handle) => handle,
                Err(e) => {
                    // We sent the task successfully, but failed to get its
                    // join handle... We assume something happened to the worker
                    // and the task was not spawned. Propagate the error as a
                    // panic in the join handle.
                    panic!("Worker failed to send join handle: {e}");
                }
            };

            // Wait for the task to complete
            let join_result = join_handle.await;

            match join_result {
                Ok(Ok(output)) => output,
                Ok(Err(_)) => {
                    // Pinned task was aborted. But that only happens if this
                    // task is aborted. So this is an impossible branch.
                    unreachable!(
                        "Reaching this branch means this task was previously \
                         aborted but it continued running anyways"
                    )
                }
                Err(e) => {
                    if e.is_panic() {
                        std::panic::resume_unwind(e.into_panic());
                    } else if e.is_cancelled() {
                        // No one else should have the join handle, so this is
                        // unexpected. Forward this error as a panic in the join
                        // handle.
                        panic!("spawn_pinned task was canceled: {e}");
                    } else {
                        // Something unknown happened (not a panic or
                        // cancellation). Forward this error as a panic in the
                        // join handle.
                        panic!("spawn_pinned task failed: {e}");
                    }
                }
            }
        })
    }

    /// Find the worker with the least number of tasks, increment its task
    /// count, and return its handle. Make sure to actually spawn a task on
    /// the worker so the task count is kept consistent with load.
    ///
    /// A job count guard is also returned to ensure the task count gets
    /// decremented when the job is done.
    fn find_and_incr_least_burdened_worker(&self) -> (&LocalWorkerHandle, JobCountGuard) {
        loop {
            let (worker, task_count) = self
                .workers
                .iter()
                .map(|worker| (worker, worker.task_count.load(Ordering::SeqCst)))
                .min_by_key(|&(_, count)| count)
                .expect("There must be more than one worker");

            // Make sure the task count hasn't changed since when we choose this
            // worker. Otherwise, restart the search.
            if worker
                .task_count
                .compare_exchange(
                    task_count,
                    task_count + 1,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return (worker, JobCountGuard(Arc::clone(&worker.task_count)));
            }
        }
    }

    #[track_caller]
    fn find_worker_by_idx(&self, idx: usize) -> (&LocalWorkerHandle, JobCountGuard) {
        let worker = &self.workers[idx];
        worker.task_count.fetch_add(1, Ordering::SeqCst);

        (worker, JobCountGuard(Arc::clone(&worker.task_count)))
    }
}

/// Automatically decrements a worker's job count when a job finishes (when
/// this gets dropped).
struct JobCountGuard(Arc<AtomicUsize>);

impl Drop for JobCountGuard {
    fn drop(&mut self) {
        // Decrement the job count
        let previous_value = self.0.fetch_sub(1, Ordering::SeqCst);
        debug_assert!(previous_value >= 1);
    }
}

/// Calls abort on the handle when dropped.
struct AbortGuard(AbortHandle);

impl Drop for AbortGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

type PinnedFutureSpawner = Box<dyn FnOnce() + Send + 'static>;

struct LocalWorkerHandle {
    runtime_handle: tokio::runtime::Handle,
    spawner: UnboundedSender<PinnedFutureSpawner>,
    task_count: Arc<AtomicUsize>,
}

impl LocalWorkerHandle {
    /// Create a new worker for executing pinned tasks
    fn new_worker() -> LocalWorkerHandle {
        let (sender, receiver) = unbounded_channel();
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to start a pinned worker thread runtime");
        let runtime_handle = runtime.handle().clone();
        let task_count = Arc::new(AtomicUsize::new(0));
        let task_count_clone = Arc::clone(&task_count);

        std::thread::spawn(|| Self::run(runtime, receiver, task_count_clone));

        LocalWorkerHandle {
            runtime_handle,
            spawner: sender,
            task_count,
        }
    }

    fn run(
        runtime: tokio::runtime::Runtime,
        mut task_receiver: UnboundedReceiver<PinnedFutureSpawner>,
        task_count: Arc<AtomicUsize>,
    ) {
        let local_set = LocalSet::new();
        local_set.block_on(&runtime, async {
            while let Some(spawn_task) = task_receiver.recv().await {
                // Calls spawn_local(future)
                (spawn_task)();
            }
        });

        // If there are any tasks on the runtime associated with a LocalSet task
        // that has already completed, but whose output has not yet been
        // reported, let that task complete.
        //
        // Since the task_count is decremented when the runtime task exits,
        // reading that counter lets us know if any such tasks completed during
        // the call to `block_on`.
        //
        // Tasks on the LocalSet can't complete during this loop since they're
        // stored on the LocalSet and we aren't accessing it.
        let mut previous_task_count = task_count.load(Ordering::SeqCst);
        loop {
            // This call will also run tasks spawned on the runtime.
            runtime.block_on(tokio::task::yield_now());
            let new_task_count = task_count.load(Ordering::SeqCst);
            if new_task_count == previous_task_count {
                break;
            } else {
                previous_task_count = new_task_count;
            }
        }

        // It's now no longer possible for a task on the runtime to be
        // associated with a LocalSet task that has completed. Drop both the
        // LocalSet and runtime to let tasks on the runtime be cancelled if and
        // only if they are still on the LocalSet.
        //
        // Drop the LocalSet task first so that anyone awaiting the runtime
        // JoinHandle will see the cancelled error after the LocalSet task
        // destructor has completed.
        drop(local_set);
        drop(runtime);
    }
}
