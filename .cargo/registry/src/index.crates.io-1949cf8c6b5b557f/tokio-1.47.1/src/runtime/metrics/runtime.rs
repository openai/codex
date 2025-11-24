use crate::runtime::Handle;
use std::time::Duration;

cfg_64bit_metrics! {
    use std::sync::atomic::Ordering::Relaxed;
}

cfg_unstable_metrics! {
    use std::ops::Range;
    use std::thread::ThreadId;
}

/// Handle to the runtime's metrics.
///
/// This handle is internally reference-counted and can be freely cloned. A
/// `RuntimeMetrics` handle is obtained using the [`Runtime::metrics`] method.
///
/// [`Runtime::metrics`]: crate::runtime::Runtime::metrics()
#[derive(Clone, Debug)]
pub struct RuntimeMetrics {
    handle: Handle,
}

impl RuntimeMetrics {
    pub(crate) fn new(handle: Handle) -> RuntimeMetrics {
        RuntimeMetrics { handle }
    }

    /// Returns the number of worker threads used by the runtime.
    ///
    /// The number of workers is set by configuring `worker_threads` on
    /// `runtime::Builder`. When using the `current_thread` runtime, the return
    /// value is always `1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Handle;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let metrics = Handle::current().metrics();
    ///
    ///     let n = metrics.num_workers();
    ///     println!("Runtime is using {} workers", n);
    /// }
    /// ```
    pub fn num_workers(&self) -> usize {
        self.handle.inner.num_workers()
    }

    /// Returns the current number of alive tasks in the runtime.
    ///
    /// This counter increases when a task is spawned and decreases when a
    /// task exits.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Handle;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///    let metrics = Handle::current().metrics();
    ///
    ///     let n = metrics.num_alive_tasks();
    ///     println!("Runtime has {} alive tasks", n);
    /// }
    /// ```
    pub fn num_alive_tasks(&self) -> usize {
        self.handle.inner.num_alive_tasks()
    }

    /// Returns the number of tasks currently scheduled in the runtime's
    /// global queue.
    ///
    /// Tasks that are spawned or notified from a non-runtime thread are
    /// scheduled using the runtime's global queue. This metric returns the
    /// **current** number of tasks pending in the global queue. As such, the
    /// returned value may increase or decrease as new tasks are scheduled and
    /// processed.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::runtime::Handle;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let metrics = Handle::current().metrics();
    ///
    ///     let n = metrics.global_queue_depth();
    ///     println!("{} tasks currently pending in the runtime's global queue", n);
    /// }
    /// ```
    pub fn global_queue_depth(&self) -> usize {
        self.handle.inner.injection_queue_depth()
    }

    cfg_64bit_metrics! {
        /// Returns the amount of time the given worker thread has been busy.
        ///
        /// The worker busy duration starts at zero when the runtime is created and
        /// increases whenever the worker is spending time processing work. Using
        /// this value can indicate the load of the given worker. If a lot of time
        /// is spent busy, then the worker is under load and will check for inbound
        /// events less often.
        ///
        /// The timer is monotonically increasing. It is never decremented or reset
        /// to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_total_busy_duration(0);
        ///     println!("worker 0 was busy for a total of {:?}", n);
        /// }
        /// ```
        pub fn worker_total_busy_duration(&self, worker: usize) -> Duration {
            let nanos = self
                .handle
                .inner
                .worker_metrics(worker)
                .busy_duration_total
                .load(Relaxed);
            Duration::from_nanos(nanos)
        }

        /// Returns the total number of times the given worker thread has parked.
        ///
        /// The worker park count starts at zero when the runtime is created and
        /// increases by one each time the worker parks the thread waiting for new
        /// inbound events to process. This usually means the worker has processed
        /// all pending work and is currently idle.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_park_count(0);
        ///     println!("worker 0 parked {} times", n);
        /// }
        /// ```
        pub fn worker_park_count(&self, worker: usize) -> u64 {
            self.handle
                .inner
                .worker_metrics(worker)
                .park_count
                .load(Relaxed)
        }

        /// Returns the total number of times the given worker thread has parked
        /// and unparked.
        ///
        /// The worker park/unpark count starts at zero when the runtime is created
        /// and increases by one each time the worker parks the thread waiting for
        /// new inbound events to process. This usually means the worker has processed
        /// all pending work and is currently idle. When new work becomes available,
        /// the worker is unparked and the park/unpark count is again increased by one.
        ///
        /// An odd count means that the worker is currently parked.
        /// An even count means that the worker is currently active.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///     let n = metrics.worker_park_unpark_count(0);
        ///
        ///     println!("worker 0 parked and unparked {} times", n);
        ///
        ///     if n % 2 == 0 {
        ///         println!("worker 0 is active");
        ///     } else {
        ///         println!("worker 0 is parked");
        ///     }
        /// }
        /// ```
        pub fn worker_park_unpark_count(&self, worker: usize) -> u64 {
            self.handle
                .inner
                .worker_metrics(worker)
                .park_unpark_count
                .load(Relaxed)
        }
    }

    cfg_unstable_metrics! {

        /// Returns the number of additional threads spawned by the runtime.
        ///
        /// The number of workers is set by configuring `max_blocking_threads` on
        /// `runtime::Builder`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let _ = tokio::task::spawn_blocking(move || {
        ///         // Stand-in for compute-heavy work or using synchronous APIs
        ///         1 + 1
        ///     }).await;
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.num_blocking_threads();
        ///     println!("Runtime has created {} threads", n);
        /// }
        /// ```
        pub fn num_blocking_threads(&self) -> usize {
            self.handle.inner.num_blocking_threads()
        }

        #[deprecated = "Renamed to num_alive_tasks"]
        /// Renamed to [`RuntimeMetrics::num_alive_tasks`]
        pub fn active_tasks_count(&self) -> usize {
            self.num_alive_tasks()
        }

        /// Returns the number of idle threads, which have spawned by the runtime
        /// for `spawn_blocking` calls.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let _ = tokio::task::spawn_blocking(move || {
        ///         // Stand-in for compute-heavy work or using synchronous APIs
        ///         1 + 1
        ///     }).await;
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.num_idle_blocking_threads();
        ///     println!("Runtime has {} idle blocking thread pool threads", n);
        /// }
        /// ```
        pub fn num_idle_blocking_threads(&self) -> usize {
            self.handle.inner.num_idle_blocking_threads()
        }

        /// Returns the thread id of the given worker thread.
        ///
        /// The returned value is `None` if the worker thread has not yet finished
        /// starting up.
        ///
        /// If additional information about the thread, such as its native id, are
        /// required, those can be collected in [`on_thread_start`] and correlated
        /// using the thread id.
        ///
        /// [`on_thread_start`]: crate::runtime::Builder::on_thread_start
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let id = metrics.worker_thread_id(0);
        ///     println!("worker 0 has id {:?}", id);
        /// }
        /// ```
        pub fn worker_thread_id(&self, worker: usize) -> Option<ThreadId> {
            self.handle
                .inner
                .worker_metrics(worker)
                .thread_id()
        }
    }

    feature! {
        #![all(
            tokio_unstable,
            target_has_atomic = "64"
        )]
        /// Returns the number of tasks spawned in this runtime since it was created.
        ///
        /// This count starts at zero when the runtime is created and increases by one each time a task is spawned.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///    let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.spawned_tasks_count();
        ///     println!("Runtime has had {} tasks spawned", n);
        /// }
        /// ```
        pub fn spawned_tasks_count(&self) -> u64 {
            self.handle.inner.spawned_tasks_count()
        }

        /// Returns the number of tasks scheduled from **outside** of the runtime.
        ///
        /// The remote schedule count starts at zero when the runtime is created and
        /// increases by one each time a task is woken from **outside** of the
        /// runtime. This usually means that a task is spawned or notified from a
        /// non-runtime thread and must be queued using the Runtime's injection
        /// queue, which tends to be slower.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.remote_schedule_count();
        ///     println!("{} tasks were scheduled from outside the runtime", n);
        /// }
        /// ```
        pub fn remote_schedule_count(&self) -> u64 {
            self.handle
                .inner
                .scheduler_metrics()
                .remote_schedule_count
                .load(Relaxed)
        }

        /// Returns the number of times that tasks have been forced to yield back to the scheduler
        /// after exhausting their task budgets.
        ///
        /// This count starts at zero when the runtime is created and increases by one each time a task yields due to exhausting its budget.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        pub fn budget_forced_yield_count(&self) -> u64 {
            self.handle
                .inner
                .scheduler_metrics()
                .budget_forced_yield_count
                .load(Relaxed)
        }

        /// Returns the number of times the given worker thread unparked but
        /// performed no work before parking again.
        ///
        /// The worker no-op count starts at zero when the runtime is created and
        /// increases by one each time the worker unparks the thread but finds no
        /// new work and goes back to sleep. This indicates a false-positive wake up.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_noop_count(0);
        ///     println!("worker 0 had {} no-op unparks", n);
        /// }
        /// ```
        pub fn worker_noop_count(&self, worker: usize) -> u64 {
            self.handle
                .inner
                .worker_metrics(worker)
                .noop_count
                .load(Relaxed)
        }

        /// Returns the number of tasks the given worker thread stole from
        /// another worker thread.
        ///
        /// This metric only applies to the **multi-threaded** runtime and will
        /// always return `0` when using the current thread runtime.
        ///
        /// The worker steal count starts at zero when the runtime is created and
        /// increases by `N` each time the worker has processed its scheduled queue
        /// and successfully steals `N` more pending tasks from another worker.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_steal_count(0);
        ///     println!("worker 0 has stolen {} tasks", n);
        /// }
        /// ```
        pub fn worker_steal_count(&self, worker: usize) -> u64 {
            self.handle
                .inner
                .worker_metrics(worker)
                .steal_count
                .load(Relaxed)
        }

        /// Returns the number of times the given worker thread stole tasks from
        /// another worker thread.
        ///
        /// This metric only applies to the **multi-threaded** runtime and will
        /// always return `0` when using the current thread runtime.
        ///
        /// The worker steal count starts at zero when the runtime is created and
        /// increases by one each time the worker has processed its scheduled queue
        /// and successfully steals more pending tasks from another worker.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_steal_operations(0);
        ///     println!("worker 0 has stolen tasks {} times", n);
        /// }
        /// ```
        pub fn worker_steal_operations(&self, worker: usize) -> u64 {
            self.handle
                .inner
                .worker_metrics(worker)
                .steal_operations
                .load(Relaxed)
        }

        /// Returns the number of tasks the given worker thread has polled.
        ///
        /// The worker poll count starts at zero when the runtime is created and
        /// increases by one each time the worker polls a scheduled task.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_poll_count(0);
        ///     println!("worker 0 has polled {} tasks", n);
        /// }
        /// ```
        pub fn worker_poll_count(&self, worker: usize) -> u64 {
            self.handle
                .inner
                .worker_metrics(worker)
                .poll_count
                .load(Relaxed)
        }

        /// Returns the number of tasks scheduled from **within** the runtime on the
        /// given worker's local queue.
        ///
        /// The local schedule count starts at zero when the runtime is created and
        /// increases by one each time a task is woken from **inside** of the
        /// runtime on the given worker. This usually means that a task is spawned
        /// or notified from within a runtime thread and will be queued on the
        /// worker-local queue.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_local_schedule_count(0);
        ///     println!("{} tasks were scheduled on the worker's local queue", n);
        /// }
        /// ```
        pub fn worker_local_schedule_count(&self, worker: usize) -> u64 {
            self.handle
                .inner
                .worker_metrics(worker)
                .local_schedule_count
                .load(Relaxed)
        }

        /// Returns the number of times the given worker thread saturated its local
        /// queue.
        ///
        /// This metric only applies to the **multi-threaded** scheduler.
        ///
        /// The worker overflow count starts at zero when the runtime is created and
        /// increases by one each time the worker attempts to schedule a task
        /// locally, but its local queue is full. When this happens, half of the
        /// local queue is moved to the injection queue.
        ///
        /// The counter is monotonically increasing. It is never decremented or
        /// reset to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_overflow_count(0);
        ///     println!("worker 0 has overflowed its queue {} times", n);
        /// }
        /// ```
        pub fn worker_overflow_count(&self, worker: usize) -> u64 {
            self.handle
                .inner
                .worker_metrics(worker)
                .overflow_count
                .load(Relaxed)
        }
    }

    cfg_unstable_metrics! {

        /// Renamed to [`RuntimeMetrics::global_queue_depth`]
        #[deprecated = "Renamed to global_queue_depth"]
        #[doc(hidden)]
        pub fn injection_queue_depth(&self) -> usize {
            self.handle.inner.injection_queue_depth()
        }

        /// Returns the number of tasks currently scheduled in the given worker's
        /// local queue.
        ///
        /// Tasks that are spawned or notified from within a runtime thread are
        /// scheduled using that worker's local queue. This metric returns the
        /// **current** number of tasks pending in the worker's local queue. As
        /// such, the returned value may increase or decrease as new tasks are
        /// scheduled and processed.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_local_queue_depth(0);
        ///     println!("{} tasks currently pending in worker 0's local queue", n);
        /// }
        /// ```
        pub fn worker_local_queue_depth(&self, worker: usize) -> usize {
            self.handle.inner.worker_local_queue_depth(worker)
        }

        /// Returns `true` if the runtime is tracking the distribution of task poll
        /// times.
        ///
        /// Task poll times are not instrumented by default as doing so requires
        /// calling [`Instant::now()`] twice per task poll. The feature is enabled
        /// by calling [`enable_metrics_poll_time_histogram()`] when building the
        /// runtime.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::{self, Handle};
        ///
        /// fn main() {
        ///     runtime::Builder::new_current_thread()
        ///         .enable_metrics_poll_time_histogram()
        ///         .build()
        ///         .unwrap()
        ///         .block_on(async {
        ///             let metrics = Handle::current().metrics();
        ///             let enabled = metrics.poll_time_histogram_enabled();
        ///
        ///             println!("Tracking task poll time distribution: {:?}", enabled);
        ///         });
        /// }
        /// ```
        ///
        /// [`enable_metrics_poll_time_histogram()`]: crate::runtime::Builder::enable_metrics_poll_time_histogram
        /// [`Instant::now()`]: std::time::Instant::now
        pub fn poll_time_histogram_enabled(&self) -> bool {
            self.handle
                .inner
                .worker_metrics(0)
                .poll_count_histogram
                .is_some()
        }

        #[deprecated(note = "Renamed to `poll_time_histogram_enabled`")]
        #[doc(hidden)]
        pub fn poll_count_histogram_enabled(&self) -> bool {
            self.poll_time_histogram_enabled()
        }

        /// Returns the number of histogram buckets tracking the distribution of
        /// task poll times.
        ///
        /// This value is configured by calling
        /// [`metrics_poll_time_histogram_configuration()`] when building the runtime.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::{self, Handle};
        ///
        /// fn main() {
        ///     runtime::Builder::new_current_thread()
        ///         .enable_metrics_poll_time_histogram()
        ///         .build()
        ///         .unwrap()
        ///         .block_on(async {
        ///             let metrics = Handle::current().metrics();
        ///             let buckets = metrics.poll_time_histogram_num_buckets();
        ///
        ///             println!("Histogram buckets: {:?}", buckets);
        ///         });
        /// }
        /// ```
        ///
        /// [`metrics_poll_time_histogram_configuration()`]:
        ///     crate::runtime::Builder::metrics_poll_time_histogram_configuration
        pub fn poll_time_histogram_num_buckets(&self) -> usize {
            self.handle
                .inner
                .worker_metrics(0)
                .poll_count_histogram
                .as_ref()
                .map(|histogram| histogram.num_buckets())
                .unwrap_or_default()
        }

        /// Deprecated. Use [`poll_time_histogram_num_buckets()`] instead.
        ///
        /// [`poll_time_histogram_num_buckets()`]: Self::poll_time_histogram_num_buckets
        #[doc(hidden)]
        #[deprecated(note = "renamed to `poll_time_histogram_num_buckets`.")]
        pub fn poll_count_histogram_num_buckets(&self) -> usize {
            self.poll_time_histogram_num_buckets()
        }

        /// Returns the range of task poll times tracked by the given bucket.
        ///
        /// This value is configured by calling
        /// [`metrics_poll_time_histogram_configuration()`] when building the runtime.
        ///
        /// # Panics
        ///
        /// The method panics if `bucket` represents an invalid bucket index, i.e.
        /// is greater than or equal to `poll_time_histogram_num_buckets()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::{self, Handle};
        ///
        /// fn main() {
        ///     runtime::Builder::new_current_thread()
        ///         .enable_metrics_poll_time_histogram()
        ///         .build()
        ///         .unwrap()
        ///         .block_on(async {
        ///             let metrics = Handle::current().metrics();
        ///             let buckets = metrics.poll_time_histogram_num_buckets();
        ///
        ///             for i in 0..buckets {
        ///                 let range = metrics.poll_time_histogram_bucket_range(i);
        ///                 println!("Histogram bucket {} range: {:?}", i, range);
        ///             }
        ///         });
        /// }
        /// ```
        ///
        /// [`metrics_poll_time_histogram_configuration()`]:
        ///     crate::runtime::Builder::metrics_poll_time_histogram_configuration
        #[track_caller]
        pub fn poll_time_histogram_bucket_range(&self, bucket: usize) -> Range<Duration> {
            self.handle
                .inner
                .worker_metrics(0)
                .poll_count_histogram
                .as_ref()
                .map(|histogram| {
                    let range = histogram.bucket_range(bucket);
                    std::ops::Range {
                        start: Duration::from_nanos(range.start),
                        end: Duration::from_nanos(range.end),
                    }
                })
                .unwrap_or_default()
        }

        /// Deprecated. Use [`poll_time_histogram_bucket_range()`] instead.
        ///
        /// [`poll_time_histogram_bucket_range()`]: Self::poll_time_histogram_bucket_range
        #[track_caller]
        #[doc(hidden)]
        #[deprecated(note = "renamed to `poll_time_histogram_bucket_range`")]
        pub fn poll_count_histogram_bucket_range(&self, bucket: usize) -> Range<Duration> {
            self.poll_time_histogram_bucket_range(bucket)
        }
    }

    feature! {
    #![all(
        tokio_unstable,
        target_has_atomic = "64"
    )]
        /// Returns the number of times the given worker polled tasks with a poll
        /// duration within the given bucket's range.
        ///
        /// Each worker maintains its own histogram and the counts for each bucket
        /// starts at zero when the runtime is created. Each time the worker polls a
        /// task, it tracks the duration the task poll time took and increments the
        /// associated bucket by 1.
        ///
        /// Each bucket is a monotonically increasing counter. It is never
        /// decremented or reset to zero.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// `bucket` is the index of the bucket being queried. The bucket is scoped
        /// to the worker. The range represented by the bucket can be queried by
        /// calling [`poll_time_histogram_bucket_range()`]. Each worker maintains
        /// identical bucket ranges.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()` or if `bucket` represents an
        /// invalid bucket.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::{self, Handle};
        ///
        /// fn main() {
        ///     runtime::Builder::new_current_thread()
        ///         .enable_metrics_poll_time_histogram()
        ///         .build()
        ///         .unwrap()
        ///         .block_on(async {
        ///             let metrics = Handle::current().metrics();
        ///             let buckets = metrics.poll_time_histogram_num_buckets();
        ///
        ///             for worker in 0..metrics.num_workers() {
        ///                 for i in 0..buckets {
        ///                     let count = metrics.poll_time_histogram_bucket_count(worker, i);
        ///                     println!("Poll count {}", count);
        ///                 }
        ///             }
        ///         });
        /// }
        /// ```
        ///
        /// [`poll_time_histogram_bucket_range()`]: crate::runtime::RuntimeMetrics::poll_time_histogram_bucket_range
        #[track_caller]
        pub fn poll_time_histogram_bucket_count(&self, worker: usize, bucket: usize) -> u64 {
            self.handle
                .inner
                .worker_metrics(worker)
                .poll_count_histogram
                .as_ref()
                .map(|histogram| histogram.get(bucket))
                .unwrap_or_default()
        }

        #[doc(hidden)]
        #[deprecated(note = "use `poll_time_histogram_bucket_count` instead")]
        pub fn poll_count_histogram_bucket_count(&self, worker: usize, bucket: usize) -> u64 {
            self.poll_time_histogram_bucket_count(worker, bucket)
        }

        /// Returns the mean duration of task polls, in nanoseconds.
        ///
        /// This is an exponentially weighted moving average. Currently, this metric
        /// is only provided by the multi-threaded runtime.
        ///
        /// # Arguments
        ///
        /// `worker` is the index of the worker being queried. The given value must
        /// be between 0 and `num_workers()`. The index uniquely identifies a single
        /// worker and will continue to identify the worker throughout the lifetime
        /// of the runtime instance.
        ///
        /// # Panics
        ///
        /// The method panics when `worker` represents an invalid worker, i.e. is
        /// greater than or equal to `num_workers()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.worker_mean_poll_time(0);
        ///     println!("worker 0 has a mean poll time of {:?}", n);
        /// }
        /// ```
        #[track_caller]
        pub fn worker_mean_poll_time(&self, worker: usize) -> Duration {
            let nanos = self
                .handle
                .inner
                .worker_metrics(worker)
                .mean_poll_time
                .load(Relaxed);
            Duration::from_nanos(nanos)
        }
    }

    cfg_unstable_metrics! {
        /// Returns the number of tasks currently scheduled in the blocking
        /// thread pool, spawned using `spawn_blocking`.
        ///
        /// This metric returns the **current** number of tasks pending in
        /// blocking thread pool. As such, the returned value may increase
        /// or decrease as new tasks are scheduled and processed.
        ///
        /// # Examples
        ///
        /// ```
        /// use tokio::runtime::Handle;
        ///
        /// #[tokio::main]
        /// async fn main() {
        ///     let metrics = Handle::current().metrics();
        ///
        ///     let n = metrics.blocking_queue_depth();
        ///     println!("{} tasks currently pending in the blocking thread pool", n);
        /// }
        /// ```
        pub fn blocking_queue_depth(&self) -> usize {
            self.handle.inner.blocking_queue_depth()
        }
    }

    feature! {
        #![all(
            tokio_unstable,
            target_has_atomic = "64",
            feature = "net"
        )]
            /// Returns the number of file descriptors that have been registered with the
            /// runtime's I/O driver.
            ///
            /// # Examples
            ///
            /// ```
            /// use tokio::runtime::Handle;
            ///
            /// #[tokio::main]
            /// async fn main() {
            ///     let metrics = Handle::current().metrics();
            ///
            ///     let registered_fds = metrics.io_driver_fd_registered_count();
            ///     println!("{} fds have been registered with the runtime's I/O driver.", registered_fds);
            ///
            ///     let deregistered_fds = metrics.io_driver_fd_deregistered_count();
            ///
            ///     let current_fd_count = registered_fds - deregistered_fds;
            ///     println!("{} fds are currently registered by the runtime's I/O driver.", current_fd_count);
            /// }
            /// ```
            pub fn io_driver_fd_registered_count(&self) -> u64 {
                self.with_io_driver_metrics(|m| {
                    m.fd_registered_count.load(Relaxed)
                })
            }

            /// Returns the number of file descriptors that have been deregistered by the
            /// runtime's I/O driver.
            ///
            /// # Examples
            ///
            /// ```
            /// use tokio::runtime::Handle;
            ///
            /// #[tokio::main]
            /// async fn main() {
            ///     let metrics = Handle::current().metrics();
            ///
            ///     let n = metrics.io_driver_fd_deregistered_count();
            ///     println!("{} fds have been deregistered by the runtime's I/O driver.", n);
            /// }
            /// ```
            pub fn io_driver_fd_deregistered_count(&self) -> u64 {
                self.with_io_driver_metrics(|m| {
                    m.fd_deregistered_count.load(Relaxed)
                })
            }

            /// Returns the number of ready events processed by the runtime's
            /// I/O driver.
            ///
            /// # Examples
            ///
            /// ```
            /// use tokio::runtime::Handle;
            ///
            /// #[tokio::main]
            /// async fn main() {
            ///     let metrics = Handle::current().metrics();
            ///
            ///     let n = metrics.io_driver_ready_count();
            ///     println!("{} ready events processed by the runtime's I/O driver.", n);
            /// }
            /// ```
            pub fn io_driver_ready_count(&self) -> u64 {
                self.with_io_driver_metrics(|m| m.ready_count.load(Relaxed))
            }

            fn with_io_driver_metrics<F>(&self, f: F) -> u64
            where
                F: Fn(&super::IoDriverMetrics) -> u64,
            {
                // TODO: Investigate if this should return 0, most of our metrics always increase
                // thus this breaks that guarantee.
                self.handle
                    .inner
                    .driver()
                    .io
                    .as_ref()
                    .map(|h| f(&h.metrics))
                    .unwrap_or(0)
            }
    }
}
